//! Painting a space through its kept layer: the plain entities from kept
//! triangles, a new layer made at once or a few chunks a frame while the old
//! one is shown moved, and the highlights live over it. Each place the drawing
//! is painted (the model tab, a viewport, the sheet) has its own layer, a
//! [`Slot`]. What is kept and why is in `layer_cache`.

use std::cell::RefMut;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::entity_paint::Highlights;
use super::*;
use crate::layer_cache::{
    camera_of, capture, chunk_count, covered, draw_moved, move_between, selection_hash, tessellate,
    Advance, Inputs, LayerCache, LayerKey, Motion, Piece, SelectedKey, Slot, AUTO_SMOOTH_MS,
    CHUNK_OBJECTS, KEEP_FROM,
};
use egui::Painter;
use opendraft_core::{Bounds, Object};

/// A layer that took no longer than this to make is made in one frame once the
/// view has settled, not a slice at a time.
const AT_ONCE_US: u64 = 150_000;

/// The old picture is shown moved only while it covers this part of the screen.
const MIN_COVERED: f32 = 0.5;

/// Whether a kept layer drawn for the `hits`th time is checked against a fresh
/// one: every 256th time (every time in tests), in debug builds.
#[cfg(debug_assertions)]
fn verify_due(hits: u64) -> bool {
    hits.is_multiple_of(if cfg!(test) { 1 } else { 256 })
}

/// The objects of chunk `n` that show in `space` through `visible`, leaving
/// out the layers in `frozen`, in draw order.
fn chunk_objects<'d>(
    document: &'d Document,
    n: usize,
    space: Option<LayoutId>,
    frozen: &[LayerId],
    visible: &Bounds,
) -> Vec<&'d Object> {
    let all = document.objects();
    let from = n * CHUNK_OBJECTS;
    let Some(range) = all.get(from..(from + CHUNK_OBJECTS).min(all.len())) else {
        return Vec::new();
    };
    let shows = |o: &&Object| {
        o.bounds().intersects(visible)
            && o.layout == space
            && document.is_visible(o)
            && !frozen.contains(&o.attrs.layer)
    };
    range.iter().filter(shows).collect()
}

/// What a frame of one slot is painted with, besides the view: the places to
/// paint, what to show, and the camera's view of it.
#[derive(Clone, Copy)]
struct Frame<'a> {
    painter: &'a Painter,
    rect: Rect,
    style: &'a CanvasStyle,
    document: &'a Document,
    slot: Slot,
    space: Option<LayoutId>,
    frozen: &'a [LayerId],
    visible: Bounds,
    /// The egui pass, to tell slots that are in use from those that are not.
    pass: u64,
}

