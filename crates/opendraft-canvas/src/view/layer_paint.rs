//! Painting the model tab through the kept layer: the plain entities from kept
//! triangles, a new layer made at once or a few chunks a frame while the old
//! one is shown moved, and the highlights live over it. What is kept and why
//! is in `layer_cache`.

use std::time::{Duration, Instant};

use super::*;
use crate::layer_cache::{
    camera_of, capture, chunk_count, draw_moved, move_between, tessellate, Advance, Inputs,
    LayerKey, Motion, Piece, AUTO_SMOOTH_MS, CHUNK_OBJECTS,
};
use egui::Painter;
use opendraft_core::{Bounds, Object};

/// Whether a kept layer drawn for the `hits`th time is checked against a fresh
/// one: every 256th time (every time in tests), in debug builds.
#[cfg(debug_assertions)]
fn verify_due(hits: u64) -> bool {
    hits.is_multiple_of(if cfg!(test) { 1 } else { 256 })
}

/// The objects of chunk `n` that show in `space` through `visible`, in draw
/// order.
fn chunk_objects<'d>(
    document: &'d Document,
    n: usize,
    space: Option<LayoutId>,
    visible: &Bounds,
) -> Vec<&'d Object> {
    let all = document.objects();
    let from = n * CHUNK_OBJECTS;
    let Some(range) = all.get(from..(from + CHUNK_OBJECTS).min(all.len())) else {
        return Vec::new();
    };
    let shows =
        |o: &&Object| o.bounds().intersects(visible) && o.layout == space && document.is_visible(o);
    range.iter().filter(shows).collect()
}

