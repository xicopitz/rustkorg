use crate::config::Config;
use crate::midi::{MidiListener, MidiMessage, MidiOutput};
use crate::pipewire_control::PipeWireController;
use crate::ui::UiState;
use log::info;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

const MIDI_TO_PERCENT_FACTOR: f32 = 100.0 / 127.0;

pub struct MidiVolumeApp {
    ui_state: UiState,
    midi_rx: mpsc::Receiver<MidiMessage>,
    _midi_listener: MidiListener,
    midi_output: MidiOutput,  // MIDI output for LED feedback
    pipewire: PipeWireController,
    cc_mapping: HashMap<u8, String>,  // Maps CC number to audio target name
    cc_types: HashMap<u8, bool>,  // Maps CC to is_sink (true=sink, false=app)
    last_volume_values: HashMap<u8, u8>,  // Cache last sent volume for each CC
    last_volume_time: HashMap<u8, Instant>,  // Track last volume change time
    cc_to_sink_index: HashMap<u8, usize>,  // Maps CC to sink UI index
    cc_to_app_index: HashMap<u8, usize>,  // Maps CC to app UI index
    mute_button_mapping: HashMap<u8, u8>,  // Maps mute button CC to target fader CC
    debounce_ms: u32,  // Cached debounce value
    logging_enabled: bool,  // Cached logging flag
}

impl MidiVolumeApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Load configuration
        let config = Config::load("config.toml")
            .unwrap_or_else(|e| {
                eprintln!("⚠️  Failed to load config.toml: {}", e);
                eprintln!("Using default configuration");
                Config::default()
            });

        let logging_enabled = config.logging.enabled.unwrap_or(true);
        let debounce_ms = config.audio.debounce_ms.unwrap_or(0);
        
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
        let mut pipewire = PipeWireController::new(use_api);
        let _ = pipewire.discover_apps();

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

        let mut app = MidiVolumeApp {
            ui_state: UiState::new(sink_labels.clone(), app_labels.clone()),
            midi_rx: rx,
            _midi_listener: listener,
            midi_output,
            pipewire,
            cc_mapping,
            cc_types,
            last_volume_values: HashMap::with_capacity(cc_count),
            last_volume_time: HashMap::with_capacity(cc_count),
            cc_to_sink_index,
            cc_to_app_index,
            mute_button_mapping,
            debounce_ms,
            logging_enabled,
        };

        // Initialize UI fader values for sink controls
        for (i, (cc, target)) in sink_labels.iter().enumerate() {
            let current_volume = app.pipewire.get_volume_for_sink(target);
            
            // Set UI fader to current volume (0-127 range)
            app.ui_state.system_fader_values[i] = ((current_volume as f32 / 100.0) * 127.0) as u8;
            app.last_volume_values.insert(*cc, current_volume);
        }

        // Initialize UI fader values for application controls
        for (i, (cc, app_name)) in app_labels.iter().enumerate() {
            let current_volume = app.pipewire.get_volume_for_app(app_name);
            
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
            if let MidiMessage::ControlChange { cc, value } = msg {
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
                if let Some(target) = self.cc_mapping.get(&cc) {
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
                        // Sink control
                        let target_clone = target.clone();
                        thread::spawn(move || {
                            let pipewire = PipeWireController::new(false);
                            let _ = pipewire.set_volume_for_sink(&target_clone, percent);
                        });
                        
                        // Update UI fader for this sink using cached index
                        if let Some(&ui_index) = self.cc_to_sink_index.get(&cc) {
                            if ui_index < self.ui_state.system_fader_values.len() {
                                self.ui_state.system_fader_values[ui_index] = value;
                            }
                        }
                    } else {
                        // App control
                        let target_clone = target.clone();
                        thread::spawn(move || {
                            let pipewire = PipeWireController::new(false);
                            let _ = pipewire.set_volume_for_app(&target_clone, percent);
                        });
                        
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
                let target_clone = target.clone();
                thread::spawn(move || {
                    let pipewire = PipeWireController::new(false);
                    let _ = pipewire.set_volume_for_sink(&target_clone, percent);
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
                let target_clone = target.clone();
                thread::spawn(move || {
                    let pipewire = PipeWireController::new(false);
                    let _ = pipewire.set_volume_for_sink(&target_clone, 0);
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
                let target_clone = target.clone();
                thread::spawn(move || {
                    let pipewire = PipeWireController::new(false);
                    let _ = pipewire.set_volume_for_app(&target_clone, percent);
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
                let target_clone = target.clone();
                thread::spawn(move || {
                    let pipewire = PipeWireController::new(false);
                    let _ = pipewire.set_volume_for_app(&target_clone, 0);
                });
            }
        }
    }
}

impl eframe::App for MidiVolumeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process incoming MIDI messages immediately
        self.process_midi_messages();

        // Render UI
        self.ui_state.render_tabs(ctx);

        match self.ui_state.selected_tab {
            crate::ui::Tab::Control => self.ui_state.render_faders_tab(ctx),
            crate::ui::Tab::Console => self.ui_state.render_console_tab(ctx),
        }

        // Request immediate repaint for instant MIDI response
        // The UI will update as fast as possible when MIDI events occur
        ctx.request_repaint();
    }
}
