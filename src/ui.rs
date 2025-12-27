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
    pub system_muted: Vec<bool>,  // Track mute state for each system fader
    pub system_muted_volume: Vec<u8>,  // Store previous volume when muted
    pub system_available: Vec<bool>,  // Track if sink is currently available
    pub app_fader_values: Vec<u8>,
    pub app_fader_labels: Vec<(u8, String)>,  // (CC number, app name)
    pub app_muted: Vec<bool>,  // Track mute state for each app fader
    pub app_muted_volume: Vec<u8>,  // Store previous volume when muted
    pub app_available: Vec<bool>,  // Track if app is currently available
    pub console_output: Vec<(String, chrono::DateTime<chrono::Local>)>,
    max_console_lines: usize,  // Max number of console messages to keep
}

// Dark theme color palette
mod theme {
    use egui::Color32;
    
    pub const BG_PRIMARY: Color32 = Color32::from_rgb(18, 18, 22);
    pub const BG_SECONDARY: Color32 = Color32::from_rgb(28, 28, 35);
    pub const BG_TERTIARY: Color32 = Color32::from_rgb(38, 38, 45);
    
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 230, 235);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(150, 150, 160);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(100, 100, 110);
    
    pub const ACCENT_BLUE: Color32 = Color32::from_rgb(100, 160, 220);
    pub const ACCENT_ORANGE: Color32 = Color32::from_rgb(220, 140, 80);
    pub const ACCENT_RED: Color32 = Color32::from_rgb(220, 100, 100);
    pub const ACCENT_GREEN: Color32 = Color32::from_rgb(100, 200, 150);
    
    pub const BORDER: Color32 = Color32::from_rgb(50, 50, 60);
}

impl UiState {
    pub fn new(system_labels: Vec<(u8, String)>, app_labels: Vec<(u8, String)>, _show_console: bool, max_console_lines: usize) -> Self {
        let system_count = system_labels.len();
        let app_count = app_labels.len();
        Self {
            selected_tab: Tab::Control,
            system_fader_values: vec![0; system_count],
            system_fader_labels: system_labels,
            system_muted: vec![false; system_count],
            system_muted_volume: vec![0; system_count],
            system_available: vec![true; system_count],
            app_fader_values: vec![0; app_count],
            app_fader_labels: app_labels,
            app_muted: vec![false; app_count],
            app_muted_volume: vec![0; app_count],
            app_available: vec![true; app_count],
            console_output: Vec::with_capacity(max_console_lines),
            max_console_lines,
        }
    }

    pub fn add_console_message(&mut self, message: String) {
        let now = chrono::Local::now();
        self.console_output.push((message, now));
        
        // Keep only last max_console_lines messages
        if self.console_output.len() > self.max_console_lines {
            self.console_output.drain(0..self.console_output.len() - self.max_console_lines);
        }
    }

    pub fn apply_dark_theme(ctx: &Context) {
        let mut visuals = Visuals::dark();
        
        // Background colors
        visuals.panel_fill = theme::BG_PRIMARY;
        visuals.extreme_bg_color = theme::BG_PRIMARY;
        visuals.faint_bg_color = theme::BG_SECONDARY;
        visuals.window_fill = theme::BG_SECONDARY;
        
        // Text colors
        visuals.override_text_color = Some(theme::TEXT_PRIMARY);
        visuals.weak_text_color = Some(theme::TEXT_SECONDARY);
        
        // Widget colors
        visuals.widgets.inactive.bg_fill = theme::BG_TERTIARY;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, theme::TEXT_SECONDARY);
        
