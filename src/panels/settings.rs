use super::theme;
use crate::ui::UiState;
use egui::{
    CentralPanel, Color32, Context, CornerRadius, Frame, Margin, RichText, ScrollArea, Stroke,
};

pub fn render_settings_tab(ui_state: &mut UiState, ctx: &Context, _tray_functional: bool) -> bool {
    let mut settings_changed = false;

    CentralPanel::default()
        .frame(Frame::default().fill(theme::BG_PRIMARY))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    Frame::default()
                        .inner_margin(Margin {
                            left: 20,
                            right: 20,
                            top: 8,
                            bottom: 8,
                        })
                        .fill(theme::BG_PRIMARY)
                        .show(ui, |ui| {
                            // Show save message if present
                            if let Some((msg, instant)) = &ui_state.settings_save_message {
                                if instant.elapsed().as_secs() < 3 {
                                    let is_success = msg.starts_with("SUCCESS:");
                                    ui.label(RichText::new(msg).size(14.0).color(if is_success {
                                        theme::ACCENT_GREEN
                                    } else {
                                        theme::ACCENT_RED
                                    }));
                                    ui.add_space(2.0);
                                }
                            }

                            // ===== MIDI CONTROLS SECTION =====
                            ui.add_space(8.0);
                            render_section_header(ui, "MIDI Controls", theme::ACCENT_BLUE);
                            ui.add_space(8.0);

                            // --- Sink Mappings ---
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
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("Audio Sinks (CC -> Sink Name)")
                                            .size(14.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    ui.add_space(8.0);

                                    let mut to_remove_sink: Option<usize> = None;
                                    for (idx, (cc, name)) in
                                        ui_state.cfg_sinks.iter_mut().enumerate()
                                    {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!("CC {}:", cc))
                                                    .size(12.0)
                                                    .color(theme::TEXT_SECONDARY),
                                            );
                                            let old_name = name.clone();
                                            ui.add(
                                                egui::TextEdit::singleline(name)
                                                    .desired_width(200.0),
                                            );
                                            if *name != old_name {
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }
                                            if ui.small_button("🗑").clicked() {
                                                to_remove_sink = Some(idx);
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }
                                        });
                                    }
                                    if let Some(idx) = to_remove_sink {
                                        ui_state.cfg_sinks.remove(idx);
                                    }

                                    ui.add_space(8.0);

                                    // Add new sink - directly add on button click
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Add:")
                                                .size(12.0)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(&mut ui_state.new_sink_cc)
                                                .desired_width(40.0)
                                                .hint_text("CC"),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(&mut ui_state.new_sink_name)
                                                .desired_width(150.0)
                                                .hint_text("Sink name"),
                                        );
                                        if ui.button("➕ Add").clicked() {
                                            if let Ok(cc) = ui_state.new_sink_cc.parse::<u8>() {
                                                if !ui_state.new_sink_name.is_empty() {
                                                    ui_state
                                                        .cfg_sinks
                                                        .push((cc, ui_state.new_sink_name.clone()));
                                                    ui_state.cfg_sinks.sort_by_key(|(cc, _)| *cc);
                                                    ui_state.new_sink_cc.clear();
                                                    ui_state.new_sink_name.clear();
                                                    ui_state.settings_dirty = true;
                                                    settings_changed = true;
                                                }
                                            }
                                        }
                                    });
                                });

                            ui.add_space(8.0);

                            // --- Application Mappings ---
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
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("Applications (CC -> App Name)")
                                            .size(14.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    ui.add_space(8.0);

                                    let mut to_remove_app: Option<usize> = None;
                                    for (idx, (cc, name)) in
                                        ui_state.cfg_applications.iter_mut().enumerate()
                                    {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!("CC {}:", cc))
                                                    .size(12.0)
                                                    .color(theme::TEXT_SECONDARY),
                                            );
                                            let old_name = name.clone();
                                            ui.add(
                                                egui::TextEdit::singleline(name)
                                                    .desired_width(200.0),
                                            );
                                            if *name != old_name {
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }
                                            if ui.small_button("🗑").clicked() {
                                                to_remove_app = Some(idx);
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }
                                        });
                                    }
                                    if let Some(idx) = to_remove_app {
                                        ui_state.cfg_applications.remove(idx);
                                    }

                                    ui.add_space(8.0);

                                    // Add new app - directly add on button click
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Add:")
                                                .size(12.0)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(&mut ui_state.new_app_cc)
                                                .desired_width(40.0)
                                                .hint_text("CC"),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(&mut ui_state.new_app_name)
                                                .desired_width(150.0)
                                                .hint_text("App name"),
                                        );
                                        if ui.button("➕ Add").clicked() {
                                            if let Ok(cc) = ui_state.new_app_cc.parse::<u8>() {
                                                if !ui_state.new_app_name.is_empty() {
                                                    ui_state
                                                        .cfg_applications
                                                        .push((cc, ui_state.new_app_name.clone()));
                                                    ui_state
                                                        .cfg_applications
                                                        .sort_by_key(|(cc, _)| *cc);
                                                    ui_state.new_app_cc.clear();
                                                    ui_state.new_app_name.clear();
                                                    ui_state.settings_dirty = true;
                                                    settings_changed = true;
                                                }
                                            }
                                        }
                                    });
                                });

                            ui.add_space(8.0);

                            // --- Mute Button Mappings ---
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
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("Mute Buttons (Button CC -> Fader CC)")
                                            .size(14.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    ui.add_space(8.0);

                                    let mut to_remove_mute: Option<usize> = None;
                                    for (idx, (button_cc, fader_cc)) in
                                        ui_state.cfg_mute_buttons.iter().enumerate()
                                    {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!(
                                                    "CC {} -> CC {}",
                                                    button_cc, fader_cc
                                                ))
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                            );
                                            if ui.small_button("🗑").clicked() {
                                                to_remove_mute = Some(idx);
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }
                                        });
                                    }
                                    if let Some(idx) = to_remove_mute {
                                        ui_state.cfg_mute_buttons.remove(idx);
                                    }

                                    ui.add_space(8.0);

                                    // Add new mute button - directly add on button click
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Add:")
                                                .size(12.0)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut ui_state.new_mute_button_cc,
                                            )
                                            .desired_width(50.0)
                                            .hint_text("Btn CC"),
                                        );
                                        ui.label(RichText::new("->").color(theme::TEXT_MUTED));
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut ui_state.new_mute_fader_cc,
                                            )
                                            .desired_width(50.0)
                                            .hint_text("Fader CC"),
                                        );
                                        if ui.button("➕ Add").clicked() {
                                            if let (Ok(btn_cc), Ok(fader_cc)) = (
                                                ui_state.new_mute_button_cc.parse::<u8>(),
                                                ui_state.new_mute_fader_cc.parse::<u8>(),
                                            ) {
                                                ui_state.cfg_mute_buttons.push((btn_cc, fader_cc));
                                                ui_state
                                                    .cfg_mute_buttons
                                                    .sort_by_key(|(cc, _)| *cc);
                                                ui_state.new_mute_button_cc.clear();
                                                ui_state.new_mute_fader_cc.clear();
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }
                                        }
                                    });
                                });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Show MIDI UI button
                            if ui
                                .button(
                                    RichText::new("Show MIDI UI Layout")
                                        .size(13.0)
                                        .color(theme::TEXT_PRIMARY),
                                )
                                .clicked()
                            {
                                ui_state.show_midi_ui_modal = true;
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // ===== FADER VISIBILITY & ORDER =====
                            ui.add_space(8.0);
                            render_section_header(ui, "Fader Display", theme::ACCENT_GREEN);
                            ui.add_space(8.0);

                            // Audio Sinks Subsection
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
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("🔊 Audio Sinks")
                                            .size(13.0)
                                            .color(theme::ACCENT_BLUE)
                                            .strong(),
                                    );
                                    ui.add_space(8.0);

                                    let sink_order = ui_state.sink_display_order.clone();
                                    for &i in &sink_order {
                                        // Skip if index is out of bounds (can happen if sink list changed)
                                        if i >= ui_state.system_fader_labels.len() {
                                            continue;
                                        }

                                        let mut visible = ui_state
                                            .sink_visibility
                                            .get(i)
                                            .copied()
                                            .unwrap_or(true);
                                        ui.horizontal(|ui| {
                                            if ui
                                                .checkbox(
                                                    &mut visible,
                                                    &ui_state.system_fader_labels[i].1,
                                                )
                                                .changed()
                                            {
                                                if i < ui_state.sink_visibility.len() {
                                                    ui_state.sink_visibility[i] = visible;
                                                }
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }

                                            ui.add_space(8.0);

                                            // Find position in display order
                                            let pos = ui_state
                                                .sink_display_order
                                                .iter()
                                                .position(|&idx| idx == i)
                                                .unwrap_or(i);

                                            // Up button
                                            let can_move_up = pos > 0;
                                            if ui
                                                .add_enabled(can_move_up, egui::Button::new("🔼"))
                                                .clicked()
                                            {
                                                ui_state.sink_display_order.swap(pos, pos - 1);
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }

                                            // Down button
                                            let can_move_down =
                                                pos < ui_state.sink_display_order.len() - 1;
                                            if ui
                                                .add_enabled(can_move_down, egui::Button::new("🔽"))
                                                .clicked()
                                            {
                                                ui_state.sink_display_order.swap(pos, pos + 1);
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }
                                        });
                                    }
                                });

                            ui.add_space(12.0);

                            // Applications Subsection
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
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("🎵 Applications")
                                            .size(13.0)
                                            .color(theme::ACCENT_ORANGE)
                                            .strong(),
                                    );
                                    ui.add_space(8.0);

                                    let app_order = ui_state.app_display_order.clone();
                                    for &i in &app_order {
                                        // Skip if index is out of bounds (can happen if app list changed)
                                        if i >= ui_state.app_fader_labels.len() {
                                            continue;
                                        }

                                        let mut visible =
                                            ui_state.app_visibility.get(i).copied().unwrap_or(true);
                                        ui.horizontal(|ui| {
                                            if ui
                                                .checkbox(
                                                    &mut visible,
                                                    &ui_state.app_fader_labels[i].1,
                                                )
                                                .changed()
                                            {
                                                if i < ui_state.app_visibility.len() {
                                                    ui_state.app_visibility[i] = visible;
                                                }
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }

                                            ui.add_space(8.0);

                                            // Find position in display order
                                            let pos = ui_state
                                                .app_display_order
                                                .iter()
                                                .position(|&idx| idx == i)
                                                .unwrap_or(i);

                                            // Up button
                                            let can_move_up = pos > 0;
                                            if ui
                                                .add_enabled(can_move_up, egui::Button::new("🔼"))
                                                .clicked()
                                            {
                                                ui_state.app_display_order.swap(pos, pos - 1);
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }

                                            // Down button
                                            let can_move_down =
                                                pos < ui_state.app_display_order.len() - 1;
                                            if ui
                                                .add_enabled(can_move_down, egui::Button::new("🔽"))
                                                .clicked()
                                            {
                                                ui_state.app_display_order.swap(pos, pos + 1);
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }
                                        });
                                    }
                                });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // ===== AUDIO SECTION =====
                            ui.add_space(8.0);
                            render_section_header(ui, "Audio Settings", theme::ACCENT_ORANGE);
                            ui.add_space(8.0);

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
                                    ui.set_width(ui.available_width());
                                    // Use PipeWire
                                    let old_use_pipewire = ui_state.cfg_use_pipewire;
                                    ui.checkbox(
                                        &mut ui_state.cfg_use_pipewire,
                                        RichText::new("Use PipeWire")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    if old_use_pipewire != ui_state.cfg_use_pipewire {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }

                                    ui.add_space(8.0);

                                    // Default Sink
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Default Sink:")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        let sink_before = ui_state.cfg_default_sink.clone();
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut ui_state.cfg_default_sink,
                                            )
                                            .desired_width(250.0),
                                        );
                                        if sink_before != ui_state.cfg_default_sink {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });

                                    ui.add_space(8.0);

                                    // Volume Control Mode
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Volume Control Mode:")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        let mode_before = ui_state.cfg_volume_control_mode.clone();
                                        egui::ComboBox::from_id_salt("volume_mode")
                                            .selected_text(&ui_state.cfg_volume_control_mode)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_volume_control_mode,
                                                    "pipewire-api".to_string(),
                                                    "pipewire-api",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_volume_control_mode,
                                                    "pw-volume".to_string(),
                                                    "pw-volume",
                                                );
                                            });
                                        if mode_before != ui_state.cfg_volume_control_mode {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });

                                    ui.add_space(8.0);

                                    // Volume Curve
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Volume Curve:")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        let curve_before = ui_state.cfg_volume_curve.clone();
                                        egui::ComboBox::from_id_salt("volume_curve")
                                            .selected_text(&ui_state.cfg_volume_curve)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_volume_curve,
                                                    "linear".to_string(),
                                                    "linear",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_volume_curve,
                                                    "exponential".to_string(),
                                                    "exponential",
                                                );
                                            });
                                        if curve_before != ui_state.cfg_volume_curve {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });

                                    ui.add_space(8.0);

                                    // Debounce MS
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Debounce (ms):")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        let old_debounce = ui_state.cfg_debounce_ms;
                                        ui.add(
                                            egui::DragValue::new(&mut ui_state.cfg_debounce_ms)
                                                .range(0..=1000),
                                        );
                                        if old_debounce != ui_state.cfg_debounce_ms {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });

                                    ui.add_space(8.0);

                                    // App search interval
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("App Search Interval (s):")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        let old_search = ui_state.cfg_applications_sink_search;
                                        let mut search_val =
                                            ui_state.cfg_applications_sink_search as i64;
                                        ui.add(
                                            egui::DragValue::new(&mut search_val).range(1..=120),
                                        );
                                        ui_state.cfg_applications_sink_search = search_val as u64;
                                        if old_search != ui_state.cfg_applications_sink_search {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });
                                });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // ===== UI SECTION =====
                            ui.add_space(8.0);
                            render_section_header(ui, "UI Settings", theme::ACCENT_GREEN);
                            ui.add_space(8.0);

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
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Window Width:")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        if ui
                                            .text_edit_singleline(&mut ui_state.window_width_str)
                                            .changed()
                                        {
                                            // Try to parse and update the value
                                            if let Ok(val) =
                                                ui_state.window_width_str.parse::<u32>()
                                            {
                                                ui_state.cfg_window_width = val.max(400).min(3000);
                                            }
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }

                                        ui.add_space(16.0);

                                        ui.label(
                                            RichText::new("Height:")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        if ui
                                            .text_edit_singleline(&mut ui_state.window_height_str)
                                            .changed()
                                        {
                                            // Try to parse and update the value
                                            if let Ok(val) =
                                                ui_state.window_height_str.parse::<u32>()
                                            {
                                                ui_state.cfg_window_height = val.max(300).min(2000);
                                            }
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });

                                    ui.add_space(8.0);

                                    // Theme
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Theme:")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        let theme_before = ui_state.cfg_theme.clone();
                                        egui::ComboBox::from_id_salt("theme")
                                            .selected_text(&ui_state.cfg_theme)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_theme,
                                                    "default".to_string(),
                                                    "default",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_theme,
                                                    "dark".to_string(),
                                                    "dark",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_theme,
                                                    "light".to_string(),
                                                    "light",
                                                );
                                            });
                                        if theme_before != ui_state.cfg_theme {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });

                                    ui.add_space(8.0);

                                    // Show console
                                    let old_show_console = ui_state.cfg_show_console;
                                    ui.checkbox(
                                        &mut ui_state.cfg_show_console,
                                        RichText::new("Show Console by Default")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    if old_show_console != ui_state.cfg_show_console {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }

                                    ui.add_space(8.0);

                                    // Show spectrum
                                    let old_show_spectrum = ui_state.cfg_show_spectrum;
                                    ui.checkbox(
                                        &mut ui_state.cfg_show_spectrum,
                                        RichText::new("Show Spectrum Analyzer")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    if old_show_spectrum != ui_state.cfg_show_spectrum {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }

                                    if ui_state.cfg_show_spectrum {
                                        ui.indent("spectrum_options", |ui| {
                                            ui.add_space(8.0);

                                            // Stereo mode
                                            let old_stereo = ui_state.cfg_spectrum_stereo_mode;
                                            ui.checkbox(
                                                &mut ui_state.cfg_spectrum_stereo_mode,
                                                RichText::new("Stereo Mode (L/R split)")
                                                    .size(11.0)
                                                    .color(theme::TEXT_SECONDARY),
                                            );
                                            if old_stereo != ui_state.cfg_spectrum_stereo_mode {
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }

                                            // Show waterfall
                                            let old_waterfall =
                                                ui_state.cfg_spectrum_show_waterfall;
                                            ui.checkbox(
                                                &mut ui_state.cfg_spectrum_show_waterfall,
                                                RichText::new("Show Waterfall History")
                                                    .size(11.0)
                                                    .color(theme::TEXT_SECONDARY),
                                            );
                                            if old_waterfall != ui_state.cfg_spectrum_show_waterfall
                                            {
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }

                                            // Show frequency labels
                                            let old_labels = ui_state.cfg_spectrum_show_labels;
                                            ui.checkbox(
                                                &mut ui_state.cfg_spectrum_show_labels,
                                                RichText::new("Show Frequency Labels")
                                                    .size(11.0)
                                                    .color(theme::TEXT_SECONDARY),
                                            );
                                            if old_labels != ui_state.cfg_spectrum_show_labels {
                                                ui_state.settings_dirty = true;
                                                settings_changed = true;
                                            }

                                            ui.add_space(8.0);

                                            // Select sink to monitor
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new("Monitor Sink:")
                                                        .size(11.0)
                                                        .color(theme::TEXT_SECONDARY),
                                                );
                                                let sink_before =
                                                    ui_state.cfg_spectrum_sink_name.clone();

                                                // Create a list of available sinks
                                                let sink_names: Vec<String> = ui_state
                                                    .system_fader_labels
                                                    .iter()
                                                    .map(|(_, name)| name.clone())
                                                    .collect();

                                                egui::ComboBox::from_id_salt("spectrum_sink")
                                                    .selected_text(&ui_state.cfg_spectrum_sink_name)
                                                    .show_ui(ui, |ui| {
                                                        for sink_name in sink_names {
                                                            ui.selectable_value(
                                                                &mut ui_state
                                                                    .cfg_spectrum_sink_name,
                                                                sink_name.clone(),
                                                                sink_name,
                                                            );
                                                        }
                                                    });

                                                if sink_before != ui_state.cfg_spectrum_sink_name {
                                                    ui_state.settings_dirty = true;
                                                    settings_changed = true;
                                                }
                                            });
                                        });
                                    }

                                    ui.add_space(8.0);

                                    // Max console lines
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Max Console Lines:")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        let old_lines = ui_state.cfg_max_console_lines;
                                        let mut lines_val = ui_state.cfg_max_console_lines as i32;
                                        ui.add(
                                            egui::DragValue::new(&mut lines_val).range(10..=10000),
                                        );
                                        ui_state.cfg_max_console_lines = lines_val as usize;
                                        if old_lines != ui_state.cfg_max_console_lines {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });
                                });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // ===== LOGGING SECTION =====
                            ui.add_space(8.0);
                            render_section_header(ui, "Logging Settings", theme::ACCENT_BLUE);
                            ui.add_space(8.0);

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
                                    ui.set_width(ui.available_width());
                                    let old_logging = ui_state.cfg_logging_enabled;
                                    ui.checkbox(
                                        &mut ui_state.cfg_logging_enabled,
                                        RichText::new("Enable Logging")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    if old_logging != ui_state.cfg_logging_enabled {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }

                                    ui.add_space(8.0);

                                    // Log level
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Log Level:")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        let level_before = ui_state.cfg_log_level.clone();
                                        egui::ComboBox::from_id_salt("log_level")
                                            .selected_text(&ui_state.cfg_log_level)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_log_level,
                                                    "off".to_string(),
                                                    "off",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_log_level,
                                                    "error".to_string(),
                                                    "error",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_log_level,
                                                    "warn".to_string(),
                                                    "warn",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_log_level,
                                                    "info".to_string(),
                                                    "info",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_log_level,
                                                    "debug".to_string(),
                                                    "debug",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_log_level,
                                                    "trace".to_string(),
                                                    "trace",
                                                );
                                            });
                                        if level_before != ui_state.cfg_log_level {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });

                                    ui.add_space(8.0);

                                    // Timestamps
                                    let old_timestamps = ui_state.cfg_timestamps;
                                    ui.checkbox(
                                        &mut ui_state.cfg_timestamps,
                                        RichText::new("Show Timestamps")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    if old_timestamps != ui_state.cfg_timestamps {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }

                                    ui.add_space(8.0);

                                    // Log fader events
                                    let old_fader_events = ui_state.cfg_log_fader_events;
                                    ui.checkbox(
                                        &mut ui_state.cfg_log_fader_events,
                                        RichText::new("Log Fader Events")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    if old_fader_events != ui_state.cfg_log_fader_events {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }

                                    ui.add_space(8.0);

                                    // Log device info
                                    let old_device_info = ui_state.cfg_log_device_info;
                                    ui.checkbox(
                                        &mut ui_state.cfg_log_device_info,
                                        RichText::new("Log Device Info")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    if old_device_info != ui_state.cfg_log_device_info {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }
                                });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // ===== SAVE BUTTON =====
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Save Settings")
                                                .size(14.0)
                                                .color(Color32::WHITE),
                                        )
                                        .fill(theme::ACCENT_BLUE),
                                    )
                                    .clicked()
                                {
                                    settings_changed = true;
                                    ui_state.settings_dirty = true;
                                }

                                ui.add_space(16.0);

                                if ui_state.settings_dirty {
                                    ui.label(
                                        RichText::new("Unsaved changes")
                                            .size(12.0)
                                            .color(theme::ACCENT_ORANGE),
                                    );
                                }
                            });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // ===== ABOUT SECTION =====
                            render_section_header(ui, "About", theme::TEXT_SECONDARY);
                            ui.add_space(8.0);

                            ui.label(
                                RichText::new("nanoKontrol2 Volume Controller")
                                    .size(14.0)
                                    .color(theme::TEXT_SECONDARY),
                            );

                            ui.label(
                                RichText::new("MIDI-controlled audio volume management")
                                    .size(12.0)
                                    .color(theme::TEXT_MUTED),
                            );

                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Config file:")
                                        .size(12.0)
                                        .color(theme::TEXT_MUTED),
                                );
                                ui.label(
                                    RichText::new(&ui_state.config_path)
                                        .size(12.0)
                                        .color(theme::ACCENT_BLUE),
                                );
                            });

                            ui.add_space(8.0);
                        });
                });
        });

    // Render the MIDI UI modal
    render_midi_ui_modal(ui_state, ctx);

    settings_changed
}

