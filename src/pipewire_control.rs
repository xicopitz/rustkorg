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
    // Set by background subscribe thread when sink-inputs change
    availability_changed: Arc<AtomicBool>,
}

const VOLUME_CACHE_TTL: Duration = Duration::from_secs(1);

impl PipeWireController {
    pub fn new(default_sink_name: &str) -> Self {
        // Start as true so the first availability check runs immediately
        let availability_changed = Arc::new(AtomicBool::new(true));
        Self::spawn_subscribe_listener(Arc::clone(&availability_changed));
        PipeWireController {
            sink_volume_cache: Arc::new(Mutex::new(HashMap::new())),
            app_volume_cache: Arc::new(Mutex::new(HashMap::new())),
            default_sink_name: default_sink_name.to_string(),
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

    /// Every active sink-input with the sink it's currently routed to, for the read-only
    /// routing view (Graph tab) — unlike `get_matching_app_inputs`, this isn't filtered to
    /// a configured app name, it's every stream PipeWire currently knows about.
    pub fn list_all_sink_inputs(&self) -> Vec<(u32, String, Option<u32>)> {
        let mut results = Vec::new();

        let Ok(output) = Command::new("pactl").args(&["list", "sink-inputs"]).output() else {
            return results;
        };
        if !output.status.success() {
            return results;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut current_input_index: Option<u32> = None;
        let mut current_sink_index: Option<u32> = None;
        let mut current_app_name: Option<String> = None;

        for line in text.lines() {
            if line.starts_with("Sink Input #") {
                if let (Some(idx), Some(name)) = (current_input_index, current_app_name.take()) {
                    results.push((idx, name, current_sink_index));
                }
                current_input_index = line
                    .strip_prefix("Sink Input #")
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|s| s.parse::<u32>().ok());
                current_sink_index = None;
                continue;
            }

            let trimmed = line.trim();
            if current_app_name.is_none() {
                if let Some((key, _)) = trimmed.split_once('=') {
                    let key = key.trim().to_lowercase();
                    if key == "application.name" || key == "application.process.binary" {
                        if let Some(name) = extract_pipewire_property_value(trimmed) {
                            if !name.is_empty() {
                                current_app_name = Some(name);
                            }
                        }
                    }
                }
            }
            if current_sink_index.is_none() {
                if let Some(idx_str) = trimmed.strip_prefix("Sink:") {
                    current_sink_index = idx_str.trim().parse::<u32>().ok();
                }
            }
        }

        if let (Some(idx), Some(name)) = (current_input_index, current_app_name) {
            results.push((idx, name, current_sink_index));
        }

        results
    }

    fn get_matching_app_inputs(&self, app_name: &str) -> Vec<(u32, u8)> {
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
                let mut matched_app: bool = false;
                let mut matched_volume: Option<u8> = None;

                for line in &lines {
                    if line.starts_with("Sink Input #") {
                        if let Some(idx) = current_input_index {
                            if matched_app {
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
                        matched_app = false;
                        matched_volume = None;
                        continue;
                    }

                    if let Some(_) = current_input_index {
                        if !matched_app {
                            let line_trimmed = line.trim();
                            let property_key = line_trimmed
                                .split_once('=')
                                .map(|(key, _)| key.trim().to_lowercase())
                                .unwrap_or_default();
                            let property_value = extract_pipewire_property_value(line_trimmed);

                            let normalized_value = property_value
                                .as_deref()
                                .map(normalize_app_name)
                                .unwrap_or_default();
                            let matches_property = (property_key == "application.name"
                                || property_key == "application.process.binary")
                                && normalized_value.contains(&normalized_config);
                            if matches_property {
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
                    if matched_app {
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

    /// Returns `None` if the sink doesn't exist or `pactl` failed/couldn't be parsed,
    /// so callers can tell "really 50%" apart from "couldn't read it" instead of both
    /// looking identical.
    pub fn get_volume_for_sink(&self, sink_name: &str) -> Option<u8> {
        // Check cache first
        if let Ok(cache) = self.sink_volume_cache.lock() {
            if let Some(cached) = cache.get(sink_name) {
                if cached.timestamp.elapsed() < VOLUME_CACHE_TTL {
                    return Some(cached.value);
                }
            }
        }

        let result = Self::fetch_sink_volume(sink_name)?;

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

        Some(result)
    }

    #[inline]
    fn fetch_sink_volume(sink_name: &str) -> Option<u8> {
        let output = Command::new("pactl")
            .args(&["get-sink-volume", sink_name])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        // Parse output like "Volume: front-left: 65536 /  100% / 0.00 dB"
        for part in text.split('/') {
            if let Some(pct) = part.trim().strip_suffix('%') {
                if let Ok(vol) = pct.trim().parse::<u8>() {
                    return Some(vol);
                }
            }
        }
        None
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

        let mut errors = Vec::new();
        for (input_index, _) in &matching_inputs {
            let result = Command::new("pactl")
                .args(&[
                    "set-sink-input-volume",
                    &input_index.to_string(),
                    &format!("{}%", volume_percent),
                ])
                .output();

            match result {
                Ok(output) if !output.status.success() => errors.push(format!(
                    "input {}: {}",
                    input_index,
                    String::from_utf8_lossy(&output.stderr).trim()
                )),
                Err(e) => errors.push(format!("input {}: {}", input_index, e)),
                Ok(_) => {}
            }
        }

        if !errors.is_empty() {
            return Err(anyhow!(
                "pactl set-sink-input-volume failed for app '{}' ({}%) on {} of {} streams: {}",
                app_name,
                volume_percent,
                errors.len(),
                matching_inputs.len(),
                errors.join("; ")
            ));
        }

        Ok(())
    }

    /// Returns `None` if the app has no matching sink-inputs right now, so callers can
    /// tell "really 50%" apart from "app isn't playing anything".
    pub fn get_volume_for_app(&self, app_name: &str) -> Option<u8> {
        // Check cache first
        if let Ok(cache) = self.app_volume_cache.lock() {
            if let Some(cached) = cache.get(app_name) {
                if cached.timestamp.elapsed() < VOLUME_CACHE_TTL {
                    return Some(cached.value);
                }
            }
        }

        let result = self.fetch_app_volume(app_name)?;

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

        Some(result)
    }

    fn fetch_app_volume(&self, app_name: &str) -> Option<u8> {
        let matching_inputs = self.get_matching_app_inputs(app_name);
        if matching_inputs.is_empty() {
            return None;
        }
        let sum: u32 = matching_inputs.iter().map(|(_, v)| *v as u32).sum();
        Some((sum / matching_inputs.len() as u32) as u8)
    }

    /// Checks availability and input count in a single `pactl list sink-inputs` call.
    /// Use this instead of calling `is_app_available` + `get_app_input_count` separately.
    pub fn get_app_availability_and_count(&self, app_name: &str) -> (bool, usize) {
        let inputs = self.get_matching_app_inputs(app_name);
        (!inputs.is_empty(), inputs.len())
    }

    pub fn set_default_sink_name(&mut self, sink_name: &str) {
        self.default_sink_name = sink_name.to_string();
    }

    /// Lists real sink names currently known to PipeWire/PulseAudio, so Settings can offer
    /// a picker instead of relying on free-text entry that fails silently on a typo.
    pub fn list_sink_names(&self) -> Vec<String> {
        let output = match Command::new("pactl").args(&["list", "sinks", "short"]).output() {
            Ok(output) if output.status.success() => output,
            _ => return Vec::new(),
        };

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.split_whitespace().nth(1))
            .map(String::from)
            .collect()
    }

    /// (software sink name, hardware/destination sink name) pairs, parsed from `pactl list
    /// modules`' `module-loopback` entries — this is how a virtual sink like `master_sink`
    /// actually reaches real hardware in a loopback-based routing setup (its `Argument`
    /// looks like `source=master_sink.monitor sink=alsa_output...`), and there's no other
    /// way to discover that relationship from the sink/sink-input listings alone.
    pub fn list_loopback_routes(&self) -> Vec<(String, String)> {
        let output = match Command::new("pactl").args(&["list", "modules"]).output() {
            Ok(output) if output.status.success() => output,
            _ => return Vec::new(),
        };

        let mut routes = Vec::new();
        let mut in_loopback_module = false;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Module #") {
                in_loopback_module = false;
            } else if let Some(name) = trimmed.strip_prefix("Name:") {
                in_loopback_module = name.trim() == "module-loopback";
            } else if in_loopback_module {
                if let Some(args) = trimmed.strip_prefix("Argument:") {
                    let source = args
                        .split_whitespace()
                        .find_map(|kv| kv.strip_prefix("source="))
                        .and_then(|s| s.strip_suffix(".monitor"));
                    let sink = args
                        .split_whitespace()
                        .find_map(|kv| kv.strip_prefix("sink="));
                    if let (Some(source), Some(sink)) = (source, sink) {
                        routes.push((source.to_string(), sink.to_string()));
                    }
                    in_loopback_module = false;
                }
            }
        }

        routes
    }

    /// Sink index + name pairs, for the routing view (Graph tab) where sink-inputs need
    /// to be matched to a sink by index rather than just displaying names.
    pub fn list_sinks_indexed(&self) -> Vec<(u32, String)> {
        let output = match Command::new("pactl").args(&["list", "sinks", "short"]).output() {
            Ok(output) if output.status.success() => output,
            _ => return Vec::new(),
        };

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let idx = fields.next()?.parse::<u32>().ok()?;
                let name = fields.next()?.to_string();
                Some((idx, name))
            })
            .collect()
    }

    pub fn list_active_application_names(&self) -> Vec<String> {
        let mut apps: Vec<String> = Vec::new();

        if let Ok(output) = Command::new("pactl")
            .args(&["list", "sink-inputs"])
            .output()
        {
            if !output.status.success() {
                return apps;
            }

            let text = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = text.lines().collect();

            let mut current_app: Option<String> = None;

            for line in &lines {
                if line.starts_with("Sink Input #") {
                    if let Some(name) = current_app.take() {
                        if !apps
                            .iter()
                            .any(|existing| existing.eq_ignore_ascii_case(&name.as_str()))
                        {
                            apps.push(name);
                        }
                    }
                    current_app = None;
                    continue;
                }

                if current_app.is_none() {
                    let trimmed = line.trim();
                    if trimmed
                        .split_once('=')
                        .map(|(key, _)| key.trim().to_lowercase())
                        .unwrap_or_default()
                        == "application.name"
                    {
                        if let Some(name) = extract_pipewire_property_value(trimmed) {
                            if !name.is_empty() {
                                current_app = Some(name);
                            }
                        }
                    }
                }
            }

            if let Some(name) = current_app {
                if !apps
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&name))
                {
                    apps.push(name);
                }
            }
        }

        apps.sort();
        apps
    }
}

#[inline]
fn extract_pipewire_property_value(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let (key, value) = trimmed.split_once('=')?;
    let _key_name = key.trim();
    let raw_value = value.trim();
    let unquoted = raw_value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(raw_value)
        .trim();
    if unquoted.is_empty() {
        return None;
    }
    Some(unquoted.to_string())
}

// Helper function to normalize application names for matching
// Converts "google chrome" -> "chrome", "google-chrome" -> "chrome", etc.
#[inline]
fn normalize_app_name(name: &str) -> String {
    name.trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_lowercase()
        .replace("google-", "")
        .replace("google ", "")
        .replace(" ", "")
        .replace("-", "")
        .replace("_", "")
}

#[cfg(test)]
mod tests {
    use super::{extract_pipewire_property_value, normalize_app_name};

    #[test]
    fn app_name_matching_ignores_property_names() {
        let line = "                application.name = \"Firefox\"";
        let value = extract_pipewire_property_value(line).unwrap();
        let normalized_firefox = normalize_app_name(&value);
        let normalized_config = normalize_app_name("firefox");
        assert!(normalized_firefox.contains(&normalized_config));

        let property_name = "application.name";
        let normalized_property_name = normalize_app_name(property_name);
        let normalized_name_search = normalize_app_name("name");
        assert_ne!(normalized_property_name, normalized_name_search);
        assert_ne!(normalized_firefox, normalized_property_name);
    }

    #[test]
    fn pipewire_property_parser_handles_real_pactl_format() {
        let input = "                application.process.binary = \"firefox\"";
        assert_eq!(
            extract_pipewire_property_value(input),
            Some("firefox".to_string())
        );
        let input2 = "                application.name = \"Firefox\"";
        assert_eq!(
            extract_pipewire_property_value(input2),
            Some("Firefox".to_string())
        );
    }
}
