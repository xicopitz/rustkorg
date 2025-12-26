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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub use_pipewire: Option<bool>,
    pub default_sink: Option<String>,
    pub volume_control_mode: Option<String>,
    pub volume_curve: Option<String>,
    pub debounce_ms: Option<u32>,
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
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path))?;
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
    

}

impl Default for Config {
    fn default() -> Self {
        let mut sinks = HashMap::new();
        sinks.insert("cc_0".to_string(), "alsa_output.pci-0000_25_00.0.analog-stereo".to_string());
        sinks.insert("cc_1".to_string(), "comms_sink".to_string());
        
        let applications = HashMap::new();
        
        Config {
            midi_controls: MidiControlsConfig { sinks, applications },
            audio: AudioConfig {
                use_pipewire: Some(true),
                default_sink: Some("alsa_output.pci-0000_25_00.0.analog-stereo".to_string()),
                volume_control_mode: Some("pipewire-api".to_string()),
                volume_curve: Some("linear".to_string()),
                debounce_ms: Some(10),
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
