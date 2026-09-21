//! MIDI-to-audio control loop, running on its own thread independent of whether a GUI
//! window currently exists. This is what lets hardware fader/mute-button input keep
//! controlling volume while the window is hidden to tray — the GUI is just a periodic
//! viewer/editor of `SharedFaderState`, not the thing driving audio changes.

use crate::midi::{MidiListener, MidiMessage, MidiOutput};
use crate::pipewire_control::PipeWireController;
use log::{info, warn};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

const MIDI_TO_PERCENT_FACTOR: f32 = 100.0 / 127.0;
const MIDI_FAST_MOVE_DELTA_PERCENT: u8 = 2;
const MIDI_OUTPUT_RECONNECT_INTERVAL: Duration = Duration::from_secs(10);

/// Converts a 0-100 volume percent back to a 0-127 fader position, matching the
/// percent-to-fader formula used when hydrating faders from a real PipeWire volume.
pub fn percent_to_fader_value(percent: u8) -> u8 {
    ((percent as f32 / 100.0) * 127.0).round() as u8
}

pub struct AudioUpdateCommand {
    pub target: String,
    pub percent: u8,
    pub is_sink: bool,
    pub context: &'static str,
}

/// Fader/mute state shared between the control thread and the GUI (when a window is
/// open). The GUI copies this into `UiState`'s same-named fields each frame to render;
/// the control thread is the only writer for MIDI-driven changes, and applies UI-slider
/// changes here too so both sources of truth are one and the same.
#[derive(Clone)]
pub struct SharedFaderState {
    pub system_fader_values: Vec<u8>,
    pub system_muted: Vec<bool>,
    pub system_muted_volume: Vec<u8>,
    pub app_fader_values: Vec<u8>,
    pub app_muted: Vec<bool>,
    pub app_muted_volume: Vec<u8>,
}

/// CC routing table the control thread uses to interpret incoming MIDI. Rebuilt whenever
/// settings are saved (see `crate::app::build_cc_mapping`, which both the GUI and this
/// module's caller use so they can't drift apart).
pub struct ControlMapping {
    pub cc_mapping: HashMap<u8, String>,
    pub cc_types: HashMap<u8, bool>,
    pub cc_to_sink_index: HashMap<u8, usize>,
    pub cc_to_app_index: HashMap<u8, usize>,
    pub mute_button_mapping: HashMap<u8, u8>,
}

/// Builds routing tables straight from the UI's `(cc, name)` lists — used for live,
/// unsaved-to-disk mapping updates (auto-assign, in-place edits) where going through a
/// full `Config` round-trip would be overkill.
pub fn control_mapping_from_lists(
    sinks: &[(u8, String)],
    applications: &[(u8, String)],
    mute_buttons: &[(u8, u8)],
) -> ControlMapping {
    let mut cc_mapping = HashMap::new();
    let mut cc_types = HashMap::new();
    let mut cc_to_sink_index = HashMap::new();
    let mut cc_to_app_index = HashMap::new();

    for (i, (cc, name)) in sinks.iter().enumerate() {
        cc_mapping.insert(*cc, name.clone());
        cc_types.insert(*cc, true);
        cc_to_sink_index.insert(*cc, i);
    }
    for (i, (cc, name)) in applications.iter().enumerate() {
        cc_mapping.insert(*cc, name.clone());
        cc_types.insert(*cc, false);
        cc_to_app_index.insert(*cc, i);
    }
    let mute_button_mapping = mute_buttons.iter().copied().collect();

    ControlMapping {
        cc_mapping,
        cc_types,
        cc_to_sink_index,
        cc_to_app_index,
        mute_button_mapping,
    }
}

pub enum UiInputEvent {
    /// A fader was dragged in the GUI; apply it exactly like a hardware move would.
    SliderChanged { is_sink: bool, cc: u8, fader_value: u8 },
    UpdateMapping(ControlMapping),
    UpdateSettings {
        debounce_ms: u32,
        logging_enabled: bool,
        volume_curve: String,
    },
}

enum ThreadInput {
    Midi(MidiMessage),
    Ui(UiInputEvent),
}

