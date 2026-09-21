use crate::config::Config;
use crate::control::{self, ControlHandle, ControlMapping, SharedFaderState};
use crate::spectrum::SpectrumAnalyzer;
use crate::ui::UiState;
use log::{info, warn};
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// CC→target mappings derived from a `Config`, shared by initial load and settings-save
/// reload so the two stay in sync instead of duplicating this logic by hand.
pub struct CcMapping {
    pub control: ControlMapping,
    pub sink_labels: Vec<(u8, String)>,
    pub app_labels: Vec<(u8, String)>,
    pub warnings: Vec<String>,
}

pub fn build_cc_mapping(config: &Config) -> CcMapping {
    let cc_mapping = config.get_cc_mapping();
    let sink_labels = config.get_sink_labels();
    let app_labels = config.get_app_labels();
    let warnings = config.collect_validation_warnings(&cc_mapping);
    let mute_button_mapping = config.get_mute_button_mappings(&cc_mapping);
    // Mute buttons are keyed differently in `Config` (by button CC only), so patch that
    // field in after building the rest from the same lists the UI uses.
    let control = ControlMapping {
        mute_button_mapping,
        ..control::control_mapping_from_lists(&sink_labels, &app_labels, &[])
    };

    CcMapping {
        control,
        sink_labels,
        app_labels,
        warnings,
    }
}

/// Everything that must keep running independent of whether a GUI window exists: the
/// MIDI/audio control loop and the system tray. Built once at process startup and shared
/// (via cheap handle clones) across however many windows `main.rs` opens and closes.
#[derive(Clone)]
pub struct Backend {
    pub control: ControlHandle,
    pub tray_handle: Option<Arc<ksni::Handle<crate::tray::AppTray>>>,
    pub tray_rx: Option<Arc<Mutex<mpsc::Receiver<crate::tray::TrayCommand>>>>,
}

/// What the tray asked for while no window was open.
pub enum TrayWait {
    Show,
    Quit,
}

