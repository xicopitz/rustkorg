use crate::config::Config;
use crate::midi::{MidiListener, MidiMessage, MidiOutput};
use crate::pipewire_control::PipeWireController;
use crate::ui::UiState;
use crate::spectrum::SpectrumAnalyzer;
use log::info;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

const MIDI_TO_PERCENT_FACTOR: f32 = 100.0 / 127.0;

pub struct MidiVolumeApp {
    ui_state: UiState,
    midi_rx: mpsc::Receiver<MidiMessage>,
    _midi_listener: MidiListener,
    midi_output: MidiOutput,  // MIDI output for LED feedback
    pipewire: Arc<Mutex<PipeWireController>>,  // Wrapped in Arc<Mutex> for thread-safe access
    cc_mapping: HashMap<u8, String>,  // Maps CC number to audio target name
    cc_types: HashMap<u8, bool>,  // Maps CC to is_sink (true=sink, false=app)
    last_volume_values: HashMap<u8, u8>,  // Cache last sent volume for each CC
    last_volume_time: HashMap<u8, Instant>,  // Track last volume change time
    cc_to_sink_index: HashMap<u8, usize>,  // Maps CC to sink UI index
    cc_to_app_index: HashMap<u8, usize>,  // Maps CC to app UI index
    mute_button_mapping: HashMap<u8, u8>,  // Maps mute button CC to target fader CC
    debounce_ms: u32,  // Cached debounce value
    logging_enabled: bool,  // Cached logging flag
    last_availability_check: Instant,  // Track last availability check time
    applications_sink_search_interval_secs: u64,  // Interval (in seconds) for checking app availability
    spectrum_analyzer: SpectrumAnalyzer,  // Spectrum analyzer for visualizer
    last_window_width: u32,  // Track previous window width for live resizing
    last_window_height: u32,  // Track previous window height for live resizing
    last_spectrum_sink_name: String,  // Track spectrum sink name for change detection
}

