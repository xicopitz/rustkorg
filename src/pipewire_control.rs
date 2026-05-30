use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
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
    // Cached sink index — avoids double subprocess on every app volume operation
    cached_sink_index: Mutex<Option<(String, u32, Instant)>>,
    // Set by background subscribe thread when sink-inputs change
    availability_changed: Arc<AtomicBool>,
}

const VOLUME_CACHE_TTL: Duration = Duration::from_secs(1);
const SINK_INDEX_CACHE_TTL: Duration = Duration::from_secs(30);

impl PipeWireController {
    pub fn new(_use_api: bool, default_sink_name: &str) -> Self {
        // Start as true so the first availability check runs immediately
        let availability_changed = Arc::new(AtomicBool::new(true));
        Self::spawn_subscribe_listener(Arc::clone(&availability_changed));
        PipeWireController {
            sink_volume_cache: Arc::new(Mutex::new(HashMap::new())),
            app_volume_cache: Arc::new(Mutex::new(HashMap::new())),
            default_sink_name: default_sink_name.to_string(),
            cached_sink_index: Mutex::new(None),
            availability_changed,
        }
    }

    /// Returns true (and clears the flag) if a sink-input or sink change was detected
    /// since the last call. Used by the availability check to skip polling when quiet.
    pub fn take_availability_changed(&self) -> bool {
        self.availability_changed.swap(false, Ordering::Relaxed)
    }

    /// Spawns a long-running `pactl subscribe` thread that sets `availability_changed`
    /// whenever a sink or sink-input event is received. Falls back to polling if pactl fails.
    fn spawn_subscribe_listener(availability_changed: Arc<AtomicBool>) {
        use std::io::{BufRead, BufReader};
        std::thread::spawn(move || loop {
            let child = Command::new("pactl")
                .args(&["subscribe"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn();

            match child {
                Ok(mut child) => {
                    if let Some(stdout) = child.stdout.take() {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines() {
                            match line {
                                Ok(line) => {
                                    // React to sink-input changes (apps) and sink changes
                                    if line.contains("sink-input") || line.contains(" sink #") {
                                        availability_changed.store(true, Ordering::Relaxed);
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }
                    let _ = child.wait();
                }
                Err(_) => {}
            }
            // Retry after a short delay if pactl subscribe exits unexpectedly
            std::thread::sleep(Duration::from_secs(5));
        });
    }

    fn get_sink_index(&self, sink_name: &str) -> Option<u32> {
        // Check cached value first — avoids an extra `pactl list sinks` subprocess
        // on every call to get_matching_app_inputs / list_active_application_names
        if let Ok(cache) = self.cached_sink_index.lock() {
            if let Some((ref cached_name, cached_idx, cached_at)) = *cache {
                if cached_name == sink_name && cached_at.elapsed() < SINK_INDEX_CACHE_TTL {
                    return Some(cached_idx);
                }
            }
        }

        let result = Self::fetch_sink_index(sink_name);

        if let Some(idx) = result {
            if let Ok(mut cache) = self.cached_sink_index.lock() {
                *cache = Some((sink_name.to_string(), idx, Instant::now()));
            }
        }

        result
    }

    fn fetch_sink_index(sink_name: &str) -> Option<u32> {
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
        let output = Command::new("pactl")
            .args(&[
                "set-sink-volume",
                sink_name,
                &format!("{}%", volume_percent),
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "pactl set-sink-volume failed for '{}' ({}%): {}",
                sink_name,
                volume_percent,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

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
            let output = Command::new("pactl")
                .args(&[
                    "set-sink-input-volume",
                    &input_index.to_string(),
                    &format!("{}%", volume_percent),
                ])
                .output()?;

            if !output.status.success() {
                return Err(anyhow!(
                    "pactl set-sink-input-volume failed for app '{}' input {} ({}%): {}",
                    app_name,
                    input_index,
                    volume_percent,
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
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

    /// Checks availability and input count in a single `pactl list sink-inputs` call.
    /// Use this instead of calling `is_app_available` + `get_app_input_count` separately.
    pub fn get_app_availability_and_count(&self, app_name: &str) -> (bool, usize) {
        let inputs = self.get_matching_app_inputs(app_name);
        (!inputs.is_empty(), inputs.len())
    }

    pub fn set_default_sink_name(&mut self, sink_name: &str) {
        self.default_sink_name = sink_name.to_string();
        // Invalidate cached sink index — the target sink has changed
        if let Ok(mut cache) = self.cached_sink_index.lock() {
            *cache = None;
        }
    }

    pub fn list_active_application_names(&self) -> Vec<String> {
        let mut apps: Vec<String> = Vec::new();
        let target_sink_index = match self.get_sink_index(&self.default_sink_name) {
            Some(idx) => idx,
            None => return apps,
        };

        if let Ok(output) = Command::new("pactl").args(&["list", "sink-inputs"]).output() {
            if !output.status.success() {
                return apps;
            }

            let text = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = text.lines().collect();

            let mut current_sink: Option<u32> = None;
            let mut current_app: Option<String> = None;

            for line in &lines {
                if line.starts_with("Sink Input #") {
                    if matched_sink(current_sink, target_sink_index) {
                        if let Some(name) = current_app.take() {
                            if !apps.iter().any(|existing| existing.eq_ignore_ascii_case(&name)) {
                                apps.push(name);
                            }
                        }
                    }
                    current_sink = None;
                    current_app = None;
                    continue;
                }

                if line.trim().starts_with("Sink:") {
                    if let Some(val_str) = line.trim().strip_prefix("Sink:").map(|s| s.trim()) {
                        current_sink = val_str.parse::<u32>().ok();
                    }
                }

                if current_app.is_none() && line.trim().starts_with("application.name =") {
                    if let Some((_, value)) = line.split_once('=') {
                        let name = value.trim().trim_matches('"').to_string();
                        if !name.is_empty() {
                            current_app = Some(name);
                        }
                    }
                }
            }

            if matched_sink(current_sink, target_sink_index) {
                if let Some(name) = current_app {
                    if !apps.iter().any(|existing| existing.eq_ignore_ascii_case(&name)) {
                        apps.push(name);
                    }
                }
            }
        }

        apps.sort();
        apps
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
