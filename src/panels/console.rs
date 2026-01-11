use super::theme;
use egui::*;

pub fn render_console_tab(
    console_output: &[(String, chrono::DateTime<chrono::Local>)],
    ctx: &Context,
) {
    CentralPanel::default()
        .frame(Frame::default().fill(theme::BG_PRIMARY))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    Frame::default()
                        .fill(theme::BG_PRIMARY)
                        .inner_margin(Margin {
                            left: 20,
                            right: 20,
                            top: 8,
                            bottom: 8,
                        })
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("📋 Console Output")
                                    .strong()
                                    .size(16.0)
                                    .color(theme::ACCENT_GREEN),
                            );
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Console box frame
                            Frame::default()
                                .fill(theme::BG_SECONDARY)
                                .stroke(Stroke::new(1.0, theme::BORDER))
                                .inner_margin(Margin {
                                    left: 20,
                                    right: 20,
                                    top: 8,
                                    bottom: 8,
                                })
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
                                            for (message, timestamp) in console_output {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "[{}]",
                                                            timestamp.format("%H:%M:%S")
                                                        ))
                                                        .color(theme::ACCENT_BLUE)
                                                        .size(10.0)
                                                        .monospace(),
                                                    );
                                                    ui.label(
                                                        RichText::new(message)
                                                            .color(theme::TEXT_PRIMARY)
                                                            .size(11.0),
                                                    );
                                                });
                                            }
                                        });
                                });
                        });
                });
        });
}
