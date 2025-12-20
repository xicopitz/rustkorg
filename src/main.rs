mod app;
mod config;
mod midi;
mod pipewire_control;
mod ui;

use anyhow::Result;
use app::MidiVolumeApp;

fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

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