/// Handle the GUI (or main.rs, while no window is open) uses to talk to the control
/// thread and read its shared state. Cheap to clone (everything inside is an `Arc`).
#[derive(Clone)]
pub struct ControlHandle {
    pub shared_fader: Arc<Mutex<SharedFaderState>>,
    pub pipewire: Arc<Mutex<PipeWireController>>,
    pub audio_failure_count: Arc<AtomicU64>,
    pub last_runtime_error: Arc<Mutex<Option<String>>>,
    pub midi_connected: Arc<AtomicBool>,
    pub midi_output_connected: Arc<AtomicBool>,
    pub midi_reconnect_count: Arc<AtomicU32>,
    ui_input_tx: mpsc::Sender<ThreadInput>,
    // Kept alive for the life of the process so the MIDI listener's sender stays open.
    _midi_listener: Arc<MidiListener>,
}

impl ControlHandle {
    pub fn send_slider_changed(&self, is_sink: bool, cc: u8, fader_value: u8) {
        let _ = self
            .ui_input_tx
            .send(ThreadInput::Ui(UiInputEvent::SliderChanged {
                is_sink,
                cc,
                fader_value,
            }));
    }

    pub fn send_update_mapping(&self, mapping: ControlMapping) {
        let _ = self
            .ui_input_tx
            .send(ThreadInput::Ui(UiInputEvent::UpdateMapping(mapping)));
    }

    pub fn send_update_settings(&self, debounce_ms: u32, logging_enabled: bool, volume_curve: String) {
        let _ = self
            .ui_input_tx
            .send(ThreadInput::Ui(UiInputEvent::UpdateSettings {
                debounce_ms,
                logging_enabled,
                volume_curve,
            }));
    }
}

fn set_last_runtime_error(last_runtime_error: &Arc<Mutex<Option<String>>>, message: String) {
    if let Ok(mut last_error) = last_runtime_error.lock() {
        *last_error = Some(message);
    }
}

fn spawn_audio_update(
    tx: &mpsc::Sender<AudioUpdateCommand>,
    audio_failure_count: &Arc<AtomicU64>,
    last_runtime_error: &Arc<Mutex<Option<String>>>,
    target: String,
    percent: u8,
    is_sink: bool,
    context: &'static str,
) {
    if let Err(e) = tx.send(AudioUpdateCommand {
        target,
        percent,
        is_sink,
        context,
    }) {
        let message = format!("Failed to enqueue audio update: {}", e);
        warn!("{}", message);
        audio_failure_count.fetch_add(1, Ordering::Relaxed);
        set_last_runtime_error(last_runtime_error, message);
    }
}

/// Serializes all `pactl` calls onto one thread — unchanged from before this rework,
/// already independent of the GUI.
fn spawn_audio_worker(
    pipewire: Arc<Mutex<PipeWireController>>,
    rx: mpsc::Receiver<AudioUpdateCommand>,
    audio_failure_count: Arc<AtomicU64>,
    last_runtime_error: Arc<Mutex<Option<String>>>,
) {
    std::thread::spawn(move || {
        let mut pending: HashMap<(bool, String), AudioUpdateCommand> = HashMap::new();

        loop {
            let first_cmd = match rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => break,
            };

            pending.insert((first_cmd.is_sink, first_cmd.target.clone()), first_cmd);

            while let Ok(cmd) = rx.try_recv() {
                pending.insert((cmd.is_sink, cmd.target.clone()), cmd);
            }

            match pipewire.lock() {
                Ok(pw) => {
                    for (_, cmd) in pending.drain() {
                        let result = if cmd.is_sink {
                            pw.set_volume_for_sink(&cmd.target, cmd.percent)
                        } else {
                            pw.set_volume_for_app(&cmd.target, cmd.percent)
                        };

                        if let Err(e) = result {
                            let message = format!(
                                "{} failed for '{}' at {}%: {}",
                                cmd.context, cmd.target, cmd.percent, e
                            );
                            warn!("{}", message);
                            audio_failure_count.fetch_add(1, Ordering::Relaxed);
                            set_last_runtime_error(&last_runtime_error, message);
                        }
                    }
                }
                Err(e) => {
                    let failed_updates = pending.len().max(1) as u64;
                    let message = format!(
                        "Audio worker lock failure ({} pending updates): {}",
                        failed_updates, e
                    );
                    warn!("{}", message);
                    audio_failure_count.fetch_add(failed_updates, Ordering::Relaxed);
                    set_last_runtime_error(&last_runtime_error, message);
                    pending.clear();
                }
            }
        }
    });
}

