//! The kept layer: the plain entities in draw order, in chunks, each chunk a
//! few meshes. A chunk that holds a selected or hovered entity is drawn from a
//! variant made without it, so the highlight is drawn over nothing.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::epaint::{Mesh, Vertex};
use egui::{Painter, Shape, TextureHandle};
use opendraft_core::EntityId;

#[cfg(debug_assertions)]
use super::same_triangles;
use super::{CacheStats, LayerKey};

/// Entities in a chunk. Small enough that rebuilding the chunk under the
/// pointer takes well under a millisecond, large enough that a drawing is a
/// few hundred chunks, not thousands. Tests use small chunks to get several.
pub(crate) const CHUNK_OBJECTS: usize = if cfg!(test) { 64 } else { 2048 };

/// Some meshes, with the textures they point at so that those outlive the
/// picture cache of small text.
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

    #[cfg(debug_assertions)]
    fn matches(&self, fresh: &Piece) -> bool {
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

struct Chunk {
    /// Its entities, in draw order.
    ids: Vec<EntityId>,
    base: Piece,
    variant: Option<Variant>,
}

struct Layer {
    chunks: Vec<Chunk>,
    /// The drawing-order position of each chunk's first entity, ascending:
    /// an entity belongs to the last chunk that starts at or before it.
    starts: Vec<usize>,
}

/// A chunk as built: the drawing-order position of its first entity, its
/// entities, and their picture.
pub(crate) type Built = (usize, Vec<EntityId>, Piece);

/// What to leave out of each chunk, by chunk: the highlighted entities in it,
/// sorted.
pub(crate) type Exclusions = BTreeMap<usize, Vec<EntityId>>;

/// A chunk's entities and what to leave out, for making its variant.
pub(crate) struct Wanted {
    pub chunk: usize,
    pub ids: Vec<EntityId>,
    pub excluded: Vec<EntityId>,
}

/// The kept layer, if there is one for `key`.
#[derive(Default)]
pub(crate) struct LayerCache {
    key: Option<LayerKey>,
    /// `None` with a key: it was too big to keep, so do not try again.
    layer: Option<Layer>,
    stats: CacheStats,
}

impl LayerCache {
    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    /// Whether the cache was made for `key` (kept or too big to keep).
    pub fn has(&self, key: &LayerKey) -> bool {
        self.key.as_ref() == Some(key)
    }

    pub fn clear(&mut self) {
        self.key = None;
        self.layer = None;
        self.stats.bytes = 0;
    }

    /// Keep `chunks` (each the position of its first entity, its entities and
    /// its picture) as the layer for `key`, unless they are over `limit`.
    pub fn store(&mut self, key: LayerKey, chunks: Vec<Built>, limit: usize) {
        let bytes: usize = chunks.iter().map(|(_, _, p)| p.bytes).sum();
        self.stats.builds += 1;
        self.key = Some(key);
        if bytes > limit {
            self.layer = None;
            self.stats.bytes = 0;
            return;
        }
        let starts = chunks.iter().map(|(at, _, _)| *at).collect();
        let chunks = chunks
            .into_iter()
            .map(|(_, ids, base)| Chunk {
                ids,
                base,
                variant: None,
            })
            .collect();
        self.stats.bytes = bytes;
        self.layer = Some(Layer { chunks, starts });
    }

    /// For each chunk holding some of `highlighted`, those, sorted. `position`
    /// says where in the drawing order an entity is; one that comes before the
    /// first chunk, or is not there, is in none. One that was not in the layer
    /// but falls inside a chunk's range is left out of it to no effect.
    pub fn exclusions(
        &self,
        highlighted: impl Iterator<Item = EntityId>,
        position: impl Fn(EntityId) -> Option<usize>,
    ) -> Exclusions {
        let mut out = Exclusions::new();
        let Some(layer) = &self.layer else {
            return out;
        };
        for id in highlighted {
            let Some(at) = position(id) else { continue };
            let after = layer.starts.partition_point(|&start| start <= at);
            if let Some(chunk) = after.checked_sub(1) {
                out.entry(chunk).or_default().push(id);
            }
        }
        out.values_mut().for_each(|v| v.sort_unstable());
        out
    }

    /// The chunks that have no variant for `exclusions` yet.
    pub fn missing(&self, exclusions: &Exclusions) -> Vec<Wanted> {
        let Some(layer) = &self.layer else {
            return Vec::new();
        };
        exclusions
            .iter()
            .filter(|(&n, excluded)| {
                layer.chunks[n]
                    .variant
                    .as_ref()
                    .is_none_or(|v| &v.excluded != *excluded)
            })
            .map(|(&n, excluded)| Wanted {
                chunk: n,
                ids: layer.chunks[n].ids.clone(),
                excluded: excluded.clone(),
            })
            .collect()
    }

    /// Keep a variant, unless the layer would then be over `limit`; the chunk
    /// is then drawn whole, as before the highlight.
    pub fn set_variant(&mut self, wanted: Wanted, piece: Piece, limit: usize) {
        let Some(layer) = &mut self.layer else {
            return;
        };
        self.stats.variant_builds += 1;
        let chunk = &mut layer.chunks[wanted.chunk];
        let freed = chunk.variant.take().map_or(0, |v| v.piece.bytes);
        self.stats.bytes -= freed;
        if self.stats.bytes + piece.bytes > limit {
            return;
        }
        self.stats.bytes += piece.bytes;
        chunk.variant = Some(Variant {
            excluded: wanted.excluded,
            piece,
        });
    }

    /// Draw the layer, each chunk without what `exclusions` leaves out of it;
    /// false when there is none (too big to keep).
    pub fn draw(&mut self, painter: &Painter, exclusions: &Exclusions) -> bool {
        let Some(layer) = &mut self.layer else {
            return false;
        };
        self.stats.hits += 1;
        for (n, chunk) in layer.chunks.iter_mut().enumerate() {
            let wanted = exclusions.get(&n);
            if wanted.is_none() {
                if let Some(v) = chunk.variant.take() {
                    self.stats.bytes -= v.piece.bytes;
                }
            }
            let piece = match (&chunk.variant, wanted) {
                (Some(v), Some(w)) if &v.excluded == w => &v.piece,
                _ => &chunk.base,
            };
            for mesh in &piece.meshes {
                painter.add(Shape::Mesh(mesh.clone()));
            }
        }
        true
    }

    /// Every chunk's entities, for checking the layer against a fresh one.
    #[cfg(debug_assertions)]
    pub fn chunk_ids(&self) -> Vec<Vec<EntityId>> {
        self.layer
            .iter()
            .flat_map(|l| l.chunks.iter().map(|c| c.ids.clone()))
            .collect()
    }

    /// The variants kept, as what they leave out, for the same check.
    #[cfg(debug_assertions)]
    pub fn variants(&self) -> Vec<Wanted> {
        let Some(layer) = &self.layer else {
            return Vec::new();
        };
        let held = layer.chunks.iter().enumerate();
        held.filter_map(|(n, c)| {
            c.variant.as_ref().map(|v| Wanted {
                chunk: n,
                ids: c.ids.clone(),
                excluded: v.excluded.clone(),
            })
        })
        .collect()
    }

    /// Whether chunk `n` as kept is `fresh`; with `excluded`, its variant.
    #[cfg(debug_assertions)]
    pub fn matches(&self, n: usize, excluded: Option<&[EntityId]>, fresh: &Piece) -> bool {
        let Some(chunk) = self.layer.as_ref().and_then(|l| l.chunks.get(n)) else {
            return false;
        };
        match (excluded, &chunk.variant) {
            (None, _) => chunk.base.matches(fresh),
            (Some(e), Some(v)) => v.excluded == e && v.piece.matches(fresh),
            (Some(_), None) => false,
        }
    }
}
