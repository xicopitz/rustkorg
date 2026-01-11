use egui::*;

pub use crate::panels::{render_faders_tab, render_console_tab, render_settings_tab};
pub use crate::panels::theme;
use crate::spectrum::SpectrumData;
use crate::panels::VisualizerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Control,
    Console,
    Settings,
}

pub struct UiState {
    pub selected_tab: Tab,
    pub system_fader_values: Vec<u8>,
    pub system_fader_labels: Vec<(u8, String)>,  // (CC number, label)
    pub system_muted: Vec<bool>,  // Track mute state for each system fader
    pub system_muted_volume: Vec<u8>,  // Store previous volume when muted
    pub system_available: Vec<bool>,  // Track if sink is currently available
    pub app_fader_values: Vec<u8>,
    pub app_fader_labels: Vec<(u8, String)>,  // (CC number, app name)
    pub app_muted: Vec<bool>,  // Track mute state for each app fader
    pub app_muted_volume: Vec<u8>,  // Store previous volume when muted
    pub app_available: Vec<bool>,  // Track if app is currently available
    pub console_output: Vec<(String, chrono::DateTime<chrono::Local>)>,
    pub max_console_lines: usize,  // Max number of console messages to keep
    // Tray settings
    pub enable_tray: bool,
    pub close_to_tray: bool,
    pub start_minimized: bool,
    pub config_path: String,
    
    // Editable config fields - Audio
    pub cfg_use_pipewire: bool,
    pub cfg_default_sink: String,
    pub cfg_volume_control_mode: String,
    pub cfg_volume_curve: String,
    pub cfg_debounce_ms: u32,
    pub cfg_applications_sink_search: u64,
    
    // Editable config fields - UI
    pub cfg_window_width: u32,
    pub cfg_window_height: u32,
    pub cfg_theme: String,
    pub cfg_show_console: bool,
    pub cfg_max_console_lines: usize,
    
    // Editable config fields - Logging
    pub cfg_logging_enabled: bool,
    pub cfg_log_level: String,
    pub cfg_timestamps: bool,
    pub cfg_log_fader_events: bool,
    pub cfg_log_device_info: bool,
    
    // Editable config fields - MIDI Controls (as strings for editing)
    pub cfg_sinks: Vec<(u8, String)>,  // (CC number, sink name)
    pub cfg_applications: Vec<(u8, String)>,  // (CC number, app name)
    pub cfg_mute_buttons: Vec<(u8, u8)>,  // (button CC, fader CC)
    
    // Settings UI state
    pub settings_dirty: bool,
    pub settings_save_message: Option<(String, std::time::Instant)>,
    pub new_sink_cc: String,
    pub new_sink_name: String,
    pub new_app_cc: String,
    pub new_app_name: String,
    pub new_mute_button_cc: String,
    pub new_mute_fader_cc: String,
    pub window_width_str: String,
    pub window_height_str: String,
    
    // Settings UI category selection
    pub settings_category: u32,  // 0=MIDI, 1=Audio, 2=UI, 3=Logging, 4=Fader Display
    
    // MIDI UI modal state
    pub show_midi_ui_modal: bool,
    pub midi_ui_texture: Option<egui::TextureHandle>,
    pub midi_ui_dimensions: Option<[f32; 2]>,  // Width and height of the image
    
    // Fader visibility and ordering
    pub sink_visibility: Vec<bool>,  // Track which sinks are visible
    pub sink_display_order: Vec<usize>,  // Track sink display order (indices into system_fader_labels)
    pub app_visibility: Vec<bool>,  // Track which apps are visible
    pub app_display_order: Vec<usize>,  // Track app display order (indices into app_fader_labels)
    
    // Spectrum analyzer state
    pub spectrum_data: SpectrumData,
    pub visualizer_state: VisualizerState,
    
    // UI config for spectrum visibility
    pub cfg_show_spectrum: bool,
    pub cfg_spectrum_stereo_mode: bool,  // true = stereo separate, false = combined
    pub cfg_spectrum_show_waterfall: bool,
    pub cfg_spectrum_show_labels: bool,
}

