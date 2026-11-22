//! The drawing cursor: AutoCAD's full crosshair and pickbox, painted in
//! place of the system cursor over the canvas.

use egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

use opendraft_core::Document;

use super::types::{Crosshair, GridSettings, PointerSizes};
use super::{View, GRID_MIN_PX, WHEEL_ZOOM_RATE};
use crate::grid::grid_step;
use crate::snap::{SnapModes, SnapOverride};
use crate::snap_axis::Aim;

/// Which parts of the cursor the canvas shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CursorShape {
    /// Waiting for a point: the crosshair alone.
    Crosshair,
    /// Nothing running: the crosshair with the pickbox on it.
    Both,
    /// Waiting for an object to be clicked, or for objects to be selected:
    /// the pickbox alone.
    PickBox,
}

impl CursorShape {
    /// The shape AutoCAD shows: a point prompt has the crosshair alone, an
    /// object or selection prompt the pickbox alone, and no command both.
    pub(super) fn for_state(prompting: bool, picking: bool, selecting_objects: bool) -> Self {
        match (prompting, picking) {
            (true, true) => Self::PickBox,
            (true, false) => Self::Crosshair,
            _ if selecting_objects => Self::PickBox,
            _ => Self::Both,
        }
    }
}

/// How far each arm reaches from the centre. A size of 100 spans the whole
/// canvas in both directions wherever the cursor is; below that every arm is
/// the same length, that share of the canvas's longer side (so the cross is
/// square, not stretched with the window).
pub(super) fn arms(size: u32, canvas: Vec2) -> Vec2 {
    if size >= 100 {
        canvas
    } else {
        Vec2::splat(canvas.max_elem() * (size as f32 / 200.0))
    }
}

fn arm_segments(center: f32, extent: f32, gap: f32) -> Option<[(f32, f32); 2]> {
    (extent > gap).then_some([
        (center - extent, center - gap),
        (center + gap, center + extent),
    ])
}

fn paint(
    painter: &Painter,
    canvas: Rect,
    at: Pos2,
    shape: CursorShape,
    size: u32,
    color: Color32,
    pick: f64,
) {
    let stroke = Stroke::new(1.0_f32, color);
    if shape != CursorShape::PickBox {
        let arm = arms(size, canvas.size());
        let gap = pick as f32 + stroke.width * 0.5;
        if let Some([(left, left_end), (right_start, right)]) = arm_segments(at.x, arm.x, gap) {
            painter.hline(left..=left_end, at.y, stroke);
            painter.hline(right_start..=right, at.y, stroke);
        }
        if let Some([(top, top_end), (bottom_start, bottom)]) = arm_segments(at.y, arm.y, gap) {
            painter.vline(at.x, top..=top_end, stroke);
            painter.vline(at.x, bottom_start..=bottom, stroke);
        }
    }
    if shape != CursorShape::Crosshair {
        let half = pick as f32;
        let square = Rect::from_center_size(at, Vec2::splat(2.0 * half));
        painter.rect_stroke(square, 0.0, stroke, StrokeKind::Middle);
    }
}

impl View {
    pub fn set_pointer_sizes(&mut self, sizes: PointerSizes) {
        self.sizes = sizes;
    }

    /// Hold points to the horizontal or vertical from a prompt's base point.
    pub fn set_ortho(&mut self, ortho: crate::snap_axis::Ortho) {
        self.ortho = ortho;
    }

