use egui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Faders,
    Console,
}

pub struct UiState {
    pub selected_tab: Tab,
    pub fader_values: Vec<u8>,  // Dynamic - only includes configured faders
    pub fader_labels: Vec<String>,
    pub console_output: Vec<(String, chrono::DateTime<chrono::Local>)>,
    pub show_details: bool,
}

impl UiState {
    pub fn new(fader_labels: Vec<String>) -> Self {
        let fader_count = fader_labels.len();
        Self {
            selected_tab: Tab::Faders,
            fader_values: vec![0; fader_count],  // Create vector with configured count
            fader_labels,
            console_output: Vec::new(),
            show_details: false,
        }
    }

    pub fn add_console_message(&mut self, message: String) {
        let now = chrono::Local::now();
        self.console_output.push((message, now));
        
        // Keep only last 1000 messages to prevent memory bloat
        if self.console_output.len() > 1000 {
            self.console_output.remove(0);
        }
    }

    pub fn render_tabs(&mut self, ctx: &Context) {
        TopBottomPanel::top("tab_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.selected_tab == Tab::Faders, "🎚️ Faders")
                    .clicked()
                {
                    self.selected_tab = Tab::Faders;
                }
                if ui
                    .selectable_label(self.selected_tab == Tab::Console, "📝 Console")
                    .clicked()
                {
                    self.selected_tab = Tab::Console;
                }
            });
        });
    }

    pub fn render_faders_tab(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.heading(format!("MIDI Fader Values ({} faders)", self.fader_values.len()));

            ui.separator();

            // Create grid for faders - dynamically sized based on configured faders
            Grid::new("faders_grid")
                .num_columns(2)
                .spacing([20.0, 20.0])
                .show(ui, |ui| {
                    for i in 0..self.fader_values.len() {
                        ui.label(format!("{}: {}", self.fader_labels[i], self.fader_values[i]));
                        let value = self.fader_values[i];
                        let percent = (value as f32 / 127.0 * 100.0) as u8;
                        ui.add(
                            Slider::new(&mut self.fader_values[i], 0..=127)
                                .show_value(false)
                                .text(format!("{} %", percent))
                        );
                        ui.end_row();
                    }
                });

            ui.separator();
            ui.label(format!("Move faders on nanoKontrol2 to control {} volume zones", self.fader_values.len()));
        });
    }

    pub fn render_console_tab(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.heading("Console Output");

            ui.separator();

            // Console output with scrolling - always shows latest messages
            let scroll_area = ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true);

            scroll_area.show(ui, |ui| {
                for (message, timestamp) in &self.console_output {
                    ui.label(format!("[{}] {}", timestamp.format("%H:%M:%S"), message));
                }
            });
        });
    }
}
