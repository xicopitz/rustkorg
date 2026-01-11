mod app;
mod config;
mod midi;
mod pipewire_control;
mod ui;
pub mod panels;
pub mod spectrum;

use anyhow::Result;
use app::MidiVolumeApp;
use config::Config;

fn main() -> Result<()> {
    // Load config with fallback
    let config = Config::load_with_fallback(
        "config.toml",
        "~/.bin/audio/nanokontrol2/config.toml"
    ).unwrap_or_else(|_| Config::default());
    
    // Only initialize logging if enabled in config
    if config.logging.enabled.unwrap_or(true) {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    } else {
        // Set log level to Off to disable all logging
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Off)
            .init();
    }

    // Get window size from config with defaults
    let window_width = config.ui.window_width.unwrap_or(1000) as f32;
    let window_height = config.ui.window_height.unwrap_or(800) as f32;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([window_width, window_height]),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "nanoKontrol2 Volume Controller",
        options,
        Box::new(|cc| {
            Ok(Box::new(MidiVolumeApp::new(cc)))
        }),
    );

    Ok(())
}
