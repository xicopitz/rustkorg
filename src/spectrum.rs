use std::sync::{Arc, Mutex};
use std::thread;
use rustfft::{FftPlanner, num_complex::Complex};

/// Number of frequency bands to display
pub const NUM_BANDS: usize = 32;

/// Sample rate for audio capture
const SAMPLE_RATE: u32 = 44100;

/// FFT size (must be power of 2) - smaller = lower latency but less frequency resolution
const FFT_SIZE: usize = 512;

/// Read smaller chunks for faster updates (hop size)
const HOP_SIZE: usize = 128;  // ~2.9ms at 44100Hz

/// Spectrum analyzer state shared between audio thread and UI
#[derive(Clone)]
pub struct SpectrumData {
    /// Frequency band magnitudes (0.0 to 1.0) - mono or left channel
    pub bands: [f32; NUM_BANDS],
    /// Peak hold values for each band
    pub peaks: [f32; NUM_BANDS],
    /// Right channel bands (stereo support)
    pub bands_right: [f32; NUM_BANDS],
    /// Right channel peaks (stereo support)
    pub peaks_right: [f32; NUM_BANDS],
    /// Whether the analyzer is running
    pub running: bool,
}

impl Default for SpectrumData {
    fn default() -> Self {
        Self {
            bands: [0.0; NUM_BANDS],
            peaks: [0.0; NUM_BANDS],
            bands_right: [0.0; NUM_BANDS],
            peaks_right: [0.0; NUM_BANDS],
            running: false,
        }
    }
}

/// Spectrum analyzer that captures audio from master sink monitor
pub struct SpectrumAnalyzer {
    data: Arc<Mutex<SpectrumData>>,
    handle: Option<thread::JoinHandle<()>>,
    stop_flag: Arc<Mutex<bool>>,
}

impl SpectrumAnalyzer {
    pub fn new() -> Self {
        let data = Arc::new(Mutex::new(SpectrumData::default()));
        let stop_flag = Arc::new(Mutex::new(false));
        
        Self {
            data,
            handle: None,
            stop_flag,
        }
    }

    /// Start the spectrum analyzer
    pub fn start(&mut self, sink_name: &str) {
        // Stop any existing analyzer first
        self.stop();
        
        // Reset stop flag
        if let Ok(mut stop) = self.stop_flag.lock() {
            *stop = false;
        }
        
        let data = self.data.clone();
        let stop_flag = self.stop_flag.clone();
        let sink_monitor = get_monitor_source(sink_name);
        
        let handle = thread::spawn(move || {
            run_analyzer(data, stop_flag, &sink_monitor);
        });
        
        self.handle = Some(handle);
        
        if let Ok(mut d) = self.data.lock() {
            d.running = true;
        }
    }

    /// Stop the spectrum analyzer
    pub fn stop(&mut self) {
        if let Ok(mut stop) = self.stop_flag.lock() {
            *stop = true;
        }
        
        if let Ok(mut d) = self.data.lock() {
            d.running = false;
        }
        
        // Wait for the thread to finish
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Get the current spectrum data
    pub fn get_data(&self) -> SpectrumData {
        self.data.lock().map(|d| d.clone()).unwrap_or_default()
    }
}

impl Default for SpectrumAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SpectrumAnalyzer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Get the monitor source name for a sink
fn get_monitor_source(sink_name: &str) -> String {
    if sink_name == "master_sink" || sink_name.is_empty() {
        // Use default sink monitor
        "@DEFAULT_SINK@.monitor".to_string()
    } else {
        format!("{}.monitor", sink_name)
    }
}

/// Main analyzer loop running in separate thread
fn run_analyzer(data: Arc<Mutex<SpectrumData>>, stop_flag: Arc<Mutex<bool>>, source_name: &str) {
    use libpulse_binding::sample::{Spec, Format};
    use libpulse_binding::stream::Direction;
    use libpulse_binding::def::BufferAttr;
    use libpulse_simple_binding::Simple;

    let spec = Spec {
        format: Format::F32le,
        channels: 2,  // Stereo capture
        rate: SAMPLE_RATE,
    };

    // Low-latency buffer configuration
    // fragsize: size of each fragment we read (in bytes)
    // We want to read HOP_SIZE samples * 4 bytes per f32
    let buffer_attr = BufferAttr {
        maxlength: u32::MAX,  // Let server decide
        tlength: u32::MAX,    // Not used for recording
        prebuf: u32::MAX,     // Not used for recording  
        minreq: u32::MAX,     // Not used for recording
        fragsize: (HOP_SIZE * std::mem::size_of::<f32>()) as u32,  // Request small fragments
    };

    // Try to connect to PulseAudio with low-latency settings
    let simple = match Simple::new(
        None,                      // Server name (None = default)
        "korg-spectrum",           // Application name
        Direction::Record,         // Direction
        Some(source_name),         // Device (monitor source)
        "spectrum-analyzer",       // Stream name
        &spec,                     // Sample spec
        None,                      // Channel map
        Some(&buffer_attr),        // Low-latency buffering
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect to PulseAudio: {:?}", e);
            if let Ok(mut d) = data.lock() {
                d.running = false;
            }
            return;
        }
    };

    // Ring buffers for left and right channels
    let mut ring_buffer_left = vec![0.0f32; FFT_SIZE];
    let mut ring_buffer_right = vec![0.0f32; FFT_SIZE];
    let mut ring_pos = 0usize;
    
    // Small read buffer for faster updates (stereo: 2 channels)
    let mut read_buffer = vec![0.0f32; HOP_SIZE * 2];
    
    // Peak decay only - no smoothing for immediate response
    let peak_decay = 0.92f32;
    
