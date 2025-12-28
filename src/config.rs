use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub midi_controls: MidiControlsConfig,
    pub audio: AudioConfig,
    pub ui: UiConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MidiControlsConfig {
    // Map MIDI CC number to sink targets (sink names)
    // Example: cc_0 = "alsa_output.pci-0000_25_00.0.analog-stereo", cc_1 = "comms_sink"
    #[serde(default)]
    pub sinks: HashMap<String, String>,
    // Map MIDI CC number to application names
    // Example: cc_16 = "Google Chrome"
    #[serde(default)]
    pub applications: HashMap<String, String>,
    // Map mute button CC to target fader CC (e.g., cc_64 = "cc_0" means CC64 mutes CC0)
    // The key is the mute button CC, the value is the target fader CC number
    #[serde(default)]
    pub mute_buttons: HashMap<String, u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub use_pipewire: Option<bool>,
    pub default_sink: Option<String>,
    pub volume_control_mode: Option<String>,
    pub volume_curve: Option<String>,
    pub debounce_ms: Option<u32>,
    pub applications_sink_search: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub theme: Option<String>,
    pub show_console: Option<bool>,
    pub max_console_lines: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub enabled: Option<bool>,
    pub log_level: Option<String>,
    pub timestamps: Option<bool>,
    pub log_fader_events: Option<bool>,
    pub log_device_info: Option<bool>,
}

impl Config {
    pub fn load_with_fallback(primary: &str, fallback: &str) -> Result<Self> {
        // Try primary path first
        if let Ok(content) = fs::read_to_string(primary) {
            return toml::from_str(&content)
                .with_context(|| format!("Failed to parse config from: {}", primary));
        }
        
        // If primary fails, try fallback path
        let fallback_expanded = shellexpand::tilde(fallback).to_string();
        let content = fs::read_to_string(&fallback_expanded)
            .with_context(|| format!("Failed to read config from primary ({}) or fallback ({})", primary, fallback_expanded))?;
        toml::from_str(&content)
            .context("Failed to parse config.toml")
    }

    pub fn get_cc_mapping(&self) -> HashMap<u8, String> {
        // Parse CC controls from both sinks and applications
        let capacity = self.midi_controls.sinks.len() + self.midi_controls.applications.len();
        let mut mapping = HashMap::with_capacity(capacity);
        
        // Add sink controls
        for (key, target) in &self.midi_controls.sinks {
            if let Some(cc_str) = key.strip_prefix("cc_") {
                if let Ok(cc_num) = cc_str.parse::<u8>() {
                    mapping.insert(cc_num, target.trim().to_string());
                }
            }
        }
        
        // Add application controls
        for (key, app_name) in &self.midi_controls.applications {
            if let Some(cc_str) = key.strip_prefix("cc_") {
                if let Ok(cc_num) = cc_str.parse::<u8>() {
                    mapping.insert(cc_num, app_name.trim().to_string());
                }
            }
        }
        
        mapping
    }
    
    pub fn get_sink_labels(&self) -> Vec<(u8, String)> {
        // Returns sorted list of sink controls
        let mut controls = Vec::with_capacity(self.midi_controls.sinks.len());
        for (key, target) in &self.midi_controls.sinks {
            if let Some(cc_str) = key.strip_prefix("cc_") {
                if let Ok(cc_num) = cc_str.parse::<u8>() {
                    controls.push((cc_num, target.trim().to_string()));
                }
            }
        }
        controls.sort_by_key(|(cc, _)| *cc);
        controls
    }

    pub fn get_app_labels(&self) -> Vec<(u8, String)> {
        // Returns sorted list of application controls
        let mut controls = Vec::with_capacity(self.midi_controls.applications.len());
        for (key, app_name) in &self.midi_controls.applications {
            if let Some(cc_str) = key.strip_prefix("cc_") {
                if let Ok(cc_num) = cc_str.parse::<u8>() {
                    controls.push((cc_num, app_name.trim().to_string()));
                }
            }
        }
        controls.sort_by_key(|(cc, _)| *cc);
        controls
    }

    pub fn get_mute_button_mappings(&self) -> HashMap<u8, u8> {
        // Returns mapping of mute button CC to target fader CC
        let mut mappings = HashMap::with_capacity(self.midi_controls.mute_buttons.len());
        for (key, &target_cc) in &self.midi_controls.mute_buttons {
            if let Some(cc_str) = key.strip_prefix("cc_") {
                if let Ok(cc_num) = cc_str.parse::<u8>() {
                    mappings.insert(cc_num, target_cc);
                }
            }
        }
        mappings
    }

}

