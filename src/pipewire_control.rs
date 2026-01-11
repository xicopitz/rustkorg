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
}

const VOLUME_CACHE_TTL: Duration = Duration::from_secs(1);

impl PipeWireController {
    pub fn new(_use_api: bool) -> Self {
        PipeWireController {
            sink_volume_cache: Arc::new(Mutex::new(HashMap::new())),
            app_volume_cache: Arc::new(Mutex::new(HashMap::new())),
        }
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
        // Invalidate cache for this app
        if let Ok(mut cache) = self.app_volume_cache.lock() {
            cache.remove(app_name);
        }
        // Use pactl to find and set the volume for a specific application
        let app_name_lower = app_name.to_lowercase();
        let normalized_config = normalize_app_name(&app_name_lower);

        if let Ok(output) = Command::new("pactl")
            .args(&["list", "sink-inputs"])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = text.lines().collect();

                // First pass: look for exact matches or close matches
                for (i, line) in lines.iter().enumerate() {
                    let line_lower = line.to_lowercase();
                    let normalized_line = normalize_app_name(&line_lower);

                    // Check for application.name or application.process.binary fields
                    if (line_lower.contains("application.name")
                        && normalized_line.contains(&normalized_config))
                        || (line_lower.contains("application.process.binary")
                            && normalized_line.contains(&normalized_config))
                        || normalized_line.contains(&normalized_config)
                    {
                        // Look backwards for the sink input index
                        for j in (0..=i).rev() {
                            if lines[j].starts_with("Sink Input #") {
                                if let Some(index_str) = lines[j]
                                    .strip_prefix("Sink Input #")
                                    .and_then(|s| s.split_whitespace().next())
                                {
                                    if let Ok(_index) = index_str.parse::<u32>() {
                                        let _ = Command::new("pactl")
                                            .args(&[
                                                "set-sink-input-volume",
                                                index_str,
                                                &format!("{}%", volume_percent),
                                            ])
                                            .output();
                                        return Ok(());
                                    }
                                }
                                break;
                            }
                        }
                    }
                }

                eprintln!(
                    "App '{}' not found in sink inputs (tried matching: '{}')",
                    app_name, normalized_config
                );
            }
        }

        // Fallback: silently ignore if app not found (it might not be playing audio)
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

        let result = Self::fetch_app_volume(app_name);

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

    #[inline]
    fn fetch_app_volume(app_name: &str) -> u8 {
        // Try to get the current volume for an application
        let app_name_lower = app_name.to_lowercase();
        let normalized_config = normalize_app_name(&app_name_lower);

        if let Ok(output) = Command::new("pactl")
            .args(&["list", "sink-inputs"])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = text.lines().collect();

                // Parse the sink-inputs to find the one matching the app name
                for (i, line) in lines.iter().enumerate() {
                    let line_lower = line.to_lowercase();
                    let normalized_line = normalize_app_name(&line_lower);

                    // Check for application.name or application.process.binary fields
                    if (line_lower.contains("application.name")
                        && normalized_line.contains(&normalized_config))
                        || (line_lower.contains("application.process.binary")
                            && normalized_line.contains(&normalized_config))
                        || normalized_line.contains(&normalized_config)
                    {
                        // Look forward for the volume information
                        for j in i..std::cmp::min(i + 20, lines.len()) {
                            if lines[j].contains("Volume:") {
                                for part in lines[j].split('/') {
                                    if let Some(pct) = part.trim().strip_suffix('%') {
                                        if let Ok(vol) = pct.trim().parse::<u8>() {
                                            return vol;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        50 // Default fallback
    }

    pub fn is_app_available(&self, app_name: &str) -> bool {
        // Check if an application is currently available (has an active sink input)
        let app_name_lower = app_name.to_lowercase();
        let normalized_config = normalize_app_name(&app_name_lower);

        if let Ok(output) = Command::new("pactl")
            .args(&["list", "sink-inputs"])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);

                // Parse the sink-inputs to find one matching the app name
                for line in text.lines() {
                    let line_lower = line.to_lowercase();
                    let normalized_line = normalize_app_name(&line_lower);

                    // Check for application.name or application.process.binary fields
                    if (line_lower.contains("application.name")
                        && normalized_line.contains(&normalized_config))
                        || (line_lower.contains("application.process.binary")
                            && normalized_line.contains(&normalized_config))
                        || normalized_line.contains(&normalized_config)
                    {
                        return true;
                    }
                }
            }
        }

        false
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
