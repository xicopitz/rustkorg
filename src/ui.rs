use egui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Control,
    Console,
}

pub struct UiState {
    pub selected_tab: Tab,
    pub system_fader_values: Vec<u8>,
    pub system_fader_labels: Vec<(u8, String)>,  // (CC number, label)
    pub console_output: Vec<(String, chrono::DateTime<chrono::Local>)>,
    pub _show_details: bool,
}

impl UiState {
    pub fn new(system_labels: Vec<(u8, String)>) -> Self {
        let system_count = system_labels.len();
        Self {
            selected_tab: Tab::Control,
            system_fader_values: vec![0; system_count],
            system_fader_labels: system_labels,
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
                    .selectable_label(self.selected_tab == Tab::Control, "♪ Control")
                    .clicked()
                {
                    self.selected_tab = Tab::Control;
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
            let total_ccs = self.system_fader_values.len();

            // System/Sink Controls Section
            if !self.system_fader_values.is_empty() {
                ui.add_space(10.0);
                ui.heading(RichText::new("🔊 Sinks").color(Color32::from_rgb(100, 150, 220)));
                ui.add_space(5.0);
                
                Grid::new("system_faders_grid")
                    .num_columns(1)
                    .spacing([60.0, 20.0])
                    .show(ui, |ui| {
                        for i in 0..self.system_fader_values.len() {
                            Self::render_fader_static(
                                ui,
                                &mut self.system_fader_values[i],
                                &self.system_fader_labels[i].1,
                                self.system_fader_labels[i].0,
                                Color32::from_rgb(100, 150, 220)
                            );
                            ui.end_row();
                        }
                    });
                
                ui.add_space(15.0);
                ui.separator();
            }

            ui.add_space(15.0);
            ui.separator();
            ui.label(RichText::new(format!("♪ nanoKontrol2 CC's in use: {}", total_ccs))
                .color(Color32::from_rgb(150, 150, 150)));
        });
    }

    fn render_fader_static(
        ui: &mut Ui,
        fader_value: &mut u8,
        label: &str,
        cc_num: u8,
        section_color: Color32,
    ) {
        // Label with CC number
        ui.vertical(|ui| {
            ui.label(RichText::new(label).strong().color(section_color));
            ui.label(RichText::new(format!("CC{}", cc_num)).small().color(Color32::GRAY));
        });
        
        let value = *fader_value;
        let percent = (value as f32 / 127.0 * 100.0) as u8;
        
        ui.vertical(|ui| {
            ui.style_mut().visuals.selection.bg_fill = section_color;
            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 40, 45);
            ui.style_mut().visuals.widgets.active.bg_fill = section_color;
            
            ui.add_sized(
                vec2(800.0, 10.0),
                Slider::new(fader_value, 0..=127)
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
            
            // Filled portion
            if filled_width > 0.0 {
                let filled_rect = Rect::from_min_size(
                    rect.min,
                    vec2(filled_width, bar_height)
                );
                ui.painter().rect_filled(
                    filled_rect,
                    2.0,
                    section_color
                );
            }
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