impl Default for Config {
    fn default() -> Self {
        let mut sinks = HashMap::new();
        sinks.insert("cc_0".to_string(), "alsa_output.pci-0000_25_00.0.analog-stereo".to_string());
        sinks.insert("cc_1".to_string(), "comms_sink".to_string());
        
        let applications = HashMap::new();
        let mute_buttons = HashMap::new();
        
        Config {
            midi_controls: MidiControlsConfig { sinks, applications, mute_buttons },
            audio: AudioConfig {
                use_pipewire: Some(true),
                default_sink: Some("alsa_output.pci-0000_25_00.0.analog-stereo".to_string()),
                volume_control_mode: Some("pipewire-api".to_string()),
                volume_curve: Some("linear".to_string()),
                debounce_ms: Some(10),
                applications_sink_search: Some(10),
            },
            ui: UiConfig {
                window_width: Some(1000),
                window_height: Some(600),
                theme: Some("default".to_string()),
                show_console: Some(false),
                max_console_lines: Some(1000),
            },
            logging: LoggingConfig {
                enabled: Some(true),
                log_level: Some("info".to_string()),
                timestamps: Some(true),
                log_fader_events: Some(true),
                log_device_info: Some(true),
            },
        }
    }
}

impl Config {
    /// Save the configuration to a TOML file
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let toml_string = self.to_toml_string()?;
        fs::write(path, toml_string)
            .with_context(|| format!("Failed to write config to: {}", path))?;
        Ok(())
    }
    
    /// Convert config to a formatted TOML string with comments
    pub fn to_toml_string(&self) -> Result<String> {
        let mut output = String::new();
        
        // Header comment
        output.push_str("# nanoKontrol2 MIDI Volume Controller Configuration\n");
        output.push_str("# This file allows you to customize MIDI CC to audio target mappings\n\n");
        
        // MIDI Controls - Sinks
        output.push_str("[midi_controls.sinks]\n");
        output.push_str("# Map MIDI CC numbers to audio sinks (faders)\n");
        output.push_str("# Use CC numbers 0-31 for sink volume controls\n");
        let mut sink_entries: Vec<_> = self.midi_controls.sinks.iter().collect();
        sink_entries.sort_by(|a, b| {
            let a_num = a.0.strip_prefix("cc_").and_then(|s| s.parse::<u8>().ok()).unwrap_or(255);
            let b_num = b.0.strip_prefix("cc_").and_then(|s| s.parse::<u8>().ok()).unwrap_or(255);
            a_num.cmp(&b_num)
        });
        for (key, value) in sink_entries {
            output.push_str(&format!("{} = \"{}\"\n", key, value));
        }
        output.push('\n');
        
        // MIDI Controls - Applications
        output.push_str("[midi_controls.applications]\n");
        output.push_str("# Map MIDI CC numbers to application names\n");
        output.push_str("# Use CC numbers 32-63 or 16-31 for app volume controls\n");
        let mut app_entries: Vec<_> = self.midi_controls.applications.iter().collect();
        app_entries.sort_by(|a, b| {
            let a_num = a.0.strip_prefix("cc_").and_then(|s| s.parse::<u8>().ok()).unwrap_or(255);
            let b_num = b.0.strip_prefix("cc_").and_then(|s| s.parse::<u8>().ok()).unwrap_or(255);
            a_num.cmp(&b_num)
        });
        for (key, value) in app_entries {
            output.push_str(&format!("{} = \"{}\"\n", key, value));
        }
        output.push('\n');
        
        // MIDI Controls - Mute Buttons
        output.push_str("[midi_controls.mute_buttons]\n");
        output.push_str("# Map mute button CC numbers to the CC number of the fader they should mute\n");
        output.push_str("# Format: cc_BUTTON_CC = FADER_CC_NUMBER (where FADER_CC_NUMBER is an integer)\n");
        output.push_str("# Example: cc_64 = 0 means CC64 button mutes the CC0 fader\n");
        let mut mute_entries: Vec<_> = self.midi_controls.mute_buttons.iter().collect();
        mute_entries.sort_by(|a, b| {
            let a_num = a.0.strip_prefix("cc_").and_then(|s| s.parse::<u8>().ok()).unwrap_or(255);
            let b_num = b.0.strip_prefix("cc_").and_then(|s| s.parse::<u8>().ok()).unwrap_or(255);
            a_num.cmp(&b_num)
        });
        for (key, value) in mute_entries {
            output.push_str(&format!("{} = {}\n", key, value));
        }
        output.push('\n');
        
        // Audio section
        output.push_str("[audio]\n");
        output.push_str("# PipeWire settings\n");
        if let Some(use_pipewire) = self.audio.use_pipewire {
            output.push_str(&format!("use_pipewire = {}\n", use_pipewire));
        }
        if let Some(ref default_sink) = self.audio.default_sink {
            output.push_str(&format!("default_sink = \"{}\"\n", default_sink));
        }
        output.push('\n');
        output.push_str("# Volume control mode:\n");
        output.push_str("# \"pw-volume\"     - Use pw-volume command (default, simple)\n");
        output.push_str("# \"pipewire-api\"  - Use PipeWire Rust API (requires libpipewire-dev)\n");
        if let Some(ref mode) = self.audio.volume_control_mode {
            output.push_str(&format!("volume_control_mode = \"{}\"\n", mode));
        }
        output.push('\n');
        output.push_str("# Volume curve (linear/exponential)\n");
        if let Some(ref curve) = self.audio.volume_curve {
            output.push_str(&format!("volume_curve = \"{}\"\n", curve));
        }
        output.push('\n');
        output.push_str("# Debounce MIDI events (ms) to prevent excessive updates and phantom inputs\n");
        if let Some(debounce) = self.audio.debounce_ms {
            output.push_str(&format!("debounce_ms = {}\n", debounce));
        }
        output.push('\n');
        output.push_str("# Interval in seconds to search for application audio sinks\n");
        if let Some(search) = self.audio.applications_sink_search {
            output.push_str(&format!("applications_sink_search = {}\n", search));
        }
        output.push('\n');
        
        // UI section
        output.push_str("[ui]\n");
        output.push_str("# UI settings\n");
        if let Some(width) = self.ui.window_width {
            output.push_str(&format!("window_width = {}\n", width));
        }
        if let Some(height) = self.ui.window_height {
            output.push_str(&format!("window_height = {}\n", height));
        }
        if let Some(ref theme) = self.ui.theme {
            output.push_str(&format!("theme = \"{}\"  # default, dark, light\n", theme));
        }
        output.push('\n');
        output.push_str("# Show console by default\n");
        if let Some(show) = self.ui.show_console {
            output.push_str(&format!("show_console = {}\n", show));
        }
        output.push('\n');
        output.push_str("# Max console messages to keep\n");
        if let Some(lines) = self.ui.max_console_lines {
            output.push_str(&format!("max_console_lines = {}\n", lines));
        }
        output.push('\n');
        
        // Logging section
        output.push_str("[logging]\n");
        output.push_str("# Enable or disable logging globally\n");
        if let Some(enabled) = self.logging.enabled {
            output.push_str(&format!("enabled = {}\n", enabled));
        }
        output.push('\n');
        output.push_str("# Log level: off, error, warn, info, debug, trace\n");
        if let Some(ref level) = self.logging.log_level {
            output.push_str(&format!("log_level = \"{}\"\n", level));
        }
        output.push('\n');
        output.push_str("# Show timestamps in console\n");
        if let Some(timestamps) = self.logging.timestamps {
            output.push_str(&format!("timestamps = {}\n", timestamps));
        }
        output.push('\n');
        output.push_str("# Show fader movements in console\n");
        if let Some(fader) = self.logging.log_fader_events {
            output.push_str(&format!("log_fader_events = {}\n", fader));
        }
        output.push('\n');
        output.push_str("# Show MIDI device info at startup\n");
        if let Some(device) = self.logging.log_device_info {
            output.push_str(&format!("log_device_info = {}\n", device));
        }
        
        Ok(output)
    }
    
    /// Create a Config from UI state values
    pub fn from_ui_state(
        sinks: &[(u8, String)],
        applications: &[(u8, String)],
        mute_buttons: &[(u8, u8)],
        use_pipewire: bool,
        default_sink: &str,
        volume_control_mode: &str,
        volume_curve: &str,
        debounce_ms: u32,
        applications_sink_search: u64,
        window_width: u32,
        window_height: u32,
        theme: &str,
        show_console: bool,
        max_console_lines: usize,
        logging_enabled: bool,
        log_level: &str,
        timestamps: bool,
        log_fader_events: bool,
        log_device_info: bool,
    ) -> Self {
        let mut sinks_map = HashMap::new();
        for (cc, name) in sinks {
            sinks_map.insert(format!("cc_{}", cc), name.clone());
        }
        
        let mut apps_map = HashMap::new();
        for (cc, name) in applications {
            apps_map.insert(format!("cc_{}", cc), name.clone());
        }
        
        let mut mute_map = HashMap::new();
        for (button_cc, fader_cc) in mute_buttons {
            mute_map.insert(format!("cc_{}", button_cc), *fader_cc);
        }
        
        Config {
            midi_controls: MidiControlsConfig {
                sinks: sinks_map,
                applications: apps_map,
                mute_buttons: mute_map,
            },
            audio: AudioConfig {
                use_pipewire: Some(use_pipewire),
                default_sink: Some(default_sink.to_string()),
                volume_control_mode: Some(volume_control_mode.to_string()),
                volume_curve: Some(volume_curve.to_string()),
                debounce_ms: Some(debounce_ms),
                applications_sink_search: Some(applications_sink_search),
            },
            ui: UiConfig {
                window_width: Some(window_width),
                window_height: Some(window_height),
                theme: Some(theme.to_string()),
                show_console: Some(show_console),
                max_console_lines: Some(max_console_lines),
            },
            logging: LoggingConfig {
                enabled: Some(logging_enabled),
                log_level: Some(log_level.to_string()),
                timestamps: Some(timestamps),
                log_fader_events: Some(log_fader_events),
                log_device_info: Some(log_device_info),
            },
        }
    }
}