    /// What the pointer snaps to this frame. Ortho holds points to the axis
    /// from the prompt's base, unless a pick is wanted; Shift flips it.
    pub(super) fn aim<'a>(
        &self,
        document: &'a Document,
        modes: &SnapModes,
        grid: &GridSettings,
        picking: bool,
        shift: bool,
    ) -> Aim<'a> {
        let held = !picking && self.ortho.enabled != shift;
        Aim {
            document,
            modes: self
                .snap_cycle
                .over
                .as_ref()
                .map_or(*modes, |o| o.applied(modes)),
            step: grid_step(grid.spacing, self.camera.zoom, GRID_MIN_PX),
            radius: self.sizes.snap / self.camera.zoom,
            ortho: self.ortho,
            base: self
                .point_prompt
                .as_ref()
                .and_then(|p| p.base)
                .filter(|_| held),
            from: self.point_prompt.as_ref().and_then(|p| p.base),
            chosen: None,
            tracks: Vec::new(),
            track: None,
            extra: self.snap_extra(),
            insist: self
                .snap_cycle
                .over
                .as_ref()
                .is_some_and(SnapOverride::insists),
        }
    }

    /// The prompt's extra snap geometry as objects, for the snap pass.
    fn snap_extra(&self) -> std::sync::Arc<[opendraft_core::Object]> {
        let Some(prompt) = &self.point_prompt else {
            return Default::default();
        };
        (prompt.snap_geometry.iter().enumerate())
            .map(|(i, entity)| {
                let id = opendraft_core::EntityId(u64::MAX - i as u64);
                let attrs = opendraft_core::Attributes::on_layer(opendraft_core::LayerId::DEFAULT);
                opendraft_core::Object::new(id, entity.clone(), attrs, None)
            })
            .collect()
    }

    pub fn set_crosshair(&mut self, crosshair: Crosshair) {
        self.crosshair = crosshair;
    }

    /// Set the wheel zoom speed, in percent of the base rate (100 is normal).
    pub fn set_zoom_speed(&mut self, percent: u32) {
        self.zoom_speed = Some(percent as f32 / 100.0);
    }

    pub(super) fn wheel_rate(&self) -> f32 {
        WHEEL_ZOOM_RATE * self.zoom_speed.unwrap_or(1.0)
    }

    /// Zoom the model view about the pointer by the wheel or a pinch, when
    /// the canvas is `hovered` with the pointer at `pointer` (none when it is
    /// over something else).
    pub(super) fn wheel_zoom(
        &mut self,
        ui: &egui::Ui,
        rect: Rect,
        hovered: bool,
        pointer: Option<Pos2>,
    ) {
        // With an editor over the drawing the wheel still zooms, over the
        // editor too, but not over a toolbar, menu or dialog above it.
        let wheel_at = if self.wheel_through {
            let ctx = ui.ctx();
            ui.input(|i| i.pointer.latest_pos()).filter(|p| {
                rect.contains(*p)
                    && ctx
                        .layer_id_at(*p)
                        .is_none_or(|l| l.order == egui::Order::Background)
            })
        } else {
            pointer
        };
        let over = hovered || (self.wheel_through && wheel_at.is_some());
        let (true, Some(pos)) = (over, wheel_at) else {
            return;
        };
        // The whole notch at once, as AutoCAD does: egui's smoothed delta
        // creeps on for a sixth of a second, and the sharp picture waits for
        // the view to stand still.
        let (scroll, pinch) = ui.input(|i| (i.raw_scroll_delta.y, i.zoom_delta()));
        let factor = f64::from((scroll * self.wheel_rate()).exp() * pinch);
        if (factor - 1.0).abs() > 1e-9 {
            self.camera.zoom_at(pos, rect, factor);
        }
    }

    /// Hide the system cursor and draw ours at `at` (screen position), in
    /// the colour the user chose, else `over`, the colour for what is under it.
    pub(super) fn paint_cursor(
        &self,
        ui: &egui::Ui,
        canvas: Rect,
        at: Pos2,
        shape: CursorShape,
        over: Color32,
    ) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        let painter = ui.painter_at(canvas);
        let color = self.crosshair.color.unwrap_or(over);
        let size = self.crosshair.size;
        paint(&painter, canvas, at, shape, size, color, self.sizes.pick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arms_are_equal_and_a_hundred_spans_the_canvas() {
        let canvas = Vec2::new(1000.0, 600.0);
        assert_eq!(arms(5, canvas), Vec2::splat(25.0));
        assert_eq!(arms(50, canvas), Vec2::splat(250.0));
        assert_eq!(
            arms(100, canvas),
            canvas,
            "reaches either edge from anywhere"
        );
    }

    #[test]
    fn crosshair_arms_leave_the_pickbox_clear() {
        assert_eq!(
            arm_segments(100.0, 40.0, 6.5),
            Some([(60.0, 93.5), (106.5, 140.0)])
        );
        assert_eq!(arm_segments(100.0, 6.5, 6.5), None);
    }

    #[test]
    fn the_cursor_shape_follows_what_the_command_waits_for() {
        use CursorShape::*;
        // (prompting for a point or object, picking an object, selecting)
        assert_eq!(CursorShape::for_state(false, false, false), Both);
        assert_eq!(CursorShape::for_state(true, false, false), Crosshair);
        assert_eq!(CursorShape::for_state(true, true, false), PickBox);
        assert_eq!(CursorShape::for_state(false, false, true), PickBox);
    }
}