    let mut peak_values_left = [0.0f32; NUM_BANDS];
    let mut peak_values_right = [0.0f32; NUM_BANDS];

    loop {
        // Check stop flag
        if let Ok(stop) = stop_flag.lock() {
            if *stop {
                break;
            }
        }

        // Read a small chunk of stereo audio data
        let byte_buffer: &mut [u8] = unsafe {
            std::slice::from_raw_parts_mut(
                read_buffer.as_mut_ptr() as *mut u8,
                read_buffer.len() * std::mem::size_of::<f32>(),
            )
        };
        
        if simple.read(byte_buffer).is_err() {
            continue;
        }

        // Deinterleave stereo samples and add to ring buffers
        for i in 0..HOP_SIZE {
            let left = read_buffer[i * 2];
            let right = read_buffer[i * 2 + 1];
            
            ring_buffer_left[ring_pos] = left;
            ring_buffer_right[ring_pos] = right;
            ring_pos = (ring_pos + 1) % FFT_SIZE;
        }

        // Calculate bands for both channels
        let bands_left = calculate_bands_from_ring(&ring_buffer_left, ring_pos);
        let bands_right = calculate_bands_from_ring(&ring_buffer_right, ring_pos);
        
        // Update peaks for both channels
        let mut peaks_left = [0.0f32; NUM_BANDS];
        let mut peaks_right = [0.0f32; NUM_BANDS];
        
        for i in 0..NUM_BANDS {
            if bands_left[i] > peak_values_left[i] {
                peak_values_left[i] = bands_left[i];
            } else {
                peak_values_left[i] *= peak_decay;
            }
            peaks_left[i] = peak_values_left[i];
            
            if bands_right[i] > peak_values_right[i] {
                peak_values_right[i] = bands_right[i];
            } else {
                peak_values_right[i] *= peak_decay;
            }
            peaks_right[i] = peak_values_right[i];
        }

        // Update shared data with both channels
        if let Ok(mut d) = data.lock() {
            d.bands = bands_left;
            d.peaks = peaks_left;
            d.bands_right = bands_right;
            d.peaks_right = peaks_right;
        }
    }
}

/// Calculate frequency band magnitudes from FFT output
fn calculate_bands(fft_output: &[Complex<f32>]) -> [f32; NUM_BANDS] {
    let mut bands = [0.0f32; NUM_BANDS];
    
    // Frequency range: 20Hz to 20kHz (log scale)
    let min_freq = 20.0f32;
    let max_freq = 20000.0f32;
    
    let bin_size = SAMPLE_RATE as f32 / FFT_SIZE as f32;
    let useful_bins = FFT_SIZE / 2;
    
    for band in 0..NUM_BANDS {
        // Calculate frequency range for this band (logarithmic)
        let t0 = band as f32 / NUM_BANDS as f32;
        let t1 = (band + 1) as f32 / NUM_BANDS as f32;
        
        let freq_low = min_freq * (max_freq / min_freq).powf(t0);
        let freq_high = min_freq * (max_freq / min_freq).powf(t1);
        
        // Convert to bin indices
        let bin_low = ((freq_low / bin_size) as usize).min(useful_bins - 1);
        let bin_high = ((freq_high / bin_size) as usize).min(useful_bins - 1).max(bin_low + 1);
        
        // Sum magnitudes in this band
        let mut sum = 0.0f32;
        let mut count = 0;
        
        for bin in bin_low..=bin_high {
            if bin < useful_bins {
                let magnitude = fft_output[bin].norm();
                sum += magnitude;
                count += 1;
            }
        }
        
        if count > 0 {
            let avg = sum / count as f32;
            // Convert to dB-like scale and normalize
            let db = 20.0 * (avg + 1e-10).log10();
            // Normalize to 0-1 range (assuming -60dB to 0dB range)
            bands[band] = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        }
    }
    
    bands
}

/// Calculate bands from a ring buffer (used for both left and right channels)
fn calculate_bands_from_ring(
    ring_buffer: &[f32],
    ring_pos: usize,
) -> [f32; NUM_BANDS] {
    let mut planner: FftPlanner<f32> = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut fft_buffer: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); FFT_SIZE];
    
    // Hanning window
    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos()))
        .collect();
    
    // Apply window and prepare FFT input (read from ring buffer in order)
    for i in 0..FFT_SIZE {
        let idx = (ring_pos + i) % FFT_SIZE;
        fft_buffer[i] = Complex::new(ring_buffer[idx] * window[i], 0.0);
    }

    // Perform FFT
    fft.process(&mut fft_buffer);

    // Calculate band magnitudes
    calculate_bands(&fft_buffer)
}

/// Get frequency in Hz for a band index
pub fn get_band_frequency(band: usize) -> f32 {
    let min_freq = 20.0f32;
    let max_freq = 20000.0f32;
    let t = (band as f32 + 0.5) / NUM_BANDS as f32;
    min_freq * (max_freq / min_freq).powf(t)
}

/// Convert frequency to musical note name
pub fn frequency_to_note(freq: f32) -> String {
    if freq < 20.0 {
        return "< 20 Hz".to_string();
    }
    if freq > 20000.0 {
        return "> 20k Hz".to_string();
    }
    
    let notes = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let a4 = 440.0f32;
    let semitones_from_a4 = 12.0 * (freq / a4).log2();
    let semitones_from_c0 = semitones_from_a4 + 57.0; // A4 is 57 semitones above C0
    let octave = (semitones_from_c0 / 12.0).floor() as i32;
    let note_index = (semitones_from_c0 % 12.0) as usize;
    
    if octave >= 0 && octave < 10 && note_index < notes.len() {
        format!("{}{}", notes[note_index], octave)
    } else {
        format!("{:.0} Hz", freq)
    }
}
