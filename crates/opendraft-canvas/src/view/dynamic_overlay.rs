//! The Block Editor's picture of a dynamic block's schema: each parameter
//! as a line with its name, each action's frame (a stretch) and name. It is
//! painted over the drawing and is no object: nothing picks it, snaps to it
//! or plots it, and it is gone with the editor.

use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind};
use opendraft_core::block::{DynamicActionKind, DynamicParameterKind};
use opendraft_core::{Document, Point};

use super::*;

/// Distinct from the grips and the selection, in both themes.
const SCHEMA: Color32 = Color32::from_rgb(214, 130, 40);
const TICK: f32 = 5.0;

impl View {
    pub(super) fn paint_block_schema(&self, painter: &Painter, rect: Rect, document: &Document) {
        let Some(schema) = document.block_editor_dynamic() else {
            return;
        };
        let stroke = Stroke::new(1.5_f32, SCHEMA);
        let font = FontId::proportional(12.0);
        let to_screen = |p| self.camera.world_to_screen(p, rect);
        for action in &schema.actions {
            let at = match &action.kind {
                DynamicActionKind::Stretch { frame, .. } => {
                    let (a, b) = (to_screen(frame.min), to_screen(frame.max));
                    let frame = Rect::from_two_pos(a, b);
                    painter.rect_stroke(
                        frame,
                        0.0,
                        Stroke::new(1.0_f32, SCHEMA.gamma_multiply(0.6)),
                        StrokeKind::Middle,
                    );
                    frame.left_top()
                }
                DynamicActionKind::Move { .. }
                | DynamicActionKind::Rotate { .. }
                | DynamicActionKind::Flip { .. }
                | DynamicActionKind::Scale { .. } => schema
                    .parameters
                    .iter()
                    .find(|p| p.id == action.parameter)
                    .map_or(rect.center(), |p| {
                        let (DynamicParameterKind::Linear { start: at, .. }
                        | DynamicParameterKind::Rotation { base: at, .. }
                        | DynamicParameterKind::Flip { base: at, .. }
                        | DynamicParameterKind::Polar { base: at, .. }
                        | DynamicParameterKind::Xy { base: at, .. }
                        | DynamicParameterKind::Point { position: at }) = &p.kind;
                        to_screen(*at) + egui::vec2(0.0, 14.0)
                    }),
            };
            painter.text(
                at,
                Align2::LEFT_BOTTOM,
                &action.name,
                font.clone(),
                SCHEMA.gamma_multiply(0.8),
            );
        }
        for parameter in &schema.parameters {
            let (start, end) = match &parameter.kind {
                // The mirror line is drawn like a distance.
                DynamicParameterKind::Linear { start, end, .. }
                | DynamicParameterKind::Flip {
                    base: start, end, ..
                }
                | DynamicParameterKind::Polar {
                    base: start, end, ..
                }
                | DynamicParameterKind::Xy {
                    base: start, end, ..
                } => (*start, *end),
                // A point: a small circle where it is.
                DynamicParameterKind::Point { position } => {
                    painter.circle_stroke(to_screen(*position), TICK, stroke);
                    (*position, *position)
                }
                // A rotation: its circle, and the radius the grip starts on.
                DynamicParameterKind::Rotation { base, radius, .. } => {
                    let end = Point::new(base.x + radius, base.y);
                    let centre = to_screen(*base);
                    painter.circle_stroke(
                        centre,
                        (to_screen(end) - centre).length(),
                        Stroke::new(1.0_f32, SCHEMA.gamma_multiply(0.6)),
                    );
                    (*base, end)
                }
            };
            let (a, b) = (to_screen(start), to_screen(end));
            painter.line_segment([a, b], stroke);
            let along = (b - a).normalized();
            if along.is_finite() {
                let across = egui::vec2(-along.y, along.x) * TICK;
                for end in [a, b] {
                    painter.line_segment([end - across, end + across], stroke);
                }
            }
            painter.text(
                (a + b.to_vec2()) / 2.0 - egui::vec2(0.0, 4.0),
                Align2::CENTER_BOTTOM,
                &parameter.name,
                font.clone(),
                SCHEMA,
            );
        }
    }
}