impl Backend {
    pub fn new(config: &Config) -> Self {
        let logging_enabled = config.logging.enabled.unwrap_or(true);
        let debounce_ms = config.audio.debounce_ms.unwrap_or(0);
        let default_sink = config
            .audio
            .default_sink
            .clone()
            .unwrap_or_else(|| "master_sink".to_string());
        let volume_curve = config
            .audio
            .volume_curve
            .clone()
            .unwrap_or_else(|| "linear".to_string());

        let CcMapping {
            control: mapping,
            sink_labels,
            app_labels,
            warnings,
        } = build_cc_mapping(config);

        if logging_enabled {
            info!("Initializing MIDI Volume Controller");
            info!(
                "Loaded {} MIDI controls from configuration",
                mapping.cc_mapping.len()
            );
            for warning in &warnings {
                warn!("{}", warning);
            }
        }

        // Read starting volumes straight from PipeWire once, here, since this only ever
        // runs at true process startup now (not on every window open/close cycle).
        let pipewire_probe = crate::pipewire_control::PipeWireController::new(&default_sink);
        let system_fader_values: Vec<u8> = sink_labels
            .iter()
            .map(|(_, target)| {
                control::percent_to_fader_value(
                    pipewire_probe.get_volume_for_sink(target).unwrap_or(50),
                )
            })
            .collect();
        let app_fader_values: Vec<u8> = app_labels
            .iter()
            .map(|(_, name)| {
                control::percent_to_fader_value(pipewire_probe.get_volume_for_app(name).unwrap_or(50))
            })
            .collect();
        drop(pipewire_probe);

        let initial_fader_state = SharedFaderState {
            system_muted: vec![false; system_fader_values.len()],
            system_muted_volume: vec![0; system_fader_values.len()],
            system_fader_values,
            app_muted: vec![false; app_fader_values.len()],
            app_muted_volume: vec![0; app_fader_values.len()],
            app_fader_values,
        };

        let control = control::start(
            mapping,
            debounce_ms,
            logging_enabled,
            volume_curve,
            initial_fader_state,
            &default_sink,
        );

        let enable_tray = config.ui.enable_tray.unwrap_or(false);
        let (tray_handle, tray_rx) = if enable_tray {
            match crate::tray::init_tray(true) {
                Some((handle, rx)) => (
                    Some(Arc::new(handle)),
                    Some(Arc::new(Mutex::new(rx))),
                ),
                None => {
                    warn!("System tray unavailable");
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        Backend {
            control,
            tray_handle,
            tray_rx,
        }
    }

    /// Blocks until the tray asks to show the window again, or to quit. Only call this
    /// while no window is open — it's the only consumer of `tray_rx` at that point.
    pub fn wait_for_tray(&self) -> TrayWait {
        let Some(tray_rx) = &self.tray_rx else {
            // No tray to wait on; nothing will ever ask us to reopen, so just quit.
            return TrayWait::Quit;
        };
        let rx = tray_rx.lock().unwrap();
        loop {
            match rx.recv() {
                Ok(crate::tray::TrayCommand::ShowHide) => return TrayWait::Show,
                Ok(crate::tray::TrayCommand::Quit) => return TrayWait::Quit,
                Err(_) => return TrayWait::Quit,
            }
        }
    }
}

/// Why the window closed, set by `MidiVolumeApp` right before it closes so `main.rs` can
/// tell a real quit apart from a hide-to-tray.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    Quit,
    HideToTray,
}

pub struct MidiVolumeApp {
    ui_state: UiState,
    control: ControlHandle,
    tray_handle: Option<Arc<ksni::Handle<crate::tray::AppTray>>>,
    tray_rx: Option<Arc<Mutex<mpsc::Receiver<crate::tray::TrayCommand>>>>,
    exit_reason: Arc<Mutex<ExitReason>>,
    /// Set once `close_window` has been called, so a `Close` command we sent ourselves
    /// (which shows up as another `close_requested()` a frame or two later) can't
    /// re-trigger the close intercepts and overwrite an already-decided `exit_reason`.
    closing: bool,
    logging_enabled: bool,
    debounce_ms: u32,
    last_pushed_volume_curve: String,
    last_availability_check: Instant,
    applications_sink_search_interval_secs: u64,
    spectrum_analyzer: SpectrumAnalyzer,
    last_window_width: u32,
    last_window_height: u32,
    last_spectrum_sink_name: String,
}

impl MidiVolumeApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        backend: &Backend,
        config: Config,
        config_path: String,
        exit_reason: Arc<Mutex<ExitReason>>,
        start_minimized: bool,
    ) -> Self {
        let logging_enabled = config.logging.enabled.unwrap_or(true);
        let debounce_ms = config.audio.debounce_ms.unwrap_or(0);
        let applications_sink_search_interval_secs =
            config.audio.applications_sink_search.unwrap_or(10);
        let show_console = config.ui.show_console.unwrap_or(false);
        let max_console_lines = config.ui.max_console_lines.unwrap_or(1000);
        let volume_curve = config
            .audio
            .volume_curve
            .clone()
            .unwrap_or_else(|| "linear".to_string());

        let CcMapping {
            control: mapping,
            sink_labels,
            app_labels,
            ..
        } = build_cc_mapping(&config);

        // Keep the control thread's routing table in sync with whatever config this
        // window session is showing (it may differ from what the thread started with,
        // if settings were saved during a previous session).
        backend.control.send_update_mapping(mapping);
        backend
            .control
            .send_update_settings(debounce_ms, logging_enabled, volume_curve.clone());

        let enable_tray = config.ui.enable_tray.unwrap_or(false);
        let close_to_tray = config.ui.close_to_tray.unwrap_or(false);

        let mut ui_state = UiState::new(
            sink_labels.clone(),
            app_labels.clone(),
            show_console,
            max_console_lines,
            enable_tray,
            close_to_tray,
            start_minimized,
            config_path,
            &config,
        );

        // Hydrate the fader/mute display from the control thread's current state.
        if let Ok(shared) = backend.control.shared_fader.lock() {
            ui_state.system_fader_values = shared.system_fader_values.clone();
            ui_state.system_muted = shared.system_muted.clone();
            ui_state.system_muted_volume = shared.system_muted_volume.clone();
            ui_state.app_fader_values = shared.app_fader_values.clone();
            ui_state.app_muted = shared.app_muted.clone();
            ui_state.app_muted_volume = shared.app_muted_volume.clone();
        }

        if logging_enabled {
            const SEP: &str = "========================================";
            ui_state.add_console_message(SEP.to_string());
            ui_state.add_console_message("MIDI Volume Controller Started".to_string());
            ui_state.add_console_message(format!(
                "📝 Loaded {} CC-to-target mappings",
                sink_labels.len() + app_labels.len()
            ));
            ui_state.add_console_message(SEP.to_string());
        }

        let mut app = MidiVolumeApp {
            ui_state,
            control: backend.control.clone(),
            tray_handle: backend.tray_handle.clone(),
            tray_rx: backend.tray_rx.clone(),
            exit_reason,
            closing: false,
            logging_enabled,
            debounce_ms,
            last_pushed_volume_curve: volume_curve,
            last_availability_check: Instant::now(),
            applications_sink_search_interval_secs,
            spectrum_analyzer: SpectrumAnalyzer::new(),
            last_window_width: config.ui.window_width.unwrap_or(1000),
            last_window_height: config.ui.window_height.unwrap_or(800),
            last_spectrum_sink_name: config
                .ui
                .spectrum_sink_name
                .clone()
                .unwrap_or_else(|| "master_sink".to_string()),
        };

        app.spectrum_analyzer.start(&app.last_spectrum_sink_name);
        app.check_audio_availability_now();

        if let Some(handle) = &app.tray_handle {
            handle.update(|tray| tray.visible = true);
        }

        if start_minimized && app.tray_handle.is_some() {
            app.close_window(&cc.egui_ctx, ExitReason::HideToTray);
        }

        app
    }