impl MidiVolumeApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Load configuration with fallback
        let config = Config::load_with_fallback(
            "config.toml",
            "~/.bin/audio/nanokontrol2/config.toml"
        )
            .unwrap_or_else(|e| {
                eprintln!("⚠️  Failed to load config.toml: {}", e);
                eprintln!("Using default configuration");
                Config::default()
            });

        let logging_enabled = config.logging.enabled.unwrap_or(true);
        let debounce_ms = config.audio.debounce_ms.unwrap_or(0);
        let applications_sink_search_interval_secs = config.audio.applications_sink_search.unwrap_or(10);
        let show_console = config.ui.show_console.unwrap_or(false);
        let max_console_lines = config.ui.max_console_lines.unwrap_or(1000);
        
        if logging_enabled {
            info!("Initializing MIDI Volume Controller");
        }

        let cc_mapping = config.get_cc_mapping();
        let sink_labels = config.get_sink_labels();
        let app_labels = config.get_app_labels();
        let cc_count = cc_mapping.len();
        
        // Build mapping of CC to type (bool: true=sink, false=app)
        let mut cc_types = HashMap::with_capacity(cc_count);
        for (cc, _) in &sink_labels {
            cc_types.insert(*cc, true);
        }
        for (cc, _) in &app_labels {
            cc_types.insert(*cc, false);
        }
        
        if logging_enabled {
            info!("Loaded {} MIDI controls from configuration", cc_count);
            info!("Sink controls:");
            for (cc, target) in &sink_labels {
                info!("  CC{}: {}", cc, target);
            }
            info!("Application controls:");
            for (cc, app_name) in &app_labels {
                info!("  CC{}: {}", cc, app_name);
            }
        }

        // Start MIDI listener
        let (listener, rx) = MidiListener::start()
            .expect("Failed to initialize MIDI listener");

        // Initialize PipeWire controller with config mode
        let use_api = config.audio.volume_control_mode.as_deref() == Some("pipewire-api");
        if logging_enabled {
            info!("DEBUG: volume_control_mode = {:?}, use_api = {}", config.audio.volume_control_mode, use_api);
        }
        let pipewire = Arc::new(Mutex::new(PipeWireController::new(use_api)));

        // Build CC to UI index mappings for fast lookup
        let mut cc_to_sink_index = HashMap::with_capacity(sink_labels.len());
        for (i, (cc, _)) in sink_labels.iter().enumerate() {
            cc_to_sink_index.insert(*cc, i);
        }
        
        let mut cc_to_app_index = HashMap::with_capacity(app_labels.len());
        for (i, (cc, _)) in app_labels.iter().enumerate() {
            cc_to_app_index.insert(*cc, i);
        }

        // Load mute button mappings
        let mute_button_mapping = config.get_mute_button_mappings();

        // Initialize MIDI output for LED feedback
        let midi_output = match MidiOutput::new() {
            Ok(output) => output,
            Err(e) => {
                if logging_enabled {
                    info!("Warning: Could not initialize MIDI output for LED feedback: {}", e);
                }
                // Try fallback, but if both fail, panic with clear message
                panic!("Failed to initialize MIDI output: {}. Is the nanoKontrol2 device connected?", e);
            }
        };

        // Initialize spectrum analyzer
        let default_sink = config.audio.default_sink.clone().unwrap_or_else(|| "master_sink".to_string());
        let mut spectrum_analyzer = SpectrumAnalyzer::new();
        spectrum_analyzer.start(&default_sink);

        let mut app = MidiVolumeApp {
            ui_state: UiState::new(
                sink_labels.clone(), 
                app_labels.clone(), 
                show_console, 
                max_console_lines,
                false,  // enable_tray
                false,  // close_to_tray
                false,  // start_minimized
                "config.toml".to_string(),  // config_path
                &config,
            ),
            midi_rx: rx,
            _midi_listener: listener,
            midi_output,
            pipewire: pipewire.clone(),
            cc_mapping,
            cc_types,
            last_volume_values: HashMap::with_capacity(cc_count),
            last_volume_time: HashMap::with_capacity(cc_count),
            cc_to_sink_index,
            cc_to_app_index,
            mute_button_mapping,
            debounce_ms,
            logging_enabled,
            last_availability_check: Instant::now(),
            applications_sink_search_interval_secs,
            spectrum_analyzer,
            last_window_width: config.ui.window_width.unwrap_or(1000),
            last_window_height: config.ui.window_height.unwrap_or(800),
            last_spectrum_sink_name: config.ui.spectrum_sink_name.clone()
                .unwrap_or_else(|| "master_sink".to_string()),
        };

        // Initialize UI fader values for sink controls
        for (i, (cc, target)) in sink_labels.iter().enumerate() {
            let current_volume = pipewire.lock().unwrap().get_volume_for_sink(target);
            
            // Set UI fader to current volume (0-127 range)
            app.ui_state.system_fader_values[i] = ((current_volume as f32 / 100.0) * 127.0) as u8;
            app.last_volume_values.insert(*cc, current_volume);
        }

        // Initialize UI fader values for application controls
        for (i, (cc, app_name)) in app_labels.iter().enumerate() {
            let current_volume = pipewire.lock().unwrap().get_volume_for_app(app_name);
            
            // Set UI fader to current volume (0-127 range)
            app.ui_state.app_fader_values[i] = ((current_volume as f32 / 100.0) * 127.0) as u8;
            app.last_volume_values.insert(*cc, current_volume);
        }


        // Only show console messages if logging is enabled
        if app.logging_enabled {
            const SEP: &str = "========================================";
            app.ui_state.add_console_message(SEP.to_string());
            app.ui_state.add_console_message("MIDI Volume Controller Started".to_string());
            app.ui_state.add_console_message("Listening for nanoKontrol2 MIDI input...".to_string());
            app.ui_state.add_console_message(String::new());
            app.ui_state.add_console_message(format!("📝 Loaded {} CC-to-target mappings", cc_count));
            app.ui_state.add_console_message(String::new());
            app.ui_state.add_console_message("Waiting for MIDI CC messages...".to_string());
            app.ui_state.add_console_message(SEP.to_string());
        }

        app
    }

    fn process_midi_messages(&mut self) {
        // Process all pending MIDI messages immediately for instant response
        while let Ok(msg) = self.midi_rx.try_recv() {
            let MidiMessage::ControlChange { cc, value } = msg;
            // Log MIDI CC message to console if logging is enabled
            if self.logging_enabled {
                    self.ui_state.add_console_message(format!("MIDI CC{} -> value: {}", cc, value));
                }
                
                // Check if this CC is a mute button
                if let Some(&target_cc) = self.mute_button_mapping.get(&cc) {
                    // Mute button pressed (CC value > 0 means button pressed on nanoKontrol2)
                    if value > 0 {
                        self.handle_mute_button(cc, target_cc);
                    }
                    continue;
                }
                
                // Check if this CC is mapped to an audio target (volume fader)
                if self.cc_mapping.contains_key(&cc) {
                    let percent = ((value as f32) * MIDI_TO_PERCENT_FACTOR) as u8;
                    
                    // Debounce: Skip if value hasn't changed or updated too recently
                    let now = Instant::now();
                    let should_update = if let Some(&last_val) = self.last_volume_values.get(&cc) {
                        if last_val == percent {
                            false  // Same value, skip
                        } else if let Some(&last_time) = self.last_volume_time.get(&cc) {
                            now.duration_since(last_time).as_millis() >= self.debounce_ms as u128
                        } else {
                            true
                        }
                    } else {
                        true  // First update
                    };
                    
                    if !should_update {
                        continue;  // Skip this update
                    }
                    
                    // Cache the new value and time
                    self.last_volume_values.insert(cc, percent);
                    self.last_volume_time.insert(cc, now);
                    
                    // Determine if this is a sink or app control
                    if self.cc_types.get(&cc).copied().unwrap_or(true) {
                        // Sink control - spawn thread to avoid blocking UI
                        if let Some(target) = self.cc_mapping.get(&cc) {
                            let pipewire = self.pipewire.clone();
                            let target_clone = target.clone();
                            thread::spawn(move || {
                                if let Ok(pw) = pipewire.lock() {
                                    let _ = pw.set_volume_for_sink(&target_clone, percent);
                                }
                            });
                        }
                        
                        // Update UI fader for this sink using cached index
                        if let Some(&ui_index) = self.cc_to_sink_index.get(&cc) {
                            if ui_index < self.ui_state.system_fader_values.len() {
                                self.ui_state.system_fader_values[ui_index] = value;
                            }
                        }
                    } else {
                        // App control - spawn thread to avoid blocking UI
                        if let Some(target) = self.cc_mapping.get(&cc) {
                            let pipewire = self.pipewire.clone();
                            let target_clone = target.clone();
                            thread::spawn(move || {
                                if let Ok(pw) = pipewire.lock() {
                                    let _ = pw.set_volume_for_app(&target_clone, percent);
                                }
                            });
                        }
                        
                        // Update UI fader for this app using cached index
                        if let Some(&ui_index) = self.cc_to_app_index.get(&cc) {
                            if ui_index < self.ui_state.app_fader_values.len() {
                                self.ui_state.app_fader_values[ui_index] = value;
                            }
                        }
                    }
                }
            }
        }
    
    fn handle_mute_button(&mut self, button_cc: u8, target_cc: u8) {
        // Determine if target is a sink or app
        let is_sink = self.cc_types.get(&target_cc).copied().unwrap_or(true);
        
        if is_sink {
            // Handle sink mute
            if let Some(&ui_index) = self.cc_to_sink_index.get(&target_cc) {
                if ui_index < self.ui_state.system_muted.len() {
                    self.toggle_sink_mute(ui_index, target_cc, button_cc);
                    if self.logging_enabled {
                        let muted = self.ui_state.system_muted[ui_index];
                        self.ui_state.add_console_message(
                            format!("🔇 CC{} {} ", target_cc, if muted { "muted" } else { "unmuted" })
                        );
                    }
                }
            }
        } else {
            // Handle app mute
            if let Some(&ui_index) = self.cc_to_app_index.get(&target_cc) {
                if ui_index < self.ui_state.app_muted.len() {
                    self.toggle_app_mute(ui_index, target_cc, button_cc);
                    if self.logging_enabled {
                        let muted = self.ui_state.app_muted[ui_index];
                        self.ui_state.add_console_message(
                            format!("🔇 CC{} {}", target_cc, if muted { "muted" } else { "unmuted" })
                        );
                    }
                }
            }
        }
    }
    
    fn toggle_sink_mute(&mut self, ui_index: usize, cc: u8, button_cc: u8) {
        let is_muted = self.ui_state.system_muted[ui_index];
        
        if is_muted {
            // Unmute: restore previous volume
            let previous_volume = self.ui_state.system_muted_volume[ui_index];
            self.ui_state.system_fader_values[ui_index] = previous_volume;
            self.ui_state.system_muted[ui_index] = false;
            
            // Turn off LED on button
            self.midi_output.unlight_button(button_cc);
            
            if let Some(target) = self.cc_mapping.get(&cc) {
                let percent = ((previous_volume as f32) * MIDI_TO_PERCENT_FACTOR) as u8;
                let pipewire = self.pipewire.clone();
                let target_clone = target.clone();
                
                // Spawn thread to avoid blocking UI
                thread::spawn(move || {
                    if let Ok(pw) = pipewire.lock() {
                        let _ = pw.set_volume_for_sink(&target_clone, percent);
                    }
                });
            }
        } else {
            // Mute: save current volume and set to 0
            let current_volume = self.ui_state.system_fader_values[ui_index];
            self.ui_state.system_muted_volume[ui_index] = current_volume;
            self.ui_state.system_fader_values[ui_index] = 0;
            self.ui_state.system_muted[ui_index] = true;
            
            // Turn on LED on button
            self.midi_output.light_button(button_cc);
            
            if let Some(target) = self.cc_mapping.get(&cc) {
                let pipewire = self.pipewire.clone();
                let target_clone = target.clone();
                
                // Spawn thread to avoid blocking UI
                thread::spawn(move || {
                    if let Ok(pw) = pipewire.lock() {
                        let _ = pw.set_volume_for_sink(&target_clone, 0);
                    }
                });
            }
        }
    }
    
    fn toggle_app_mute(&mut self, ui_index: usize, cc: u8, button_cc: u8) {
        let is_muted = self.ui_state.app_muted[ui_index];
        
        if is_muted {
            // Unmute: restore previous volume
            let previous_volume = self.ui_state.app_muted_volume[ui_index];
            self.ui_state.app_fader_values[ui_index] = previous_volume;
            self.ui_state.app_muted[ui_index] = false;
            
            // Turn off LED on button
            self.midi_output.unlight_button(button_cc);
            
            if let Some(target) = self.cc_mapping.get(&cc) {
                let percent = ((previous_volume as f32) * MIDI_TO_PERCENT_FACTOR) as u8;
                let pipewire = self.pipewire.clone();
                let target_clone = target.clone();
                
                // Spawn thread to avoid blocking UI
                thread::spawn(move || {
                    if let Ok(pw) = pipewire.lock() {
                        let _ = pw.set_volume_for_app(&target_clone, percent);
                    }
                });
            }
        } else {
            // Mute: save current volume and set to 0
            let current_volume = self.ui_state.app_fader_values[ui_index];
            self.ui_state.app_muted_volume[ui_index] = current_volume;
            self.ui_state.app_fader_values[ui_index] = 0;
            self.ui_state.app_muted[ui_index] = true;
            
            // Turn on LED on button
            self.midi_output.light_button(button_cc);
            
            if let Some(target) = self.cc_mapping.get(&cc) {
                let pipewire = self.pipewire.clone();
                let target_clone = target.clone();
                
                // Spawn thread to avoid blocking UI
                thread::spawn(move || {
                    if let Ok(pw) = pipewire.lock() {
                        let _ = pw.set_volume_for_app(&target_clone, 0);
                    }
                });
            }
        }
    }
    
    fn process_ui_slider_changes(&mut self, changed_faders: Vec<(bool, usize, u8)>) {
        for (is_sink, ui_index, new_value) in changed_faders {
            if is_sink {
                // Handle sink volume change from UI
                if ui_index < self.ui_state.system_fader_labels.len() {
                    let cc = self.ui_state.system_fader_labels[ui_index].0;
                    let percent = ((new_value as f32) * MIDI_TO_PERCENT_FACTOR) as u8;
                    
                    if let Some(target) = self.cc_mapping.get(&cc) {
                        let pipewire = self.pipewire.clone();
                        let target_clone = target.clone();
                        
                        // Spawn thread to avoid blocking UI
                        thread::spawn(move || {
                            if let Ok(pw) = pipewire.lock() {
                                let _ = pw.set_volume_for_sink(&target_clone, percent);
                            }
                        });
                    }
                    
                    if self.logging_enabled {
                        self.ui_state.add_console_message(
                            format!("UI Slider CC{}: {}", cc, percent)
                        );
                    }
                }
            } else {
                // Handle app volume change from UI
                if ui_index < self.ui_state.app_fader_labels.len() {
                    let cc = self.ui_state.app_fader_labels[ui_index].0;
                    let percent = ((new_value as f32) * MIDI_TO_PERCENT_FACTOR) as u8;
                    
                    if let Some(target) = self.cc_mapping.get(&cc) {
                        let pipewire = self.pipewire.clone();
                        let target_clone = target.clone();
                        
                        // Spawn thread to avoid blocking UI
                        thread::spawn(move || {
                            if let Ok(pw) = pipewire.lock() {
                                let _ = pw.set_volume_for_app(&target_clone, percent);
                            }
                        });
                    }
                    
                    if self.logging_enabled {
                        self.ui_state.add_console_message(
                            format!("UI Slider CC{}: {}", cc, percent)
                        );
                    }
                }
            }
        }
    }

    fn check_audio_availability(&mut self) {
        // Check at configured interval (default 10 seconds)
        if self.last_availability_check.elapsed().as_secs() < self.applications_sink_search_interval_secs {
            return;
        }
        self.last_availability_check = Instant::now();

        // Check sink availability - assume available unless it errors
        if let Ok(pipewire) = self.pipewire.lock() {
            for i in 0..self.ui_state.system_fader_labels.len() {
                let sink_name = &self.ui_state.system_fader_labels[i].1;
                // Sinks are typically always available, so default to true
                let _ = pipewire.get_volume_for_sink(sink_name);
                self.ui_state.system_available[i] = true;
            }

            // Check app availability
            for i in 0..self.ui_state.app_fader_labels.len() {
                let app_name = &self.ui_state.app_fader_labels[i].1;
                // For apps, we need to check if they appear in the pactl list
                let is_available = pipewire.is_app_available(app_name);
                self.ui_state.app_available[i] = is_available;
            }
        }
    }
    
    fn save_settings(&mut self) {
        // Create config from UI state
        let config = Config::from_ui_state(
            &self.ui_state.cfg_sinks,
            &self.ui_state.cfg_applications,
            &self.ui_state.cfg_mute_buttons,
            self.ui_state.cfg_use_pipewire,
            &self.ui_state.cfg_default_sink,
            &self.ui_state.cfg_volume_control_mode,
            &self.ui_state.cfg_volume_curve,
            self.ui_state.cfg_debounce_ms,
            self.ui_state.cfg_applications_sink_search,
            self.ui_state.cfg_window_width,
            self.ui_state.cfg_window_height,
            &self.ui_state.cfg_theme,
            self.ui_state.cfg_show_console,
            self.ui_state.cfg_max_console_lines,
            self.ui_state.cfg_show_spectrum,
            self.ui_state.cfg_spectrum_stereo_mode,
            self.ui_state.cfg_spectrum_show_waterfall,
            self.ui_state.cfg_spectrum_show_labels,
            &self.ui_state.cfg_spectrum_sink_name,
            self.ui_state.cfg_logging_enabled,
            &self.ui_state.cfg_log_level,
            self.ui_state.cfg_timestamps,
            self.ui_state.cfg_log_fader_events,
            self.ui_state.cfg_log_device_info,
        );
        
        // Save to file
        match config.save_to_file(&self.ui_state.config_path) {
            Ok(()) => {
                self.ui_state.settings_dirty = false;
                self.ui_state.settings_save_message = Some((
                    "SUCCESS: Settings saved".to_string(),
                    std::time::Instant::now()
                ));
                
                // Reload config from file
                if let Ok(reloaded_config) = Config::load_with_fallback(
                    &self.ui_state.config_path,
                    "~/.bin/audio/nanokontrol2/config.toml"
                ) {
                    // Update runtime values from reloaded config
                    self.debounce_ms = reloaded_config.audio.debounce_ms.unwrap_or(0);
                    self.applications_sink_search_interval_secs = reloaded_config.audio.applications_sink_search.unwrap_or(10);
                    self.logging_enabled = reloaded_config.logging.enabled.unwrap_or(true);
                    
                    // Reload sink and app mappings
                    self.cc_mapping = reloaded_config.get_cc_mapping();
                    let sink_labels = reloaded_config.get_sink_labels();
                    let app_labels = reloaded_config.get_app_labels();
                    
                    // Update UI state fader arrays to match new configuration
                    self.ui_state.system_fader_labels = sink_labels.clone();
                    self.ui_state.system_fader_values.resize(sink_labels.len(), 0);
                    self.ui_state.system_muted.resize(sink_labels.len(), false);
                    self.ui_state.system_muted_volume.resize(sink_labels.len(), 0);
                    self.ui_state.system_available.resize(sink_labels.len(), true);
                    
                    self.ui_state.app_fader_labels = app_labels.clone();
                    self.ui_state.app_fader_values.resize(app_labels.len(), 0);
                    self.ui_state.app_muted.resize(app_labels.len(), false);
                    self.ui_state.app_muted_volume.resize(app_labels.len(), 0);
                    self.ui_state.app_available.resize(app_labels.len(), true);
                    
                    // Rebuild CC type mappings
                    self.cc_types.clear();
                    for (cc, _) in &sink_labels {
                        self.cc_types.insert(*cc, true);
                    }
                    for (cc, _) in &app_labels {
                        self.cc_types.insert(*cc, false);
                    }
                    
                    // Rebuild CC to UI index mappings
                    self.cc_to_sink_index.clear();
                    for (i, (cc, _)) in sink_labels.iter().enumerate() {
                        self.cc_to_sink_index.insert(*cc, i);
                    }
                    self.cc_to_app_index.clear();
                    for (i, (cc, _)) in app_labels.iter().enumerate() {
                        self.cc_to_app_index.insert(*cc, i);
                    }
                    
                    // Reload mute button mappings
                    self.mute_button_mapping = reloaded_config.get_mute_button_mappings();
                }
                
                if self.logging_enabled {
                    self.ui_state.add_console_message("Settings saved and reloaded from config.toml".to_string());
                }
            }
            Err(e) => {
                self.ui_state.settings_save_message = Some((
                    format!("ERROR: {}", e),
                    std::time::Instant::now()
                ));
                
                if self.logging_enabled {
                    self.ui_state.add_console_message(format!("Error saving settings: {}", e));
                }
            }
        }
    }
}

