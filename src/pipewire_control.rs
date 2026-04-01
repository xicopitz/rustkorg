use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// Cache entry for volume lookups with TTL
struct CachedVolume {
    value: u8,
    timestamp: Instant,
}

pub struct PipeWireController {
    sink_volume_cache: Arc<Mutex<HashMap<String, CachedVolume>>>,
    app_volume_cache: Arc<Mutex<HashMap<String, CachedVolume>>>,
    default_sink_name: String,
}

const VOLUME_CACHE_TTL: Duration = Duration::from_secs(1);

impl PipeWireController {
    pub fn new(_use_api: bool, default_sink_name: &str) -> Self {
        PipeWireController {
            sink_volume_cache: Arc::new(Mutex::new(HashMap::new())),
            app_volume_cache: Arc::new(Mutex::new(HashMap::new())),
            default_sink_name: default_sink_name.to_string(),
        }
    }

    fn get_sink_index(&self, sink_name: &str) -> Option<u32> {
        if let Ok(output) = Command::new("pactl").args(&["list", "sinks"]).output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = text.lines().collect();

                let mut current_index: Option<u32> = None;
                for line in &lines {
                    if line.starts_with("Sink #") {
                        if let Some(idx_str) = line
                            .strip_prefix("Sink #")
                            .and_then(|s| s.split_whitespace().next())
                        {
                            current_index = idx_str.parse::<u32>().ok();
                        }
                    }
                    if let Some(idx) = current_index {
                        if line.trim().starts_with("Name:") && line.contains(sink_name) {
                            return Some(idx);
                        }
                    }
                }
            }
        }
        None
    }

    fn get_matching_app_inputs(&self, app_name: &str) -> Vec<(u32, u8)> {
        let target_sink_index = match self.get_sink_index(&self.default_sink_name) {
            Some(idx) => idx,
            None => return Vec::new(),
        };

        let app_name_lower = app_name.to_lowercase();
        let normalized_config = normalize_app_name(&app_name_lower);

        let mut results = Vec::new();

        if let Ok(output) = Command::new("pactl")
            .args(&["list", "sink-inputs"])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = text.lines().collect();

                let mut current_input_index: Option<u32> = None;
                let mut current_sink: Option<u32> = None;
                let mut matched_app: bool = false;
                let mut matched_volume: Option<u8> = None;

                for line in &lines {
                    if line.starts_with("Sink Input #") {
                        if let Some(idx) = current_input_index {
                            if matched_app && matched_sink(current_sink, target_sink_index) {
                                if let Some(vol) = matched_volume {
                                    results.push((idx, vol));
                                }
                            }
                        }
                        if let Some(idx_str) = line
                            .strip_prefix("Sink Input #")
                            .and_then(|s| s.split_whitespace().next())
                        {
                            current_input_index = idx_str.parse::<u32>().ok();
                        }
                        current_sink = None;
                        matched_app = false;
                        matched_volume = None;
                        continue;
                    }

                    if let Some(_) = current_input_index {
                        if line.trim().starts_with("Sink:") {
                            if let Some(val_str) =
                                line.trim().strip_prefix("Sink:").map(|s| s.trim())
                            {
                                current_sink = val_str.parse::<u32>().ok();
                            }
                        }

                        if !matched_app {
                            let line_lower = line.to_lowercase();
                            let normalized_line = normalize_app_name(&line_lower);
                            if (line_lower.contains("application.name")
                                && normalized_line.contains(&normalized_config))
                                || (line_lower.contains("application.process.binary")
                                    && normalized_line.contains(&normalized_config))
                            {
                                matched_app = true;
                            }
                        }

                        if matched_volume.is_none() && line.contains("Volume:") {
                            for part in line.split('/') {
                                if let Some(pct) = part.trim().strip_suffix('%') {
                                    if let Ok(vol) = pct.trim().parse::<u8>() {
                                        matched_volume = Some(vol);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(idx) = current_input_index {
                    if matched_app && matched_sink(current_sink, target_sink_index) {
                        if let Some(vol) = matched_volume {
                            results.push((idx, vol));
                        }
                    }
                }
            }
        }

        results
    }

    pub fn set_volume_for_sink(&self, sink_name: &str, volume_percent: u8) -> Result<()> {
        // Invalidate cache for this sink
        if let Ok(mut cache) = self.sink_volume_cache.lock() {
            cache.remove(sink_name);
        }

        // Use pactl to set sink volume directly
        Command::new("pactl")
            .args(&[
                "set-sink-volume",
                sink_name,
                &format!("{}%", volume_percent),
            ])
            .output()?;
        Ok(())
    }

    pub fn get_volume_for_sink(&self, sink_name: &str) -> u8 {
        // Check cache first
        if let Ok(cache) = self.sink_volume_cache.lock() {
            if let Some(cached) = cache.get(sink_name) {
                if cached.timestamp.elapsed() < VOLUME_CACHE_TTL {
                    return cached.value;
                }
            }
        }

        let result = Self::fetch_sink_volume(sink_name);

        // Update cache
        if let Ok(mut cache) = self.sink_volume_cache.lock() {
            cache.insert(
                sink_name.to_string(),
                CachedVolume {
                    value: result,
                    timestamp: Instant::now(),
                },
            );
        }

        result
    }

    #[inline]
    fn fetch_sink_volume(sink_name: &str) -> u8 {
        if let Ok(output) = Command::new("pactl")
            .args(&["get-sink-volume", sink_name])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                // Parse output like "Volume: front-left: 65536 /  100% / 0.00 dB"
                for part in text.split('/') {
                    if let Some(pct) = part.trim().strip_suffix('%') {
                        if let Ok(vol) = pct.trim().parse::<u8>() {
                            return vol;
                        }
                    }
                }
            }
        }
        50 // Default fallback
    }

    pub fn set_volume_for_app(&self, app_name: &str, volume_percent: u8) -> Result<()> {
        if let Ok(mut cache) = self.app_volume_cache.lock() {
            cache.remove(app_name);
        }

        let matching_inputs = self.get_matching_app_inputs(app_name);
        if matching_inputs.is_empty() {
            eprintln!(
                "App '{}' not found on sink '{}' in sink inputs",
                app_name, self.default_sink_name
            );
            return Ok(());
        }

        for (input_index, _) in &matching_inputs {
            let _ = Command::new("pactl")
                .args(&[
                    "set-sink-input-volume",
                    &input_index.to_string(),
                    &format!("{}%", volume_percent),
                ])
                .output();
        }

        Ok(())
    }

    pub fn get_volume_for_app(&self, app_name: &str) -> u8 {
        // Check cache first
        if let Ok(cache) = self.app_volume_cache.lock() {
            if let Some(cached) = cache.get(app_name) {
                if cached.timestamp.elapsed() < VOLUME_CACHE_TTL {
                    return cached.value;
                }
            }
        }

        let result = self.fetch_app_volume(app_name);

        // Update cache
        if let Ok(mut cache) = self.app_volume_cache.lock() {
            cache.insert(
                app_name.to_string(),
                CachedVolume {
                    value: result,
                    timestamp: Instant::now(),
                },
            );
        }

        result
    }

    fn fetch_app_volume(&self, app_name: &str) -> u8 {
        let matching_inputs = self.get_matching_app_inputs(app_name);
        if matching_inputs.is_empty() {
            return 50;
        }
        let sum: u32 = matching_inputs.iter().map(|(_, v)| *v as u32).sum();
        (sum / matching_inputs.len() as u32) as u8
    }

    pub fn is_app_available(&self, app_name: &str) -> bool {
        !self.get_matching_app_inputs(app_name).is_empty()
    }

    pub fn get_app_input_count(&self, app_name: &str) -> usize {
        self.get_matching_app_inputs(app_name).len()
    }
}

fn matched_sink(current_sink: Option<u32>, target_sink: u32) -> bool {
    match current_sink {
        Some(idx) => idx == target_sink,
        None => false,
    }
}

// Helper function to normalize application names for matching
// Converts "google chrome" -> "chrome", "google-chrome" -> "chrome", etc.
#[inline]
fn normalize_app_name(name: &str) -> String {
    name.replace("google-", "")
        .replace("google ", "")
        .replace(" ", "")
        .replace("-", "")
        .replace("_", "")
}
