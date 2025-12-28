use egui::*;
use super::theme;

pub fn render_faders_tab(ui_state: &mut crate::ui::UiState, ctx: &Context) -> Vec<(bool, usize, u8)> {
    let mut changed_faders = Vec::new();
    
    CentralPanel::default()
        .frame(Frame::default().fill(theme::BG_PRIMARY))
        .show(ctx, |ui| {
            let total_ccs = ui_state.system_fader_values.len() + ui_state.app_fader_values.len();

            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    Frame::default()
                        .fill(theme::BG_PRIMARY)
                        .inner_margin(Margin { left: 20, right: 20, top: 8, bottom: 8 })
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                    // System/Sink Controls Section
                    if !ui_state.system_fader_values.is_empty() {
                        ui.add_space(16.0);
                        render_section_header(ui, "🔊 Audio Sinks", theme::ACCENT_BLUE);
                        ui.add_space(8.0);
                        
                        for i in 0..ui_state.system_fader_values.len() {
                            let is_muted = ui_state.system_muted[i];
                            let is_available = ui_state.system_available[i];
                            let old_value = ui_state.system_fader_values[i];
                            render_fader_with_mute(
                                ui,
                                &mut ui_state.system_fader_values[i],
                                &ui_state.system_fader_labels[i].1,
                                ui_state.system_fader_labels[i].0,
                                theme::ACCENT_BLUE,
                                is_muted,
                                is_available,
                            );
                            if old_value != ui_state.system_fader_values[i] {
                                changed_faders.push((true, i, ui_state.system_fader_values[i]));
                            }
                            ui.add_space(2.0);
                        }
                        
                        ui.add_space(8.0);
                        ui.separator();
                    }

                    // Applications Controls Section
                    if !ui_state.app_fader_values.is_empty() {
                        ui.add_space(16.0);
                        render_section_header(ui, "🎵 Applications", theme::ACCENT_ORANGE);
                        ui.add_space(8.0);
                        
                        for i in 0..ui_state.app_fader_values.len() {
                            let is_muted = ui_state.app_muted[i];
                            let is_available = ui_state.app_available[i];
                            let old_value = ui_state.app_fader_values[i];
                            render_fader_with_mute(
                                ui,
                                &mut ui_state.app_fader_values[i],
                                &ui_state.app_fader_labels[i].1,
                                ui_state.app_fader_labels[i].0,
                                theme::ACCENT_ORANGE,
                                is_muted,
                                is_available,
                            );
                            if old_value != ui_state.app_fader_values[i] {
                                changed_faders.push((false, i, ui_state.app_fader_values[i]));
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
        .fill(theme::BG_SECONDARY)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .inner_margin(Margin { left: 20, right: 20, top: 8, bottom: 8 })
        .corner_radius(CornerRadius::same(4))
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
