//! The grid and the axes behind the entities. Both lie in the coordinate
//! system in force: the grid along its axes, and the axes from its origin.

use opendraft_core::Ccs;

use super::*;

/// Length of each arm of the corner icon, in screen points.
const ICON_ARM: f32 = 28.0;
/// Gap between the corner icon and the canvas edge.
const ICON_INSET: f32 = 14.0;

impl View {
    pub(super) fn paint_grid(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        grid: &GridSettings,
        ccs: Ccs,
    ) {
        let step = grid_step(grid.spacing, self.camera.zoom, GRID_MIN_PX);
        let minor = Stroke::new(1.0_f32, style.grid_minor);
        let major = Stroke::new(1.0_f32, style.grid_major);
        let pick = |i: i64| {
            if i.rem_euclid(GRID_MAJOR_EVERY) == 0 {
                major
            } else {
                minor
            }
        };
        if ccs.angle != 0.0 || self.camera.twist != 0.0 {
            return self.paint_turned_grid(painter, rect, step, ccs, &pick);
        }
        let visible = self.camera.visible_world(rect);
        let at = |x: f64, y: f64| self.camera.world_to_screen(Point::new(x, y), rect);
        let (ox, oy) = (ccs.origin.x, ccs.origin.y);
        for i in line_indices(visible.min.x - ox, visible.max.x - ox, step) {
            let x = at(ox + i as f64 * step, 0.0).x;
            painter.vline(x.round() + 0.5, rect.y_range(), pick(i));
        }
        for i in line_indices(visible.min.y - oy, visible.max.y - oy, step) {
            let y = at(0.0, oy + i as f64 * step).y;
            painter.hline(rect.x_range(), y.round() + 0.5, pick(i));
        }
    }

    /// The grid of a turned system: lines along its axes across the part of
    /// it on screen.
    fn paint_turned_grid(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        step: f64,
        ccs: Ccs,
        pick: &dyn Fn(i64) -> Stroke,
    ) {
        let corners = [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ]
        .map(|c| ccs.from_world(self.camera.screen_to_world(c, rect)));
        let (mut min, mut max) = (corners[0], corners[0]);
        for c in &corners[1..] {
            (min.x, min.y) = (min.x.min(c.x), min.y.min(c.y));
            (max.x, max.y) = (max.x.max(c.x), max.y.max(c.y));
        }
        let at = |x: f64, y: f64| {
            self.camera
                .world_to_screen(ccs.to_world(Point::new(x, y)), rect)
        };
        for i in line_indices(min.x, max.x, step) {
            let x = i as f64 * step;
            painter.line_segment([at(x, min.y), at(x, max.y)], pick(i));
        }
        for i in line_indices(min.y, max.y, step) {
            let y = i as f64 * step;
            painter.line_segment([at(min.x, y), at(max.x, y)], pick(i));
        }
    }

    pub(super) fn paint_axes(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        ccs: Ccs,
    ) {
        let o = self.camera.world_to_screen(ccs.origin, rect);
        if ccs.angle == 0.0 && self.camera.twist == 0.0 {
            // Each axis runs from the origin along its positive direction only.
            let x_start = o.x.max(rect.left());
            if x_start < rect.right() {
                painter.hline(
                    x_start..=rect.right(),
                    o.y.round() + 0.5,
                    Stroke::new(1.0_f32, style.axis_x),
                );
            }
            let y_start = o.y.min(rect.bottom());
            if y_start > rect.top() {
                painter.vline(
                    o.x.round() + 0.5,
                    rect.top()..=y_start,
                    Stroke::new(1.0_f32, style.axis_y),
                );
            }
        } else {
            // Far enough to leave the canvas from anywhere on it.
            let reach = (rect.width() + rect.height()) + (o - rect.center()).length();
            let (x, y) = screen_axes(ccs, self.camera.twist);
            painter.line_segment([o, o + x * reach], Stroke::new(1.0_f32, style.axis_x));
            painter.line_segment([o, o + y * reach], Stroke::new(1.0_f32, style.axis_y));
        }
        // Shown for a custom system, and in a turned view whatever the system.
        if !ccs.is_world() || self.camera.twist != 0.0 {
            paint_icon(painter, rect, style, ccs, self.camera.twist);
        }
    }
}

/// The unit directions of a system's X and Y axes on a screen (whose y runs
/// down) showing the drawing turned by `twist`.
pub(super) fn screen_axes(ccs: Ccs, twist: f64) -> (egui::Vec2, egui::Vec2) {
    let on_screen = Ccs::new(ccs.origin, ccs.angle + twist);
    let (x, y) = (on_screen.x_axis(), on_screen.y_axis());
    (
        egui::vec2(x.x as f32, -x.y as f32),
        egui::vec2(y.x as f32, -y.y as f32),
    )
}

/// The coordinate system icon in the lower-left corner of the canvas (of the
/// viewport, in a layout): two arms along its axes, labelled, so a custom
/// system or a turned view is never in force unseen.
fn paint_icon(painter: &egui::Painter, rect: Rect, style: &CanvasStyle, ccs: Ccs, twist: f64) {
    let area = painter.clip_rect().intersect(rect);
    let room = ICON_ARM + ICON_INSET + 6.0;
    if area.width() < 2.0 * room || area.height() < 2.0 * room {
        return;
    }
    let corner = egui::pos2(area.left() + room, area.bottom() - room);
    let (x, y) = screen_axes(ccs, twist);
    let font = egui::FontId::proportional(11.0);
    for (dir, color, label) in [(x, style.axis_x, "X"), (y, style.axis_y, "Y")] {
        let end = corner + dir * ICON_ARM;
        painter.line_segment([corner, end], Stroke::new(1.5_f32, color));
        let text_at = corner + dir * (ICON_ARM + 8.0);
        painter.text(
            text_at,
            egui::Align2::CENTER_CENTER,
            label,
            font.clone(),
            color,
        );
    }
    let square = Rect::from_center_size(corner, egui::vec2(4.0, 4.0));
    painter.rect_filled(square, 0.0, style.axis_x);
}
