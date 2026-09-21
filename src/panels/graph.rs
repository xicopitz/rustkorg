//! Read-only routing view: programs on the left, the software sinks they're playing
//! through in the middle, and the hardware device(s) those sinks actually reach on the
//! right — connected by animated lines showing audio flowing left to right. A much
//! simplified stand-in for something like qpwgraph — no drag-to-reconnect.

use super::theme;
use crate::ui::UiState;
use egui::{
    pos2, vec2, Align2, CentralPanel, Color32, Context, FontId, Frame, Pos2, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind,
};
use std::collections::{HashMap, HashSet};

const NODE_WIDTH: f32 = 210.0;
const NODE_HEIGHT: f32 = 36.0;
const ROW_GAP: f32 = 12.0;
const COLUMN_GAP: f32 = 90.0;
const SIDE_MARGIN: f32 = 20.0;

pub fn render_graph_tab(ui_state: &mut UiState, ctx: &Context) {
    // Keep the flow animation smooth while this tab is open — the app-wide repaint pacing
    // otherwise backs off to a slow idle rate when nothing else is animating.
    ctx.request_repaint_after(std::time::Duration::from_millis(16));
    let time = ctx.input(|i| i.time);

    CentralPanel::default()
        .frame(Frame::default().fill(theme::BG_PRIMARY))
        .show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(SIDE_MARGIN);
                ui.label(
                    RichText::new("Audio Routing")
                        .size(15.0)
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                );
            });
            ui.add_space(4.0);

            if ui_state.graph_streams.is_empty() && ui_state.graph_sinks.is_empty() {
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(SIDE_MARGIN);
                    ui.label(
                        RichText::new("No active audio streams or sinks detected.")
                            .color(theme::TEXT_MUTED),
                    );
                });
                return;
            }

            // A sink is "hardware" if some other sink loops back into it (see
            // `list_loopback_routes`); everything else is a software/virtual sink that
            // apps route to directly.
            let hardware_names: HashSet<&str> = ui_state
                .graph_loopback_routes
                .iter()
                .map(|(_, hw)| hw.as_str())
                .collect();
            let software_sinks: Vec<&(u32, String)> = ui_state
                .graph_sinks
                .iter()
                .filter(|(_, name)| !hardware_names.contains(name.as_str()))
                .collect();
            let hardware_sinks: Vec<&(u32, String)> = ui_state
                .graph_sinks
                .iter()
                .filter(|(_, name)| hardware_names.contains(name.as_str()))
                .collect();

            ScrollArea::both().auto_shrink([false; 2]).show(ui, |ui| {
                let row_count = ui_state
                    .graph_streams
                    .len()
                    .max(software_sinks.len())
                    .max(hardware_sinks.len())
                    .max(1);
                let canvas_height = row_count as f32 * (NODE_HEIGHT + ROW_GAP) + ROW_GAP;
                let canvas_width = ui
                    .available_width()
                    .max(SIDE_MARGIN * 2.0 + NODE_WIDTH * 3.0 + COLUMN_GAP * 2.0);

                let (rect, _response) =
                    ui.allocate_exact_size(vec2(canvas_width, canvas_height), Sense::hover());
                let painter = ui.painter_at(rect);

                let left_x = rect.left() + SIDE_MARGIN;
                let right_x = rect.right() - SIDE_MARGIN - NODE_WIDTH;
                let mid_x = left_x + (right_x - left_x) / 2.0;

                // Program (left) nodes: remember each one's right-center point, keyed to
                // the sink index it's currently routed to.
                let mut stream_anchors: Vec<(Option<u32>, Pos2)> = Vec::new();
                for (i, (_, app_name, sink_idx)) in ui_state.graph_streams.iter().enumerate() {
                    let node_rect = row_rect(rect, left_x, i);
                    draw_node(&painter, node_rect, app_name, theme::ACCENT_ORANGE);
                    stream_anchors.push((*sink_idx, node_rect.right_center()));
                }

                // Software sink (middle) nodes: keyed both by index (for the program ->
                // sink lines) and by name (for the sink -> hardware loopback lines).
                let mut sink_left_by_index: HashMap<u32, Pos2> = HashMap::new();
                let mut sink_right_by_name: HashMap<&str, Pos2> = HashMap::new();
                for (i, (idx, name)) in software_sinks.iter().enumerate() {
                    let node_rect = row_rect(rect, mid_x, i);
                    draw_node(&painter, node_rect, name, theme::ACCENT_BLUE);
                    sink_left_by_index.insert(*idx, node_rect.left_center());
                    sink_right_by_name.insert(name.as_str(), node_rect.right_center());
                }

                // Hardware (right) nodes: keyed both ways too, since a program can in
                // principle route straight to a hardware sink without a software sink in
                // between.
                let mut hw_left_by_index: HashMap<u32, Pos2> = HashMap::new();
                let mut hw_left_by_name: HashMap<&str, Pos2> = HashMap::new();
                for (i, (idx, name)) in hardware_sinks.iter().enumerate() {
                    let node_rect = row_rect(rect, right_x, i);
                    draw_node(&painter, node_rect, name, theme::ACCENT_GOLD);
                    hw_left_by_index.insert(*idx, node_rect.left_center());
                    hw_left_by_name.insert(name.as_str(), node_rect.left_center());
                }

                // Programs -> whichever sink (software or hardware) they're routed to.
                for (sink_idx, from) in stream_anchors {
                    let Some(sink_idx) = sink_idx else { continue };
                    let to = sink_left_by_index
                        .get(&sink_idx)
                        .or_else(|| hw_left_by_index.get(&sink_idx));
                    if let Some(&to) = to {
                        draw_flow_line(&painter, from, to, time, theme::ACCENT_GREEN);
                    }
                }

                // Software sinks -> the hardware they loop back into.
                for (software_name, hardware_name) in &ui_state.graph_loopback_routes {
                    let Some(&from) = sink_right_by_name.get(software_name.as_str()) else {
                        continue;
                    };
                    let Some(&to) = hw_left_by_name.get(hardware_name.as_str()) else {
                        continue;
                    };
                    draw_flow_line(&painter, from, to, time, theme::ACCENT_GOLD);
                }
            });
        });
}