        visuals.widgets.hovered.bg_fill = theme::ACCENT_BLUE.gamma_multiply(0.5);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, theme::ACCENT_BLUE);
        
        visuals.widgets.active.bg_fill = theme::ACCENT_BLUE;
        visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
        
        // Selection
        visuals.selection.bg_fill = theme::ACCENT_BLUE;
        visuals.selection.stroke = Stroke::new(1.0, theme::ACCENT_BLUE);
        
        // Borders
        visuals.window_stroke = Stroke::new(1.0, theme::BORDER);
        
        ctx.set_visuals(visuals);
    }

    pub fn render_tabs(&mut self, ctx: &Context) {
        Self::apply_dark_theme(ctx);
        
        TopBottomPanel::top("tab_panel")
            .frame(Frame::default()
                .fill(theme::BG_SECONDARY)
                .stroke(Stroke::new(1.0, theme::BORDER)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing = vec2(12.0, 0.0);
                    ui.add_space(8.0);
                    
                    // Control tab
                    if ui
                        .selectable_label(self.selected_tab == Tab::Control, 
                            RichText::new("🎚 Control").size(14.0))
                        .clicked()
                    {
                        self.selected_tab = Tab::Control;
                    }
                    
                    // Console tab
                    if ui
                        .selectable_label(self.selected_tab == Tab::Console, 
                            RichText::new("📋 Console").size(14.0))
                        .clicked()
                    {
                        self.selected_tab = Tab::Console;
                    }
                    
                    ui.add_space(8.0);
                });
            });
    }

    pub fn render_faders_tab(&mut self, ctx: &Context) -> Vec<(bool, usize, u8)> {
        let mut changed_faders = Vec::new();
        
        CentralPanel::default()
            .frame(Frame::default().fill(theme::BG_PRIMARY))
            .show(ctx, |ui| {
                let total_ccs = self.system_fader_values.len() + self.app_fader_values.len();

                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        Frame::default()
                            .inner_margin(egui::Margin { left: 20, right: 20, top: 8, bottom: 8 })
                            .fill(theme::BG_PRIMARY)
                            .show(ui, |ui| {
                        // System/Sink Controls Section
                        if !self.system_fader_values.is_empty() {
                            ui.add_space(16.0);
                            Self::render_section_header(ui, "🔊 Audio Sinks", theme::ACCENT_BLUE);
                            ui.add_space(8.0);
                            
                            for i in 0..self.system_fader_values.len() {
                                let is_muted = self.system_muted[i];
                                let is_available = self.system_available[i];
                                let old_value = self.system_fader_values[i];
                                Self::render_fader_with_mute(
                                    ui,
                                    &mut self.system_fader_values[i],
                                    &self.system_fader_labels[i].1,
                                    self.system_fader_labels[i].0,
                                    theme::ACCENT_BLUE,
                                    is_muted,
                                    is_available,
                                );
                                if old_value != self.system_fader_values[i] {
                                    changed_faders.push((true, i, self.system_fader_values[i]));
                                }
                                ui.add_space(2.0);
                            }
                            
                            ui.add_space(8.0);
                            ui.separator();
                        }

                        // Applications Controls Section
                        if !self.app_fader_values.is_empty() {
                            ui.add_space(16.0);
                            Self::render_section_header(ui, "🎵 Applications", theme::ACCENT_ORANGE);
                            ui.add_space(8.0);
                            
                            for i in 0..self.app_fader_values.len() {
                                let is_muted = self.app_muted[i];
                                let is_available = self.app_available[i];
                                let old_value = self.app_fader_values[i];
                                Self::render_fader_with_mute(
                                    ui,
                                    &mut self.app_fader_values[i],
                                    &self.app_fader_labels[i].1,
                                    self.app_fader_labels[i].0,
                                    theme::ACCENT_ORANGE,
                                    is_muted,
                                    is_available,
                                );
                                if old_value != self.app_fader_values[i] {
                                    changed_faders.push((false, i, self.app_fader_values[i]));
                                }
                                ui.add_space(12.0);
                            }
                            
                            ui.add_space(8.0);
                            ui.separator();
                        }

                        // Footer
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(RichText::new(format!("⚙ {} CC controls active", total_ccs))
                                .color(theme::TEXT_SECONDARY)
                                .size(12.0));
                        });
                        ui.add_space(16.0);
                            });  // Close Frame
                    });  // Close ScrollArea
        });  // Close CentralPanel
        
        changed_faders
    }
    
    fn render_section_header(ui: &mut Ui, title: &str, color: Color32) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(title)
                .strong()
                .size(16.0)
                .color(color));
        });
    }
    
    fn render_fader_with_mute(
        ui: &mut Ui,
        fader_value: &mut u8,
        label: &str,
        cc_num: u8,
        section_color: Color32,
        is_muted: bool,
        is_available: bool,
    ) {
        // Container for each fader
        Frame::default()
            .fill(if is_available { theme::BG_SECONDARY } else { Color32::from_rgb(20, 20, 25) })
            .stroke(Stroke::new(1.0, if is_available { theme::BORDER } else { Color32::from_rgb(40, 40, 45) }))
            .inner_margin(Margin::symmetric(20, 20))
            .corner_radius(CornerRadius::same(6))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Header with label and mute icon
                    ui.horizontal(|ui| {
                        let mute_icon = if is_muted { "🔇" } else { "🔊" };
                        let label_color = if !is_available {
                            theme::TEXT_MUTED
                        } else if is_muted {
                            theme::TEXT_MUTED
                        } else {
                            section_color
                        };
                        
                        ui.label(RichText::new(mute_icon).size(14.0).color(label_color));
                        ui.label(RichText::new(label)
                            .strong()
                            .size(13.0)
                            .color(label_color));
                        
                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("[CC{}]", cc_num))
                            .size(10.0)
                            .color(if is_available { theme::TEXT_MUTED } else { Color32::from_rgb(60, 60, 70) }));
                    });
                    
                    ui.add_space(2.0);
                    
                    // Fader slider
                    let value = *fader_value;
                    let percent = (value as f32 / 127.0 * 100.0) as u8;
                    
                    let fader_color = if is_muted {
                        theme::TEXT_MUTED
                    } else {
                        section_color
                    };
                    
                    // Volume percentage display at the front
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}%", percent))
                            .color(fader_color)
                            .size(11.0));
                        
                        if is_muted {
                            ui.add_space(4.0);
                            ui.label(RichText::new("(MUTED)")
                                .color(theme::ACCENT_RED)
                                .size(10.0)
                                .italics());
                        }
                        
                        ui.add_space(8.0);
                        
                        // Custom slider styling
                        ui.style_mut().visuals.selection.bg_fill = fader_color;
                        ui.style_mut().visuals.widgets.active.bg_fill = fader_color;
                        
                        ui.add(
                            Slider::new(fader_value, 0..=127)
                                .show_value(false)
                                .text("")
                        );
                    });
                    
                    ui.add_space(4.0);
                    
                    // Visual bar display
                    let bar_width = ui.available_width();
                    let bar_height = 7.0;
                    let filled_width = bar_width * (percent as f32 / 100.0);
                    
                    let (rect, _response) = ui.allocate_exact_size(
                        vec2(bar_width, bar_height),
                        Sense::hover()
                    );
                    
                    // Background bar
                    ui.painter().rect_filled(
                        rect,
                        3.0,
                        theme::BG_TERTIARY
                    );
                    
                    // Filled bar
                    if filled_width > 0.5 {
                        let filled_rect = Rect::from_min_size(rect.min, vec2(filled_width, bar_height));
                        ui.painter().rect_filled(filled_rect, 3.0, fader_color);
                    }
                });
            });
    }

    pub fn render_console_tab(&mut self, ctx: &Context) {
        CentralPanel::default()
            .frame(Frame::default()
                .fill(theme::BG_PRIMARY)
                .inner_margin(Margin { left: 12, right: 12, top: 8, bottom: 12 }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📋 Console Output")
                        .strong()
                        .size(16.0)
                        .color(theme::ACCENT_GREEN));
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Console box frame
                Frame::default()
                    .fill(theme::BG_SECONDARY)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .inner_margin(Margin::same(8))
                    .corner_radius(CornerRadius::same(4))
                    .show(ui, |ui| {
                        // Vertical scroll for logs
                        ScrollArea::vertical()
                            .auto_shrink([false; 2])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.style_mut().spacing.item_spacing.y = 4.0;
                                
                                // Show all available messages vertically
                                for (message, timestamp) in &self.console_output {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("[{}]", timestamp.format("%H:%M:%S")))
                                            .color(theme::ACCENT_BLUE)
                                            .size(10.0)
                                            .monospace());
                                        ui.label(RichText::new(message)
                                            .color(theme::TEXT_PRIMARY)
                                            .size(11.0));
                                    });
                                }
                            });
                    });
            });
    }
}
