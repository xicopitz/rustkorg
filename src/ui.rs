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
    pub _show_details: bool,
}

impl UiState {
    pub fn new(fader_labels: Vec<String>) -> Self {
        let fader_count = fader_labels.len();
        Self {
            selected_tab: Tab::Faders,
            fader_values: vec![0; fader_count],  // Create vector with configured count
            fader_labels,
            console_output: Vec::new(),
            _show_details: false,
        }
    }

    pub fn add_console_message(&mut self, message: String) {
        let now = chrono::Local::now();
        self.console_output.push((message, now));
        
        // Keep only last 30 messages for display
        if self.console_output.len() > 30 {
            self.console_output.remove(0);
        }
    }

    pub fn render_tabs(&mut self, ctx: &Context) {
        TopBottomPanel::top("tab_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing = vec2(10.0, 0.0);
                if ui
                    .selectable_label(self.selected_tab == Tab::Faders, "♪ Faders")
                    .clicked()
                {
                    self.selected_tab = Tab::Faders;
                }
                if ui
                    .selectable_label(self.selected_tab == Tab::Console, "▣ Console")
                    .clicked()
                {
                    self.selected_tab = Tab::Console;
                }
            });
        });
    }

    pub fn render_faders_tab(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.heading(format!("♪ MIDI Fader Values ({} faders)", self.fader_values.len()));

            ui.separator();

            // Create grid for faders - dynamically sized based on configured faders
            Grid::new("faders_grid")
                .num_columns(2)
                .spacing([30.0, 25.0])
                .show(ui, |ui| {
                    for i in 0..self.fader_values.len() {
                        let label = &self.fader_labels[i];
                        let label_lower = label.to_lowercase();
                        let is_master = label_lower.contains("master");
                        
                        // Choose text label and color based on label
                        let (text_label, color) = if is_master {
                            ("[M]", Color32::from_rgb(120, 150, 200)) // Soft blue for master
                        } else if label_lower.contains("chrome") {
                            ("[C]", Color32::from_rgb(200, 100, 100)) // Soft red for Chrome
                        } else if label_lower.contains("spotify") {
                            ("[S]", Color32::from_rgb(100, 200, 120)) // Soft green for Spotify
                        } else if label_lower.contains("discord") {
                            ("[D]", Color32::from_rgb(140, 120, 200)) // Soft purple for Discord
                        } else if label_lower.contains("slack") {
                            ("[K]", Color32::from_rgb(200, 150, 100)) // Soft orange for Slack
                        } else {
                            ("[A]", Color32::from_rgb(150, 150, 150)) // Gray for others
                        };
                        
                        ui.horizontal(|ui| {
                            // Draw colored box icon
                            let box_size = vec2(24.0, 24.0);
                            let (rect, _) = ui.allocate_exact_size(box_size, Sense::hover());
                            
                            // Draw filled colored rectangle
                            ui.painter().rect_filled(rect, 3.0, color);
                            
                            // Draw text label in center of box
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                text_label,
                                FontId::proportional(14.0),
                                Color32::WHITE
                            );
                            
                            ui.label(RichText::new(format!("{}", label))
                                .color(color)
                                .strong());
                        });
                        
                        let value = self.fader_values[i];
                        let percent = (value as f32 / 127.0 * 100.0) as u8;
                        
                        ui.vertical(|ui| {
                            let slider_color = if is_master {
                                Color32::from_rgb(80, 110, 160)
                            } else {
                                Color32::from_rgb(100, 100, 100)
                            };
                            
                            ui.style_mut().visuals.selection.bg_fill = slider_color;
                            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 40, 45);
                            ui.style_mut().visuals.widgets.active.bg_fill = slider_color;
                            
                            ui.add(
                                Slider::new(&mut self.fader_values[i], 0..=127)
                                    .show_value(false)
                                    .text(format!("{}%", percent))
                            );
                            
                            // Add visual volume bar
                            let bar_width = ui.available_width();
                            let bar_height = 6.0;
                            let filled_width = bar_width * (percent as f32 / 100.0);
                            
                            let (rect, _response) = ui.allocate_exact_size(
                                vec2(bar_width, bar_height),
                                Sense::hover()
                            );
                            
                            // Background bar
                            ui.painter().rect_filled(
                                rect,
                                2.0,
                                Color32::from_rgb(30, 30, 35)
                            );
                            
                            // Filled portion with gradient effect
                            if filled_width > 0.0 {
                                let filled_rect = Rect::from_min_size(
                                    rect.min,
                                    vec2(filled_width, bar_height)
                                );
                                ui.painter().rect_filled(
                                    filled_rect,
                                    2.0,
                                    color
                                );
                            }
                        });
                        
                        ui.end_row();
                    }
                });

            ui.separator();
            ui.label(RichText::new(format!("♪ Move faders on nanoKontrol2 to control {} volume zones", self.fader_values.len()))
                .color(Color32::from_rgb(150, 150, 150)));
        });
    }

    pub fn render_console_tab(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.heading("▣ Console Output");

            ui.separator();

            // Console output with scrolling - always shows latest messages
            let scroll_area = ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true);

            scroll_area.show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 4.0;
                
                // Display only the last 30 messages
                let start_index = self.console_output.len().saturating_sub(30);
                for (message, timestamp) in &self.console_output[start_index..] {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("[{}]", timestamp.format("%H:%M:%S")))
                            .color(Color32::from_rgb(100, 150, 200))
                            .monospace());
                        ui.label(RichText::new(message)
                            .color(Color32::from_rgb(200, 200, 200)));
                    });
                }
            });
        });
    }
}