/// Internal per-CC debounce/curve state, private to the control thread — the GUI never
/// needs to see this, only the fader values it produces.
#[derive(Default)]
struct CurveState {
    last_volume_values: HashMap<u8, u8>,
    last_volume_time: HashMap<u8, Instant>,
    soft_takeover_armed: HashMap<u8, bool>,
    inertia_smoothed_percent: HashMap<u8, f32>,
}

impl CurveState {
    fn map_midi_value_to_percent(&mut self, cc: u8, value: u8, volume_curve: &str) -> Option<u8> {
        let raw_percent = ((value as f32) * MIDI_TO_PERCENT_FACTOR) as u8;

        match volume_curve {
            "linear" => Some(raw_percent),
            "logarithmic" | "exponential" => {
                let normalized = (raw_percent as f32 / 100.0).clamp(0.0, 1.0);
                let curved = normalized.powf(1.8) * 100.0;
                Some(curved.round().clamp(0.0, 100.0) as u8)
            }
            "soft-takeover" => {
                let current_percent = self
                    .last_volume_values
                    .get(&cc)
                    .copied()
                    .unwrap_or(raw_percent);
                let diff = (raw_percent as i16 - current_percent as i16).unsigned_abs();
                let armed = self.soft_takeover_armed.entry(cc).or_insert(false);

                if !*armed && diff > 7 {
                    *armed = true;
                    return None;
                }

                if *armed {
                    if diff <= 2 {
                        *armed = false;
                        return Some(raw_percent);
                    }
                    return None;
                }

                Some(raw_percent)
            }
            "inertia" => {
                let current_percent = self
                    .last_volume_values
                    .get(&cc)
                    .copied()
                    .unwrap_or(raw_percent) as f32;
                let smoothed = self
                    .inertia_smoothed_percent
                    .entry(cc)
                    .or_insert(current_percent);
                *smoothed = (*smoothed * 0.72) + (raw_percent as f32 * 0.28);
                Some(smoothed.round().clamp(0.0, 100.0) as u8)
            }
            _ => Some(raw_percent),
        }
    }

    fn should_update(&self, cc: u8, percent: u8, debounce_ms: u32, now: Instant) -> bool {
        match self.last_volume_values.get(&cc) {
            None => true,
            Some(&last_val) => {
                if last_val == percent {
                    false
                } else if last_val.abs_diff(percent) >= MIDI_FAST_MOVE_DELTA_PERCENT {
                    true
                } else if let Some(&last_time) = self.last_volume_time.get(&cc) {
                    now.duration_since(last_time).as_millis() >= debounce_ms as u128
                } else {
                    true
                }
            }
        }
    }
}

struct ControlThread {
    mapping: ControlMapping,
    debounce_ms: u32,
    logging_enabled: bool,
    volume_curve: String,
    curve: CurveState,
    shared_fader: Arc<Mutex<SharedFaderState>>,
    midi_output: MidiOutput,
    midi_output_connected: Arc<AtomicBool>,
    midi_connected: Arc<AtomicBool>,
    midi_reconnect_count: Arc<AtomicU32>,
    audio_update_tx: mpsc::Sender<AudioUpdateCommand>,
    audio_failure_count: Arc<AtomicU64>,
    last_runtime_error: Arc<Mutex<Option<String>>>,
    last_midi_output_reconnect: Instant,
}

impl ControlThread {
    fn send_audio_update(&self, target: String, percent: u8, is_sink: bool, context: &'static str) {
        spawn_audio_update(
            &self.audio_update_tx,
            &self.audio_failure_count,
            &self.last_runtime_error,
            target,
            percent,
            is_sink,
            context,
        );
    }

