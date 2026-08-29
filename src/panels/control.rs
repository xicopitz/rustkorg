use super::theme;
use super::visualizer::render_spectrum_visualizer;
use egui::*;
use std::collections::HashMap;

pub fn render_faders_tab(
    ui_state: &mut crate::ui::UiState,
    ctx: &Context,
) -> Vec<(bool, usize, u8)> {
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
                        .inner_margin(Margin {
                            left: 20,
                            right: 20,
                            top: 8,
                            bottom: 8,
                        })
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());

                            // Spectrum Visualizer Section
                            ui.add_space(16.0);
                            render_spectrum_visualizer(
                                ui,
                                &mut ui_state.cfg_show_spectrum,
                                &mut ui_state.spectrum_attack_speed,
                                &mut ui_state.spectrum_release_speed,
                                &ui_state.cfg_spectrum_color_palette,
                                &ui_state.spectrum_data,
                                &mut ui_state.visualizer_state,
                                true,
                                ui_state.cfg_spectrum_stereo_mode,
                                ui_state.cfg_spectrum_show_waterfall,
                                ui_state.cfg_spectrum_show_labels,
                            );
                            ui.add_space(8.0);
                            ui.separator();

                            // Minimalist CC assignment list (CC -> App), right after spectrum
                            if ui_state.show_cc_assignments {
                                ui.add_space(12.0);
                                Frame::default()
                                    .fill(theme::BG_SECONDARY)
                                    .stroke(Stroke::new(1.0_f32, theme::BORDER))
                                    .inner_margin(Margin {
                                        left: 12,
                                        right: 12,
                                        top: 8,
                                        bottom: 8,
                                    })
                                    .corner_radius(CornerRadius::same(4))
                                    .show(ui, |ui| {
                                        ui.checkbox(
                                            &mut ui_state.cc_assignments_expanded,
                                            RichText::new("CC Assignments")
                                                .size(13.0)
                                                .color(theme::TEXT_PRIMARY)
                                                .strong(),
                                        );
                                        if !ui_state.cc_assignments_expanded {
                                            return;
                                        }

                                        ui.add_space(6.0);

                                        let app_map: HashMap<u8, &str> = ui_state
                                            .app_fader_labels
                                            .iter()
                                            .map(|(cc, app_name)| (*cc, app_name.as_str()))
                                            .collect();

                                        // nanoKontrol2 knobs: display all 8 slots (CC16..CC23).
                                        ui.horizontal_wrapped(|ui| {
                                            for cc in 16u8..=23u8 {
                                                let app_name = app_map
                                                    .get(&cc)
                                                    .copied()
                                                    .unwrap_or("Unassigned");
                                                let label_color = if app_name == "Unassigned" {
                                                    theme::TEXT_MUTED
                                                } else {
                                                    theme::TEXT_SECONDARY
                                                };

                                                Frame::default()
                                                    .fill(theme::BG_TERTIARY)
                                                    .stroke(Stroke::new(1.0_f32, theme::BORDER))
                                                    .inner_margin(Margin {
                                                        left: 8,
                                                        right: 8,
                                                        top: 4,
                                                        bottom: 4,
                                                    })
                                                    .corner_radius(CornerRadius::same(4))
                                                    .show(ui, |ui| {
                                                        ui.horizontal(|ui| {
                                                            ui.label(
                                                                RichText::new(format!("CC{}", cc))
                                                                    .size(10.0)
                                                                    .color(theme::ACCENT_ORANGE),
                                                            );
                                                            ui.label(
                                                                RichText::new(app_name)
                                                                    .size(10.5)
                                                                    .color(label_color),
                                                            );
                                                        });
                                                    });
                                            }
                                        });
                                    });
                            }

                            // System/Sink Controls Section
                            if !ui_state.system_fader_values.is_empty() {
                                ui.add_space(16.0);
                                let active_sinks = ui_state
                                    .sink_display_order
                                    .iter()
                                    .filter(|&&display_idx| {
                                        ui_state
                                            .sink_visibility
                                            .get(display_idx)
                                            .copied()
                                            .unwrap_or(true)
                                            && ui_state
                                                .system_available
                                                .get(display_idx)
                                                .copied()
                                                .unwrap_or(false)
                                    })
                                    .count();

                                Frame::default()
                                    .fill(theme::BG_SECONDARY)
                                    .stroke(Stroke::new(1.0_f32, theme::BORDER))
                                    .inner_margin(Margin {
                                        left: 12,
                                        right: 12,
                                        top: 8,
                                        bottom: 8,
                                    })
                                    .corner_radius(CornerRadius::same(5))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            render_section_header(
                                                ui,
                                                "🔊 Audio Sinks",
                                                theme::ACCENT_BLUE,
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{} active",
                                                            active_sinks
                                                        ))
                                                        .size(11.0)
                                                        .color(theme::TEXT_SECONDARY),
                                                    );
                                                },
                                            );
                                        });
                                        ui.add_space(8.0);

                                        for &display_idx in &ui_state.sink_display_order {
                                            if display_idx >= ui_state.system_fader_labels.len()
                                                || display_idx >= ui_state.system_fader_values.len()
                                                || display_idx >= ui_state.system_available.len()
                                                || display_idx >= ui_state.system_muted.len()
                                            {
                                                continue;
                                            }

                                            // Skip if not visible
                                            if !ui_state
                                                .sink_visibility
                                                .get(display_idx)
                                                .copied()
                                                .unwrap_or(true)
                                            {
                                                continue;
                                            }

                                            let is_muted = ui_state.system_muted[display_idx];
                                            let is_available =
                                                ui_state.system_available[display_idx];
                                            let old_value =
                                                ui_state.system_fader_values[display_idx];
                                            render_fader_with_mute(
                                                ui,
                                                &mut ui_state.system_fader_values[display_idx],
                                                &ui_state.system_fader_labels[display_idx].1,
                                                ui_state.system_fader_labels[display_idx].0,
                                                theme::ACCENT_BLUE,
                                                is_muted,
                                                is_available,
                                                None,
                                                &mut ui_state.system_peak_values[display_idx],
                                                &mut ui_state.system_peak_times[display_idx],
                                            );
                                            if old_value
                                                != ui_state.system_fader_values[display_idx]
                                            {
                                                changed_faders.push((
                                                    true,
                                                    display_idx,
                                                    ui_state.system_fader_values[display_idx],
                                                ));
                                            }
                                            ui.add_space(2.0);
                                        }
                                    });

                                ui.add_space(8.0);
                                ui.separator();
                            }

                            // Applications Controls Section
                            if !ui_state.app_fader_values.is_empty() {
                                ui.add_space(16.0);
                                let active_apps = ui_state
                                    .app_display_order
                                    .iter()
                                    .filter(|&&display_idx| {
                                        ui_state
                                            .app_visibility
                                            .get(display_idx)
                                            .copied()
                                            .unwrap_or(true)
                                            && ui_state
                                                .app_available
                                                .get(display_idx)
                                                .copied()
                                                .unwrap_or(false)
                                    })
                                    .count();

                                Frame::default()
                                    .fill(theme::BG_SECONDARY)
                                    .stroke(Stroke::new(1.0_f32, theme::BORDER))
                                    .inner_margin(Margin {
                                        left: 12,
                                        right: 12,
                                        top: 8,
                                        bottom: 8,
                                    })
                                    .corner_radius(CornerRadius::same(5))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            render_section_header(
                                                ui,
                                                "🎵 Applications",
                                                theme::ACCENT_ORANGE,
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{} active",
                                                            active_apps
                                                        ))
                                                        .size(11.0)
                                                        .color(theme::TEXT_SECONDARY),
                                                    );
                                                },
                                            );
                                        });
                                        ui.add_space(8.0);

                                        for &display_idx in &ui_state.app_display_order {
                                            if display_idx >= ui_state.app_fader_labels.len()
                                                || display_idx >= ui_state.app_fader_values.len()
                                                || display_idx >= ui_state.app_available.len()
                                                || display_idx >= ui_state.app_muted.len()
                                                || display_idx >= ui_state.app_peak_values.len()
                                            {
                                                continue;
                                            }

                                            // Skip if not visible
                                            if !ui_state
                                                .app_visibility
                                                .get(display_idx)
                                                .copied()
                                                .unwrap_or(true)
                                            {
                                                continue;
                                            }

                                            let is_muted = ui_state.app_muted[display_idx];
                                            let is_available = ui_state.app_available[display_idx];
                                            let old_value = ui_state.app_fader_values[display_idx];
                                            render_fader_with_mute(
                                                ui,
                                                &mut ui_state.app_fader_values[display_idx],
                                                &ui_state.app_fader_labels[display_idx].1,
                                                ui_state.app_fader_labels[display_idx].0,
                                                theme::ACCENT_ORANGE,
                                                is_muted,
                                                is_available,
                                                Some(
                                                    ui_state
                                                        .app_input_count
                                                        .get(display_idx)
                                                        .copied()
                                                        .unwrap_or(0),
                                                ),
                                                &mut ui_state.app_peak_values[display_idx],
                                                &mut ui_state.app_peak_times[display_idx],
                                            );
                                            if old_value != ui_state.app_fader_values[display_idx] {
                                                changed_faders.push((
                                                    false,
                                                    display_idx,
                                                    ui_state.app_fader_values[display_idx],
                                                ));
                                            }
                                            ui.add_space(12.0);
                                        }
                                    });

                                ui.add_space(8.0);
                                ui.separator();
                            }

                            // Footer
                            ui.add_space(16.0);
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(format!("⚙ {} CC controls active", total_ccs))
                                        .color(theme::TEXT_SECONDARY)
                                        .size(12.0),
                                );
                            });

                            if ui_state.show_device_health {
                                ui.add_space(8.0);
                                Frame::default()
                                    .fill(theme::BG_SECONDARY)
                                    .stroke(Stroke::new(1.0_f32, theme::BORDER))
                                    .inner_margin(Margin {
                                        left: 12,
                                        right: 12,
                                        top: 8,
                                        bottom: 8,
                                    })
                                    .corner_radius(CornerRadius::same(4))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("🩺 Device Health")
                                                .size(13.0)
                                                .color(theme::TEXT_PRIMARY)
                                                .strong(),
                                        );
                                        ui.add_space(4.0);

                                        let midi_in = if ui_state.health_midi_input_connected {
                                            "MIDI In: connected"
                                        } else {
                                            "MIDI In: disconnected"
                                        };
                                        let midi_out = if ui_state.health_midi_output_connected {
                                            "MIDI Out: connected"
                                        } else {
                                            "MIDI Out: disconnected"
                                        };

                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(
                                                RichText::new(midi_in)
                                                    .size(11.0)
                                                    .color(theme::TEXT_SECONDARY),
                                            );
                                            ui.label(
                                                RichText::new("|")
                                                    .size(11.0)
                                                    .color(theme::TEXT_MUTED),
                                            );
                                            ui.label(
                                                RichText::new(midi_out)
                                                    .size(11.0)
                                                    .color(theme::TEXT_SECONDARY),
                                            );
                                            ui.label(
                                                RichText::new("|")
                                                    .size(11.0)
                                                    .color(theme::TEXT_MUTED),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "Sink target: {}",
                                                    ui_state.health_pipewire_default_sink
                                                ))
                                                .size(11.0)
                                                .color(theme::TEXT_SECONDARY),
                                            );
                                            ui.label(
                                                RichText::new("|")
                                                    .size(11.0)
                                                    .color(theme::TEXT_MUTED),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "Audio failures: {}",
                                                    ui_state.health_audio_failure_count
                                                ))
                                                .size(11.0)
                                                .color(theme::TEXT_SECONDARY),
                                            );
                                            ui.label(
                                                RichText::new("|")
                                                    .size(11.0)
                                                    .color(theme::TEXT_MUTED),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "MIDI retries: {}",
                                                    ui_state.health_midi_retry_count
                                                ))
                                                .size(11.0)
                                                .color(theme::TEXT_SECONDARY),
                                            );
                                        });

                                        if let Some(last_error) = &ui_state.health_last_error {
                                            ui.label(
                                                RichText::new(format!(
                                                    "Last error: {}",
                                                    last_error
                                                ))
                                                .size(10.0)
                                                .color(theme::ACCENT_ORANGE),
                                            );
                                        }
                                    });
                            }
                            ui.add_space(16.0);
                        }); // Close Frame
                }); // Close ScrollArea
        }); // Close CentralPanel

    changed_faders
}

