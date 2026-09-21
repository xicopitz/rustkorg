mod app;
mod config;
mod control;
mod midi;
pub mod panels;
mod pipewire_control;
pub mod spectrum;
mod tray;
mod ui;

use anyhow::Result;
use app::{Backend, ExitReason, MidiVolumeApp, TrayWait};
use config::Config;
use std::sync::{Arc, Mutex};

fn load_app_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/logo.png");
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn main() -> Result<()> {
    // Single source of truth for config location
    let config_path = shellexpand::tilde("~/.bin/audio/nanokontrol2/config.toml").to_string();

    let config = Config::load_with_fallback(&config_path, &config_path)
        .unwrap_or_else(|_| Config::default());

    if config.logging.enabled.unwrap_or(true) {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Off)
            .init();
    }

    // MIDI/audio processing and the tray live here, independent of whether a GUI window
    // is currently open — this is what lets hardware control keep working while hidden.
    let backend = Backend::new(&config);

    let mut start_minimized = config.ui.start_minimized.unwrap_or(false);

    loop {
        // Reload from disk each time a window opens, so a save made in a previous
        // session is reflected.
        let config = Config::load_with_fallback(&config_path, &config_path)
            .unwrap_or_else(|_| Config::default());
        let window_width = config.ui.window_width.unwrap_or(1000) as f32;
        let window_height = config.ui.window_height.unwrap_or(800) as f32;

        let mut viewport = egui::ViewportBuilder::default()
            .with_inner_size([window_width, window_height])
            // On Wayland the dock/taskbar icon comes from a .desktop file matched by
            // app_id, not from a runtime-painted icon — see the installed .desktop entry.
            .with_app_id("korg-midi-volume");
        if let Some(icon) = load_app_icon() {
            viewport = viewport.with_icon(icon);
        }

        let options = eframe::NativeOptions {
            viewport,
            ..Default::default()
        };

        let exit_reason = Arc::new(Mutex::new(ExitReason::Quit));
        let config_path_clone = config_path.clone();
        let exit_reason_clone = exit_reason.clone();
        let backend_clone = backend.clone();

        let _ = eframe::run_native(
            "nanoKontrol2 Volume Controller",
            options,
            Box::new(move |cc| {
                Ok(Box::new(MidiVolumeApp::new(
                    cc,
                    &backend_clone,
                    config,
                    config_path_clone,
                    exit_reason_clone,
                    start_minimized,
                )))
            }),
        );

        start_minimized = false;

        if *exit_reason.lock().unwrap() == ExitReason::Quit {
            break;
        }

        match backend.wait_for_tray() {
            TrayWait::Show => continue,
            TrayWait::Quit => break,
        }
    }

    Ok(())
}
