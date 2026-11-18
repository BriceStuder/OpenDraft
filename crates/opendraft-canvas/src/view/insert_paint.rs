//! Painting a block reference: each piece in its own colour, after the
//! layer-0 inheritance the document applies (`Document::insert_pieces`).

use super::*;

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
    /// smaller than `MIN_DRAW_PX` (points excepted, whose mode gives them a size).
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
        !bounds.intersects(visible) || (!point && size * self.camera.zoom < MIN_DRAW_PX)
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