fn render_section_header(ui: &mut Ui, title: &str, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(RichText::new(title).strong().size(16.0).color(color));
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
    input_count: Option<usize>,
    peak_value: &mut u8,
    peak_time: &mut std::time::Instant,
) {
    // Container for each fader
    Frame::default()
        .fill(theme::BG_SECONDARY)
        .stroke(Stroke::new(1.0_f32, theme::BORDER))
        .inner_margin(Margin {
            left: 20,
            right: 20,
            top: 8,
            bottom: 8,
        })
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
                    ui.label(RichText::new(label).strong().size(13.0).color(label_color));

                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("[CC{}]", cc_num)).size(10.0).color(
                        if is_available {
                            theme::TEXT_MUTED
                        } else {
                            Color32::from_rgb(60, 60, 70)
                        },
                    ));

                    if let Some(count) = input_count {
                        if count > 0 {
                            ui.label(
                                RichText::new(format!("({} inputs)", count))
                                    .size(12.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                        }
                    }
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
                    ui.label(
                        RichText::new(format!("{}%", percent))
                            .color(fader_color)
                            .size(11.0),
                    );

                    if is_muted {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("(MUTED)")
                                .color(theme::ACCENT_RED)
                                .size(10.0)
                                .italics(),
                        );
                    }

                    ui.add_space(8.0);

                    // Minus button
                    if ui.button("−").clicked() {
                        *fader_value = fader_value.saturating_sub(1);
                    }

                    ui.add_space(4.0);

                    // Custom slider styling with bright highlight and visible border
                    let slider_handle_color = if is_muted {
                        Color32::from_rgb(150, 150, 160)
                    } else {
                        Color32::from_rgb(255, 255, 255)
                    };

                    // Set slider colors
                    ui.style_mut().visuals.selection.bg_fill = slider_handle_color;
                    ui.style_mut().visuals.widgets.active.bg_fill = slider_handle_color;
                    ui.style_mut().visuals.widgets.hovered.bg_fill = slider_handle_color;
                    ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::from_rgb(60, 60, 70);

                    // Add border to slider widget
                    ui.style_mut().visuals.selection.stroke =
                        Stroke::new(2.0_f32, slider_handle_color);
                    ui.style_mut().visuals.widgets.active.bg_stroke =
                        Stroke::new(2.0_f32, slider_handle_color);
                    ui.style_mut().visuals.widgets.hovered.bg_stroke =
                        Stroke::new(2.0_f32, slider_handle_color);

                    ui.add(Slider::new(fader_value, 0..=127).show_value(false).text(""));

                    ui.add_space(4.0);

                    // Plus button
                    if ui.button("+").clicked() {
                        *fader_value = fader_value.saturating_add(1);
                    }
                });

                ui.add_space(4.0);

                // Visual bar + peak marker
                let bar_width = ui.available_width();
                let bar_height = 7.0;
                let filled_width = bar_width * (percent as f32 / 100.0);

                let (rect, _response) =
                    ui.allocate_exact_size(vec2(bar_width, bar_height), Sense::hover());

                // Update peak: raise on new high, or reset after full decay
                const PEAK_HOLD_SECS: f32 = 1.5;
                const PEAK_DECAY_SECS: f32 = 1.0;
                let elapsed = peak_time.elapsed().as_secs_f32();

                if value > *peak_value {
                    *peak_value = value;
                    *peak_time = std::time::Instant::now();
                } else if elapsed > PEAK_HOLD_SECS + PEAK_DECAY_SECS {
                    *peak_value = value;
                }

                // Background bar
                ui.painter().rect_filled(rect, 3.0, theme::BG_TERTIARY);

                // Filled bar
                if filled_width > 0.5 {
                    let filled_rect = Rect::from_min_size(rect.min, vec2(filled_width, bar_height));
                    ui.painter().rect_filled(filled_rect, 3.0, fader_color);
                }

                // Peak marker line (only draw if above current value)
                let peak_norm = *peak_value as f32 / 127.0;
                let current_norm = value as f32 / 127.0;
                if peak_norm > current_norm && *peak_value > 0 {
                    let displayed_norm = if elapsed < PEAK_HOLD_SECS {
                        peak_norm
                    } else {
                        let decay_t =
                            ((elapsed - PEAK_HOLD_SECS) / PEAK_DECAY_SECS).clamp(0.0, 1.0);
                        let d = peak_norm - (peak_norm - current_norm) * decay_t;
                        if d <= current_norm {
                            current_norm
                        } else {
                            d
                        }
                    };

                    if displayed_norm > current_norm {
                        let peak_x =
                            (rect.min.x + bar_width * displayed_norm).min(rect.max.x - 2.0);
                        let marker_rect = Rect::from_min_size(
                            egui::pos2(peak_x - 1.0, rect.min.y),
                            vec2(2.0, bar_height),
                        );
                        let alpha = if elapsed < PEAK_HOLD_SECS {
                            255
                        } else {
                            let decay_t =
                                ((elapsed - PEAK_HOLD_SECS) / PEAK_DECAY_SECS).clamp(0.0, 1.0);
                            (255.0 * (1.0 - decay_t)) as u8
                        };
                        ui.painter().rect_filled(
                            marker_rect,
                            0.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                        );
                    }
                }
            });
        });
}
