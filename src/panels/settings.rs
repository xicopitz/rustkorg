use super::theme;
use crate::ui::{Tab, UiState};
use egui::{
    CentralPanel, Color32, Context, CornerRadius, Frame, Margin, RichText, ScrollArea, Stroke,
};
use std::collections::HashSet;

/// Marks settings dirty/changed if `after` differs from the value captured before the
/// widget was drawn. Returns whether it changed, so callers needing extra side effects
/// (tab switching, cascading a checkbox, etc.) can still branch on it.
fn track<T: PartialEq>(before: T, after: &T, dirty: &mut bool, changed: &mut bool) -> bool {
    if before != *after {
        *dirty = true;
        *changed = true;
        true
    } else {
        false
    }
}

/// Every CC currently claimed by a sink, application, or mute button mapping. A real
/// nanoKontrol2 control (fader or button) only sends one CC, so these three lists must
/// not share a CC — besides being physically wrong, two rows with the same CC in the
/// same list silently collapse into one entry when saved (they're stored as a
/// `HashMap<"cc_N", _>` keyed by this number).
fn used_ccs(ui_state: &UiState) -> HashSet<u8> {
    ui_state
        .cfg_sinks
        .iter()
        .map(|(cc, _)| *cc)
        .chain(ui_state.cfg_applications.iter().map(|(cc, _)| *cc))
        .chain(ui_state.cfg_mute_buttons.iter().map(|(cc, _)| *cc))
        .collect()
}

/// Renders a (CC -> name) mapping list — used for both sink and application mappings,
/// which were previously two copy-pasted ~70-line blocks differing only in labels/hints.
#[allow(clippy::too_many_arguments)]
fn render_cc_name_list(
    ui: &mut egui::Ui,
    entries: &mut Vec<(u8, String)>,
    new_cc: &mut String,
    new_name: &mut String,
    name_hint: &str,
    used: &HashSet<u8>,
    add_error: &mut Option<String>,
    settings_dirty: &mut bool,
    settings_changed: &mut bool,
) {
    let mut to_remove: Option<usize> = None;
    for (idx, (cc, name)) in entries.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("CC {}:", cc))
                    .size(12.0)
                    .color(theme::TEXT_SECONDARY),
            );
            let old_name = name.clone();
            ui.add(egui::TextEdit::singleline(name).desired_width(200.0));
            track(old_name, name, settings_dirty, settings_changed);
            if ui.small_button("🗑").clicked() {
                to_remove = Some(idx);
                *settings_dirty = true;
                *settings_changed = true;
            }
        });
    }
    if let Some(idx) = to_remove {
        entries.remove(idx);
    }

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Add:").size(12.0).color(theme::TEXT_MUTED));
        ui.add(
            egui::TextEdit::singleline(new_cc)
                .desired_width(40.0)
                .hint_text("CC"),
        );
        ui.add(
            egui::TextEdit::singleline(new_name)
                .desired_width(150.0)
                .hint_text(name_hint),
        );
        if ui.button("➕ Add").clicked() {
            *add_error = match new_cc.trim().parse::<u8>() {
                Ok(_) if new_name.trim().is_empty() => Some("Enter a name".to_string()),
                Ok(cc) if used.contains(&cc) => {
                    Some(format!("CC {} is already assigned", cc))
                }
                Ok(cc) => {
                    entries.push((cc, new_name.clone()));
                    entries.sort_by_key(|(cc, _)| *cc);
                    new_cc.clear();
                    new_name.clear();
                    *settings_dirty = true;
                    *settings_changed = true;
                    None
                }
                Err(_) => Some("CC must be a number 0-127".to_string()),
            };
        }
    });
    if let Some(msg) = add_error {
        ui.label(RichText::new(msg.as_str()).size(11.0).color(theme::ACCENT_RED));
    }
}

