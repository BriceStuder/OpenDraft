//! Painting the drawing's entities: every visible one directly, or the plain
//! ones from the kept layer (see `layer_cache`) with the selected and hovered
//! ones drawn live over it.

use super::*;
use opendraft_core::Object;

/// Which highlights to draw.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Highlights {
    All,
    /// The selected entities only.
    Selected,
    /// The hovered entity only.
    Hovered,
}

impl View {
    /// The selected and hovered entities, over the kept layer.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_highlighted(
        &self,
        args: (&egui::Painter, Rect, &CanvasStyle, &Document),
        space: Option<LayoutId>,
        frozen: &[LayerId],
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
    ) {
        self.paint_highlights(args, space, frozen, selection, hover, Highlights::All);
    }

    /// The part of the highlights `which` names.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_highlights(
        &self,
        (painter, rect, style, document): (&egui::Painter, Rect, &CanvasStyle, &Document),
        space: Option<LayoutId>,
        frozen: &[LayerId],
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
        which: Highlights,
    ) {
        let visible = self.camera.visible_world(rect);
        let hovered = hover.filter(|h| !selection.contains(h));
        let chosen = (which != Highlights::Hovered).then(|| selection.iter().copied());
        let hovered = hovered.filter(|_| which != Highlights::Selected);
        for id in chosen.into_iter().flatten().chain(hovered) {
            let Some(object) = document.object(id) else {
                continue;
            };
            if object.layout != space
                || !document.is_visible(object)
                || frozen.contains(&object.attrs.layer)
                || !object.bounds().intersects(&visible)
            {
                continue;
            }
            self.paint_object(
                painter, rect, style, document, object, frozen, selection, hover,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_entities(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        space: Option<LayoutId>,
        frozen: &[LayerId],
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
    ) {
        let visible = self.camera.visible_world(rect);
        for object in document.visible_in_rect(space, frozen, visible) {
            self.paint_object(
                painter, rect, style, document, object, frozen, selection, hover,
            );
        }
    }

    /// One entity as the drawing shows it: plain, selected or hovered.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_object(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        object: &Object,
        frozen: &[LayerId],
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
    ) {
        let (id, entity) = (&object.id, &object.entity);
        let bounds = object.bounds();
        // Entities smaller than a pixel add nothing but triangles when
        // zoomed far out. Selected ones are always drawn.
        let on_screen =
            (bounds.max.x - bounds.min.x).max(bounds.max.y - bounds.min.y) * self.camera.zoom;
        // A point has no size of its own: its mode gives it one.
        let sized = !matches!(entity, Entity::Point { .. });
        if sized && on_screen < self.min_draw_px() && !selection.contains(id) {
            return;
        }
        if self.hidden == Some(*id) {
            return;
        }
        let line = match document.color_of(object) {
            Color::Foreground => style.entity,
            Color::Rgb(r, g, b) => egui::Color32::from_rgb(r, g, b),
        };
        if let Entity::MText(text) = entity {
            let (selected, hovered) = (selection.contains(id), hover == Some(*id));
            self.paint_mtext(
                painter, rect, style, document, text, line, selected, hovered,
            );
            return;
        }
        if let Entity::Text(text) = entity {
            let (selected, hovered) = (selection.contains(id), hover == Some(*id));
            let projected = text.projection();
            self.paint_mtext(
                painter, rect, style, document, &projected, line, selected, hovered,
            );
            return;
        }
        if let Entity::AttributeDefinition(d) = entity {
            let (selected, hovered) = (selection.contains(id), hover == Some(*id));
            let tag = d.tag_view();
            self.paint_mtext(
                painter, rect, style, document, &tag, line, selected, hovered,
            );
            return;
        }
        if let Entity::Hatch(h) = entity {
            let highlight = if selection.contains(id) {
                Some(hatch_paint::Highlight {
                    glow: style.selected_glow,
                    edge: style.selected,
                    width: 2.0,
                })
            } else if hover == Some(*id) {
                Some(hatch_paint::Highlight {
                    glow: style.hover,
                    edge: line,
                    width: 1.5,
                })
            } else {
                None
            };
            self.paint_hatch(painter, rect, Some(*id), h, line, highlight);
            return;
        }
        let plain = !selection.contains(id) && hover != Some(*id);
        let pieces = plain.then(|| {
            let mut kept = self.insert_pieces.borrow_mut();
            kept.of(self.document_token, document, object)
        });
        if let Some(pieces) = pieces.flatten() {
            self.paint_insert_pieces(painter, rect, style, document, &pieces, frozen);
            return;
        }
        // A linetype cuts the line into dashes; the glow follows them.
        let draw = |stroke: Stroke| self.stroke_object(painter, rect, document, object, stroke);
        // Selected and hovered entities get a wide translucent overlay
        // under a thicker, brighter line.
        if selection.contains(id) {
            draw(Stroke::new(HIGHLIGHT_WIDTH, style.selected_glow));
            draw(Stroke::new(2.0_f32, style.selected));
        } else if hover == Some(*id) {
            draw(Stroke::new(HIGHLIGHT_WIDTH, style.hover));
            draw(Stroke::new(1.5_f32, line));
            if let Entity::Circle { center, .. } | Entity::Arc { center, .. } = entity {
                self.centre_hint(painter, rect, *center, line);
            }
        } else {
            draw(Stroke::new(1.5_f32, line));
        }
    }
}