fn render_section_header(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(
        RichText::new(format!("[{}]", text.to_uppercase()))
            .size(15.0)
            .color(color)
            .strong(),
    );
}
pub fn render_midi_ui_modal(ui_state: &mut UiState, ctx: &egui::Context) {
    if ui_state.show_midi_ui_modal {
        // Load and display the image
        let image_bytes = include_bytes!("../../assets/korg_detailed.png");

        // Load texture and dimensions if not already loaded
        if ui_state.midi_ui_texture.is_none() {
            if let Ok(image) = image::load_from_memory(image_bytes) {
                let image_rgba = image.to_rgba8();
                let width = image_rgba.width() as f32;
                let height = image_rgba.height() as f32;
                let size = [image_rgba.width() as usize, image_rgba.height() as usize];
                let pixels = image_rgba.to_vec();

                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

                let texture = ctx.load_texture("midi_ui_texture", color_image, Default::default());
                ui_state.midi_ui_texture = Some(texture);
                ui_state.midi_ui_dimensions = Some([width, height]);
            }
        }

        // Get dimensions for the modal size
        let modal_size = ui_state.midi_ui_dimensions.unwrap_or([800.0, 600.0]);

        egui::Window::new("MIDI UI Layout")
            .collapsible(false)
            .resizable(true)
            .default_size(modal_size)
            .open(&mut ui_state.show_midi_ui_modal)
            .show(ctx, |ui| {
                // Display the texture if available
                if let Some(texture) = &ui_state.midi_ui_texture {
                    ui.image(texture);
                }
            });
    }
}
