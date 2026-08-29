use super::theme;
use crate::spectrum::{frequency_to_note, get_band_frequency, SpectrumData, NUM_BANDS};
use egui::*;

/// Maximum waterfall history (rows)
pub const WATERFALL_HISTORY: usize = 128;

/// Smooth display state for the visualizer (stored in UI to persist between frames)
#[derive(Clone)]
pub struct VisualizerState {
    pub display_bands: [f32; NUM_BANDS],
    pub display_peaks: [f32; NUM_BANDS],
    pub display_bands_right: [f32; NUM_BANDS],
    pub display_peaks_right: [f32; NUM_BANDS],
    /// Waterfall history (circular buffer): each row contains band magnitudes
    pub waterfall_history: Vec<[f32; NUM_BANDS]>,
    pub waterfall_pos: usize,
}

impl Default for VisualizerState {
    fn default() -> Self {
        Self {
            display_bands: [0.0; NUM_BANDS],
            display_peaks: [0.0; NUM_BANDS],
            display_bands_right: [0.0; NUM_BANDS],
            display_peaks_right: [0.0; NUM_BANDS],
            waterfall_history: vec![[0.0; NUM_BANDS]; WATERFALL_HISTORY],
            waterfall_pos: 0,
        }
    }
}

impl VisualizerState {
    /// Smoothly interpolate towards target values
    pub fn update(
        &mut self,
        target: &SpectrumData,
        dt: f32,
        attack_speed: f32,
        release_speed: f32,
    ) {
        // Exponential mapping makes control changes more perceptible across the range.
        let attack = (1.0 - (-attack_speed * dt).exp()).clamp(0.0, 1.0);
        let release = (1.0 - (-release_speed * dt).exp()).clamp(0.0, 1.0);
        let peak_release = (1.0 - (-(release_speed * 0.35) * dt).exp()).clamp(0.0, 1.0);

        for i in 0..NUM_BANDS {
            self.display_bands[i] = if target.bands[i] > self.display_bands[i] {
                lerp(self.display_bands[i], target.bands[i], attack)
            } else {
                lerp(self.display_bands[i], target.bands[i], release)
            };
            self.display_bands_right[i] = if target.bands_right[i] > self.display_bands_right[i] {
                lerp(self.display_bands_right[i], target.bands_right[i], attack)
            } else {
                lerp(self.display_bands_right[i], target.bands_right[i], release)
            };

            // Peaks: instant attack, slow decay
            if target.peaks[i] > self.display_peaks[i] {
                self.display_peaks[i] = target.peaks[i];
            } else {
                self.display_peaks[i] = lerp(self.display_peaks[i], target.peaks[i], peak_release);
            }

            if target.peaks_right[i] > self.display_peaks_right[i] {
                self.display_peaks_right[i] = target.peaks_right[i];
            } else {
                self.display_peaks_right[i] = lerp(
                    self.display_peaks_right[i],
                    target.peaks_right[i],
                    peak_release,
                );
            }
        }

        // Update waterfall history with combined stereo data (average both channels)
        let mut combined = [0.0f32; NUM_BANDS];
        for i in 0..NUM_BANDS {
            combined[i] = (target.bands[i] + target.bands_right[i]) * 0.5;
        }

        self.waterfall_history[self.waterfall_pos] = combined;
        self.waterfall_pos = (self.waterfall_pos + 1) % WATERFALL_HISTORY;
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Render the frequency spectrum visualizer
pub fn render_spectrum_visualizer(
    ui: &mut Ui,
    show_spectrum: &mut bool,
    attack_speed: &mut f32,
    release_speed: &mut f32,
    color_palette: &str,
    spectrum: &SpectrumData,
    state: &mut VisualizerState,
    enabled: bool,
    stereo_mode: bool,
    show_waterfall: bool,
    show_labels: bool,
) {
    let spectrum_enabled = enabled && *show_spectrum;

    // Update with smoothing
    let dt = ui.ctx().input(|i| i.predicted_dt);
    if spectrum_enabled {
        state.update(spectrum, dt, *attack_speed, *release_speed);
    } else {
        // Fade out when disabled
        for i in 0..NUM_BANDS {
            state.display_bands[i] = lerp(state.display_bands[i], 0.0, 8.0 * dt);
            state.display_peaks[i] = lerp(state.display_peaks[i], 0.0, 8.0 * dt);
            state.display_bands_right[i] = lerp(state.display_bands_right[i], 0.0, 8.0 * dt);
            state.display_peaks_right[i] = lerp(state.display_peaks_right[i], 0.0, 8.0 * dt);
        }
    }

    Frame::default()
        .fill(theme::BG_SECONDARY)
        .stroke(Stroke::new(1.0_f32, theme::BORDER))
        .inner_margin(Margin::same(12))
        .corner_radius(CornerRadius::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            // Header
            ui.horizontal(|ui| {
                ui.checkbox(show_spectrum, "");
                ui.label(
                    RichText::new("📊 Spectrum Analyzer")
                        .strong()
                        .size(14.0)
                        .color(theme::ACCENT_CYAN),
                );

                if *show_spectrum {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.scope(|ui| {
                            // Keep these sliders visually distinct from panel background.
                            ui.style_mut().visuals.widgets.inactive.bg_fill =
                                Color32::from_rgb(60, 60, 70);
                            ui.style_mut().visuals.widgets.hovered.bg_fill =
                                Color32::from_rgb(80, 80, 92);
                            ui.style_mut().visuals.widgets.active.bg_fill = Color32::WHITE;
                            ui.style_mut().visuals.selection.bg_fill = Color32::WHITE;
                            ui.style_mut().visuals.widgets.active.bg_stroke =
                                Stroke::new(1.5_f32, Color32::WHITE);
                            ui.style_mut().visuals.widgets.hovered.bg_stroke =
                                Stroke::new(1.0_f32, Color32::from_rgb(190, 190, 200));

                            ui.add_sized(
                                [130.0, 0.0],
                                Slider::new(release_speed, 1.0..=36.0).text("Release"),
                            );
                            ui.add_space(4.0);
                            ui.add_sized(
                                [130.0, 0.0],
                                Slider::new(attack_speed, 4.0..=64.0).text("Attack"),
                            );
                        });
                    });
                }
            });

            if spectrum_enabled {
                ui.add_space(8.0);

                // Main visualizer
                let height = if show_waterfall { 150.0 } else { 120.0 };
                let width = ui.available_width();

                let (rect, _response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());

                if show_waterfall {
                    render_spectrum_with_waterfall(
                        ui.painter(),
                        rect,
                        state,
                        enabled,
                        stereo_mode,
                        show_labels,
                        color_palette,
                    );
                } else {
                    render_spectrum_bars(
                        ui.painter(),
                        rect,
                        state,
                        enabled,
                        stereo_mode,
                        show_labels,
                        color_palette,
                    );
                }
            } else {
                ui.add_space(4.0);
            }
        });
}