impl UiState {
    pub fn new(
        system_labels: Vec<(u8, String)>, 
        app_labels: Vec<(u8, String)>, 
        _show_console: bool, 
        max_console_lines: usize,
        enable_tray: bool,
        close_to_tray: bool,
        start_minimized: bool,
        config_path: String,
        config: &crate::config::Config,
    ) -> Self {
        let system_count = system_labels.len();
        let app_count = app_labels.len();
        
        Self {
            selected_tab: Tab::Control,
            system_fader_values: vec![0; system_count],
            system_fader_labels: system_labels,
            system_muted: vec![false; system_count],
            system_muted_volume: vec![0; system_count],
            system_available: vec![true; system_count],
            app_fader_values: vec![0; app_count],
            app_fader_labels: app_labels,
            app_muted: vec![false; app_count],
            app_muted_volume: vec![0; app_count],
            app_available: vec![true; app_count],
            console_output: Vec::new(),
            max_console_lines,
            enable_tray,
            close_to_tray,
            start_minimized,
            config_path,
            cfg_use_pipewire: config.audio.use_pipewire.unwrap_or(true),
            cfg_default_sink: config.audio.default_sink.clone().unwrap_or_default(),
            cfg_volume_control_mode: config.audio.volume_control_mode.clone().unwrap_or_else(|| "pipewire-api".to_string()),
            cfg_volume_curve: config.audio.volume_curve.clone().unwrap_or_else(|| "linear".to_string()),
            cfg_debounce_ms: config.audio.debounce_ms.unwrap_or(100),
            cfg_applications_sink_search: config.audio.applications_sink_search.unwrap_or(10),
            cfg_window_width: config.ui.window_width.unwrap_or(1200),
            cfg_window_height: config.ui.window_height.unwrap_or(1000),
            window_width_str: config.ui.window_width.unwrap_or(1200).to_string(),
            window_height_str: config.ui.window_height.unwrap_or(1000).to_string(),
            cfg_theme: config.ui.theme.clone().unwrap_or_else(|| "default".to_string()),
            cfg_show_console: config.ui.show_console.unwrap_or(false),
            cfg_max_console_lines: config.ui.max_console_lines.unwrap_or(1000),
            cfg_logging_enabled: config.logging.enabled.unwrap_or(true),
            cfg_log_level: config.logging.log_level.clone().unwrap_or_else(|| "info".to_string()),
            cfg_timestamps: config.logging.timestamps.unwrap_or(true),
            cfg_log_fader_events: config.logging.log_fader_events.unwrap_or(false),
            cfg_log_device_info: config.logging.log_device_info.unwrap_or(false),
            cfg_sinks: convert_hashmap_to_cc_vec(&config.midi_controls.sinks),
            cfg_applications: convert_hashmap_to_cc_vec(&config.midi_controls.applications),
            cfg_mute_buttons: convert_mute_buttons_hashmap(&config.midi_controls.mute_buttons),
            settings_dirty: false,
            settings_save_message: None,
            new_sink_cc: String::new(),
            new_sink_name: String::new(),
            new_app_cc: String::new(),
            new_app_name: String::new(),
            new_mute_button_cc: String::new(),
            new_mute_fader_cc: String::new(),
            settings_category: 0,
            show_midi_ui_modal: false,
            midi_ui_texture: None,
            midi_ui_dimensions: None,
            sink_visibility: vec![true; system_count],
            sink_display_order: (0..system_count).collect(),
            app_visibility: vec![true; app_count],
            app_display_order: (0..app_count).collect(),
            spectrum_data: SpectrumData::default(),
            visualizer_state: VisualizerState::default(),
            cfg_show_spectrum: config.ui.show_spectrum.unwrap_or(true),
            cfg_spectrum_stereo_mode: config.ui.spectrum_stereo_mode.unwrap_or(false),
            cfg_spectrum_show_waterfall: config.ui.spectrum_show_waterfall.unwrap_or(false),
            cfg_spectrum_show_labels: config.ui.spectrum_show_labels.unwrap_or(true),
        }
    }

