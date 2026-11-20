//! Painting the kept layer as an image (see `layer_cache/bitmap.rs`): the
//! drawing is one rectangle; selected and hovered entities are drawn over it,
//! live, or for a large selection from a kept picture of their own. They are
//! drawn over the entity they highlight, not in place of it as with triangles,
//! which only shows at the soft edge of a thick highlight.

use super::entity_paint::Highlights;
use super::layer_paint::Frame;
use super::*;
use crate::layer_cache::{
    selection_hash, Geometry, LayerCache, LayerKey, Piece, Render, IMAGE_MARGIN, KEEP_FROM,
};
use opendraft_core::EntityId;

/// What it takes to keep a new layer, as the cache's own settings say.
pub(super) struct Keep {
    pub render: Render,
    pub ctx: egui::Context,
    pub name: String,
    pub geometry: Geometry,
    pub limit: usize,
}

impl Keep {
    /// Keep `pieces` as the layer for `key` in `cache`: as an image, or as the
    /// triangles themselves.
    pub fn into(self, cache: &mut LayerCache, key: LayerKey, pieces: Vec<Piece>) {
        match self.render {
            Render::Image => cache.store_image(&self.ctx, &self.name, key, pieces, self.geometry),
            Render::Triangles => cache.store(key, pieces, self.limit),
        }
    }
}

impl View {
    /// The margin round the canvas a new picture covers (see [`IMAGE_MARGIN`]):
    /// none for kept triangles, which cost every frame however far they reach.
    pub(super) fn margin(&self, canvas: Rect) -> egui::Vec2 {
        match self.cache_config.render {
            Render::Image => (canvas.size() * IMAGE_MARGIN).round(),
            Render::Triangles => egui::Vec2::ZERO,
        }
    }

    /// How the frame's new layer is to be kept.
    pub(super) fn keep_plan(&self, frame: &Frame<'_>) -> Keep {
        let ctx = frame.painter.ctx().clone();
        let scale = ctx.pixels_per_point();
        Keep {
            render: self.cache_config.render,
            name: format!("opendraft_layer_{:?}", frame.slot),
            geometry: Geometry {
                canvas: frame.rect,
                scale,
            },
            limit: self.limit_for(frame),
            ctx,
        }
    }

    /// One frame of a layer kept as an image: the image, then the highlights.
    pub(super) fn paint_image(
        &self,
        frame: &Frame<'_>,
        key: &LayerKey,
        checked: bool,
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
    ) {
        self.cache(frame).draw_bitmap(frame.painter, None);
        self.cache(frame).note_hit();
        let large = selection.len() >= KEEP_FROM;
        let hash = if large { selection_hash(selection) } else { 0 };
        let kept = large && self.draw_kept_selection(frame, key, hash, selection);
        if !large {
            self.cache(frame).drop_selected();
        }
        #[cfg(debug_assertions)]
        if checked && super::layer_paint::verify_due(self.cache(frame).stats().hits) {
            debug_assert!(
                self.image_matches_fresh(frame, selection),
                "the kept image differs from a fresh one: a painting input is missing from the key"
            );
        }
        #[cfg(not(debug_assertions))]
        let _ = checked;
        let which = if kept {
            Highlights::Hovered
        } else {
            Highlights::All
        };
        let args = (frame.painter, frame.rect, frame.style, frame.document);
        self.paint_highlights(args, frame.space, frame.frozen, selection, hover, which);
    }

    /// Whether the kept image (and kept highlight) are what painting now
    /// would give.
    #[cfg(debug_assertions)]
    fn image_matches_fresh(&self, frame: &Frame<'_>, selection: &BTreeSet<EntityId>) -> bool {
        let total = crate::layer_cache::chunk_count(frame.document.objects().len());
        let pieces: Vec<Piece> = (0..total).map(|n| self.make_chunk(frame, n, &[])).collect();
        let geometry = self.keep_plan(frame).geometry;
        let kept_selection = self.cache(frame).has_any_selected();
        let selected = kept_selection.then(|| self.make_selected(frame, selection));
        self.cache(frame).bitmap_matches(&pieces, geometry)
            && selected.is_none_or(|fresh| self.cache(frame).selected_matches(&fresh, geometry))
    }
}