fn render_spectrum_with_waterfall(
    painter: &Painter,
    rect: Rect,
    state: &VisualizerState,
    enabled: bool,
    stereo_mode: bool,
    show_labels: bool,
    color_palette: &str,
) {
    let spectrum_height = rect.height() * 0.75;

    let spectrum_rect =
        Rect::from_min_max(rect.min, pos2(rect.max.x, rect.min.y + spectrum_height));

    let waterfall_rect =
        Rect::from_min_max(pos2(rect.min.x, rect.min.y + spectrum_height), rect.max);

    // Draw spectrum on top
    render_spectrum_bars(
        painter,
        spectrum_rect,
        state,
        enabled,
        stereo_mode,
        show_labels,
        color_palette,
    );

    // Draw waterfall below
    render_waterfall(painter, waterfall_rect, state, color_palette);
}

fn render_waterfall(painter: &Painter, rect: Rect, state: &VisualizerState, color_palette: &str) {
    painter.rect_filled(rect, 2.0, theme::BG_TERTIARY);

    let bar_width = (rect.width() - 4.0) / NUM_BANDS as f32;
    let pixel_height = rect.height() / WATERFALL_HISTORY as f32;

    for row in 0..WATERFALL_HISTORY {
        // Get history index (oldest to newest from bottom to top)
        let history_idx = (state.waterfall_pos + row) % WATERFALL_HISTORY;
        let y = rect.max.y - pixel_height * (row + 1) as f32;

        for band in 0..NUM_BANDS {
            let value = state.waterfall_history[history_idx][band];
            let color = get_bar_color(band, value, color_palette);

            let x = rect.min.x + 2.0 + band as f32 * bar_width;
            let pixel_rect =
                Rect::from_min_max(pos2(x, y), pos2(x + bar_width - 2.0, y + pixel_height));

            painter.rect_filled(pixel_rect, 0.0, color);
        }
    }
}