/// What a frame is drawn with, besides the document: the places to paint, the
/// camera's view and the space.
#[derive(Clone, Copy)]
struct Frame<'a> {
    painter: &'a Painter,
    rect: Rect,
    style: &'a CanvasStyle,
    document: &'a Document,
    space: Option<LayoutId>,
    visible: Bounds,
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
        let cfg = self.cache_config;
        let wanted = cfg.wants(document.objects().len());
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
        let frame = Frame {
            painter,
            rect,
            style,
            document,
            space,
            visible: self.camera.visible_world(rect),
        };
        let have = self.layer_cache.borrow().has(&key);
        let mut settled = have;
        if have {
            self.layer_cache.borrow_mut().view_reached();
        } else if self.shows_moved(&key) {
            match self.advance_pending(&frame, &key) {
                Advance::Done => settled = true,
                Advance::NoRoom => {}
                Advance::Waiting(_) | Advance::Going => {
                    return self.paint_moved(&frame, selection, hover);
                }
            }
        }
        if !settled {
            self.build_now(&frame, key);
        }
        // Each chunk holding a selected or hovered entity is drawn without it.
        let hovered = hover.filter(|h| !selection.contains(h));
        let exclusions = {
            let highlighted = selection.iter().copied().chain(hovered);
            self.layer_cache
                .borrow()
                .exclusions(highlighted, |id| document.position(id))
        };
        let missing = self.layer_cache.borrow().missing(&exclusions);
        for want in missing {
            let piece = self.make_chunk(&frame, want.chunk, &want.excluded);
            self.layer_cache
                .borrow_mut()
                .set_variant(want, piece, cfg.memory_limit);
        }
        if !self.layer_cache.borrow_mut().draw(painter, &exclusions) {
            return direct();
        }
        #[cfg(debug_assertions)]
        if have && verify_due(self.layer_cache.borrow().stats().hits) {
            debug_assert!(
                self.layer_matches_fresh(&frame),
                "the kept layer differs from a fresh one: a painting input is missing from the key"
            );
        }
        self.paint_highlighted(painter, rect, style, document, space, selection, hover);
    }

    /// Whether, the kept layer being for another view, the old one is to be
    /// shown moved until a new one is made.
    fn shows_moved(&self, key: &LayerKey) -> bool {
        let cache = self.layer_cache.borrow();
        if !cache.has_content_of(key) {
            return false;
        }
        match self.cache_config.motion {
            Motion::Exact => false,
            Motion::Smooth => true,
            Motion::Automatic => cache.stats().last_build_us > AUTO_SMOOTH_MS * 1000,
        }
    }

    /// Make a layer for `key` now, in this frame.
    fn build_now(&self, frame: &Frame<'_>, key: LayerKey) {
        let started = Instant::now();
        self.layer_cache.borrow_mut().drop_pending();
        let total = chunk_count(frame.document.objects().len());
        let pieces = (0..total).map(|n| self.make_chunk(frame, n, &[])).collect();
        let mut cache = self.layer_cache.borrow_mut();
        cache.store(key, pieces, self.cache_config.memory_limit);
        cache.note_build_time(started.elapsed());
    }

    /// Do a slice of the work of the layer for `key`, once the view has stood
    /// still for the settle delay.
    fn advance_pending(&self, frame: &Frame<'_>, key: &LayerKey) -> Advance {
        let cfg = self.cache_config;
        let ctx = frame.painter.ctx();
        let still = self
            .layer_cache
            .borrow_mut()
            .stood_still(key.view(), Instant::now());
        let settle = Duration::from_millis(cfg.settle_ms.into());
        if still < settle {
            ctx.request_repaint_after(settle - still);
            return Advance::Waiting(settle - still);
        }
        let total = chunk_count(frame.document.objects().len());
        self.layer_cache.borrow_mut().begin_pending(key, total);
        let started = Instant::now();
        let budget = Duration::from_millis(cfg.slice_ms.into());
        loop {
            // A statement of its own, so the borrow ends before the push.
            let next = self.layer_cache.borrow().pending_next();
            let Some(n) = next else {
                break;
            };
            let piece = self.make_chunk(frame, n, &[]);
            let step = self
                .layer_cache
                .borrow_mut()
                .pending_push(piece, cfg.memory_limit);
            if step == Advance::NoRoom {
                return step;
            }
            if started.elapsed() >= budget {
                break;
            }
        }
        if self
            .layer_cache
            .borrow_mut()
            .finish_pending(cfg.memory_limit)
        {
            self.layer_cache
                .borrow_mut()
                .note_build_time(started.elapsed());
            return Advance::Done;
        }
        ctx.request_repaint();
        Advance::Going
    }

    /// The old layer moved to the camera's view, with the highlights over it.
    fn paint_moved(&self, f: &Frame<'_>, selection: &BTreeSet<EntityId>, hover: Option<EntityId>) {
        let (meshes, from) = {
            let cache = self.layer_cache.borrow();
            (cache.base_meshes(), cache.built_view())
        };
        if let (Some(meshes), Some(from)) = (meshes, from) {
            let by = move_between(&camera_of(from), &self.camera, f.rect);
            draw_moved(f.painter, &meshes, by);
            self.layer_cache.borrow_mut().note_moved();
        }
        let (painter, rect, style, document) = (f.painter, f.rect, f.style, f.document);
        self.paint_highlighted(painter, rect, style, document, f.space, selection, hover);
    }

    /// Chunk `n` of the plain entities, but those in `excluded` (sorted), as
    /// triangles with the textures they use.
    fn make_chunk(&self, f: &Frame<'_>, n: usize, excluded: &[EntityId]) -> Piece {
        let objects = chunk_objects(f.document, n, f.space, &f.visible);
        if objects.is_empty() {
            return Piece::default();
        }
        let ctx = f.painter.ctx();
        let none = BTreeSet::new();
        let (shapes, textures) = capture(ctx, f.painter.clip_rect(), |p| {
            for object in &objects {
                if excluded.binary_search(&object.id).is_err() {
                    let (rect, style, document) = (f.rect, f.style, f.document);
                    self.paint_object(p, rect, style, document, object, &[], &none, None);
                }
            }
        });
        Piece::new(tessellate(ctx, shapes), textures)
    }

    /// Whether the kept chunks and variants are what painting now would give.
    #[cfg(debug_assertions)]
    fn layer_matches_fresh(&self, frame: &Frame<'_>) -> bool {
        let chunks = self.layer_cache.borrow().chunks();
        let whole = (0..chunks).all(|n| {
            let fresh = self.make_chunk(frame, n, &[]);
            self.layer_cache.borrow().matches(n, None, &fresh)
        });
        let variants = self.layer_cache.borrow().variants();
        whole
            && variants.iter().all(|v| {
                let fresh = self.make_chunk(frame, v.chunk, &v.excluded);
                self.layer_cache
                    .borrow()
                    .matches(v.chunk, Some(&v.excluded), &fresh)
            })
    }
}
