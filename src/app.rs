use crate::config::Config;
use crate::midi::{MidiListener, MidiMessage};
use crate::pipewire_control::PipeWireController;
use crate::ui::UiState;
use log::info;
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Instant;

    pub struct MidiVolumeApp {
    ui_state: UiState,
    midi_rx: mpsc::Receiver<MidiMessage>,
    _midi_listener: MidiListener,
    pipewire: PipeWireController,
    config: Config,
    cc_mapping: HashMap<u8, String>,  // Maps CC number to audio target name
    sink_ccs: Vec<u8>,  // CCs that control sinks
    last_volume_values: HashMap<u8, u8>,  // Cache last sent volume for each CC
    last_volume_time: HashMap<u8, Instant>,  // Track last volume change time
}

impl MidiVolumeApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        info!("Initializing MIDI Volume Controller");

        // Load configuration
        let config = Config::load("config.toml")
            .unwrap_or_else(|e| {
                eprintln!("⚠️  Failed to load config.toml: {}", e);
                eprintln!("Using default configuration");
                Config::default()
            });

        let cc_mapping = config.get_cc_mapping();
        let sink_labels = config.get_sink_labels();
        let cc_count = cc_mapping.len();
        
        // Build list of CCs that control sinks
        let sink_ccs: Vec<u8> = sink_labels.iter().map(|(cc, _)| *cc).collect();
        
        info!("Loaded {} MIDI controls from configuration", cc_count);
        info!("Sink controls:");
        for (cc, target) in &sink_labels {
            info!("  CC{}: {}", cc, target);
        }

        // Start MIDI listener
        let (listener, rx) = MidiListener::start()
            .expect("Failed to initialize MIDI listener");

        // Initialize PipeWire controller with config mode
        let use_api = config.audio.volume_control_mode.as_deref() == Some("pipewire-api");
        info!("DEBUG: volume_control_mode = {:?}, use_api = {}", config.audio.volume_control_mode, use_api);
        let mut pipewire = PipeWireController::new(use_api);
        let _ = pipewire.discover_apps();

        let mut app = MidiVolumeApp {
            ui_state: UiState::new(sink_labels.clone()),
            midi_rx: rx,
            _midi_listener: listener,
            pipewire,
            config,
            cc_mapping,
            sink_ccs,
            last_volume_values: HashMap::new(),
            last_volume_time: HashMap::new(),
        };

        // Initialize UI fader values for sink controls
        for (i, (cc, target)) in sink_labels.iter().enumerate() {
            let current_volume = app.pipewire.get_volume_for_sink(target);
            
            // Set UI fader to current volume (0-127 range)
            app.ui_state.system_fader_values[i] = ((current_volume as f32 / 100.0) * 127.0) as u8;
            app.last_volume_values.insert(*cc, current_volume);
        }


        app.ui_state
            .add_console_message("========================================".to_string());
        app.ui_state
            .add_console_message("MIDI Volume Controller Started".to_string());
        app.ui_state
            .add_console_message("Listening for nanoKontrol2 MIDI input...".to_string());
        app.ui_state
            .add_console_message("".to_string());
        app.ui_state
            .add_console_message(format!("📝 Loaded {} CC-to-target mappings", cc_count));
        app.ui_state
            .add_console_message("".to_string());
        app.ui_state
            .add_console_message("Waiting for MIDI CC messages...".to_string());
        app.ui_state
            .add_console_message("========================================".to_string());

        app
    }

    fn process_midi_messages(&mut self) {
        // Process all pending MIDI messages immediately for instant response
        while let Ok(msg) = self.midi_rx.try_recv() {
            match msg {
                MidiMessage::ControlChange { cc, value } => {
                    // Log MIDI CC message to console
                    self.ui_state.add_console_message(
                        format!("MIDI CC{} -> value: {}", cc, value)
                    );
                    
                    // Check if this CC is mapped to an audio target
                    if let Some(target) = self.cc_mapping.get(&cc) {
                        let percent = (value as f32 / 127.0 * 100.0) as u8;
                        
                        // Debounce: Skip if value hasn't changed or updated too recently
                        let debounce_ms = self.config.audio.debounce_ms.unwrap_or(0);
                        let now = Instant::now();
                        let should_update = if let Some(last_val) = self.last_volume_values.get(&cc) {
                            if *last_val == percent {
                                false  // Same value, skip
                            } else if let Some(last_time) = self.last_volume_time.get(&cc) {
                                now.duration_since(*last_time).as_millis() >= debounce_ms as u128
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
                        
                        // Update sink volume
                        let _ = self.pipewire.set_volume_for_sink(target, percent);
                        
                        // Update UI if this CC is displayed
                        let sink_labels = self.config.get_sink_labels();
                        
                        // Check if it's a sink control
                        if let Some(ui_index) = sink_labels.iter().position(|(c, _)| *c == cc) {
                            if ui_index < self.ui_state.system_fader_values.len() {
                                self.ui_state.system_fader_values[ui_index] = value;
                            }
                        }
                    }
                }
                _ => {}
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
