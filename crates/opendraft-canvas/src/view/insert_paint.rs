//! Painting a block reference: each piece in its own colour, after the
//! layer-0 inheritance the document applies (`Document::insert_pieces`).

use super::*;

/// Entities smaller than this on screen (pixels) are not drawn, unless the
/// picture is kept as triangles (see [`View::min_draw_px`]).
const MIN_DRAW_PX: f64 = 0.75;
/// Kept as triangles, every triangle costs every frame, so objects smaller
/// than this are left out: at the large drawing's extents 6 MB of triangles
/// against 17.5 MB at `MIN_DRAW_PX`.
const MIN_DRAW_PX_TRIANGLES: f64 = 2.0;

impl View {
    /// Paint the pieces of a block reference that is neither selected nor
    /// hovered (those are drawn whole in the highlight colour). Pieces on
    /// layers frozen in this viewport are left out.
    pub(super) fn paint_insert_pieces(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        pieces: &[opendraft_core::Object],
        frozen: &[LayerId],
    ) {
        let visible = self.camera.visible_world(rect);
        for piece in pieces.iter().filter(|p| !frozen.contains(&p.attrs.layer)) {
            let point = matches!(piece.entity, Entity::Point { .. });
            if self.negligible_box(&piece.bounds(), point, &visible) {
                continue;
            }
            let line = match document.color_of(piece) {
                Color::Foreground => style.entity,
                Color::Rgb(r, g, b) => egui::Color32::from_rgb(r, g, b),
            };
            match &piece.entity {
                Entity::MText(text) => {
                    self.paint_mtext(painter, rect, style, document, text, line, false, false);
                }
                entity => self.stroke_entity(painter, rect, entity, Stroke::new(1.5_f32, line)),
            }
        }
    }

    /// A piece of a block reference that need not be drawn: off screen, or
    /// smaller than `min_draw_px` (points excepted, whose mode gives them a size).
    pub(super) fn negligible(&self, entity: &Entity, visible: &opendraft_core::Bounds) -> bool {
        let point = matches!(entity, Entity::Point { .. });
        self.negligible_box(&entity.bounds(), point, visible)
    }

    /// [`negligible`](Self::negligible) for a box already known.
    pub(super) fn negligible_box(
        &self,
        bounds: &opendraft_core::Bounds,
        point: bool,
        visible: &opendraft_core::Bounds,
    ) -> bool {
        let size = (bounds.max.x - bounds.min.x).max(bounds.max.y - bounds.min.y);
        !bounds.intersects(visible) || (!point && size * self.camera.zoom < self.min_draw_px())
    }

    /// The size on screen, in pixels, under which an object is not drawn: a
    /// little under a pixel, or 2 when the picture is kept as triangles, which
    /// cost every frame (an image costs its pixels, however much is in it).
    pub(super) fn min_draw_px(&self) -> f64 {
        match self.cache_config.render {
            crate::Render::Image => MIN_DRAW_PX,
            crate::Render::Triangles => MIN_DRAW_PX_TRIANGLES,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendraft_core::Bounds;

    fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Line {
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
        }
    }

    #[test]
    fn pieces_off_screen_or_under_a_pixel_are_negligible() {
        let mut view = View::default();
        view.camera.zoom = 1.0;
        let visible = Bounds::from_points(Point::new(0.0, 0.0), Point::new(100.0, 100.0));
        assert!(!view.negligible(&line((10.0, 10.0), (50.0, 10.0)), &visible));
        assert!(view.negligible(&line((10.0, 10.0), (10.2, 10.0)), &visible));
        assert!(view.negligible(&line((500.0, 500.0), (550.0, 500.0)), &visible));
        // Zoomed in, the same short piece is worth drawing.
        view.camera.zoom = 20.0;
        assert!(!view.negligible(&line((10.0, 10.0), (10.2, 10.0)), &visible));
        // A point has no size of its own.
        let point = Entity::Point {
            at: Point::new(5.0, 5.0),
        };
        assert!(!view.negligible(&point, &visible));
    }
}