    fn handle_midi(&mut self, msg: MidiMessage) {
        match msg {
            MidiMessage::Connected => {
                self.midi_connected.store(true, Ordering::Relaxed);
                self.midi_reconnect_count.fetch_add(1, Ordering::Relaxed);
                if self.logging_enabled {
                    info!("MIDI device connected");
                }
            }
            MidiMessage::Disconnected => {
                self.midi_connected.store(false, Ordering::Relaxed);
                if self.logging_enabled {
                    warn!("MIDI device disconnected — reconnecting...");
                }
            }
            MidiMessage::ControlChange { cc, value } => {
                if self.logging_enabled {
                    info!("MIDI CC{} -> value: {}", cc, value);
                }

                if let Some(&target_cc) = self.mapping.mute_button_mapping.get(&cc) {
                    if value > 0 {
                        self.handle_mute_button(cc, target_cc);
                    }
                    return;
                }

                if !self.mapping.cc_mapping.contains_key(&cc) {
                    return;
                }

                let percent = match self
                    .curve
                    .map_midi_value_to_percent(cc, value, &self.volume_curve)
                {
                    Some(p) => p,
                    None => return,
                };

                let now = Instant::now();
                if !self.curve.should_update(cc, percent, self.debounce_ms, now) {
                    return;
                }
                self.curve.last_volume_values.insert(cc, percent);
                self.curve.last_volume_time.insert(cc, now);

                let is_sink = self.mapping.cc_types.get(&cc).copied().unwrap_or(true);
                let target = self.mapping.cc_mapping.get(&cc).cloned();
                let fader_value = percent_to_fader_value(percent);

                if let Ok(mut shared) = self.shared_fader.lock() {
                    if is_sink {
                        if let Some(&idx) = self.mapping.cc_to_sink_index.get(&cc) {
                            if idx < shared.system_fader_values.len() {
                                shared.system_fader_values[idx] = fader_value;
                            }
                        }
                    } else if let Some(&idx) = self.mapping.cc_to_app_index.get(&cc) {
                        if idx < shared.app_fader_values.len() {
                            shared.app_fader_values[idx] = fader_value;
                        }
                    }
                }

                if let Some(target) = target {
                    self.send_audio_update(
                        target,
                        percent,
                        is_sink,
                        if is_sink {
                            "MIDI sink volume update"
                        } else {
                            "MIDI app volume update"
                        },
                    );
                }
            }
        }
    }

    fn handle_mute_button(&mut self, button_cc: u8, target_cc: u8) {
        let is_sink = self.mapping.cc_types.get(&target_cc).copied().unwrap_or(true);
        let target = self.mapping.cc_mapping.get(&target_cc).cloned();

        let idx = if is_sink {
            self.mapping.cc_to_sink_index.get(&target_cc).copied()
        } else {
            self.mapping.cc_to_app_index.get(&target_cc).copied()
        };
        let Some(idx) = idx else { return };

        let mut shared_guard = match self.shared_fader.lock() {
            Ok(shared) => shared,
            Err(_) => return,
        };
        let shared: &mut SharedFaderState = &mut shared_guard;

        let (muted_slot, value_slot, muted_volume_slot) = if is_sink {
            (
                shared.system_muted.get_mut(idx),
                shared.system_fader_values.get_mut(idx),
                shared.system_muted_volume.get_mut(idx),
            )
        } else {
            (
                shared.app_muted.get_mut(idx),
                shared.app_fader_values.get_mut(idx),
                shared.app_muted_volume.get_mut(idx),
            )
        };
        let (Some(muted), Some(value), Some(muted_volume)) = (muted_slot, value_slot, muted_volume_slot)
        else {
            return;
        };

        let (percent, new_muted) = if *muted {
            // Unmute: restore the last percent actually sent to pactl (curve-adjusted)
            // rather than re-deriving it linearly from the raw fader position, which
            // would lose the curve under non-linear volume curve modes.
            let previous_volume = *muted_volume;
            *value = previous_volume;
            let percent = self
                .curve
                .last_volume_values
                .get(&target_cc)
                .copied()
                .unwrap_or_else(|| ((previous_volume as f32) * MIDI_TO_PERCENT_FACTOR) as u8);
            (percent, false)
        } else {
            *muted_volume = *value;
            *value = 0;
            (0, true)
        };
        *muted = new_muted;
        drop(shared_guard);

        if new_muted {
            self.midi_output.light_button(button_cc);
        } else {
            self.midi_output.unlight_button(button_cc);
        }

        if self.logging_enabled {
            info!("CC{} {}", target_cc, if new_muted { "muted" } else { "unmuted" });
        }

        if let Some(target) = target {
            self.send_audio_update(
                target,
                percent,
                is_sink,
                if new_muted {
                    if is_sink { "Sink mute" } else { "App mute" }
                } else if is_sink {
                    "Sink unmute restore"
                } else {
                    "App unmute restore"
                },
            );
        }
    }

