use crate::config::Config;
use crate::midi::{MidiListener, MidiMessage, MidiOutput};
use crate::pipewire_control::PipeWireController;
use crate::spectrum::SpectrumAnalyzer;
use crate::ui::UiState;
use log::{info, warn};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MIDI_TO_PERCENT_FACTOR: f32 = 100.0 / 127.0;
const MIDI_FAST_MOVE_DELTA_PERCENT: u8 = 2;

struct AudioUpdateCommand {
    target: String,
    percent: u8,
    is_sink: bool,
    context: &'static str,
}

pub struct MidiVolumeApp {
    ui_state: UiState,
    midi_rx: mpsc::Receiver<MidiMessage>,
    _midi_listener: MidiListener, // kept alive to hold the channel sender open
    midi_startup_error: Option<String>,
    midi_output: MidiOutput,                  // MIDI output for LED feedback
    last_midi_output_reconnect: Instant,      // Throttle output reconnect attempts
    pipewire: Arc<Mutex<PipeWireController>>, // Wrapped in Arc<Mutex> for thread-safe access
    audio_update_tx: mpsc::Sender<AudioUpdateCommand>,
    cc_mapping: HashMap<u8, String>,          // Maps CC number to audio target name
    cc_types: HashMap<u8, bool>,              // Maps CC to is_sink (true=sink, false=app)
    last_volume_values: HashMap<u8, u8>,      // Cache last sent volume for each CC
    last_volume_time: HashMap<u8, Instant>,   // Track last volume change time
    cc_to_sink_index: HashMap<u8, usize>,     // Maps CC to sink UI index
    cc_to_app_index: HashMap<u8, usize>,      // Maps CC to app UI index
    mute_button_mapping: HashMap<u8, u8>,     // Maps mute button CC to target fader CC
    debounce_ms: u32,                         // Cached debounce value
    logging_enabled: bool,                    // Cached logging flag
    last_availability_check: Instant,         // Track last availability check time
    applications_sink_search_interval_secs: u64, // Interval (in seconds) for checking app availability
    spectrum_analyzer: SpectrumAnalyzer,         // Spectrum analyzer for visualizer
    last_window_width: u32,                      // Track previous window width for live resizing
    last_window_height: u32,                     // Track previous window height for live resizing
    last_spectrum_sink_name: String,             // Track spectrum sink name for change detection
    soft_takeover_armed: HashMap<u8, bool>,
    inertia_smoothed_percent: HashMap<u8, f32>,
    audio_failure_count: Arc<AtomicU64>,
    last_runtime_error: Arc<Mutex<Option<String>>>,
    midi_reconnect_count: u32,
    // System tray
    tray_handle: Option<ksni::Handle<crate::tray::AppTray>>,
    tray_rx: Option<mpsc::Receiver<crate::tray::TrayCommand>>,
    window_visible: bool,
    force_quit: bool, // skip close-to-tray intercept when quitting from tray menu
}

impl MidiVolumeApp {
    fn build_tray_cc_assignments(&self) -> Vec<String> {
        let mut rows: Vec<(u8, String)> = self
            .ui_state
            .cfg_applications
            .iter()
            .enumerate()
            .filter(|(idx, _)| self.ui_state.app_available.get(*idx).copied().unwrap_or(false))
            .map(|(_, (cc, name))| (*cc, format!("CC{} -> {}", cc, name)))
            .collect();

        rows.sort_by_key(|(cc, _)| *cc);
        rows.into_iter().map(|(_, row)| row).collect()
    }

    fn refresh_tray_state(&self) {
        if let Some(handle) = &self.tray_handle {
            let visible = self.window_visible;
            let cc_assignments = self.build_tray_cc_assignments();
            handle.update(move |tray| {
                tray.visible = visible;
                tray.cc_assignments = cc_assignments;
            });
        }
    }

    fn initialize_midi_listener() -> (MidiListener, mpsc::Receiver<MidiMessage>) {
        MidiListener::start()
    }