fn render_spectrum_bars(
    painter: &Painter,
    rect: Rect,
    state: &VisualizerState,
    enabled: bool,
    stereo_mode: bool,
    show_labels: bool,
    color_palette: &str,
) {
    // Background
    painter.rect_filled(rect, 4.0, theme::BG_TERTIARY);

    // Reserve space at bottom for labels if enabled
    let label_height = if show_labels { 14.0 } else { 0.0 };
    let bars_bottom = rect.max.y - label_height;
    let available_height = (bars_bottom - rect.min.y) - 4.0;

    // Grid lines
    for i in 1..4 {
        let y = rect.min.y + (available_height * i as f32 / 4.0);
        painter.line_segment(
            [pos2(rect.min.x, y), pos2(rect.max.x, y)],
            Stroke::new(0.5_f32, Color32::from_rgba_unmultiplied(80, 80, 90, 40)),
        );
    }

    let bar_width = (rect.width() - 4.0) / NUM_BANDS as f32;
    let gap = 2.0;

    if stereo_mode {
        // Split bar display: left half for left channel, right half for right channel
        let effective_bar_width = (bar_width - gap) * 0.5;

        for i in 0..NUM_BANDS {
            let x_base = rect.min.x + 2.0 + i as f32 * bar_width;

            // Left channel (left half of bar)
            let x_left = x_base;
            let band_value_left = state.display_bands[i];
            let peak_value_left = state.display_peaks[i];
            let bar_height_left = band_value_left * available_height;
            let peak_y_left = bars_bottom - 2.0 - peak_value_left * available_height;

            let color_left = get_bar_color(i, band_value_left, color_palette);

            if bar_height_left > 0.5 {
                let bar_rect = Rect::from_min_max(
                    pos2(x_left, bars_bottom - 2.0 - bar_height_left),
                    pos2(x_left + effective_bar_width, bars_bottom - 2.0),
                );

                // Draw shadow
                draw_bar_shadow(painter, bar_rect, 3.0);

                // Draw glow effect (subtle background glow)
                draw_bar_glow(painter, bar_rect, color_left, band_value_left);

                // Draw main bar
                painter.rect_filled(bar_rect, 2.0, color_left);
            }

            if enabled && peak_value_left > 0.01 {
                painter.line_segment(
                    [
                        pos2(x_left, peak_y_left),
                        pos2(x_left + effective_bar_width, peak_y_left),
                    ],
                    Stroke::new(2.0_f32, Color32::WHITE),
                );
            }

            // Right channel (right half of bar)
            let x_right = x_base + effective_bar_width;
            let band_value_right = state.display_bands_right[i];
            let peak_value_right = state.display_peaks_right[i];
            let bar_height_right = band_value_right * available_height;
            let peak_y_right = bars_bottom - 2.0 - peak_value_right * available_height;

            let color_right = get_bar_color(i, band_value_right, color_palette);

            if bar_height_right > 0.5 {
                let bar_rect = Rect::from_min_max(
                    pos2(x_right, bars_bottom - 2.0 - bar_height_right),
                    pos2(x_right + effective_bar_width, bars_bottom - 2.0),
                );

                // Draw shadow
                draw_bar_shadow(painter, bar_rect, 3.0);

                // Draw glow effect
                draw_bar_glow(painter, bar_rect, color_right, band_value_right);

                // Draw main bar
                painter.rect_filled(bar_rect, 2.0, color_right);
            }

            if enabled && peak_value_right > 0.01 {
                painter.line_segment(
                    [
                        pos2(x_right, peak_y_right),
                        pos2(x_right + effective_bar_width, peak_y_right),
                    ],
                    Stroke::new(2.0_f32, Color32::WHITE),
                );
            }
        }
    } else {
        // Mono/combined mode
        let effective_bar_width = bar_width - gap;

        for i in 0..NUM_BANDS {
            let x = rect.min.x + 2.0 + i as f32 * bar_width;

            let band_value = state.display_bands[i];
            let peak_value = state.display_peaks[i];

            let bar_height = band_value * available_height;
            let peak_y = bars_bottom - 2.0 - peak_value * available_height;

            let color = get_bar_color(i, band_value, color_palette);

            // Draw bar
            if bar_height > 0.5 {
                let bar_rect = Rect::from_min_max(
                    pos2(x, bars_bottom - 2.0 - bar_height),
                    pos2(x + effective_bar_width, bars_bottom - 2.0),
                );

                // Draw shadow
                draw_bar_shadow(painter, bar_rect, 3.0);

                // Draw glow effect
                draw_bar_glow(painter, bar_rect, color, band_value);

                // Draw main bar
                painter.rect_filled(bar_rect, 2.0, color);
            }

            // Draw peak indicator
            if enabled && peak_value > 0.01 {
                painter.line_segment(
                    [pos2(x, peak_y), pos2(x + effective_bar_width, peak_y)],
                    Stroke::new(2.0_f32, Color32::WHITE),
                );
            }
        }
    }

    // Frequency labels with note names
    if show_labels {
        render_frequency_labels(painter, rect);
    }
}