    fn handle_ui_event(&mut self, event: UiInputEvent) {
        match event {
            UiInputEvent::SliderChanged { is_sink, cc, fader_value } => {
                let percent = ((fader_value as f32) * MIDI_TO_PERCENT_FACTOR) as u8;
                self.curve.last_volume_values.insert(cc, percent);
                self.curve.inertia_smoothed_percent.insert(cc, percent as f32);
                self.curve.soft_takeover_armed.insert(cc, false);

                let target = self.mapping.cc_mapping.get(&cc).cloned();
                if let Some(target) = target {
                    self.send_audio_update(
                        target,
                        percent,
                        is_sink,
                        if is_sink {
                            "UI sink volume update"
                        } else {
                            "UI app volume update"
                        },
                    );
                }
                if self.logging_enabled {
                    info!("UI Slider CC{}: {}", cc, percent);
                }
            }
            UiInputEvent::UpdateMapping(mapping) => {
                self.mapping = mapping;
            }
            UiInputEvent::UpdateSettings {
                debounce_ms,
                logging_enabled,
                volume_curve,
            } => {
                self.debounce_ms = debounce_ms;
                self.logging_enabled = logging_enabled;
                self.volume_curve = volume_curve;
            }
        }
    }

    fn maybe_reconnect_midi_output(&mut self) {
        if !self.midi_output.is_enabled()
            && self.last_midi_output_reconnect.elapsed() >= MIDI_OUTPUT_RECONNECT_INTERVAL
        {
            self.last_midi_output_reconnect = Instant::now();
            self.midi_output = MidiOutput::new();
            self.midi_output_connected
                .store(self.midi_output.is_enabled(), Ordering::Relaxed);
        }
    }
}

/// Starts the MIDI listener and the always-on control thread, and returns a cheap-to-clone
/// handle for the GUI (or `main.rs`, while no window is open) to read/drive it.
pub fn start(
    mapping: ControlMapping,
    debounce_ms: u32,
    logging_enabled: bool,
    volume_curve: String,
    initial_fader_state: SharedFaderState,
    default_sink: &str,
) -> ControlHandle {
    let (midi_listener, midi_rx) = MidiListener::start();
    let (unified_tx, unified_rx) = mpsc::channel::<ThreadInput>();

    // Bridge thread: MidiListener only knows how to hand us a plain MidiMessage receiver,
    // so forward it into the same channel UI events arrive on — lets the control thread
    // block on one channel instead of needing to poll two.
    {
        let unified_tx = unified_tx.clone();
        std::thread::spawn(move || {
            for msg in midi_rx {
                if unified_tx.send(ThreadInput::Midi(msg)).is_err() {
                    break;
                }
            }
        });
    }

    let midi_output = MidiOutput::new();
    let midi_output_connected = Arc::new(AtomicBool::new(midi_output.is_enabled()));
    if logging_enabled && !midi_output.is_enabled() {
        warn!("MIDI LED feedback is disabled because the nanoKontrol2 output port could not be opened");
    }

    let pipewire = Arc::new(Mutex::new(PipeWireController::new(default_sink)));
    let (audio_update_tx, audio_update_rx) = mpsc::channel();
    let audio_failure_count = Arc::new(AtomicU64::new(0));
    let last_runtime_error = Arc::new(Mutex::new(None));
    spawn_audio_worker(
        pipewire.clone(),
        audio_update_rx,
        audio_failure_count.clone(),
        last_runtime_error.clone(),
    );

    let shared_fader = Arc::new(Mutex::new(initial_fader_state));
    let midi_connected = Arc::new(AtomicBool::new(false));
    let midi_reconnect_count = Arc::new(AtomicU32::new(0));

    let mut thread = ControlThread {
        mapping,
        debounce_ms,
        logging_enabled,
        volume_curve,
        curve: CurveState::default(),
        shared_fader: shared_fader.clone(),
        midi_output,
        midi_output_connected: midi_output_connected.clone(),
        midi_connected: midi_connected.clone(),
        midi_reconnect_count: midi_reconnect_count.clone(),
        audio_update_tx: audio_update_tx.clone(),
        audio_failure_count: audio_failure_count.clone(),
        last_runtime_error: last_runtime_error.clone(),
        last_midi_output_reconnect: Instant::now(),
    };

    std::thread::spawn(move || loop {
        match unified_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(ThreadInput::Midi(msg)) => thread.handle_midi(msg),
            Ok(ThreadInput::Ui(event)) => thread.handle_ui_event(event),
            Err(mpsc::RecvTimeoutError::Timeout) => thread.maybe_reconnect_midi_output(),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    });

    ControlHandle {
        shared_fader,
        pipewire,
        audio_failure_count,
        last_runtime_error,
        midi_connected,
        midi_output_connected,
        midi_reconnect_count,
        ui_input_tx: unified_tx,
        _midi_listener: Arc::new(midi_listener),
    }
}
