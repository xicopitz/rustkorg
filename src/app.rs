use crate::config::Config;
use crate::midi::{MidiListener, MidiMessage};
use crate::pipewire_control::PipeWireController;
use crate::ui::UiState;
use log::info;
use std::sync::mpsc;
use std::time::Instant;

pub struct MidiVolumeApp {
    ui_state: UiState,
    midi_rx: mpsc::Receiver<MidiMessage>,
    _midi_listener: MidiListener,
    pipewire: PipeWireController,
    config: Config,
    fader_mapping: Vec<Option<usize>>,  // Maps physical fader ID to UI index
    last_volume_values: Vec<Option<u8>>,  // Cache last sent volume for each fader
    last_volume_time: Vec<Option<Instant>>,  // Track last volume change time
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

        let fader_labels = config.get_fader_labels();
        let fader_mapping = config.get_fader_mapping();
        
        info!("Loaded {} faders from configuration", fader_labels.len());
        for (i, label) in fader_labels.iter().enumerate() {
            info!("  Fader {}: {}", i + 1, label);
        }

        // Start MIDI listener
        let (listener, rx) = MidiListener::start()
            .expect("Failed to initialize MIDI listener");

        // Initialize PipeWire controller with config mode
        let use_api = config.audio.volume_control_mode.as_deref() == Some("pipewire-api");
        info!("DEBUG: volume_control_mode = {:?}, use_api = {}", config.audio.volume_control_mode, use_api);
        let mut pipewire = PipeWireController::new(use_api);
        let _ = pipewire.discover_apps();

        let fader_count = fader_labels.len();
        let mut app = MidiVolumeApp {
            ui_state: UiState::new(fader_labels.clone()),
            midi_rx: rx,
            _midi_listener: listener,
            pipewire,
            config,
            fader_mapping,
            last_volume_values: vec![None; fader_count],
            last_volume_time: vec![None; fader_count],
        };

        // Initialize fader values with actual current volumes
        for (i, label) in fader_labels.iter().enumerate() {
            let label_lower = label.to_lowercase();
            let is_master = label_lower.contains("master");
            
            let current_volume = if is_master {
                app.pipewire.get_volume_percent()
            } else {
                app.pipewire.get_volume_for_app(label)
            };
            
            // Set UI fader to current volume (0-127 range)
            app.ui_state.fader_values[i] = ((current_volume as f32 / 100.0) * 127.0) as u8;
            app.last_volume_values[i] = Some(current_volume);
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
            .add_console_message("📝 Current Status: All faders control MASTER VOLUME".to_string());
        app.ui_state
            .add_console_message("   (Per-app volume requires advanced PipeWire integration)".to_string());
        app.ui_state
            .add_console_message("".to_string());
        app.ui_state
            .add_console_message("Waiting for fader movements...".to_string());
        app.ui_state
            .add_console_message("========================================".to_string());

        app
    }

    fn process_midi_messages(&mut self) {
        // Process all pending MIDI messages immediately for instant response
        while let Ok(msg) = self.midi_rx.try_recv() {
            match msg {
                MidiMessage::FaderChanged { fader_id, value } => {
                    // Check if this physical fader is configured and get its UI index
                    if fader_id < self.fader_mapping.len() {
                        if let Some(ui_index) = self.fader_mapping[fader_id] {
                            // Update the UI slider at the mapped position
                            self.ui_state.fader_values[ui_index] = value;
                            
                            let percent = (value as f32 / 127.0 * 100.0) as u8;
                            let app_name = self.ui_state.fader_labels[ui_index].clone();
                            
                            // Debounce: Skip if value hasn't changed or updated too recently
                            // Reduced default from 50ms to 10ms for better UI responsiveness
                            let debounce_ms = self.config.audio.debounce_ms.unwrap_or(0);
                            let now = Instant::now();
                            let should_update = if let Some(last_val) = self.last_volume_values[ui_index] {
                                if last_val == percent {
                                    false  // Same value, skip
                                } else if let Some(last_time) = self.last_volume_time[ui_index] {
                                    now.duration_since(last_time).as_millis() >= debounce_ms as u128
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
                            self.last_volume_values[ui_index] = Some(percent);
                            self.last_volume_time[ui_index] = Some(now);
                            
                            // Determine if this is a specific app or master volume
                            let app_name_lower = app_name.to_lowercase();
                            let is_master = app_name_lower.contains("master");
                            
                            // Update volume - either master or specific app (no logging for speed)
                            if is_master {
                                let _ = self.pipewire.set_volume_percent(percent);
                            } else {
                                let _ = self.pipewire.set_volume_for_app(&app_name, percent);
                            }
                        }
                    }
                }
                MidiMessage::KnobChanged { .. } => {
                    // Knob changes ignored for now
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
            crate::ui::Tab::Faders => self.ui_state.render_faders_tab(ctx),
            crate::ui::Tab::Console => self.ui_state.render_console_tab(ctx),
        }

        // Request immediate repaint for instant MIDI response
        // The UI will update as fast as possible when MIDI events occur
        ctx.request_repaint();
    }
}
