//! Painting the drawing's entities: every visible one directly, or the plain
//! ones from the kept layer (see `layer_cache`) with the selected and hovered
//! ones drawn live over it.

use super::*;
use crate::layer_cache::{capture, tessellate, Inputs, LayerKey, Piece, CHUNK_OBJECTS};
use egui::Painter;

/// Whether a kept layer drawn for the `hits`th time is checked against a fresh
/// one: every 256th time (every time in tests), in debug builds.
#[cfg(debug_assertions)]
fn verify_due(hits: u64) -> bool {
    hits.is_multiple_of(if cfg!(test) { 1 } else { 256 })
}

impl View {
    /// The entities of the space being edited, through the kept layer when
    /// the app has said when the drawing changes and the settings want it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_model(
        &self,
        painter: &Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
    ) {
        let space = document.active_layout();
        let direct = || {
            self.paint_entities(painter, rect, style, document, space, &[], selection, hover);
        };
        let wanted = self.cache_config.wants(document.objects().len());
        let Some(token) = self.document_token.filter(|_| wanted) else {
            self.layer_cache.borrow_mut().clear();
            return direct();
        };
        let ctx = painter.ctx();
        let key = LayerKey::new(Inputs {
            token,
            space,
            frozen: &[],
            camera: &self.camera,
            canvas: rect,
            clip: painter.clip_rect(),
            line: style.entity,
            point_style: self.point_style,
            hidden: self.hidden,
            pixels_per_point: ctx.pixels_per_point(),
            tessellation: ctx.tessellation_options(|o| *o),
        });
        let limit = self.cache_config.memory_limit;
        let have = self.layer_cache.borrow().has(&key);
        if !have {
            let visible = self.camera.visible_world(rect);
            let ids: Vec<EntityId> = document
                .visible_in_rect(space, &[], visible)
                .iter()
                .map(|o| o.id)
                .collect();
            let chunks = ids
                .chunks(CHUNK_OBJECTS)
                .map(|c| {
                    (
                        c.to_vec(),
                        self.build_piece(painter, rect, style, document, c, &[]),
                    )
                })
                .collect();
            self.layer_cache.borrow_mut().store(key, chunks, limit);
        }
        // Each chunk holding a selected or hovered entity is drawn without it.
        let hovered = hover.filter(|h| !selection.contains(h));
        let exclusions = {
            let highlighted = selection.iter().copied().chain(hovered);
            self.layer_cache.borrow().exclusions(highlighted)
        };
        let missing = self.layer_cache.borrow().missing(&exclusions);
        for want in missing {
            let piece = self.build_piece(painter, rect, style, document, &want.ids, &want.excluded);
            self.layer_cache
                .borrow_mut()
                .set_variant(want, piece, limit);
        }
        if !self.layer_cache.borrow_mut().draw(painter, &exclusions) {
            return direct();
        }
        #[cfg(debug_assertions)]
        if have && verify_due(self.layer_cache.borrow().stats().hits) {
            debug_assert!(
                self.layer_matches_fresh(painter, rect, style, document),
                "the kept layer differs from a fresh one: a painting input is missing from the key"
            );
        }
        self.paint_highlighted(painter, rect, style, document, space, selection, hover);
    }

    /// The plain entities `ids` (in draw order) but those in `excluded`
    /// (sorted), as triangles with the textures they use.
    fn build_piece(
        &self,
        painter: &Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        ids: &[EntityId],
        excluded: &[EntityId],
    ) -> Piece {
        let ctx = painter.ctx();
        let none = BTreeSet::new();
        let (shapes, textures) = capture(ctx, painter.clip_rect(), |p| {
            for id in ids.iter().filter(|id| excluded.binary_search(id).is_err()) {
                if let Some(object) = document.object(*id) {
                    self.paint_object(p, rect, style, document, object, &[], &none, None);
                }
            }
        });
        Piece::new(tessellate(ctx, shapes), textures)
    }

    /// Whether the kept chunks and variants are what painting now would give.
    #[cfg(debug_assertions)]
    fn layer_matches_fresh(
        &self,
        painter: &Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
    ) -> bool {
        let whole = self.layer_cache.borrow().chunk_ids();
        for (n, ids) in whole.iter().enumerate() {
            let fresh = self.build_piece(painter, rect, style, document, ids, &[]);
            if !self.layer_cache.borrow().matches(n, None, &fresh) {
                return false;
            }
        }
        let variants = self.layer_cache.borrow().variants();
        variants.iter().all(|v| {
            let fresh = self.build_piece(painter, rect, style, document, &v.ids, &v.excluded);
            self.layer_cache
                .borrow()
                .matches(v.chunk, Some(&v.excluded), &fresh)
        })
    }

    /// The selected and hovered entities, over the kept layer.
    #[allow(clippy::too_many_arguments)]
    fn paint_highlighted(
        &self,
        painter: &Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        space: Option<LayoutId>,
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
    ) {
        let visible = self.camera.visible_world(rect);
        let hovered = hover.filter(|h| !selection.contains(h));
        for id in selection.iter().copied().chain(hovered) {
            let Some(object) = document.object(id) else {
                continue;
            };
            if object.layout != space
                || !document.is_visible(object)
                || !object.bounds().intersects(&visible)
            {
                continue;
            }
            self.paint_object(
                painter,
                rect,
                style,
                document,
                object,
                &[],
                selection,
                hover,
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
    fn paint_object(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        object: &opendraft_core::Object,
        frozen: &[LayerId],
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
    ) {
        let (id, entity) = (&object.id, &object.entity);
        let line = match document.color_of(object) {
            Color::Foreground => style.entity,
            Color::Rgb(r, g, b) => egui::Color32::from_rgb(r, g, b),
        };
        let bounds = object.bounds();
        // Entities smaller than a pixel add nothing but triangles when
        // zoomed far out. Selected ones are always drawn.
        let on_screen =
            (bounds.max.x - bounds.min.x).max(bounds.max.y - bounds.min.y) * self.camera.zoom;
        // A point has no size of its own: its mode gives it one.
        let sized = !matches!(entity, Entity::Point { .. });
        if sized && on_screen < MIN_DRAW_PX && !selection.contains(id) {
            return;
        }
        if self.hidden == Some(*id) {
            return;
        }
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
        if let Some(pieces) = plain.then(|| document.insert_pieces(object)).flatten() {
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