/// Draw a subtle shadow effect below the bar
fn draw_bar_shadow(painter: &Painter, rect: Rect, shadow_offset: f32) {
    let shadow_rect = Rect::from_min_max(
        pos2(rect.min.x, rect.max.y),
        pos2(rect.max.x, rect.max.y + shadow_offset),
    );

    // Semi-transparent dark shadow
    painter.rect_filled(
        shadow_rect,
        1.0,
        Color32::from_rgba_unmultiplied(0, 0, 0, 30),
    );
}

/// Draw a glowing aura around the bar based on value
fn draw_bar_glow(painter: &Painter, rect: Rect, color: Color32, value: f32) {
    // Create a larger rect for the glow
    let glow_expand = 2.0 + value * 3.0; // Expand based on bar value
    let glow_rect = Rect::from_min_max(
        pos2(rect.min.x - glow_expand, rect.min.y - glow_expand),
        pos2(rect.max.x + glow_expand, rect.max.y + glow_expand),
    );

    // Semi-transparent glow in the bar's color
    let glow_color =
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (30.0 * value) as u8);

    painter.rect_filled(glow_rect, 3.0, glow_color);
}

fn render_frequency_labels(painter: &Painter, rect: Rect) {
    let label_positions = [0, 4, 8, 12, 16, 20, 24, 28, 31]; // Band indices

    // Position labels in the bottom 14px area of the rect
    let label_y = rect.max.y - 2.0;

    for &band_idx in &label_positions {
        if band_idx >= NUM_BANDS {
            continue;
        }

        let freq = get_band_frequency(band_idx);
        let note = frequency_to_note(freq);

        let bar_width = (rect.width() - 4.0) / NUM_BANDS as f32;
        let x = rect.min.x + 2.0 + band_idx as f32 * bar_width + bar_width * 0.5;

        painter.text(
            pos2(x, label_y),
            Align2::CENTER_BOTTOM,
            note,
            FontId::proportional(8.0),
            theme::TEXT_MUTED,
        );
    }
}

