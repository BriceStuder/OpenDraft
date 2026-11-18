//! The kept layer: the plain entities in chunks, each chunk a few meshes. A
//! chunk is a fixed range of the drawing's object list ([`CHUNK_OBJECTS`]
//! objects), so which chunk an object is in follows from its position and a
//! chunk can be made again alone. A chunk that holds a selected or hovered
//! entity is drawn from a variant made without it, so the highlight is drawn
//! over nothing.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use egui::epaint::{Mesh, Vertex};
use egui::{Painter, Shape, TextureHandle};
use opendraft_core::EntityId;

use super::key::View;
use super::pending::Pending;
#[cfg(debug_assertions)]
use super::same_triangles;
use super::{CacheStats, LayerKey};

/// Objects of the drawing in a chunk. Small enough that making the chunk
/// under the pointer again takes well under a millisecond, and a slice of a
/// rebuild is a few chunks; large enough that a drawing is a few hundred
/// chunks. Tests use small chunks to get several.
pub(crate) const CHUNK_OBJECTS: usize = if cfg!(test) { 64 } else { 2048 };

/// How many chunks a drawing of `objects` objects has.
pub(crate) fn chunk_count(objects: usize) -> usize {
    objects.div_ceil(CHUNK_OBJECTS)
}

/// Some meshes, with the textures they point at so that those outlive the
/// picture cache of small text.
#[derive(Default)]
pub(crate) struct Piece {
    meshes: Vec<Arc<Mesh>>,
    _textures: Vec<TextureHandle>,
    bytes: usize,
}

impl Piece {
    pub fn new(meshes: Vec<Mesh>, textures: Vec<TextureHandle>) -> Self {
        let bytes = meshes
            .iter()
            .map(|m| m.vertices.len() * size_of::<Vertex>() + m.indices.len() * size_of::<u32>())
            .sum();
        Self {
            meshes: meshes.into_iter().map(Arc::new).collect(),
            _textures: textures,
            bytes,
        }
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn meshes(&self) -> &[Arc<Mesh>] {
        &self.meshes
    }

    pub fn textures(&self) -> &[TextureHandle] {
        &self._textures
    }

    #[cfg(debug_assertions)]
    pub(super) fn matches(&self, fresh: &Piece) -> bool {
        same_triangles(
            self.meshes.iter().map(|m| &**m),
            fresh.meshes.iter().map(|m| &**m),
        )
    }
}

/// A chunk made without the entities in `excluded` (sorted).
struct Variant {
    excluded: Vec<EntityId>,
    piece: Piece,
}

pub(super) struct Chunk {
    base: Piece,
    variant: Option<Variant>,
}

/// What to leave out of each chunk, by chunk: the highlighted entities in it,
/// sorted.
pub(crate) type Exclusions = BTreeMap<usize, Vec<EntityId>>;

/// A chunk to make a variant of.
pub(crate) struct Wanted {
    pub chunk: usize,
    pub excluded: Vec<EntityId>,
}

/// The kept layer, if there is one for `key`.
#[derive(Default)]
pub(crate) struct LayerCache {
    pub(super) key: Option<LayerKey>,
    /// `None` with a key: it was too big to keep, so do not try again.
    pub(super) layer: Option<Vec<Chunk>>,
    /// The next layer, being made while this one is drawn moved.
    pub(super) pending: Option<Pending>,
    /// The view last seen, and since when it has not changed.
    pub(super) settle: Option<(View, Instant)>,
    pub(super) stats: CacheStats,
    /// The highlight of a large selection (see `selected.rs`).
    pub(super) selected: Option<super::selected::SelectedLayer>,
    /// What the last large selection and hover left out of each chunk.
    pub(super) excluded: Option<(u64, Option<EntityId>, Arc<Exclusions>)>,
    /// The layer as an image, when it is kept as one (see `bitmap.rs`).
    pub(super) bitmap: Option<super::bitmap::Bitmap>,
}

impl LayerCache {
    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    /// Whether the cache was made for `key` (kept or too big to keep).
    pub fn has(&self, key: &LayerKey) -> bool {
        self.key.as_ref() == Some(key)
    }

    /// Whether a layer is kept (not too big) that was made from the same
    /// drawing and settings as `key`, though perhaps for another view.
    pub fn has_content_of(&self, key: &LayerKey) -> bool {
        self.layer.is_some() && self.key.as_ref().is_some_and(|k| k.same_content(key))
    }

    pub fn clear(&mut self) {
        self.key = None;
        self.layer = None;
        self.pending = None;
        self.selected = None;
        self.excluded = None;
        self.bitmap = None;
        self.stats.bytes = 0;
        self.stats.rebuilding = None;
    }

    /// The kept layer is for the view being drawn: nothing is being made.
    pub fn view_reached(&mut self) {
        self.pending = None;
        self.settle = None;
        self.stats.rebuilding = None;
    }

    /// Note how long a layer took to make, for Automatic motion.
    pub fn note_build_time(&mut self, took: std::time::Duration) {
        self.stats.last_build_us = took.as_micros().try_into().unwrap_or(u64::MAX);
    }

    /// Note a frame drawn from the old layer, moved.
    pub fn note_moved(&mut self) {
        self.stats.moved += 1;
    }