    /// Copies the currently-known display order/apps CC list into a control-thread
    /// mapping update and mirrors the same values into `SharedFaderState`, preserving
    /// existing values by CC (matches the fader arrays this function just built).
    fn push_live_mapping_update(&self) {
        let mapping = control::control_mapping_from_lists(
            &self.ui_state.system_fader_labels,
            &self.ui_state.app_fader_labels,
            &self.ui_state.cfg_mute_buttons,
        );
        self.control.send_update_mapping(mapping);
        if let Ok(mut shared) = self.control.shared_fader.lock() {
            shared.app_fader_values = self.ui_state.app_fader_values.clone();
            shared.app_muted = self.ui_state.app_muted.clone();
            shared.app_muted_volume = self.ui_state.app_muted_volume.clone();
        }
    }

    fn apply_app_mapping_changes(&mut self) {
        self.ui_state.cfg_applications.sort_by_key(|(cc, _)| *cc);
        let new_labels = self.ui_state.cfg_applications.clone();
        let old_labels = self.ui_state.app_fader_labels.clone();

        let mut old_values = std::collections::HashMap::new();
        let mut old_muted = std::collections::HashMap::new();
        let mut old_muted_volume = std::collections::HashMap::new();
        let mut old_availability = std::collections::HashMap::new();
        let mut old_visibility = std::collections::HashMap::new();

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

        self.push_live_mapping_update();
    }