/// Renders the (button CC -> fader CC) mute button list, mirroring `render_cc_name_list`
/// but for the `Vec<(u8, u8)>` shape mute buttons use (no editable name, two CC fields
/// to add). `used` covers button CCs only — validating that `fader_cc` refers to a
/// mapping that actually exists happens at save/reload via `Config::collect_validation_warnings`.
#[allow(clippy::too_many_arguments)]
fn render_mute_button_list(
    ui: &mut egui::Ui,
    entries: &mut Vec<(u8, u8)>,
    new_button_cc: &mut String,
    new_fader_cc: &mut String,
    used: &HashSet<u8>,
    add_error: &mut Option<String>,
    settings_dirty: &mut bool,
    settings_changed: &mut bool,
) {
    let mut to_remove: Option<usize> = None;
    for (idx, (button_cc, fader_cc)) in entries.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("CC {} -> CC {}", button_cc, fader_cc))
                    .size(12.0)
                    .color(theme::TEXT_SECONDARY),
            );
            if ui.small_button("🗑").clicked() {
                to_remove = Some(idx);
                *settings_dirty = true;
                *settings_changed = true;
            }
        });
    }
    if let Some(idx) = to_remove {
        entries.remove(idx);
    }

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Add:").size(12.0).color(theme::TEXT_MUTED));
        ui.add(
            egui::TextEdit::singleline(new_button_cc)
                .desired_width(50.0)
                .hint_text("Btn CC"),
        );
        ui.label(RichText::new("->").color(theme::TEXT_MUTED));
        ui.add(
            egui::TextEdit::singleline(new_fader_cc)
                .desired_width(50.0)
                .hint_text("Fader CC"),
        );
        if ui.button("➕ Add").clicked() {
            *add_error = match (
                new_button_cc.trim().parse::<u8>(),
                new_fader_cc.trim().parse::<u8>(),
            ) {
                (Ok(btn_cc), _) if used.contains(&btn_cc) => {
                    Some(format!("CC {} is already assigned", btn_cc))
                }
                (Ok(btn_cc), Ok(fader_cc)) => {
                    entries.push((btn_cc, fader_cc));
                    entries.sort_by_key(|(cc, _)| *cc);
                    new_button_cc.clear();
                    new_fader_cc.clear();
                    *settings_dirty = true;
                    *settings_changed = true;
                    None
                }
                _ => Some("Both CCs must be numbers 0-127".to_string()),
            };
        }
    });
    if let Some(msg) = add_error {
        ui.label(RichText::new(msg.as_str()).size(11.0).color(theme::ACCENT_RED));
    }
}

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
                            // ===== MIDI CONTROLS SECTION =====
                            ui.add_space(8.0);
                            render_section_header(ui, "MIDI Controls", theme::ACCENT_BLUE);
                            ui.add_space(8.0);

                            let used = used_ccs(ui_state);

                            // --- Sink Mappings ---
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
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("Audio Sinks (CC -> Sink Name)")
                                            .size(14.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    ui.add_space(8.0);

                                    let UiState {
                                        cfg_sinks,
                                        new_sink_cc,
                                        new_sink_name,
                                        sink_add_error,
                                        settings_dirty,
                                        ..
                                    } = ui_state;
                                    render_cc_name_list(
                                        ui,
                                        cfg_sinks,
                                        new_sink_cc,
                                        new_sink_name,
                                        "Sink name",
                                        &used,
                                        sink_add_error,
                                        settings_dirty,
                                        &mut settings_changed,
                                    );
                                });

                            ui.add_space(8.0);

                            // --- Application Mappings ---
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
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("Applications (CC -> App Name)")
                                            .size(14.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    ui.add_space(8.0);

                                    let UiState {
                                        cfg_applications,
                                        new_app_cc,
                                        new_app_name,
                                        app_add_error,
                                        settings_dirty,
                                        ..
                                    } = ui_state;
                                    render_cc_name_list(
                                        ui,
                                        cfg_applications,
                                        new_app_cc,
                                        new_app_name,
                                        "App name",
                                        &used,
                                        app_add_error,
                                        settings_dirty,
                                        &mut settings_changed,
                                    );

                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        if ui
                                            .button(
                                                RichText::new("Auto-Assign Active Apps")
                                                    .size(12.0)
                                                    .color(theme::TEXT_PRIMARY),
                                            )
                                            .clicked()
                                        {
                                            ui_state.auto_assign_apps_clicked = true;
                                        }

                                        ui.label(
                                            RichText::new(
                                                "Scans current PipeWire app streams and maps them to free CCs",
                                            )
                                            .size(11.0)
                                            .color(theme::TEXT_MUTED),
                                        );
                                    });
                                });

                            ui.add_space(8.0);

                            // --- Mute Button Mappings ---
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
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("Mute Buttons (Button CC -> Fader CC)")
                                            .size(14.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    ui.add_space(8.0);

                                    let UiState {
                                        cfg_mute_buttons,
                                        new_mute_button_cc,
                                        new_mute_fader_cc,
                                        mute_add_error,
                                        settings_dirty,
                                        ..
                                    } = ui_state;
                                    render_mute_button_list(
                                        ui,
                                        cfg_mute_buttons,
                                        new_mute_button_cc,
                                        new_mute_fader_cc,
                                        &used,
                                        mute_add_error,
                                        settings_dirty,
                                        &mut settings_changed,
                                    );
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
                                .stroke(Stroke::new(1.0_f32, theme::BORDER))
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
                                .stroke(Stroke::new(1.0_f32, theme::BORDER))
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
                                .stroke(Stroke::new(1.0_f32, theme::BORDER))
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
                                    track(
                                        old_use_pipewire,
                                        &ui_state.cfg_use_pipewire,
                                        &mut ui_state.settings_dirty,
                                        &mut settings_changed,
                                    );

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
                                        // Live-queried sinks avoid a typo silently failing later.
                                        if !ui_state.available_sinks.is_empty() {
                                            egui::ComboBox::from_id_salt("default_sink_picker")
                                                .selected_text("pick…")
                                                .show_ui(ui, |ui| {
                                                    for sink_name in
                                                        ui_state.available_sinks.clone()
                                                    {
                                                        ui.selectable_value(
                                                            &mut ui_state.cfg_default_sink,
                                                            sink_name.clone(),
                                                            sink_name,
                                                        );
                                                    }
                                                });
                                        }
                                        track(
                                            sink_before,
                                            &ui_state.cfg_default_sink,
                                            &mut ui_state.settings_dirty,
                                            &mut settings_changed,
                                        );
                                    });

                                    ui.add_space(8.0);

                                    // Volume response mode
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Volume Response:")
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
                                                    "logarithmic".to_string(),
                                                    "logarithmic",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_volume_curve,
                                                    "soft-takeover".to_string(),
                                                    "soft-takeover",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_volume_curve,
                                                    "inertia".to_string(),
                                                    "inertia",
                                                );
                                            });
                                        track(
                                            curve_before,
                                            &ui_state.cfg_volume_curve,
                                            &mut ui_state.settings_dirty,
                                            &mut settings_changed,
                                        );
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
                                        track(
                                            old_debounce,
                                            &ui_state.cfg_debounce_ms,
                                            &mut ui_state.settings_dirty,
                                            &mut settings_changed,
                                        );
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
                                        track(
                                            old_search,
                                            &ui_state.cfg_applications_sink_search,
                                            &mut ui_state.settings_dirty,
                                            &mut settings_changed,
                                        );
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
                                .stroke(Stroke::new(1.0_f32, theme::BORDER))
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
                                            // Only mark dirty on a value that actually parses,
                                            // so mid-edit garbage doesn't flag "unsaved changes".
                                            if let Ok(val) =
                                                ui_state.window_width_str.parse::<u32>()
                                            {
                                                let old = ui_state.cfg_window_width;
                                                ui_state.cfg_window_width = val.max(400).min(3000);
                                                track(
                                                    old,
                                                    &ui_state.cfg_window_width,
                                                    &mut ui_state.settings_dirty,
                                                    &mut settings_changed,
                                                );
                                            }
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
                                            if let Ok(val) =
                                                ui_state.window_height_str.parse::<u32>()
                                            {
                                                let old = ui_state.cfg_window_height;
                                                ui_state.cfg_window_height = val.max(300).min(2000);
                                                track(
                                                    old,
                                                    &ui_state.cfg_window_height,
                                                    &mut ui_state.settings_dirty,
                                                    &mut settings_changed,
                                                );
                                            }
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
                                                    "midnight".to_string(),
                                                    "midnight",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_theme,
                                                    "sunset".to_string(),
                                                    "sunset",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_theme,
                                                    "ocean".to_string(),
                                                    "ocean",
                                                );
                                                ui.selectable_value(
                                                    &mut ui_state.cfg_theme,
                                                    "light".to_string(),
                                                    "light",
                                                );
                                            });
                                        track(
                                            theme_before,
                                            &ui_state.cfg_theme,
                                            &mut ui_state.settings_dirty,
                                            &mut settings_changed,
                                        );
                                    });

                                    ui.add_space(8.0);

                                    // Show console tab
                                    let old_show_console = ui_state.cfg_show_console;
                                    ui.checkbox(
                                        &mut ui_state.cfg_show_console,
                                        RichText::new("Show Console Tab")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    if track(
                                        old_show_console,
                                        &ui_state.cfg_show_console,
                                        &mut ui_state.settings_dirty,
                                        &mut settings_changed,
                                    ) {
                                        // Apply tab visibility change immediately in the same frame.
                                        if !ui_state.cfg_show_console
                                            && ui_state.selected_tab == Tab::Console
                                        {
                                            ui_state.selected_tab = Tab::Control;
                                        }
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
                                    track(
                                        old_show_spectrum,
                                        &ui_state.cfg_show_spectrum,
                                        &mut ui_state.settings_dirty,
                                        &mut settings_changed,
                                    );

                                    ui.add_space(8.0);

                                    // Show CC assignments strip
                                    let old_show_cc_assignments = ui_state.show_cc_assignments;
                                    ui.checkbox(
                                        &mut ui_state.show_cc_assignments,
                                        RichText::new("Show CC Assignments")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    track(
                                        old_show_cc_assignments,
                                        &ui_state.show_cc_assignments,
                                        &mut ui_state.settings_dirty,
                                        &mut settings_changed,
                                    );

                                    ui.add_space(8.0);

                                    // Show device health panel
                                    let old_show_device_health = ui_state.show_device_health;
                                    ui.checkbox(
                                        &mut ui_state.show_device_health,
                                        RichText::new("Show Device Health")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    track(
                                        old_show_device_health,
                                        &ui_state.show_device_health,
                                        &mut ui_state.settings_dirty,
                                        &mut settings_changed,
                                    );

                                    // ---- System Tray ----
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("[SYSTEM TRAY]")
                                            .size(15.0)
                                            .color(theme::ACCENT_ORANGE)
                                            .strong(),
                                    );
                                    ui.add_space(4.0);
                                    let old_tray = ui_state.enable_tray;
                                    ui.checkbox(
                                        &mut ui_state.enable_tray,
                                        RichText::new("Enable system tray icon")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    track(
                                        old_tray,
                                        &ui_state.enable_tray,
                                        &mut ui_state.settings_dirty,
                                        &mut settings_changed,
                                    );
                                    if ui_state.enable_tray {
                                        let old_close = ui_state.close_to_tray;
                                        ui.checkbox(
                                            &mut ui_state.close_to_tray,
                                            RichText::new("Minimize to tray on close")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        track(
                                            old_close,
                                            &ui_state.close_to_tray,
                                            &mut ui_state.settings_dirty,
                                            &mut settings_changed,
                                        );
                                        let old_start = ui_state.start_minimized;
                                        ui.checkbox(
                                            &mut ui_state.start_minimized,
                                            RichText::new("Start minimized to tray")
                                                .size(12.0)
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        track(
                                            old_start,
                                            &ui_state.start_minimized,
                                            &mut ui_state.settings_dirty,
                                            &mut settings_changed,
                                        );
                                        ui.label(
                                            RichText::new(
                                                "Note: Tray toggle takes effect after Save + Restart",
                                            )
                                            .size(11.0)
                                            .color(theme::TEXT_MUTED),
                                        );
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
                                            track(
                                                old_stereo,
                                                &ui_state.cfg_spectrum_stereo_mode,
                                                &mut ui_state.settings_dirty,
                                                &mut settings_changed,
                                            );

                                            // Show waterfall
                                            let old_waterfall =
                                                ui_state.cfg_spectrum_show_waterfall;
                                            ui.checkbox(
                                                &mut ui_state.cfg_spectrum_show_waterfall,
                                                RichText::new("Show Waterfall History")
                                                    .size(11.0)
                                                    .color(theme::TEXT_SECONDARY),
                                            );
                                            track(
                                                old_waterfall,
                                                &ui_state.cfg_spectrum_show_waterfall,
                                                &mut ui_state.settings_dirty,
                                                &mut settings_changed,
                                            );

                                            // Show frequency labels
                                            let old_labels = ui_state.cfg_spectrum_show_labels;
                                            ui.checkbox(
                                                &mut ui_state.cfg_spectrum_show_labels,
                                                RichText::new("Show Frequency Labels")
                                                    .size(11.0)
                                                    .color(theme::TEXT_SECONDARY),
                                            );
                                            track(
                                                old_labels,
                                                &ui_state.cfg_spectrum_show_labels,
                                                &mut ui_state.settings_dirty,
                                                &mut settings_changed,
                                            );

                                            // Spectrum color palette
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new("Spectrum Colors:")
                                                        .size(11.0)
                                                        .color(theme::TEXT_SECONDARY),
                                                );

                                                let palette_before =
                                                    ui_state.cfg_spectrum_color_palette.clone();
                                                egui::ComboBox::from_id_salt(
                                                    "spectrum_color_palette",
                                                )
                                                .selected_text(&ui_state.cfg_spectrum_color_palette)
                                                .show_ui(ui, |ui| {
                                                    ui.selectable_value(
                                                        &mut ui_state.cfg_spectrum_color_palette,
                                                        "classic".to_string(),
                                                        "classic",
                                                    );
                                                    ui.selectable_value(
                                                        &mut ui_state.cfg_spectrum_color_palette,
                                                        "neon".to_string(),
                                                        "neon",
                                                    );
                                                    ui.selectable_value(
                                                        &mut ui_state.cfg_spectrum_color_palette,
                                                        "ocean".to_string(),
                                                        "ocean",
                                                    );
                                                    ui.selectable_value(
                                                        &mut ui_state.cfg_spectrum_color_palette,
                                                        "fire".to_string(),
                                                        "fire",
                                                    );
                                                    ui.selectable_value(
                                                        &mut ui_state.cfg_spectrum_color_palette,
                                                        "sunset".to_string(),
                                                        "sunset",
                                                    );
                                                    ui.selectable_value(
                                                        &mut ui_state.cfg_spectrum_color_palette,
                                                        "forest".to_string(),
                                                        "forest",
                                                    );
                                                    ui.selectable_value(
                                                        &mut ui_state.cfg_spectrum_color_palette,
                                                        "mono".to_string(),
                                                        "mono",
                                                    );
                                                });

                                                track(
                                                    palette_before,
                                                    &ui_state.cfg_spectrum_color_palette,
                                                    &mut ui_state.settings_dirty,
                                                    &mut settings_changed,
                                                );
                                            });

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

                                                track(
                                                    sink_before,
                                                    &ui_state.cfg_spectrum_sink_name,
                                                    &mut ui_state.settings_dirty,
                                                    &mut settings_changed,
                                                );
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
                                        track(
                                            old_lines,
                                            &ui_state.cfg_max_console_lines,
                                            &mut ui_state.settings_dirty,
                                            &mut settings_changed,
                                        );
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
                                .stroke(Stroke::new(1.0_f32, theme::BORDER))
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
                                    if track(
                                        old_logging,
                                        &ui_state.cfg_logging_enabled,
                                        &mut ui_state.settings_dirty,
                                        &mut settings_changed,
                                    ) && !ui_state.cfg_logging_enabled
                                    {
                                        // Keep console visibility consistent when logging is disabled.
                                        ui_state.cfg_show_console = false;
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
                                        track(
                                            level_before,
                                            &ui_state.cfg_log_level,
                                            &mut ui_state.settings_dirty,
                                            &mut settings_changed,
                                        );
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
                                    track(
                                        old_timestamps,
                                        &ui_state.cfg_timestamps,
                                        &mut ui_state.settings_dirty,
                                        &mut settings_changed,
                                    );

                                    ui.add_space(8.0);

                                    // Log fader events
                                    let old_fader_events = ui_state.cfg_log_fader_events;
                                    ui.checkbox(
                                        &mut ui_state.cfg_log_fader_events,
                                        RichText::new("Log Fader Events")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    track(
                                        old_fader_events,
                                        &ui_state.cfg_log_fader_events,
                                        &mut ui_state.settings_dirty,
                                        &mut settings_changed,
                                    );

                                    ui.add_space(8.0);

                                    // Log device info
                                    let old_device_info = ui_state.cfg_log_device_info;
                                    ui.checkbox(
                                        &mut ui_state.cfg_log_device_info,
                                        RichText::new("Log Device Info")
                                            .size(13.0)
                                            .color(theme::TEXT_PRIMARY),
                                    );
                                    track(
                                        old_device_info,
                                        &ui_state.cfg_log_device_info,
                                        &mut ui_state.settings_dirty,
                                        &mut settings_changed,
                                    );
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
                                    ui_state.save_button_clicked = true;
                                }

                                if ui_state.settings_dirty {
                                    ui.label(
                                        RichText::new("Unsaved changes")
                                            .size(12.0)
                                            .color(theme::ACCENT_ORANGE),
                                    );
                                }

                                // Show save message if present
                                if let Some((msg, instant)) = &ui_state.settings_save_message {
                                    if instant.elapsed().as_secs() < 3 {
                                        let is_success = msg.starts_with("SUCCESS:");
                                        ui.label(RichText::new(msg).size(12.0).color(
                                            if is_success {
                                                theme::ACCENT_GREEN
                                            } else {
                                                theme::ACCENT_RED
                                            },
                                        ));
                                    }
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
