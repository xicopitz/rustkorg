use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub faders: FadersConfig,
    pub audio: AudioConfig,
    pub ui: UiConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FadersConfig {
    pub fader_1: Option<String>,
    pub fader_2: Option<String>,
    pub fader_3: Option<String>,
    pub fader_4: Option<String>,
    pub fader_5: Option<String>,
    pub fader_6: Option<String>,
    pub fader_7: Option<String>,
    pub fader_8: Option<String>,
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

    pub fn get_fader_labels(&self) -> Vec<String> {
        // Only return faders that are actually defined (non-None)
        // Trim whitespace from labels
        let mut labels = Vec::new();
        
        if let Some(label) = &self.faders.fader_1 { labels.push(label.trim().to_string()); }
        if let Some(label) = &self.faders.fader_2 { labels.push(label.trim().to_string()); }
        if let Some(label) = &self.faders.fader_3 { labels.push(label.trim().to_string()); }
        if let Some(label) = &self.faders.fader_4 { labels.push(label.trim().to_string()); }
        if let Some(label) = &self.faders.fader_5 { labels.push(label.trim().to_string()); }
        if let Some(label) = &self.faders.fader_6 { labels.push(label.trim().to_string()); }
        if let Some(label) = &self.faders.fader_7 { labels.push(label.trim().to_string()); }
        if let Some(label) = &self.faders.fader_8 { labels.push(label.trim().to_string()); }
        
        labels
    }

    pub fn get_fader_mapping(&self) -> Vec<Option<usize>> {
        // Maps physical fader ID (0-7) to UI slider index
        // Returns vec of 8 elements where Some(idx) means this fader is configured at position idx
        let mut mapping = vec![None; 8];
        let mut idx = 0;
        
        if self.faders.fader_1.is_some() { mapping[0] = Some(idx); idx += 1; }
        if self.faders.fader_2.is_some() { mapping[1] = Some(idx); idx += 1; }
        if self.faders.fader_3.is_some() { mapping[2] = Some(idx); idx += 1; }
        if self.faders.fader_4.is_some() { mapping[3] = Some(idx); idx += 1; }
        if self.faders.fader_5.is_some() { mapping[4] = Some(idx); idx += 1; }
        if self.faders.fader_6.is_some() { mapping[5] = Some(idx); idx += 1; }
        if self.faders.fader_7.is_some() { mapping[6] = Some(idx); idx += 1; }
        if self.faders.fader_8.is_some() { mapping[7] = Some(idx); idx += 1; }
        
        mapping
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            faders: FadersConfig {
                fader_1: Some("Master Volume".to_string()),
                fader_2: Some("Chrome".to_string()),
                fader_3: Some("Firefox".to_string()),
                fader_4: Some("Spotify".to_string()),
                fader_5: Some("Discord".to_string()),
                fader_6: Some("VS Code".to_string()),
                fader_7: Some("Games".to_string()),
                fader_8: Some("Mic".to_string()),
            },
            audio: AudioConfig {
                use_pipewire: Some(true),
                default_sink: Some("default".to_string()),
                volume_control_mode: Some("pw-volume".to_string()),
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
                log_level: Some("info".to_string()),
                timestamps: Some(true),
                log_fader_events: Some(true),
                log_device_info: Some(true),
            },
        }
    }
}