    pub fn add_console_message(&mut self, msg: String) {
        if self.console_output.len() >= self.max_console_lines {
            self.console_output.remove(0);
        }
        self.console_output.push((msg, chrono::Local::now()));
    }

    pub fn apply_dark_theme(ctx: &Context) {
        let mut visuals = Visuals::dark();
        visuals.override_text_color = Some(theme::TEXT_PRIMARY);
        
        // Panel backgrounds
        visuals.panel_fill = theme::BG_PRIMARY;
        visuals.window_fill = theme::BG_PRIMARY;
        
        // Button styling
        visuals.widgets.inactive.bg_fill = theme::BG_SECONDARY;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme::BORDER);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, theme::TEXT_SECONDARY);
        
        visuals.widgets.hovered.bg_fill = theme::BG_TERTIARY;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme::ACCENT_BLUE);
        
        visuals.widgets.active.bg_fill = theme::ACCENT_BLUE;
        visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
        
        // Selection
        visuals.selection.bg_fill = theme::ACCENT_BLUE;
        visuals.selection.stroke = Stroke::new(1.0, theme::ACCENT_BLUE);
        
        // Borders
        visuals.window_stroke = Stroke::new(1.0, theme::BORDER);
        
        ctx.set_visuals(visuals);
    }

    pub fn render_tabs(&mut self, ctx: &Context) {
        Self::apply_dark_theme(ctx);
        
        TopBottomPanel::top("tab_panel")
            .frame(Frame::default()
                .fill(theme::BG_SECONDARY)
                .stroke(Stroke::new(1.0, theme::BORDER)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing = vec2(12.0, 0.0);
                    ui.add_space(8.0);
                    
                    // Control tab
                    if ui
                        .selectable_label(self.selected_tab == Tab::Control, 
                            RichText::new("🔈 Control").size(14.0))
                        .clicked()
                    {
                        self.selected_tab = Tab::Control;
                    }
                    
                    // Console tab
                    if ui
                        .selectable_label(self.selected_tab == Tab::Console, 
                            RichText::new("📋 Console").size(14.0))
                        .clicked()
                    {
                        self.selected_tab = Tab::Console;
                    }
                    
                    // Settings tab
                    if ui
                        .selectable_label(self.selected_tab == Tab::Settings, 
                            RichText::new("⚙ Settings").size(14.0))
                        .clicked()
                    {
                        self.selected_tab = Tab::Settings;
                    }
                    
                    ui.add_space(8.0);
                });
            });
    }

    pub fn render_faders_tab(&mut self, ctx: &Context) -> Vec<(bool, usize, u8)> {
        render_faders_tab(self, ctx)
    }

    pub fn render_console_tab(&mut self, ctx: &Context) {
        render_console_tab(&self.console_output, ctx);
    }

    pub fn render_settings_tab(&mut self, ctx: &Context, _tray_functional: bool) -> bool {
        render_settings_tab(self, ctx, false)
    }
}

// Helper function to convert HashMap<String, String> (with CC keys like "cc_0") to Vec<(u8, String)>
fn convert_hashmap_to_cc_vec(map: &std::collections::HashMap<String, String>) -> Vec<(u8, String)> {
    let mut result: Vec<(u8, String)> = map
        .iter()
        .filter_map(|(k, v)| {
            if let Some(cc_str) = k.strip_prefix("cc_") {
                if let Ok(cc) = cc_str.parse::<u8>() {
                    return Some((cc, v.clone()));
                }
            }
            None
        })
        .collect();
    result.sort_by_key(|(cc, _)| *cc);
    result
}

// Helper function to convert mute buttons HashMap
fn convert_mute_buttons_hashmap(map: &std::collections::HashMap<String, u8>) -> Vec<(u8, u8)> {
    let mut result: Vec<(u8, u8)> = map
        .iter()
        .filter_map(|(k, v)| {
            if let Some(cc_str) = k.strip_prefix("cc_") {
                if let Ok(cc) = cc_str.parse::<u8>() {
                    return Some((cc, *v));
                }
            }
            None
        })
        .collect();
    result.sort_by_key(|(cc, _)| *cc);
    result
}
