use egui::{CentralPanel, Context, Frame, RichText, ScrollArea, Stroke, CornerRadius, Margin, Color32};
use crate::ui::UiState;
use super::theme;

pub fn render_settings_tab(ui_state: &mut UiState, ctx: &Context, tray_functional: bool) -> bool {
    let mut settings_changed = false;
    
    CentralPanel::default()
        .frame(Frame::default().fill(theme::BG_PRIMARY))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    Frame::default()
                        .inner_margin(Margin { left: 20, right: 20, top: 8, bottom: 8 })
                        .fill(theme::BG_PRIMARY)
                        .show(ui, |ui| {
                    // Show save message if present
                    if let Some((msg, instant)) = &ui_state.settings_save_message {
                        if instant.elapsed().as_secs() < 3 {
                            let is_success = msg.starts_with("SUCCESS:");
                            ui.label(
                                RichText::new(msg)
                                    .size(14.0)
                                    .color(if is_success { theme::ACCENT_GREEN } else { theme::ACCENT_RED })
                            );
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
                        .inner_margin(Margin { left: 20, right: 20, top: 8, bottom: 8 })
                        .corner_radius(CornerRadius::same(4))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new("Audio Sinks (CC -> Sink Name)").size(14.0).color(theme::TEXT_PRIMARY));
                            ui.add_space(8.0);
                                    
                                    let mut to_remove_sink: Option<usize> = None;
                                    for (idx, (cc, name)) in ui_state.cfg_sinks.iter_mut().enumerate() {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("CC {}:", cc)).size(12.0).color(theme::TEXT_SECONDARY));
                                            let old_name = name.clone();
                                            ui.add(egui::TextEdit::singleline(name).desired_width(200.0));
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
                                        ui.label(RichText::new("Add:").size(12.0).color(theme::TEXT_MUTED));
                                        ui.add(egui::TextEdit::singleline(&mut ui_state.new_sink_cc).desired_width(40.0).hint_text("CC"));
                                        ui.add(egui::TextEdit::singleline(&mut ui_state.new_sink_name).desired_width(150.0).hint_text("Sink name"));
                                        if ui.button("➕ Add").clicked() {
                                            if let Ok(cc) = ui_state.new_sink_cc.parse::<u8>() {
                                                if !ui_state.new_sink_name.is_empty() {
                                                    ui_state.cfg_sinks.push((cc, ui_state.new_sink_name.clone()));
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
                                .inner_margin(Margin { left: 20, right: 20, top: 8, bottom: 8 })
                                .corner_radius(CornerRadius::same(4))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(RichText::new("Applications (CC -> App Name)").size(14.0).color(theme::TEXT_PRIMARY));
                                    ui.add_space(8.0);
                                    
                                    let mut to_remove_app: Option<usize> = None;
                                    for (idx, (cc, name)) in ui_state.cfg_applications.iter_mut().enumerate() {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("CC {}:", cc)).size(12.0).color(theme::TEXT_SECONDARY));
                                            let old_name = name.clone();
                                            ui.add(egui::TextEdit::singleline(name).desired_width(200.0));
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
                                        ui.label(RichText::new("Add:").size(12.0).color(theme::TEXT_MUTED));
                                        ui.add(egui::TextEdit::singleline(&mut ui_state.new_app_cc).desired_width(40.0).hint_text("CC"));
                                        ui.add(egui::TextEdit::singleline(&mut ui_state.new_app_name).desired_width(150.0).hint_text("App name"));
                                        if ui.button("➕ Add").clicked() {
                                            if let Ok(cc) = ui_state.new_app_cc.parse::<u8>() {
                                                if !ui_state.new_app_name.is_empty() {
                                                    ui_state.cfg_applications.push((cc, ui_state.new_app_name.clone()));
                                                    ui_state.cfg_applications.sort_by_key(|(cc, _)| *cc);
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
                                .inner_margin(Margin { left: 20, right: 20, top: 8, bottom: 8 })
                                .corner_radius(CornerRadius::same(4))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(RichText::new("Mute Buttons (Button CC -> Fader CC)").size(14.0).color(theme::TEXT_PRIMARY));
                                    ui.add_space(8.0);
                                    
                                    let mut to_remove_mute: Option<usize> = None;
                                    for (idx, (button_cc, fader_cc)) in ui_state.cfg_mute_buttons.iter().enumerate() {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("CC {} -> CC {}", button_cc, fader_cc)).size(12.0).color(theme::TEXT_SECONDARY));
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
                                        ui.label(RichText::new("Add:").size(12.0).color(theme::TEXT_MUTED));
                                        ui.add(egui::TextEdit::singleline(&mut ui_state.new_mute_button_cc).desired_width(50.0).hint_text("Btn CC"));
                                        ui.label(RichText::new("->").color(theme::TEXT_MUTED));
                                        ui.add(egui::TextEdit::singleline(&mut ui_state.new_mute_fader_cc).desired_width(50.0).hint_text("Fader CC"));
                                        if ui.button("➕ Add").clicked() {
                                            if let (Ok(btn_cc), Ok(fader_cc)) = (ui_state.new_mute_button_cc.parse::<u8>(), ui_state.new_mute_fader_cc.parse::<u8>()) {
                                                ui_state.cfg_mute_buttons.push((btn_cc, fader_cc));
                                                ui_state.cfg_mute_buttons.sort_by_key(|(cc, _)| *cc);
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
                            
                            // ===== AUDIO SECTION =====
                            ui.add_space(8.0);
                            render_section_header(ui, "Audio Settings", theme::ACCENT_ORANGE);
                            ui.add_space(8.0);
                            
                            Frame::default()
                                .fill(theme::BG_SECONDARY)
                                .stroke(Stroke::new(1.0, theme::BORDER))
                                .inner_margin(Margin { left: 20, right: 20, top: 8, bottom: 8 })
                                .corner_radius(CornerRadius::same(4))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    // Use PipeWire
                                    let old_use_pipewire = ui_state.cfg_use_pipewire;
                                    ui.checkbox(&mut ui_state.cfg_use_pipewire, 
                                        RichText::new("Use PipeWire").size(13.0).color(theme::TEXT_PRIMARY));
                                    if old_use_pipewire != ui_state.cfg_use_pipewire {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }
                                    
                                    ui.add_space(8.0);
                                    
                                    // Default Sink
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Default Sink:").size(12.0).color(theme::TEXT_SECONDARY));
                                        let old_sink = ui_state.cfg_default_sink.clone();
                                        ui.add(egui::TextEdit::singleline(&mut ui_state.cfg_default_sink).desired_width(250.0));
                                        if old_sink != ui_state.cfg_default_sink {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });
                                    
                                    ui.add_space(8.0);
                                    
                                    // Volume Control Mode
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Volume Control Mode:").size(12.0).color(theme::TEXT_SECONDARY));
                                        let old_mode = ui_state.cfg_volume_control_mode.clone();
                                        egui::ComboBox::from_id_salt("volume_mode")
                                            .selected_text(&ui_state.cfg_volume_control_mode)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut ui_state.cfg_volume_control_mode, "pipewire-api".to_string(), "pipewire-api");
                                                ui.selectable_value(&mut ui_state.cfg_volume_control_mode, "pw-volume".to_string(), "pw-volume");
                                            });
                                        if old_mode != ui_state.cfg_volume_control_mode {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });
                                    
                                    ui.add_space(8.0);
                                    
                                    // Volume Curve
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Volume Curve:").size(12.0).color(theme::TEXT_SECONDARY));
                                        let old_curve = ui_state.cfg_volume_curve.clone();
                                        egui::ComboBox::from_id_salt("volume_curve")
                                            .selected_text(&ui_state.cfg_volume_curve)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut ui_state.cfg_volume_curve, "linear".to_string(), "linear");
                                                ui.selectable_value(&mut ui_state.cfg_volume_curve, "exponential".to_string(), "exponential");
                                            });
                                        if old_curve != ui_state.cfg_volume_curve {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });
                                    
                                    ui.add_space(8.0);
                                    
                                    // Debounce MS
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Debounce (ms):").size(12.0).color(theme::TEXT_SECONDARY));
                                        let old_debounce = ui_state.cfg_debounce_ms;
                                        ui.add(egui::DragValue::new(&mut ui_state.cfg_debounce_ms).range(0..=1000));
                                        if old_debounce != ui_state.cfg_debounce_ms {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });
                                    
                                    ui.add_space(8.0);
                                    
                                    // App search interval
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("App Search Interval (s):").size(12.0).color(theme::TEXT_SECONDARY));
                                        let old_search = ui_state.cfg_applications_sink_search;
                                        let mut search_val = ui_state.cfg_applications_sink_search as i64;
                                        ui.add(egui::DragValue::new(&mut search_val).range(1..=120));
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
                                .inner_margin(Margin { left: 20, right: 20, top: 8, bottom: 8 })
                                .corner_radius(CornerRadius::same(4))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Window Width:").size(12.0).color(theme::TEXT_SECONDARY));
                                        let old_width = ui_state.cfg_window_width;
                                        ui.add(egui::DragValue::new(&mut ui_state.cfg_window_width).range(400..=3000));
                                        if old_width != ui_state.cfg_window_width {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                        
                                        ui.add_space(16.0);
                                        
                                        ui.label(RichText::new("Height:").size(12.0).color(theme::TEXT_SECONDARY));
                                        let old_height = ui_state.cfg_window_height;
                                        ui.add(egui::DragValue::new(&mut ui_state.cfg_window_height).range(300..=2000));
                                        if old_height != ui_state.cfg_window_height {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });
                                    
                                    ui.add_space(8.0);
                                    
                                    // Theme
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Theme:").size(12.0).color(theme::TEXT_SECONDARY));
                                        let old_theme = ui_state.cfg_theme.clone();
                                        egui::ComboBox::from_id_salt("theme")
                                            .selected_text(&ui_state.cfg_theme)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut ui_state.cfg_theme, "default".to_string(), "default");
                                                ui.selectable_value(&mut ui_state.cfg_theme, "dark".to_string(), "dark");
                                                ui.selectable_value(&mut ui_state.cfg_theme, "light".to_string(), "light");
                                            });
                                        if old_theme != ui_state.cfg_theme {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });
                                    
                                    ui.add_space(8.0);
                                    
                                    // Show console
                                    let old_show_console = ui_state.cfg_show_console;
                                    ui.checkbox(&mut ui_state.cfg_show_console, 
                                        RichText::new("Show Console by Default").size(13.0).color(theme::TEXT_PRIMARY));
                                    if old_show_console != ui_state.cfg_show_console {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }
                                    
                                    ui.add_space(8.0);
                                    
                                    // Max console lines
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Max Console Lines:").size(12.0).color(theme::TEXT_SECONDARY));
                                        let old_lines = ui_state.cfg_max_console_lines;
                                        let mut lines_val = ui_state.cfg_max_console_lines as i32;
                                        ui.add(egui::DragValue::new(&mut lines_val).range(10..=10000));
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
                                .inner_margin(Margin { left: 20, right: 20, top: 8, bottom: 8 })
                                .corner_radius(CornerRadius::same(4))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    let old_logging = ui_state.cfg_logging_enabled;
                                    ui.checkbox(&mut ui_state.cfg_logging_enabled, 
                                        RichText::new("Enable Logging").size(13.0).color(theme::TEXT_PRIMARY));
                                    if old_logging != ui_state.cfg_logging_enabled {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }
                                    
                                    ui.add_space(8.0);
                                    
                                    // Log level
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Log Level:").size(12.0).color(theme::TEXT_SECONDARY));
                                        let old_level = ui_state.cfg_log_level.clone();
                                        egui::ComboBox::from_id_salt("log_level")
                                            .selected_text(&ui_state.cfg_log_level)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut ui_state.cfg_log_level, "off".to_string(), "off");
                                                ui.selectable_value(&mut ui_state.cfg_log_level, "error".to_string(), "error");
                                                ui.selectable_value(&mut ui_state.cfg_log_level, "warn".to_string(), "warn");
                                                ui.selectable_value(&mut ui_state.cfg_log_level, "info".to_string(), "info");
                                                ui.selectable_value(&mut ui_state.cfg_log_level, "debug".to_string(), "debug");
                                                ui.selectable_value(&mut ui_state.cfg_log_level, "trace".to_string(), "trace");
                                            });
                                        if old_level != ui_state.cfg_log_level {
                                            ui_state.settings_dirty = true;
                                            settings_changed = true;
                                        }
                                    });
                                    
                                    ui.add_space(8.0);
                                    
                                    // Timestamps
                                    let old_timestamps = ui_state.cfg_timestamps;
                                    ui.checkbox(&mut ui_state.cfg_timestamps, 
                                        RichText::new("Show Timestamps").size(13.0).color(theme::TEXT_PRIMARY));
                                    if old_timestamps != ui_state.cfg_timestamps {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }
                                    
                                    ui.add_space(8.0);
                                    
                                    // Log fader events
                                    let old_fader_events = ui_state.cfg_log_fader_events;
                                    ui.checkbox(&mut ui_state.cfg_log_fader_events, 
                                        RichText::new("Log Fader Events").size(13.0).color(theme::TEXT_PRIMARY));
                                    if old_fader_events != ui_state.cfg_log_fader_events {
                                        ui_state.settings_dirty = true;
                                        settings_changed = true;
                                    }
                                    
                                    ui.add_space(8.0);
                                    
                                    // Log device info
                                    let old_device_info = ui_state.cfg_log_device_info;
                                    ui.checkbox(&mut ui_state.cfg_log_device_info, 
                                        RichText::new("Log Device Info").size(13.0).color(theme::TEXT_PRIMARY));
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
                                if ui.add(egui::Button::new(
                                    RichText::new("Save Settings")
                                        .size(14.0)
                                        .color(Color32::WHITE)
                                ).fill(theme::ACCENT_BLUE)).clicked() {
                                    settings_changed = true;
                                    ui_state.settings_dirty = true;
                                }
                                
                                ui.add_space(16.0);
                                
                                if ui_state.settings_dirty {
                                    ui.label(
                                        RichText::new("Unsaved changes")
                                            .size(12.0)
                                            .color(theme::ACCENT_ORANGE)
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
                                    .color(theme::TEXT_SECONDARY)
                            );
                            
                            ui.label(
                                RichText::new("MIDI-controlled audio volume management")
                                    .size(12.0)
                                    .color(theme::TEXT_MUTED)
                            );
                            
                            ui.add_space(8.0);
                            
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Config file:").size(12.0).color(theme::TEXT_MUTED));
                                ui.label(RichText::new(&ui_state.config_path).size(12.0).color(theme::ACCENT_BLUE));
                            });
                            
                            ui.add_space(8.0);
                            
                            ui.label(
                                RichText::new("⚠ Note: Some settings require app restart to take effect")
                                    .size(11.0)
                                    .color(theme::ACCENT_ORANGE)
                            );
                            
                            ui.add_space(8.0);
                        });
                });
        });
    
    settings_changed
}

fn render_section_header(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(
        RichText::new(format!("[{}]", text.to_uppercase()))
            .size(15.0)
            .color(color)
            .strong()
    );
}
