use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct AppStream {
    pub node_id: u32,
    pub app_name: String,
    pub volume: f32, // 0.0 to 1.0
}

pub struct PipeWireController {
    _app_streams: Arc<Mutex<HashMap<String, (u32, u8)>>>,  // app_name -> (sink_input_index, num_channels)
    _use_api: bool,
}

impl PipeWireController {
    pub fn new(use_api: bool) -> Self {
        PipeWireController {
            _app_streams: Arc::new(Mutex::new(HashMap::new())),
            _use_api: use_api,
        }
    }

    pub fn discover_apps(&mut self) -> Result<Vec<AppStream>> {
        // For now, return an empty list - we'll use pw-volume command directly
        Ok(Vec::new())
    }

    pub fn set_volume_for_app(&self, app_name: &str, volume_percent: u8) -> Result<()> {
        // Use command mode for reliability (same as master volume)
        self.set_app_volume_with_commands(app_name, volume_percent)
    }

    pub fn set_volume_percent(&self, volume_percent: u8) -> Result<()> {
        // For master volume, always use commands instead of API to avoid PulseAudio issues
        self.set_volume_with_commands(volume_percent)
    }

    pub fn get_volume_percent(&self) -> u8 {
        // Try wpctl first
        if let Ok(output) = Command::new("wpctl")
            .args(&["get-volume", "@DEFAULT_AUDIO_SINK@"])
            .output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                // Parse output like "Volume: 0.75" -> 75%
                if let Some(vol_str) = text.split_whitespace().nth(1) {
                    if let Ok(vol_float) = vol_str.parse::<f32>() {
                        return (vol_float * 100.0) as u8;
                    }
                }
            }
        }
        
        // Try pactl
        if let Ok(output) = Command::new("pactl")
            .args(&["get-sink-volume", "@DEFAULT_SINK@"])
            .output() {
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

    pub fn get_volume_for_app(&self, app_name: &str) -> u8 {
        if let Ok(output) = Command::new("pactl")
            .args(&["list", "sink-inputs"])
            .output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let app_name_lower = app_name.to_lowercase();
                let mut _current_idx = None;
                let mut in_matching_input = false;
                
                for line in text.lines() {
                    if line.contains("Sink Input #") {
                        in_matching_input = false;
                        if let Some(idx_str) = line.split('#').nth(1) {
                            _current_idx = idx_str.trim().parse::<u32>().ok();
                        }
                    } else if (line.contains("application.name") || line.contains("application.process.binary")) && 
                              line.to_lowercase().contains(&app_name_lower) {
                        in_matching_input = true;
                    } else if in_matching_input && line.contains("Volume:") {
                        // Parse volume line
                        for part in line.split('/') {
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
        
        50 // Default fallback
    }

    #[allow(dead_code)]
    fn set_app_volume_with_api(&self, app_name: &str, volume_percent: u8) -> Result<()> {
        use libpulse_binding::context::Context;
        use libpulse_binding::mainloop::standard::Mainloop;
        use libpulse_binding::volume::{ChannelVolumes, Volume};
        use libpulse_binding::context::FlagSet as ContextFlagSet;
        
        let mut mainloop = Mainloop::new().ok_or_else(|| anyhow::anyhow!("Failed to create mainloop"))?;
        let mut context = Context::new(&mainloop, "korg-volume").ok_or_else(|| anyhow::anyhow!("Failed to create context"))?;
        
        context.connect(None, ContextFlagSet::NOFLAGS, None)?;
        
        // Wait for context to be ready
        loop {
            mainloop.iterate(false);
            match context.get_state() {
                libpulse_binding::context::State::Ready => break,
                libpulse_binding::context::State::Failed | 
                libpulse_binding::context::State::Terminated => {
                    return Err(anyhow::anyhow!("PulseAudio connection failed"));
                }
                _ => {}
            }
        }
        
        let pa_volume = Volume((volume_percent as f32 / 100.0 * Volume::NORMAL.0 as f32) as u32);
        let app_name_lower = app_name.to_lowercase();
        let _app_name_owned = app_name.to_string();
        let app_name_lower_for_closure = app_name_lower.clone();
        let app_streams = Arc::clone(&self._app_streams);
        let found = Arc::new(Mutex::new(false));
        let found_clone = Arc::clone(&found);
        let sink_count = Arc::new(Mutex::new(0u32));
        let sink_count_clone = Arc::clone(&sink_count);
        let all_apps = Arc::new(Mutex::new(Vec::new()));
        let all_apps_clone = Arc::clone(&all_apps);
        
        // List all sink inputs and find matching app
        let mut introspector = context.introspect();
        introspector.get_sink_input_info_list(move |list| {
            if let libpulse_binding::callbacks::ListResult::Item(input) = list {
                // Count sink inputs
                if let Ok(mut count) = sink_count_clone.lock() {
                    *count += 1;
                }
                
                let props = &input.proplist;
                
                // Get app name and binary for debugging
                let app_name_prop = props.get_str("application.name").map(|n| n.to_string());
                let app_binary = props.get_str("application.process.binary").map(|n| n.to_string());
                let _media_name = props.get_str("media.name").map(|n| n.to_string());
                
                // Store all app names for helpful error message
                if let Some(ref name) = app_name_prop {
                    if let Ok(mut apps) = all_apps_clone.lock() {
                        apps.push(name.clone());
                    }
                } else if let Some(ref name) = app_binary {
                    if let Ok(mut apps) = all_apps_clone.lock() {
                        apps.push(name.clone());
                    }
                }
                
                // Check for app match
                
                let app_match = props.get_str("application.name")
                    .map(|n| n.to_lowercase().contains(&app_name_lower_for_closure))
                    .unwrap_or(false) ||
                    props.get_str("application.process.binary")
                    .map(|n| n.to_lowercase().contains(&app_name_lower_for_closure))
                    .unwrap_or(false) ||
                    props.get_str("media.name")
                    .map(|n| n.to_lowercase().contains(&app_name_lower_for_closure))
                    .unwrap_or(false);
                
                if app_match {
                    let idx = input.index;
                    let num_channels = input.volume.len();
                    if let Ok(mut streams) = app_streams.lock() {
                        streams.insert(app_name_lower_for_closure.clone(), (idx, num_channels));
                    }
                    if let Ok(mut f) = found_clone.lock() {
                        *f = true;
                    }
                }
            }
        });
        
        for _ in 0..10 {
            mainloop.iterate(false);
        }
        
        if let Ok(f) = found.lock() {
            if !*f {
                return Ok(());
            }
        }
        
        // Set volume on the found sink input
        let volume_to_set = Arc::new(Mutex::new(None::<ChannelVolumes>));
        let idx_to_use = if let Ok(streams) = self._app_streams.lock() {
            streams.get(&app_name_lower).map(|&(idx, _)| idx)
        } else {
            None
        };
        
        if let Some(idx) = idx_to_use {
            let volume_clone = Arc::clone(&volume_to_set);
            let pa_volume_for_closure = pa_volume;
            
            introspector.get_sink_input_info(idx, move |result| {
                if let libpulse_binding::callbacks::ListResult::Item(input_info) = result {
                    // Clone the existing volume structure (preserves channel mapping)
                    let current_volume = input_info.volume;
                    let channel_count = current_volume.len();
                    
                    // Validate channel count
                    if channel_count == 0 || channel_count > 32 {
                        eprintln!("⚠️  Invalid channel count: {}", channel_count);
                        return;
                    }
                    
                    // Modify the cloned volume
                    let mut new_volumes = current_volume;
                    for i in 0..channel_count {
                        new_volumes.set(i, pa_volume_for_closure);
                    }
                    
                    if let Ok(mut v) = volume_clone.lock() {
                        *v = Some(new_volumes);
                    }
                }
            });
            
            // Wait for callback to execute
            for _ in 0..10 {
                mainloop.iterate(false);
            }
            
            // Now set the volume
            if let Ok(vol_opt) = volume_to_set.lock() {
                if let Some(volumes) = vol_opt.as_ref() {
                    introspector.set_sink_input_volume(idx, volumes, None);
                }
            }
        }
        
        for _ in 0..10 {
            mainloop.iterate(false);
        }
        
        context.disconnect();
        Ok(())
    }

    fn set_app_volume_with_commands(&self, app_name: &str, volume_percent: u8) -> Result<()> {
        // Use pactl to find and control app
        let output = Command::new("pactl")
            .args(&["list", "sink-inputs"])
            .output();
            
        if let Ok(result) = output {
            let text = String::from_utf8_lossy(&result.stdout);
            let app_name_lower = app_name.to_lowercase();
            let mut current_idx = None;
            
            for line in text.lines() {
                if line.contains("Sink Input #") {
                    if let Some(idx_str) = line.split('#').nth(1) {
                        current_idx = idx_str.trim().parse::<u32>().ok();
                    }
                } else if (line.contains("application.name") || line.contains("application.process.binary")) && 
                          line.to_lowercase().contains(&app_name_lower) {
                    if let Some(idx) = current_idx {
                        let _ = Command::new("pactl")
                            .args(&["set-sink-input-volume", &idx.to_string(), &format!("{}%", volume_percent)])
                            .output();
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn set_volume_with_api(&self, volume_percent: u8) -> Result<()> {
        use libpulse_binding::context::Context;
        use libpulse_binding::mainloop::standard::Mainloop;
        use libpulse_binding::volume::{ChannelVolumes, Volume};
        use libpulse_binding::context::FlagSet as ContextFlagSet;
        use std::sync::{Arc, Mutex};
        
        // Create mainloop and context
        let mut mainloop = Mainloop::new().ok_or_else(|| anyhow::anyhow!("Failed to create mainloop"))?;
        let mut context = Context::new(&mainloop, "korg-volume").ok_or_else(|| anyhow::anyhow!("Failed to create context"))?;
        
        context.connect(None, ContextFlagSet::NOFLAGS, None)?;
        
        // Wait for context to be ready
        loop {
            mainloop.iterate(false);
            match context.get_state() {
                libpulse_binding::context::State::Ready => break,
                libpulse_binding::context::State::Failed | 
                libpulse_binding::context::State::Terminated => {
                    return Err(anyhow::anyhow!("PulseAudio connection failed"));
                }
                _ => {}
            }
        }
        
        // Convert percentage to PulseAudio volume (0-65536 range, where 65536 = 100%)
        let pa_volume = Volume((volume_percent as f32 / 100.0 * Volume::NORMAL.0 as f32) as u32);
        let sink_name = Arc::new(Mutex::new(None::<String>));
        let sink_name_clone = Arc::clone(&sink_name);
        let volumes_ref = Arc::new(Mutex::new(None::<ChannelVolumes>));
        let volumes_clone = Arc::clone(&volumes_ref);
        
        // Get default sink info
        let mut introspector = context.introspect();
        introspector.get_sink_info_by_name("@DEFAULT_SINK@", move |sink_list| {
            if let libpulse_binding::callbacks::ListResult::Item(sink_info) = sink_list {
                let num_channels = sink_info.volume.len();
                
                // Validate channel count
                if num_channels == 0 || num_channels > 32 {
                    return;
                }
                
                // Create a new ChannelVolumes with the correct number of channels
                let mut volumes = ChannelVolumes::default();
                volumes.set_len(num_channels);
                
                // Set all channels to the same volume
                for i in 0..num_channels {
                    volumes.set(i, pa_volume);
                }
                
                if let Ok(mut name) = sink_name_clone.lock() {
                    *name = sink_info.name.as_ref().map(|n| n.to_string());
                }
                if let Ok(mut v) = volumes_clone.lock() {
                    *v = Some(volumes);
                }
            }
        });
        
        // Let the callback execute
        for _ in 0..10 {
            mainloop.iterate(false);
        }
        
        // Now set the volume
        if let (Ok(name_opt), Ok(vol_opt)) = (sink_name.lock(), volumes_ref.lock()) {
            if let (Some(name), Some(volumes)) = (name_opt.as_ref(), vol_opt.as_ref()) {
                introspector.set_sink_volume_by_name(name, volumes, None);
                
                for _ in 0..10 {
                    mainloop.iterate(false);
                }
            }
        }
        
        context.disconnect();
        
        Ok(())
    }

    fn set_volume_with_commands(&self, volume_percent: u8) -> Result<()> {
        // Try multiple audio control methods
        
        // Method 1: wpctl (WirePlumber/PipeWire)
        if self.try_wpctl(volume_percent) {
            return Ok(());
        }
        
        // Method 2: pactl (PulseAudio)
        if self.try_pactl(volume_percent) {
            return Ok(());
        }
        
        // Method 3: amixer (ALSA)
        if self.try_amixer(volume_percent) {
            return Ok(());
        }
        
        // Silently fail - no logging in hot path
        Ok(())
    }

    fn try_wpctl(&self, percent: u8) -> bool {
        let output = Command::new("wpctl")
            .args(&["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{}%", percent)])
            .output();
        matches!(output, Ok(o) if o.status.success())
    }

    fn try_pactl(&self, percent: u8) -> bool {
        let output = Command::new("pactl")
            .args(&["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", percent)])
            .output();
        matches!(output, Ok(o) if o.status.success())
    }

    fn try_amixer(&self, percent: u8) -> bool {
        let output = Command::new("amixer")
            .args(&["set", "Master", &format!("{}%", percent)])
            .output();
        matches!(output, Ok(o) if o.status.success())
    }

    #[allow(dead_code)]
    pub fn get_apps(&self) -> Vec<AppStream> {
        vec![]  // Simplified for now
    }
}