    fn render_midi_status_banner(&mut self, ctx: &egui::Context) {
        if let Some(error) = self.midi_startup_error.clone() {
            egui::TopBottomPanel::top("midi_status_panel")
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(format!("⚠  {}", error))
                                .color(egui::Color32::from_rgb(255, 190, 120)),
                        );
                    });
                });
        }
    }

    fn set_last_runtime_error(last_runtime_error: &Arc<Mutex<Option<String>>>, message: String) {
        if let Ok(mut last_error) = last_runtime_error.lock() {
            *last_error = Some(message);
        }
    }

    fn start_audio_worker(
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

                // Coalesce bursts by keeping only the latest update per target.
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
                                Self::set_last_runtime_error(&last_runtime_error, message);
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
                        Self::set_last_runtime_error(&last_runtime_error, message);
                        pending.clear();
                    }
                }
            }
        });
    }

    fn spawn_audio_update(&self, target: String, percent: u8, is_sink: bool, context: &'static str) {
        if let Err(e) = self.audio_update_tx.send(AudioUpdateCommand {
            target,
            percent,
            is_sink,
            context,
        }) {
            let message = format!("Failed to enqueue audio update: {}", e);
            warn!("{}", message);
            self.audio_failure_count.fetch_add(1, Ordering::Relaxed);
            Self::set_last_runtime_error(&self.last_runtime_error, message);
        }
    }

    fn map_midi_value_to_percent(&mut self, cc: u8, value: u8) -> Option<u8> {
        let raw_percent = ((value as f32) * MIDI_TO_PERCENT_FACTOR) as u8;
        let mode = self.ui_state.cfg_volume_curve.to_lowercase();

        match mode.as_str() {
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

                // If hardware and software positions are far apart, wait until they meet.
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

    fn apply_app_mapping_changes(&mut self) {
        self.ui_state.cfg_applications.sort_by_key(|(cc, _)| *cc);
        let new_labels = self.ui_state.cfg_applications.clone();
        let old_labels = self.ui_state.app_fader_labels.clone();

        let mut old_values = HashMap::new();
        let mut old_muted = HashMap::new();
        let mut old_muted_volume = HashMap::new();
        let mut old_availability = HashMap::new();
        let mut old_visibility = HashMap::new();

        for (idx, (cc, _)) in old_labels.iter().enumerate() {
            old_values.insert(*cc, *self.ui_state.app_fader_values.get(idx).unwrap_or(&0));
            old_muted.insert(*cc, *self.ui_state.app_muted.get(idx).unwrap_or(&false));
            old_muted_volume.insert(*cc, *self.ui_state.app_muted_volume.get(idx).unwrap_or(&0));
            old_availability.insert(*cc, *self.ui_state.app_available.get(idx).unwrap_or(&true));
            old_visibility.insert(*cc, *self.ui_state.app_visibility.get(idx).unwrap_or(&true));
        }

        self.ui_state.app_fader_labels = new_labels.clone();
        self.ui_state.app_fader_values = new_labels
            .iter()
            .map(|(cc, _)| old_values.get(cc).copied().unwrap_or(0))
            .collect();
        self.ui_state.app_muted = new_labels
            .iter()
            .map(|(cc, _)| old_muted.get(cc).copied().unwrap_or(false))
            .collect();
        self.ui_state.app_muted_volume = new_labels
            .iter()
            .map(|(cc, _)| old_muted_volume.get(cc).copied().unwrap_or(0))
            .collect();
        self.ui_state.app_available = new_labels
            .iter()
            .map(|(cc, _)| old_availability.get(cc).copied().unwrap_or(true))
            .collect();
        self.ui_state.app_visibility = new_labels
            .iter()
            .map(|(cc, _)| old_visibility.get(cc).copied().unwrap_or(true))
            .collect();
        self.ui_state.app_input_count = vec![0; new_labels.len()];
        self.ui_state.app_display_order = (0..new_labels.len()).collect();

        for (cc, _) in old_labels {
            self.cc_mapping.remove(&cc);
            self.cc_types.remove(&cc);
            self.cc_to_app_index.remove(&cc);
        }

        for (idx, (cc, app_name)) in new_labels.iter().enumerate() {
            self.cc_mapping.insert(*cc, app_name.clone());
            self.cc_types.insert(*cc, false);
            self.cc_to_app_index.insert(*cc, idx);
        }

        self.refresh_tray_state();
    }

    fn auto_assign_active_apps(&mut self) {
        let active_apps = match self.pipewire.lock() {
            Ok(pw) => pw.list_active_application_names(),
            Err(e) => {
                let message = format!("Auto-assign failed: could not access PipeWire controller: {}", e);
                warn!("{}", message);
                Self::set_last_runtime_error(&self.last_runtime_error, message.clone());
                self.ui_state.settings_save_message =
                    Some((format!("ERROR: {}", message), std::time::Instant::now()));
                return;
            }
        };

        if active_apps.is_empty() {
            self.ui_state.settings_save_message = Some((
                "INFO: No active app streams detected".to_string(),
                std::time::Instant::now(),
            ));
            return;
        }

        let existing_names: Vec<String> = self
            .ui_state
            .cfg_applications
            .iter()
            .map(|(_, name)| name.to_lowercase())
            .collect();

        let mut used_cc = std::collections::HashSet::new();
        for (cc, _) in &self.ui_state.cfg_sinks {
            used_cc.insert(*cc);
        }
        for (cc, _) in &self.ui_state.cfg_applications {
            used_cc.insert(*cc);
        }

        let mut free_ccs: Vec<u8> = (16u8..=63u8).filter(|cc| !used_cc.contains(cc)).collect();
        let mut added = Vec::new();

        for app_name in active_apps {
            if existing_names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&app_name))
            {
                continue;
            }

            if free_ccs.is_empty() {
                break;
            }

            let cc = free_ccs.remove(0);
            self.ui_state.cfg_applications.push((cc, app_name.clone()));
            added.push((cc, app_name));
        }

        if added.is_empty() {
            self.ui_state.settings_save_message = Some((
                "INFO: Active apps already mapped or no free CC slots".to_string(),
                std::time::Instant::now(),
            ));
            return;
        }

        self.apply_app_mapping_changes();
        self.ui_state.settings_dirty = true;

        let summary = format!("SUCCESS: Auto-assigned {} app(s)", added.len());
        self.ui_state.settings_save_message = Some((summary.clone(), std::time::Instant::now()));

        if self.logging_enabled {
            self.ui_state
                .add_console_message(format!("{}", summary));
            for (cc, app_name) in added {
                self.ui_state
                    .add_console_message(format!("Mapped '{}' to CC{}", app_name, cc));
            }
        }
    }

    fn sync_device_health_to_ui(&mut self) {
        // Attempt to reconnect MIDI output if it went away (throttled to every 10s)
        if !self.midi_output.is_enabled()
            && self.last_midi_output_reconnect.elapsed() >= Duration::from_secs(10)
        {
            self.last_midi_output_reconnect = Instant::now();
            self.midi_output = MidiOutput::new();
        }

        self.ui_state.health_midi_input_connected = self.midi_startup_error.is_none();
        self.ui_state.health_midi_output_connected = self.midi_output.is_enabled();
        self.ui_state.health_pipewire_default_sink = self.ui_state.cfg_default_sink.clone();
        self.ui_state.health_audio_failure_count = self.audio_failure_count.load(Ordering::Relaxed);
        self.ui_state.health_midi_retry_count = self.midi_reconnect_count;
        self.ui_state.health_last_error = self
            .last_runtime_error
            .lock()
            .ok()
            .and_then(|last_error| (*last_error).clone());
    }

    pub fn new(cc: &eframe::CreationContext<'_>, config: Config, config_path: String) -> Self {
        let logging_enabled = config.logging.enabled.unwrap_or(true);
        let debounce_ms = config.audio.debounce_ms.unwrap_or(0);
        let applications_sink_search_interval_secs =
            config.audio.applications_sink_search.unwrap_or(10);
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

        // Start MIDI listener — always succeeds, reconnects internally.
        let (listener, rx) = Self::initialize_midi_listener();

        // Initialize PipeWire controller with config mode
        let use_api = config.audio.volume_control_mode.as_deref() == Some("pipewire-api");
        if logging_enabled {
            info!(
                "DEBUG: volume_control_mode = {:?}, use_api = {}",
                config.audio.volume_control_mode, use_api
            );
        }
        let default_sink = config
            .audio
            .default_sink
            .clone()
            .unwrap_or_else(|| "master_sink".to_string());
        let pipewire = Arc::new(Mutex::new(PipeWireController::new(use_api, &default_sink)));

        // Build CC to UI index mappings for fast lookup
        let mut cc_to_sink_index = HashMap::with_capacity(sink_labels.len());
        for (i, (cc, _)) in sink_labels.iter().enumerate() {
            cc_to_sink_index.insert(*cc, i);
        }

        let mut cc_to_app_index = HashMap::with_capacity(app_labels.len());
        for (i, (cc, _)) in app_labels.iter().enumerate() {
            cc_to_app_index.insert(*cc, i);
        }

        // Validate mappings and load mute button mappings
        let config_warnings = config.collect_validation_warnings(&cc_mapping);
        let mute_button_mapping = config.get_mute_button_mappings(&cc_mapping);

        // Initialize MIDI output for LED feedback
        let midi_output = MidiOutput::new();

        // Initialize spectrum analyzer
        let default_sink = config
            .audio
            .default_sink
            .clone()
            .unwrap_or_else(|| "master_sink".to_string());
        let mut spectrum_analyzer = SpectrumAnalyzer::new();
        spectrum_analyzer.start(&default_sink);

        let audio_failure_count = Arc::new(AtomicU64::new(0));
        let last_runtime_error = Arc::new(Mutex::new(None));
        let (audio_update_tx, audio_update_rx) = mpsc::channel();

        Self::start_audio_worker(
            pipewire.clone(),
            audio_update_rx,
            audio_failure_count.clone(),
            last_runtime_error.clone(),
        );

        let enable_tray = config.ui.enable_tray.unwrap_or(false);
        let close_to_tray = config.ui.close_to_tray.unwrap_or(false);
        let start_minimized = config.ui.start_minimized.unwrap_or(false);

        let mut app = MidiVolumeApp {
            ui_state: UiState::new(
                sink_labels.clone(),
                app_labels.clone(),
                show_console,
                max_console_lines,
                enable_tray,
                close_to_tray,
                start_minimized,
                config_path,
                &config,
            ),
            midi_rx: rx,
            _midi_listener: listener,
            midi_startup_error: None,
            midi_output,
            last_midi_output_reconnect: Instant::now(),
            pipewire: pipewire.clone(),
            audio_update_tx,
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
            last_spectrum_sink_name: config
                .ui
                .spectrum_sink_name
                .clone()
                .unwrap_or_else(|| "master_sink".to_string()),
            soft_takeover_armed: HashMap::with_capacity(cc_count),
            inertia_smoothed_percent: HashMap::with_capacity(cc_count),
            audio_failure_count,
            last_runtime_error,
            midi_reconnect_count: 0,
            tray_handle: None,
            tray_rx: None,
            window_visible: !start_minimized,
            force_quit: false,
        };

        if logging_enabled && !app.midi_output.is_enabled() {
            warn!("MIDI LED feedback is disabled because the nanoKontrol2 output port could not be opened");
            app.ui_state
                .add_console_message("Warning: MIDI LED feedback is disabled".to_string());
        }

        if logging_enabled {
            for warning in config_warnings {
                warn!("{}", warning);
                app.ui_state
                    .add_console_message(format!("Warning: {}", warning));
            }
        }

        // Initialize UI fader values for sink controls
        for (i, (cc, target)) in sink_labels.iter().enumerate() {
            let current_volume = match pipewire.lock() {
                Ok(pw) => pw.get_volume_for_sink(target),
                Err(e) => {
                    warn!(
                        "Failed to read initial sink volume for '{}': {}. Using fallback 50%.",
                        target, e
                    );
                    50
                }
            };

            // Set UI fader to current volume (0-127 range)
            app.ui_state.system_fader_values[i] = ((current_volume as f32 / 100.0) * 127.0) as u8;
            app.last_volume_values.insert(*cc, current_volume);
        }

        // Initialize UI fader values for application controls
        for (i, (cc, app_name)) in app_labels.iter().enumerate() {
            let current_volume = match pipewire.lock() {
                Ok(pw) => pw.get_volume_for_app(app_name),
                Err(e) => {
                    warn!(
                        "Failed to read initial app volume for '{}': {}. Using fallback 50%.",
                        app_name, e
                    );
                    50
                }
            };

            // Set UI fader to current volume (0-127 range)
            app.ui_state.app_fader_values[i] = ((current_volume as f32 / 100.0) * 127.0) as u8;
            app.last_volume_values.insert(*cc, current_volume);
        }

        // Only show console messages if logging is enabled
        if app.logging_enabled {
            const SEP: &str = "========================================";
            app.ui_state.add_console_message(SEP.to_string());
            app.ui_state
                .add_console_message("MIDI Volume Controller Started".to_string());
            app.ui_state
                .add_console_message("Listening for nanoKontrol2 MIDI input...".to_string());
            app.ui_state.add_console_message(String::new());
            app.ui_state
                .add_console_message(format!("📝 Loaded {} CC-to-target mappings", cc_count));
            app.ui_state.add_console_message(String::new());
            app.ui_state
                .add_console_message("Waiting for MIDI CC messages...".to_string());
            app.ui_state.add_console_message(SEP.to_string());
        }

        // Initialize system tray if enabled
        if enable_tray {
            if let Some((handle, tray_rx)) = crate::tray::init_tray(
                !start_minimized,
                app.build_tray_cc_assignments(),
            ) {
                app.tray_handle = Some(handle);
                app.tray_rx = Some(tray_rx);
                app.refresh_tray_state();
            } else if logging_enabled {
                app.ui_state
                    .add_console_message("Warning: System tray unavailable".to_string());
            }
        }

        // Hide window immediately if start_minimized and tray is active
        if start_minimized && app.tray_handle.is_some() {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        app
    }

    fn process_midi_messages(&mut self) -> bool {
        let mut had_messages = false;

        // Process all pending MIDI messages immediately for instant response
        while let Ok(msg) = self.midi_rx.try_recv() {
            had_messages = true;
            match msg {
                MidiMessage::Connected => {
                    self.midi_startup_error = None;
                    self.midi_reconnect_count = self.midi_reconnect_count.saturating_add(1);
                    if self.logging_enabled {
                        self.ui_state
                            .add_console_message("MIDI device connected".to_string());
                    }
                    continue;
                }
                MidiMessage::Disconnected => {
                    self.midi_startup_error = Some(
                        "MIDI device disconnected — reconnecting automatically...".to_string(),
                    );
                    if self.logging_enabled {
                        self.ui_state.add_console_message(
                            "MIDI device disconnected — reconnecting...".to_string(),
                        );
                    }
                    continue;
                }
                MidiMessage::ControlChange { cc, value } => {
                    // Log MIDI CC message to console if logging is enabled
                    if self.logging_enabled {
                        self.ui_state
                            .add_console_message(format!("MIDI CC{} -> value: {}", cc, value));
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
                        let percent = match self.map_midi_value_to_percent(cc, value) {
                            Some(percent) => percent,
                            None => continue,
                        };

                        // Debounce: Skip if value hasn't changed or updated too recently
                        let now = Instant::now();
                        let should_update = if let Some(&last_val) = self.last_volume_values.get(&cc) {
                            let delta = last_val.abs_diff(percent);

                            if last_val == percent {
                                false // Same value, skip
                            } else if delta >= MIDI_FAST_MOVE_DELTA_PERCENT {
                                // Large slider movement should feel immediate.
                                true
                            } else if let Some(&last_time) = self.last_volume_time.get(&cc) {
                                // Tiny movements are often controller jitter, keep debounce here.
                                now.duration_since(last_time).as_millis() >= self.debounce_ms as u128
                            } else {
                                true
                            }
                        } else {
                            true // First update
                        };

                        if !should_update {
                            continue; // Skip this update
                        }

                        // Cache the new value and time
                        self.last_volume_values.insert(cc, percent);
                        self.last_volume_time.insert(cc, now);

                        // Determine if this is a sink or app control
                        if self.cc_types.get(&cc).copied().unwrap_or(true) {
                            // Sink control - spawn thread to avoid blocking UI
                            if let Some(target) = self.cc_mapping.get(&cc) {
                                self.spawn_audio_update(
                                    target.clone(),
                                    percent,
                                    true,
                                    "MIDI sink volume update",
                                );
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
                                self.spawn_audio_update(
                                    target.clone(),
                                    percent,
                                    false,
                                    "MIDI app volume update",
                                );
                            }

                            // Update UI fader for this app using cached index
                            if let Some(&ui_index) = self.cc_to_app_index.get(&cc) {
                                if ui_index < self.ui_state.app_fader_values.len() {
                                    self.ui_state.app_fader_values[ui_index] = value;
                                }
                            }
                        }
                    }
                } // end MidiMessage::ControlChange
            } // end match msg
        }

        had_messages
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
                        self.ui_state.add_console_message(format!(
                            "🔇 CC{} {} ",
                            target_cc,
                            if muted { "muted" } else { "unmuted" }
                        ));
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
                        self.ui_state.add_console_message(format!(
                            "🔇 CC{} {}",
                            target_cc,
                            if muted { "muted" } else { "unmuted" }
                        ));
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
                self.spawn_audio_update(
                    target.clone(),
                    percent,
                    true,
                    "Sink unmute restore",
                );
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
                self.spawn_audio_update(target.clone(), 0, true, "Sink mute");
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
                self.spawn_audio_update(
                    target.clone(),
                    percent,
                    false,
                    "App unmute restore",
                );
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
                self.spawn_audio_update(target.clone(), 0, false, "App mute");
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

                    self.last_volume_values.insert(cc, percent);
                    self.inertia_smoothed_percent.insert(cc, percent as f32);
                    self.soft_takeover_armed.insert(cc, false);

                    if let Some(target) = self.cc_mapping.get(&cc) {
                        self.spawn_audio_update(
                            target.clone(),
                            percent,
                            true,
                            "UI sink volume update",
                        );
                    }

                    if self.logging_enabled {
                        self.ui_state
                            .add_console_message(format!("UI Slider CC{}: {}", cc, percent));
                    }
                }
            } else {
                // Handle app volume change from UI
                if ui_index < self.ui_state.app_fader_labels.len() {
                    let cc = self.ui_state.app_fader_labels[ui_index].0;
                    let percent = ((new_value as f32) * MIDI_TO_PERCENT_FACTOR) as u8;

                    self.last_volume_values.insert(cc, percent);
                    self.inertia_smoothed_percent.insert(cc, percent as f32);
                    self.soft_takeover_armed.insert(cc, false);

                    if let Some(target) = self.cc_mapping.get(&cc) {
                        self.spawn_audio_update(
                            target.clone(),
                            percent,
                            false,
                            "UI app volume update",
                        );
                    }

                    if self.logging_enabled {
                        self.ui_state
                            .add_console_message(format!("UI Slider CC{}: {}", cc, percent));
                    }
                }
            }
        }
    }

    fn check_audio_availability(&mut self) {
        // Run if the subscribe thread detected a change, or as a fallback timer
        let signaled = self.pipewire.lock()
            .map(|pw| pw.take_availability_changed())
            .unwrap_or(false);
        let force_by_timer = self.last_availability_check.elapsed().as_secs()
            >= self.applications_sink_search_interval_secs;
        if !signaled && !force_by_timer {
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

            // Check app availability — single pactl call per app instead of two
            for i in 0..self.ui_state.app_fader_labels.len() {
                let app_name = self.ui_state.app_fader_labels[i].1.clone();
                let (is_available, input_count) = pipewire.get_app_availability_and_count(&app_name);
                self.ui_state.app_available[i] = is_available;
                self.ui_state.app_input_count[i] = input_count;
            }

            self.refresh_tray_state();
        } else {
            warn!("Failed to lock PipeWire controller during availability check");
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
            self.ui_state.cfg_show_console && self.ui_state.cfg_logging_enabled,
            self.ui_state.cfg_max_console_lines,
            self.ui_state.show_cc_assignments,
            self.ui_state.show_device_health,
            self.ui_state.cfg_show_spectrum,
            self.ui_state.cfg_spectrum_stereo_mode,
            self.ui_state.cfg_spectrum_show_waterfall,
            self.ui_state.cfg_spectrum_show_labels,
            &self.ui_state.cfg_spectrum_color_palette,
            &self.ui_state.cfg_spectrum_sink_name,
            self.ui_state.cfg_logging_enabled,
            &self.ui_state.cfg_log_level,
            self.ui_state.cfg_timestamps,
            self.ui_state.cfg_log_fader_events,
            self.ui_state.cfg_log_device_info,
            self.ui_state.enable_tray,
            self.ui_state.close_to_tray,
            self.ui_state.start_minimized,
        );

        // Save to file
        match config.save_to_file(&self.ui_state.config_path) {
            Ok(()) => {
                self.ui_state.settings_dirty = false;
                self.ui_state.settings_save_message = Some((
                    "SUCCESS: Settings saved".to_string(),
                    std::time::Instant::now(),
                ));

                // Reload config from file
                if let Ok(reloaded_config) = Config::load_with_fallback(
                    &self.ui_state.config_path,
                    "~/.bin/audio/nanokontrol2/config.toml",
                ) {
                    // Update runtime values from reloaded config
                    self.debounce_ms = reloaded_config.audio.debounce_ms.unwrap_or(0);
                    self.applications_sink_search_interval_secs =
                        reloaded_config.audio.applications_sink_search.unwrap_or(10);
                    self.logging_enabled = reloaded_config.logging.enabled.unwrap_or(true);
                    self.ui_state.cfg_logging_enabled =
                        reloaded_config.logging.enabled.unwrap_or(true);
                    self.ui_state.show_cc_assignments =
                        reloaded_config.ui.show_cc_assignments.unwrap_or(true);
                    self.ui_state.show_device_health =
                        reloaded_config.ui.show_device_health.unwrap_or(true);

                    // Reload sink and app mappings
                    let cc_mapping = reloaded_config.get_cc_mapping();
                    self.cc_mapping = cc_mapping.clone();
                    let sink_labels = reloaded_config.get_sink_labels();
                    let app_labels = reloaded_config.get_app_labels();

                    // Update UI state fader arrays to match new configuration
                    self.ui_state.system_fader_labels = sink_labels.clone();
                    self.ui_state
                        .system_fader_values
                        .resize(sink_labels.len(), 0);
                    self.ui_state.system_muted.resize(sink_labels.len(), false);
                    self.ui_state
                        .system_muted_volume
                        .resize(sink_labels.len(), 0);
                    self.ui_state
                        .system_available
                        .resize(sink_labels.len(), true);

                    self.ui_state.app_fader_labels = app_labels.clone();
                    self.ui_state.app_fader_values.resize(app_labels.len(), 0);
                    self.ui_state.app_muted.resize(app_labels.len(), false);
                    self.ui_state.app_muted_volume.resize(app_labels.len(), 0);
                    self.ui_state.app_available.resize(app_labels.len(), true);

                    // Reset visibility, display order, and input count to match new config size
                    self.ui_state.sink_visibility = vec![true; sink_labels.len()];
                    self.ui_state.sink_display_order = (0..sink_labels.len()).collect();
                    self.ui_state.app_visibility = vec![true; app_labels.len()];
                    self.ui_state.app_display_order = (0..app_labels.len()).collect();
                    self.ui_state.app_input_count = vec![0; app_labels.len()];

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
                    let config_warnings = reloaded_config.collect_validation_warnings(&cc_mapping);
                    self.mute_button_mapping =
                        reloaded_config.get_mute_button_mappings(&cc_mapping);

                    if self.logging_enabled {
                        for warning in config_warnings {
                            warn!("{}", warning);
                            self.ui_state
                                .add_console_message(format!("Warning: {}", warning));
                        }
                    }
                }

                if self.logging_enabled {
                    self.ui_state.add_console_message(
                        "Settings saved and reloaded from config.toml".to_string(),
                    );
                }

                // Toggle system tray on/off if enable_tray setting changed
                match (self.tray_handle.is_some(), self.ui_state.enable_tray) {
                    (false, true) => {
                        if let Some((handle, rx)) = crate::tray::init_tray(
                            self.window_visible,
                            self.build_tray_cc_assignments(),
                        )
                        {
                            self.tray_handle = Some(handle);
                            self.tray_rx = Some(rx);
                            self.refresh_tray_state();
                        }
                    }
                    (true, false) => {
                        self.tray_handle = None;
                        self.tray_rx = None;
                    }
                    _ => {}
                }
            }
            Err(e) => {
                self.ui_state.settings_save_message =
                    Some((format!("ERROR: {}", e), std::time::Instant::now()));

                if self.logging_enabled {
                    self.ui_state
                        .add_console_message(format!("Error saving settings: {}", e));
                }
            }
        }
    }

    fn sync_runtime_settings_from_ui(&mut self) {
        self.logging_enabled = self.ui_state.cfg_logging_enabled;
        self.debounce_ms = self.ui_state.cfg_debounce_ms;
        self.applications_sink_search_interval_secs = self.ui_state.cfg_applications_sink_search;

        if let Ok(mut pipewire) = self.pipewire.lock() {
            pipewire.set_default_sink_name(&self.ui_state.cfg_default_sink);
        }
    }
}