/// Draws a connection as a dim base line plus a marching pattern of bright dashes that
/// travel from `from` to `to` over time — a visual "audio is flowing this way" cue.
fn draw_flow_line(painter: &egui::Painter, from: Pos2, to: Pos2, time: f64, color: Color32) {
    const DASH_LEN: f32 = 8.0;
    const GAP_LEN: f32 = 10.0;
    const PERIOD: f32 = DASH_LEN + GAP_LEN;
    const SPEED: f32 = 50.0; // pixels per second, from -> to

    let delta = to - from;
    let length = delta.length();
    if length < 1.0 {
        return;
    }
    let dir = delta / length;

    painter.line_segment([from, to], Stroke::new(1.5_f32, color.gamma_multiply(0.25)));

    let phase = (time as f32 * SPEED).rem_euclid(PERIOD);
    let mut pos = -phase;
    while pos < length {
        let seg_start = pos.max(0.0);
        let seg_end = (pos + DASH_LEN).min(length);
        if seg_end > seg_start {
            painter.line_segment(
                [from + dir * seg_start, from + dir * seg_end],
                Stroke::new(2.5_f32, color),
            );
        }
        pos += PERIOD;
    }
}

fn row_rect(canvas: Rect, x: f32, row: usize) -> Rect {
    let y = canvas.top() + ROW_GAP + row as f32 * (NODE_HEIGHT + ROW_GAP);
    Rect::from_min_size(pos2(x, y), vec2(NODE_WIDTH, NODE_HEIGHT))
}

fn draw_node(painter: &egui::Painter, rect: Rect, label: &str, accent: Color32) {
    painter.rect_filled(rect, 6.0, theme::BG_TERTIARY);
    painter.rect_stroke(rect, 6.0, Stroke::new(1.5_f32, accent), StrokeKind::Inside);

    let text_pos = rect.left_center() + vec2(10.0, 0.0);
    let max_chars = ((rect.width() - 20.0) / 7.0).max(1.0) as usize;
    let display_label: std::borrow::Cow<str> = if label.chars().count() > max_chars {
        format!(
            "{}…",
            label.chars().take(max_chars.saturating_sub(1)).collect::<String>()
        )
        .into()
    } else {
        label.into()
    };

    painter.text(
        text_pos,
        Align2::LEFT_CENTER,
        display_label,
        FontId::proportional(12.5),
        theme::TEXT_PRIMARY,
    );
}