    fn auto_assign_active_apps(&mut self) {
        let active_apps = match self.control.pipewire.lock() {
            Ok(pw) => pw.list_active_application_names(),
            Err(e) => {
                let message = format!(
                    "Auto-assign failed: could not access PipeWire controller: {}",
                    e
                );
                warn!("{}", message);
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
            self.ui_state.add_console_message(summary);
            for (cc, app_name) in added {
                self.ui_state
                    .add_console_message(format!("Mapped '{}' to CC{}", app_name, cc));
            }
        }
    }

    fn sync_device_health_to_ui(&mut self) {
        self.ui_state.health_midi_input_connected =
            self.control.midi_connected.load(Ordering::Relaxed);
        self.ui_state.health_midi_output_connected =
            self.control.midi_output_connected.load(Ordering::Relaxed);
        self.ui_state.health_pipewire_default_sink = self.ui_state.cfg_default_sink.clone();
        self.ui_state.health_audio_failure_count =
            self.control.audio_failure_count.load(Ordering::Relaxed);
        self.ui_state.health_midi_retry_count =
            self.control.midi_reconnect_count.load(Ordering::Relaxed);
        self.ui_state.health_last_error = self
            .control
            .last_runtime_error
            .lock()
            .ok()
            .and_then(|last_error| (*last_error).clone());
    }

    fn check_audio_availability_now(&mut self) {
        self.last_availability_check = Instant::now();
        if let Ok(pipewire) = self.control.pipewire.lock() {
            self.ui_state.available_sinks = pipewire.list_sink_names();

            for i in 0..self.ui_state.system_fader_labels.len() {
                let sink_name = &self.ui_state.system_fader_labels[i].1;
                self.ui_state.system_available[i] =
                    pipewire.get_volume_for_sink(sink_name).is_some();
            }

            for i in 0..self.ui_state.app_fader_labels.len() {
                let app_name = self.ui_state.app_fader_labels[i].1.clone();
                let (is_available, input_count) =
                    pipewire.get_app_availability_and_count(&app_name);
                self.ui_state.app_available[i] = is_available;
                self.ui_state.app_input_count[i] = input_count;
            }
        } else {
            warn!("Failed to lock PipeWire controller during availability check");
        }
    }

    fn check_audio_availability(&mut self) {
        let signaled = self
            .control
            .pipewire
            .lock()
            .map(|pw| pw.take_availability_changed())
            .unwrap_or(false);
        let force_by_timer = self.last_availability_check.elapsed().as_secs()
            >= self.applications_sink_search_interval_secs;
        if !signaled && !force_by_timer {
            return;
        }
        self.check_audio_availability_now();
    }

    fn save_settings(&mut self) {
        let config = Config::from_ui_state(
            &self.ui_state.cfg_sinks,
            &self.ui_state.cfg_applications,
            &self.ui_state.cfg_mute_buttons,
            self.ui_state.cfg_use_pipewire,
            &self.ui_state.cfg_default_sink,
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

        match config.save_to_file(&self.ui_state.config_path) {
            Ok(()) => {
                self.ui_state.settings_dirty = false;
                self.ui_state.settings_save_message = Some((
                    "SUCCESS: Settings saved".to_string(),
                    std::time::Instant::now(),
                ));

                if let Ok(reloaded_config) = Config::load_with_fallback(
                    &self.ui_state.config_path,
                    "~/.bin/audio/nanokontrol2/config.toml",
                ) {
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

                    let CcMapping {
                        control: mapping,
                        sink_labels,
                        app_labels,
                        warnings: config_warnings,
                    } = build_cc_mapping(&reloaded_config);

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

                    self.ui_state.sink_visibility = vec![true; sink_labels.len()];
                    self.ui_state.sink_display_order = (0..sink_labels.len()).collect();
                    self.ui_state.app_visibility = vec![true; app_labels.len()];
                    self.ui_state.app_display_order = (0..app_labels.len()).collect();
                    self.ui_state.app_input_count = vec![0; app_labels.len()];

                    // Mirror the same resize into the control thread's shared state
                    // (`Vec::resize` semantics: keeps values positionally, same as above —
                    // not remapped by CC identity, matching this save path's existing
                    // behavior for the UI-only arrays).
                    if let Ok(mut shared) = self.control.shared_fader.lock() {
                        shared.system_fader_values = self.ui_state.system_fader_values.clone();
                        shared.system_muted = self.ui_state.system_muted.clone();
                        shared.system_muted_volume = self.ui_state.system_muted_volume.clone();
                        shared.app_fader_values = self.ui_state.app_fader_values.clone();
                        shared.app_muted = self.ui_state.app_muted.clone();
                        shared.app_muted_volume = self.ui_state.app_muted_volume.clone();
                    }
                    self.control.send_update_mapping(mapping);
                    self.control.send_update_settings(
                        self.debounce_ms,
                        self.logging_enabled,
                        self.ui_state.cfg_volume_curve.clone(),
                    );
                    self.last_pushed_volume_curve = self.ui_state.cfg_volume_curve.clone();

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

    /// Closes the window for real, recording why so `main.rs` knows whether to reopen it.
    /// Auto-saves first if there are unsaved edits — otherwise they'd be lost, since (unlike
    /// the old minimize-based approach) the window and its `UiState` genuinely go away.
    fn close_window(&mut self, ctx: &egui::Context, reason: ExitReason) {
        if self.closing {
            return;
        }
        self.closing = true;
        if self.ui_state.settings_dirty {
            self.save_settings();
        }
        *self.exit_reason.lock().unwrap() = reason;
        if let Some(handle) = &self.tray_handle {
            handle.update(|tray| tray.visible = false);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn sync_runtime_settings_from_ui(&mut self) {
        if self.debounce_ms != self.ui_state.cfg_debounce_ms
            || self.logging_enabled != self.ui_state.cfg_logging_enabled
            || self.last_pushed_volume_curve != self.ui_state.cfg_volume_curve
        {
            self.debounce_ms = self.ui_state.cfg_debounce_ms;
            self.logging_enabled = self.ui_state.cfg_logging_enabled;
            self.last_pushed_volume_curve = self.ui_state.cfg_volume_curve.clone();
            self.control.send_update_settings(
                self.debounce_ms,
                self.logging_enabled,
                self.last_pushed_volume_curve.clone(),
            );
        }

        if let Ok(mut pipewire) = self.control.pipewire.lock() {
            pipewire.set_default_sink_name(&self.ui_state.cfg_default_sink);
        }
    }
}

impl eframe::App for MidiVolumeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_runtime_settings_from_ui();

        // --- Tray command processing ---
        let tray_cmds: Vec<crate::tray::TrayCommand> = self
            .tray_rx
            .as_ref()
            .map(|rx| {
                let rx = rx.lock().unwrap();
                std::iter::from_fn(|| rx.try_recv().ok()).collect()
            })
            .unwrap_or_default();
        for cmd in tray_cmds {
            match cmd {
                crate::tray::TrayCommand::ShowHide => {
                    // A window exists (we're running), so this can only mean "hide".
                    self.close_window(ctx, ExitReason::HideToTray);
                }
                crate::tray::TrayCommand::Quit => {
                    self.close_window(ctx, ExitReason::Quit);
                }
            }
        }

        // --- Close-to-tray intercept ---
        // If the user clicks the window's X button and close_to_tray is active, close it
        // as a hide-to-tray (main.rs will reopen it on the next tray Show) rather than a
        // real quit.
        if ctx.input(|i| i.viewport().close_requested()) {
            let reason = if self.ui_state.close_to_tray && self.tray_handle.is_some() {
                ExitReason::HideToTray
            } else {
                ExitReason::Quit
            };
            self.close_window(ctx, reason);
        }

        // --- Minimize-to-tray intercept ---
        // If the window is minimized (e.g. via the OS minimize button) and close_to_tray
        // is active, treat it as a hide-to-tray instead of leaving it merely minimized.
        if ctx.input(|i| i.viewport().minimized == Some(true))
            && self.ui_state.close_to_tray
            && self.tray_handle.is_some()
        {
            self.close_window(ctx, ExitReason::HideToTray);
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

        // Pull the latest fader/mute state the control thread has produced (from
        // hardware input, which keeps working even while no window was open).
        if let Ok(shared) = self.control.shared_fader.lock() {
            if shared.system_fader_values.len() == self.ui_state.system_fader_values.len() {
                self.ui_state.system_fader_values = shared.system_fader_values.clone();
                self.ui_state.system_muted = shared.system_muted.clone();
                self.ui_state.system_muted_volume = shared.system_muted_volume.clone();
            }
            if shared.app_fader_values.len() == self.ui_state.app_fader_values.len() {
                self.ui_state.app_fader_values = shared.app_fader_values.clone();
                self.ui_state.app_muted = shared.app_muted.clone();
                self.ui_state.app_muted_volume = shared.app_muted_volume.clone();
            }
        }

        // Check audio availability periodically
        self.check_audio_availability();

        // Update health panel metrics
        self.sync_device_health_to_ui();

        // Update spectrum data from analyzer
        self.ui_state.spectrum_data = self.spectrum_analyzer.get_data();

        // Show MIDI startup/reconnect status.
        if !self.control.midi_connected.load(Ordering::Relaxed) {
            egui::TopBottomPanel::top("midi_status_panel")
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(
                                "⚠  MIDI device disconnected — reconnecting automatically...",
                            )
                            .color(egui::Color32::from_rgb(255, 190, 120)),
                        );
                    });
                });
        }

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
                    self.save_settings();
                    self.ui_state.save_button_clicked = false;
                }
                Vec::new()
            }
        };

