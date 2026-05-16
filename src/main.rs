mod app;
mod config;
mod midi;
pub mod panels;
mod pipewire_control;
pub mod spectrum;
mod ui;

use anyhow::Result;
use app::MidiVolumeApp;
use config::Config;

fn main() -> Result<()> {
    // Single source of truth for config location
    let config_path = shellexpand::tilde("~/.bin/audio/nanokontrol2/config.toml").to_string();

    // Load config from the fixed path
    let config = Config::load_with_fallback(&config_path, &config_path)
        .unwrap_or_else(|_| Config::default());

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
        viewport: egui::ViewportBuilder::default().with_inner_size([window_width, window_height]),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "nanoKontrol2 Volume Controller",
        options,
        Box::new(|cc| Ok(Box::new(MidiVolumeApp::new(cc, config, config_path)))),
    );

    Ok(())
}
