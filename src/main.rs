mod app;
mod config;
mod midi;
mod pipewire_control;
mod ui;

use anyhow::Result;
use app::MidiVolumeApp;
use config::Config;

fn main() -> Result<()> {
    // Load config to check if logging is enabled
    let config = Config::load("config.toml").unwrap_or_else(|_| Config::default());
    
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

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 600.0]),
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