/// Get color for a bar based on its band index and value
fn get_bar_color(band_index: usize, value: f32, color_palette: &str) -> Color32 {
    match color_palette {
        "classic" => get_classic_bar_color(band_index, value),
        "ocean" => get_ocean_bar_color(band_index, value),
        "fire" => get_fire_bar_color(band_index, value),
        "sunset" => get_sunset_bar_color(band_index, value),
        "forest" => get_forest_bar_color(band_index, value),
        "mono" => get_mono_bar_color(value),
        // Default to neon for unknown values.
        _ => get_neon_bar_color(band_index, value),
    }
}

fn get_neon_bar_color(band_index: usize, value: f32) -> Color32 {
    // Distinct neon gradient: violet -> magenta -> amber -> lime
    let t = band_index as f32 / NUM_BANDS as f32;

    // Intensity based on value
    let intensity = 0.42 + 0.58 * value;

    let (r, g, b) = if t < 0.34 {
        // Low frequencies: Violet -> Magenta
        let t2 = t / 0.34;
        (
            (125.0 * (1.0 - t2) + 235.0 * t2) * intensity,
            (80.0 * (1.0 - t2) + 85.0 * t2) * intensity,
            (235.0 * (1.0 - t2) + 185.0 * t2) * intensity,
        )
    } else if t < 0.68 {
        // Mid frequencies: Magenta -> Amber
        let t2 = (t - 0.34) / 0.34;
        (
            (235.0 * (1.0 - t2) + 255.0 * t2) * intensity,
            (85.0 * (1.0 - t2) + 175.0 * t2) * intensity,
            (185.0 * (1.0 - t2) + 70.0 * t2) * intensity,
        )
    } else {
        // High frequencies: Amber -> Lime
        let t2 = (t - 0.68) / 0.32;
        (
            (255.0 * (1.0 - t2) + 175.0 * t2) * intensity,
            (175.0 * (1.0 - t2) + 235.0 * t2) * intensity,
            (70.0 * (1.0 - t2) + 90.0 * t2) * intensity,
        )
    };

    Color32::from_rgb(r as u8, g as u8, b as u8)
}

fn get_ocean_bar_color(band_index: usize, value: f32) -> Color32 {
    let t = band_index as f32 / NUM_BANDS as f32;
    let intensity = 0.45 + 0.55 * value;

    let (r, g, b) = if t < 0.5 {
        let t2 = t / 0.5;
        (
            (30.0 * (1.0 - t2) + 40.0 * t2) * intensity,
            (90.0 * (1.0 - t2) + 175.0 * t2) * intensity,
            (165.0 * (1.0 - t2) + 225.0 * t2) * intensity,
        )
    } else {
        let t2 = (t - 0.5) / 0.5;
        (
            (40.0 * (1.0 - t2) + 120.0 * t2) * intensity,
            (175.0 * (1.0 - t2) + 210.0 * t2) * intensity,
            (225.0 * (1.0 - t2) + 200.0 * t2) * intensity,
        )
    };

    Color32::from_rgb(r as u8, g as u8, b as u8)
}

fn get_fire_bar_color(band_index: usize, value: f32) -> Color32 {
    let t = band_index as f32 / NUM_BANDS as f32;
    let intensity = 0.5 + 0.5 * value;

    let (r, g, b) = if t < 0.4 {
        let t2 = t / 0.4;
        (
            (210.0 * (1.0 - t2) + 240.0 * t2) * intensity,
            (40.0 * (1.0 - t2) + 95.0 * t2) * intensity,
            (25.0 * (1.0 - t2) + 30.0 * t2) * intensity,
        )
    } else if t < 0.75 {
        let t2 = (t - 0.4) / 0.35;
        (
            (240.0 * (1.0 - t2) + 255.0 * t2) * intensity,
            (95.0 * (1.0 - t2) + 165.0 * t2) * intensity,
            (30.0 * (1.0 - t2) + 45.0 * t2) * intensity,
        )
    } else {
        let t2 = (t - 0.75) / 0.25;
        (
            (255.0 * (1.0 - t2) + 255.0 * t2) * intensity,
            (165.0 * (1.0 - t2) + 220.0 * t2) * intensity,
            (45.0 * (1.0 - t2) + 120.0 * t2) * intensity,
        )
    };

    Color32::from_rgb(r as u8, g as u8, b as u8)
}