impl View {
    /// The entities of the space being edited in the model tab.
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
        let slot = Slot::Model;
        let args = (painter, rect, style, document);
        self.paint_space(slot, args, space, &[], selection, hover);
    }

    /// The entities of `space` but those on `frozen` layers, through the kept
    /// layer of `slot` when the app has said when the drawing changes and the
    /// settings want it, else directly. The camera is the one to paint
    /// through now.
    pub(super) fn paint_space(
        &self,
        slot: Slot,
        (painter, rect, style, document): (&Painter, Rect, &CanvasStyle, &Document),
        space: Option<LayoutId>,
        frozen: &[LayerId],
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
    ) {
        let direct = || {
            self.paint_entities(
                painter, rect, style, document, space, frozen, selection, hover,
            );
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
            frozen,
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
            slot,
            space,
            frozen,
            visible: self.camera.visible_world(rect),
            pass: ctx.cumulative_pass_nr(),
        };
        let have = self.cache(&frame).has(&key);
        let mut settled = have;
        if have {
            self.cache(&frame).view_reached();
        } else if self.shows_moved(&frame, &key) {
            match self.advance_pending(&frame, &key) {
                Advance::Done => settled = true,
                Advance::NoRoom => {}
                Advance::Waiting(_) | Advance::Going => {
                    return self.paint_moved(&frame, selection, hover);
                }
            }
        }
        if !settled {
            self.build_now(&frame, key.clone());
        }
        // Each chunk holding a selected or hovered entity is drawn without it.
        let hovered = hover.filter(|h| !selection.contains(h));
        let large = selection.len() >= KEEP_FROM;
        let hash = if large { selection_hash(selection) } else { 0 };
        let exclusions = {
            let highlighted = selection.iter().copied().chain(hovered);
            let mut cache = self.cache(&frame);
            if large {
                let position = |id| document.position(id);
                cache.exclusions_for((hash, hovered), highlighted, position)
            } else {
                Arc::new(cache.exclusions(highlighted, |id| document.position(id)))
            }
        };
        let missing = self.cache(&frame).missing(&exclusions);
        let limit = self.limit_for(&frame);
        for want in missing {
            let piece = self.make_chunk(&frame, want.chunk, &want.excluded);
            self.cache(&frame).set_variant(want, piece, limit);
        }
        if !self.cache(&frame).draw(painter, &exclusions) {
            return direct();
        }
        // A large selection is drawn from a kept picture, a small one live.
        let kept = large && self.draw_kept_selection(&frame, &key, hash, selection);
        if !large {
            self.cache(&frame).drop_selected();
        }
        let which = if kept {
            Highlights::Hovered
        } else {
            Highlights::All
        };
        #[cfg(debug_assertions)]
        if have && verify_due(self.cache(&frame).stats().hits) {
            debug_assert!(
                self.layer_matches_fresh(&frame, selection),
                "the kept layer differs from a fresh one: a painting input is missing from the key"
            );
        }
        let highlighted = (painter, rect, style, document);
        self.paint_highlights(highlighted, space, frozen, selection, hover, which);
    }

    /// The kept layer of the frame's slot.
    fn cache(&self, frame: &Frame<'_>) -> RefMut<'_, LayerCache> {
        let (slot, pass) = (frame.slot, frame.pass);
        RefMut::map(self.layer_cache.borrow_mut(), |all| all.slot(slot, pass))
    }

    /// The memory this slot may use: what the limit leaves after the others.
    fn limit_for(&self, frame: &Frame<'_>) -> usize {
        let others = self.layer_cache.borrow().bytes_elsewhere(frame.slot);
        self.cache_config.memory_limit.saturating_sub(others)
    }

    /// Whether, the kept layer being for another view, the old one is to be
    /// shown moved until a new one is made.
    fn shows_moved(&self, frame: &Frame<'_>, key: &LayerKey) -> bool {
        let cache = self.cache(frame);
        if !cache.has_content_of(key) {
            return false;
        }
        let smooth = match self.cache_config.motion {
            Motion::Exact => false,
            Motion::Smooth => true,
            Motion::Automatic => cache.stats().last_build_us > AUTO_SMOOTH_MS * 1000,
        };
        // An old picture that fills little of the screen (after a zoom out)
        // shows the drawing as missing, which is worse than waiting.
        let by = cache
            .built_view()
            .map(|from| move_between(&camera_of(from), &self.camera, frame.rect));
        smooth && by.is_some_and(|by| covered(by, frame.rect) >= MIN_COVERED)
    }

    /// Make a layer for `key` now, in this frame.
    fn build_now(&self, frame: &Frame<'_>, key: LayerKey) {
        let started = Instant::now();
        self.cache(frame).drop_pending();
        let total = chunk_count(frame.document.objects().len());
        let pieces = (0..total).map(|n| self.make_chunk(frame, n, &[])).collect();
        let limit = self.limit_for(frame);
        let mut cache = self.cache(frame);
        cache.store(key, pieces, limit);
        cache.note_build_time(started.elapsed());
    }

    /// Do a slice of the work of the layer for `key`, once the view has stood
    /// still for the settle delay. All the slots of a frame share one slice.
    fn advance_pending(&self, frame: &Frame<'_>, key: &LayerKey) -> Advance {
        let cfg = self.cache_config;
        let ctx = frame.painter.ctx();
        let still = self.cache(frame).stood_still(key.view(), Instant::now());
        let settle = Duration::from_millis(cfg.settle_ms.into());
        if still < settle {
            ctx.request_repaint_after(settle - still);
            return Advance::Waiting(settle - still);
        }
        let total = chunk_count(frame.document.objects().len());
        self.cache(frame).begin_pending(key, total);
        let limit = self.limit_for(frame);
        let started = Instant::now();
        // Once the view is still, a layer known to be quick to make is made
        // in one go: a hitch nobody is moving through beats many slow frames.
        // (A slice of 0 keeps to one chunk a frame, for tests.)
        let quick = self.cache(frame).stats().last_build_us <= AT_ONCE_US;
        let budget = match cfg.slice_ms {
            0 => Duration::ZERO,
            _ if quick => Duration::MAX,
            ms => Duration::from_millis(ms.into()),
        };
        let mut done = Advance::Going;
        loop {
            // A statement of its own, so the borrow ends before the push.
            let next = self.cache(frame).pending_next();
            let Some(n) = next else {
                break;
            };
            let piece = self.make_chunk(frame, n, &[]);
            if self.cache(frame).pending_push(piece, limit) == Advance::NoRoom {
                done = Advance::NoRoom;
                break;
            }
            // At least one chunk a frame, so a slot is never starved.
            if self.slice_spent.get() + started.elapsed() >= budget {
                break;
            }
        }
        self.slice_spent
            .set(self.slice_spent.get() + started.elapsed());
        if done == Advance::NoRoom {
            return done;
        }
        if self.cache(frame).finish_pending(limit) {
            self.cache(frame).note_build_time(started.elapsed());
            return Advance::Done;
        }
        ctx.request_repaint();
        Advance::Going
    }

    /// The old layer moved to the camera's view, with the highlights over it.
    fn paint_moved(&self, f: &Frame<'_>, selection: &BTreeSet<EntityId>, hover: Option<EntityId>) {
        let (meshes, from) = {
            let cache = self.cache(f);
            (cache.base_meshes(), cache.built_view())
        };
        if let (Some(meshes), Some(from)) = (meshes, from) {
            let by = move_between(&camera_of(from), &self.camera, f.rect);
            draw_moved(f.painter, &meshes, by);
            self.cache(f).note_moved();
        }
        let args = (f.painter, f.rect, f.style, f.document);
        self.paint_highlighted(args, f.space, f.frozen, selection, hover);
    }

    /// Chunk `n` of the plain entities, but those in `excluded` (sorted), as
    /// triangles with the textures they use.
    fn make_chunk(&self, f: &Frame<'_>, n: usize, excluded: &[EntityId]) -> Piece {
        let objects = chunk_objects(f.document, n, f.space, f.frozen, &f.visible);
        if objects.is_empty() {
            return Piece::default();
        }
        let ctx = f.painter.ctx();
        let none = BTreeSet::new();
        let (shapes, textures) = capture(ctx, f.painter.clip_rect(), |p| {
            for object in &objects {
                if excluded.binary_search(&object.id).is_err() {
                    let (rect, style, document) = (f.rect, f.style, f.document);
                    let frozen = f.frozen;
                    self.paint_object(p, rect, style, document, object, frozen, &none, None);
                }
            }
        });
        Piece::new(tessellate(ctx, shapes), textures)
    }

    /// Draw the highlight of the selection from a kept picture, making it
    /// first if there is none for this layer and selection; false when it
    /// cannot be kept (the memory limit), and the highlight is to be drawn live.
    fn draw_kept_selection(
        &self,
        frame: &Frame<'_>,
        key: &LayerKey,
        hash: u64,
        selection: &BTreeSet<EntityId>,
    ) -> bool {
        let colours = [frame.style.selected, frame.style.selected_glow];
        let kept_key = SelectedKey::new(key.clone(), hash, colours);
        if !self.cache(frame).has_selected(&kept_key) {
            let piece = self.make_selected(frame, selection);
            let limit = self.limit_for(frame);
            if !self.cache(frame).set_selected(kept_key, piece, limit) {
                return false;
            }
        }
        self.cache(frame).draw_selected(frame.painter)
    }

    /// The highlight of `selection` as triangles.
    fn make_selected(&self, f: &Frame<'_>, selection: &BTreeSet<EntityId>) -> Piece {
        let ctx = f.painter.ctx();
        let (shapes, textures) = capture(ctx, f.painter.clip_rect(), |p| {
            let args = (p, f.rect, f.style, f.document);
            let which = Highlights::Selected;
            self.paint_highlights(args, f.space, f.frozen, selection, None, which);
        });
        Piece::new(tessellate(ctx, shapes), textures)
    }

    /// Whether the kept chunks and variants are what painting now would give.
    #[cfg(debug_assertions)]
    fn layer_matches_fresh(&self, frame: &Frame<'_>, selection: &BTreeSet<EntityId>) -> bool {
        let chunks = self.cache(frame).chunks();
        let whole = (0..chunks).all(|n| {
            let fresh = self.make_chunk(frame, n, &[]);
            self.cache(frame).matches(n, None, &fresh)
        });
        let kept_selection = self.cache(frame).has_any_selected();
        let selected = kept_selection.then(|| self.make_selected(frame, selection));
        let variants = self.cache(frame).variants();
        whole
            && selected.is_none_or(|fresh| self.cache(frame).selected_matches(&fresh))
            && variants.iter().all(|v| {
                let fresh = self.make_chunk(frame, v.chunk, &v.excluded);
                self.cache(frame)
                    .matches(v.chunk, Some(&v.excluded), &fresh)
            })
    }
}