impl eframe::App for MidiVolumeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for window size changes and apply them
        if self.last_window_width != self.ui_state.cfg_window_width || 
           self.last_window_height != self.ui_state.cfg_window_height {
            let new_size = egui::Vec2::new(
                self.ui_state.cfg_window_width as f32,
                self.ui_state.cfg_window_height as f32,
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));
            
            self.last_window_width = self.ui_state.cfg_window_width;
            self.last_window_height = self.ui_state.cfg_window_height;
        }
        
        // Check for spectrum sink name changes
        if self.last_spectrum_sink_name != self.ui_state.cfg_spectrum_sink_name {
            self.last_spectrum_sink_name = self.ui_state.cfg_spectrum_sink_name.clone();
            self.spectrum_analyzer.start(&self.ui_state.cfg_spectrum_sink_name);
        }
        
        // Process incoming MIDI messages immediately
        self.process_midi_messages();

        // Check audio availability every 10 seconds
        self.check_audio_availability();
        
        // Update spectrum data from analyzer
        self.ui_state.spectrum_data = self.spectrum_analyzer.get_data();

        // Render UI
        self.ui_state.render_tabs(ctx);

        let changed_faders = match self.ui_state.selected_tab {
            crate::ui::Tab::Control => self.ui_state.render_faders_tab(ctx),
            crate::ui::Tab::Console => {
                self.ui_state.render_console_tab(ctx);
                Vec::new()
            }
            crate::ui::Tab::Settings => {
                let settings_changed = self.ui_state.render_settings_tab(ctx, false);
                if settings_changed && self.ui_state.settings_dirty {
                    // Save settings to config file
                    self.save_settings();
                }
                Vec::new()
            }
        };
        
        // Render MIDI UI modal if open
        crate::panels::render_midi_ui_modal(&mut self.ui_state, ctx);
        
        // Handle UI slider changes
        self.process_ui_slider_changes(changed_faders);

        // Request continuous repainting for instant MIDI response
        // This ensures the UI updates immediately when MIDI events occur
        ctx.request_repaint();
        
        // Also request a repaint for the next frame to maintain responsiveness
        ctx.request_repaint_after(std::time::Duration::from_millis(16)); // ~60 FPS
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Save settings on app exit
        if self.ui_state.settings_dirty {
            self.save_settings();
        }
    }
}