impl eframe::App for MidiVolumeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply editable settings live so checkboxes/sliders take effect immediately.
        self.sync_runtime_settings_from_ui();

        // --- Tray command processing ---
        // Collect pending tray commands first (avoids borrow conflict with self).
        let tray_cmds: Vec<crate::tray::TrayCommand> = self
            .tray_rx
            .as_ref()
            .map(|rx| std::iter::from_fn(|| rx.try_recv().ok()).collect())
            .unwrap_or_default();
        for cmd in tray_cmds {
            match cmd {
                crate::tray::TrayCommand::ShowHide => {
                    self.window_visible = !self.window_visible;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(
                        self.window_visible,
                    ));
                    self.refresh_tray_state();
                }
                crate::tray::TrayCommand::AutoAssignApps => {
                    self.auto_assign_active_apps();
                    self.refresh_tray_state();
                }
                crate::tray::TrayCommand::Quit => {
                    if self.ui_state.settings_dirty {
                        self.save_settings();
                    }
                    self.force_quit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        // --- Close-to-tray intercept ---
        // If the user clicks the window's X button and close_to_tray is active,
        // hide the window instead of quitting.
        if ctx.input(|i| i.viewport().close_requested())
            && self.ui_state.close_to_tray
            && self.tray_handle.is_some()
            && !self.force_quit
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.window_visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.refresh_tray_state();
        }

        // --- Minimize-to-tray intercept ---
        // If the window is minimized and close_to_tray is active, un-minimize
        // and hide to tray instead.
        if ctx.input(|i| i.viewport().minimized == Some(true))
            && self.ui_state.close_to_tray
            && self.tray_handle.is_some()
            && self.window_visible
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            self.window_visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.refresh_tray_state();
        }

        // Check for window size changes and apply them
        if self.last_window_width != self.ui_state.cfg_window_width
            || self.last_window_height != self.ui_state.cfg_window_height
        {
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
            self.spectrum_analyzer
                .start(&self.ui_state.cfg_spectrum_sink_name);
        }

        // Process incoming MIDI messages immediately
        let had_midi_messages = self.process_midi_messages();

        // Check audio availability every 10 seconds
        self.check_audio_availability();

        // Update health panel metrics
        self.sync_device_health_to_ui();

        // Update spectrum data from analyzer
        self.ui_state.spectrum_data = self.spectrum_analyzer.get_data();

        // Show MIDI startup/reconnect status and retry action.
        self.render_midi_status_banner(ctx);

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

                // Keep app mappings in sync live (e.g. deletions in Settings).
                if settings_changed
                    && self.ui_state.cfg_applications != self.ui_state.app_fader_labels
                {
                    self.apply_app_mapping_changes();
                }

                if self.ui_state.auto_assign_apps_clicked {
                    self.auto_assign_active_apps();
                    self.ui_state.auto_assign_apps_clicked = false;
                }
                if self.ui_state.save_button_clicked {
                    // Save settings to config file
                    self.save_settings();
                    self.ui_state.save_button_clicked = false; // Reset flag after save
                }
                Vec::new()
            }
        };

        // Re-apply in case values changed in Settings tab this same frame.
        self.sync_runtime_settings_from_ui();

        // Render MIDI UI modal if open
        crate::panels::render_midi_ui_modal(&mut self.ui_state, ctx);

        // Handle UI slider changes
        let has_slider_updates = !changed_faders.is_empty();
        self.process_ui_slider_changes(changed_faders);

        // Use fast repaint only when actively changing values or animating spectrum/peaks.
        let has_active_peaks = self.ui_state.selected_tab == crate::ui::Tab::Control
            && self
                .ui_state
                .system_peak_values
                .iter()
                .chain(self.ui_state.app_peak_values.iter())
                .any(|&p| p > 0);

        let should_fast_repaint = had_midi_messages
            || has_slider_updates
            || (self.ui_state.selected_tab == crate::ui::Tab::Control
                && self.ui_state.cfg_show_spectrum);

        if should_fast_repaint {
            ctx.request_repaint_after(Duration::from_millis(16)); // ~60 FPS when active
        } else if has_active_peaks {
            ctx.request_repaint_after(Duration::from_millis(33)); // ~30 FPS during peak decay
        } else {
            ctx.request_repaint_after(Duration::from_millis(120)); // Lower idle CPU usage
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Save settings on app exit
        if self.ui_state.settings_dirty {
            self.save_settings();
        }
    }
}