        crate::panels::render_midi_ui_modal(&mut self.ui_state, ctx);

        // Forward UI-driven slider drags to the control thread, which applies them
        // exactly like a hardware move (updates shared state, sends the audio update).
        let has_slider_updates = !changed_faders.is_empty();
        for (is_sink, ui_index, new_value) in changed_faders {
            let cc = if is_sink {
                self.ui_state.system_fader_labels.get(ui_index).map(|(cc, _)| *cc)
            } else {
                self.ui_state.app_fader_labels.get(ui_index).map(|(cc, _)| *cc)
            };
            if let Some(cc) = cc {
                self.control.send_slider_changed(is_sink, cc, new_value);
                if self.logging_enabled {
                    self.ui_state
                        .add_console_message(format!("UI Slider CC{}: {}", cc, new_value));
                }
            }
        }

        let had_midi_messages = has_slider_updates; // approximate "something is actively moving" for repaint pacing below

        let has_active_peaks = self.ui_state.selected_tab == crate::ui::Tab::Control
            && self
                .ui_state
                .system_peak_values
                .iter()
                .chain(self.ui_state.app_peak_values.iter())
                .any(|&p| p > 0);

        let should_fast_repaint = had_midi_messages
            || (self.ui_state.selected_tab == crate::ui::Tab::Control
                && self.ui_state.cfg_show_spectrum);

        if should_fast_repaint {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else if has_active_peaks {
            ctx.request_repaint_after(Duration::from_millis(33));
        } else {
            // Keep polling `shared_fader` reasonably promptly so hardware-driven moves
            // still feel responsive in the UI even without an explicit wake signal.
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.ui_state.settings_dirty {
            self.save_settings();
        }
    }
}
