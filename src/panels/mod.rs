pub mod control;
pub mod console;
pub mod settings;
pub mod theme;
pub mod visualizer;

pub use control::render_faders_tab;
pub use console::render_console_tab;
pub use settings::{render_settings_tab, render_midi_ui_modal};
pub use visualizer::{render_spectrum_visualizer, VisualizerState};
pub use theme::*;