fn get_sunset_bar_color(band_index: usize, value: f32) -> Color32 {
    let t = band_index as f32 / NUM_BANDS as f32;
    let intensity = 0.45 + 0.55 * value;

    let (r, g, b) = if t < 0.5 {
        let t2 = t / 0.5;
        (
            (105.0 * (1.0 - t2) + 215.0 * t2) * intensity,
            (65.0 * (1.0 - t2) + 95.0 * t2) * intensity,
            (180.0 * (1.0 - t2) + 140.0 * t2) * intensity,
        )
    } else {
        let t2 = (t - 0.5) / 0.5;
        (
            (215.0 * (1.0 - t2) + 255.0 * t2) * intensity,
            (95.0 * (1.0 - t2) + 190.0 * t2) * intensity,
            (140.0 * (1.0 - t2) + 95.0 * t2) * intensity,
        )
    };

    Color32::from_rgb(r as u8, g as u8, b as u8)
}

fn get_forest_bar_color(band_index: usize, value: f32) -> Color32 {
    let t = band_index as f32 / NUM_BANDS as f32;
    let intensity = 0.45 + 0.55 * value;

    let (r, g, b) = if t < 0.5 {
        let t2 = t / 0.5;
        (
            (35.0 * (1.0 - t2) + 55.0 * t2) * intensity,
            (120.0 * (1.0 - t2) + 180.0 * t2) * intensity,
            (70.0 * (1.0 - t2) + 95.0 * t2) * intensity,
        )
    } else {
        let t2 = (t - 0.5) / 0.5;
        (
            (55.0 * (1.0 - t2) + 185.0 * t2) * intensity,
            (180.0 * (1.0 - t2) + 230.0 * t2) * intensity,
            (95.0 * (1.0 - t2) + 120.0 * t2) * intensity,
        )
    };

    Color32::from_rgb(r as u8, g as u8, b as u8)
}

fn get_mono_bar_color(value: f32) -> Color32 {
    let v = (90.0 + 165.0 * value).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(v, v, v)
}

fn get_classic_bar_color(band_index: usize, value: f32) -> Color32 {
    let t = band_index as f32 / NUM_BANDS as f32;
    let intensity = 0.5 + 0.5 * value;

    let (r, g, b) = if t < 0.33 {
        let t2 = t / 0.33;
        (
            (60.0 * (1.0 - t2) + 80.0 * t2) * intensity,
            (120.0 * (1.0 - t2) + 200.0 * t2) * intensity,
            (220.0 * (1.0 - t2) + 220.0 * t2) * intensity,
        )
    } else if t < 0.66 {
        let t2 = (t - 0.33) / 0.33;
        (
            (80.0 * (1.0 - t2) + 100.0 * t2) * intensity,
            (200.0 * (1.0 - t2) + 220.0 * t2) * intensity,
            (220.0 * (1.0 - t2) + 150.0 * t2) * intensity,
        )
    } else {
        let t2 = (t - 0.66) / 0.34;
        (
            (100.0 * (1.0 - t2) + 220.0 * t2) * intensity,
            (220.0 * (1.0 - t2) + 180.0 * t2) * intensity,
            (150.0 * (1.0 - t2) + 80.0 * t2) * intensity,
        )
    };

    Color32::from_rgb(r as u8, g as u8, b as u8)
}