    /// The view the kept layer was made for.
    pub fn built_view(&self) -> Option<View> {
        self.key.as_ref().map(LayerKey::view)
    }

    /// Keep `pieces` (one per chunk) as the layer for `key`, unless they are
    /// over `limit`.
    pub fn store(&mut self, key: LayerKey, pieces: Vec<Piece>, limit: usize) {
        let bytes: usize = pieces.iter().map(Piece::bytes).sum();
        self.stats.builds += 1;
        self.key = Some(key);
        self.selected = None;
        self.excluded = None;
        self.bitmap = None;
        if bytes > limit {
            self.layer = None;
            self.stats.bytes = 0;
            return;
        }
        self.stats.bytes = bytes;
        self.layer = Some(
            pieces
                .into_iter()
                .map(|base| Chunk {
                    base,
                    variant: None,
                })
                .collect(),
        );
    }

    /// For each chunk holding some of `highlighted`, those, sorted. `position`
    /// says where in the drawing order an entity is; one that is not there
    /// is in none.
    pub fn exclusions(
        &self,
        highlighted: impl Iterator<Item = EntityId>,
        position: impl Fn(EntityId) -> Option<usize>,
    ) -> Exclusions {
        let mut out = Exclusions::new();
        let Some(chunks) = &self.layer else {
            return out;
        };
        for id in highlighted {
            let Some(at) = position(id) else { continue };
            if at / CHUNK_OBJECTS < chunks.len() {
                out.entry(at / CHUNK_OBJECTS).or_default().push(id);
            }
        }
        out.values_mut().for_each(|v| v.sort_unstable());
        out
    }

    /// The chunks that have no variant for `exclusions` yet.
    pub fn missing(&self, exclusions: &Exclusions) -> Vec<Wanted> {
        let Some(chunks) = &self.layer else {
            return Vec::new();
        };
        exclusions
            .iter()
            .filter(|(&n, excluded)| {
                chunks[n]
                    .variant
                    .as_ref()
                    .is_none_or(|v| &v.excluded != *excluded)
            })
            .map(|(&n, excluded)| Wanted {
                chunk: n,
                excluded: excluded.clone(),
            })
            .collect()
    }

    /// Keep a variant, unless the layer would then be over `limit`; the chunk
    /// is then drawn whole, as before the highlight.
    pub fn set_variant(&mut self, wanted: Wanted, piece: Piece, limit: usize) {
        let Some(chunks) = &mut self.layer else {
            return;
        };
        self.stats.variant_builds += 1;
        let chunk = &mut chunks[wanted.chunk];
        let freed = chunk.variant.take().map_or(0, |v| v.piece.bytes());
        self.stats.bytes -= freed;
        if self.stats.bytes + piece.bytes() > limit {
            return;
        }
        self.stats.bytes += piece.bytes();
        chunk.variant = Some(Variant {
            excluded: wanted.excluded,
            piece,
        });
    }

    /// Draw the layer, each chunk without what `exclusions` leaves out of it;
    /// false when there is none (too big to keep).
    pub fn draw(&mut self, painter: &Painter, exclusions: &Exclusions) -> bool {
        let Some(chunks) = &mut self.layer else {
            return false;
        };
        self.stats.hits += 1;
        for (n, chunk) in chunks.iter_mut().enumerate() {
            let wanted = exclusions.get(&n);
            if wanted.is_none() {
                if let Some(v) = chunk.variant.take() {
                    self.stats.bytes -= v.piece.bytes();
                }
            }
            let piece = match (&chunk.variant, wanted) {
                (Some(v), Some(w)) if &v.excluded == w => &v.piece,
                _ => &chunk.base,
            };
            for mesh in piece.meshes() {
                painter.add(Shape::Mesh(mesh.clone()));
            }
        }
        true
    }

    /// The meshes of the whole layer as kept, without variants, to be drawn
    /// moved (see `moved.rs`); none when there is no layer.
    pub fn base_meshes(&self) -> Option<Vec<Arc<Mesh>>> {
        let chunks = self.layer.as_ref()?;
        let all = chunks.iter().flat_map(|c| c.base.meshes().iter().cloned());
        Some(all.collect())
    }

    /// How many chunks the kept layer has.
    #[cfg(debug_assertions)]
    pub fn chunks(&self) -> usize {
        self.layer.as_ref().map_or(0, Vec::len)
    }

    /// The variants kept, as what they leave out, for checking against fresh.
    #[cfg(debug_assertions)]
    pub fn variants(&self) -> Vec<Wanted> {
        let held = self.layer.iter().flatten().enumerate();
        held.filter_map(|(n, c)| {
            c.variant.as_ref().map(|v| Wanted {
                chunk: n,
                excluded: v.excluded.clone(),
            })
        })
        .collect()
    }

    /// Whether chunk `n` as kept is `fresh`; with `excluded`, its variant.
    #[cfg(debug_assertions)]
    pub fn matches(&self, n: usize, excluded: Option<&[EntityId]>, fresh: &Piece) -> bool {
        let Some(chunk) = self.layer.as_ref().and_then(|l| l.get(n)) else {
            return false;
        };
        match (excluded, &chunk.variant) {
            (None, _) => chunk.base.matches(fresh),
            (Some(e), Some(v)) => v.excluded == e && v.piece.matches(fresh),
            (Some(_), None) => false,
        }
    }
}
