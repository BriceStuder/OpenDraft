//! The highlight of a large selection, kept as triangles like the layer under
//! it. Drawing the highlight of each selected entity again in every frame, and
//! having egui turn it into triangles again, costs about 0.5 ms per thousand
//! entities: a window selection of a big drawing made every frame slow. The
//! kept picture is for one layer key (so one view), one selection and the two
//! colours the highlight is drawn in; a hovered entity is still drawn live.

use std::collections::BTreeSet;
use std::sync::Arc;

use egui::{Color32, Painter, Shape};
use opendraft_core::EntityId;

use super::chunks::{Exclusions, LayerCache, Piece};
use super::LayerKey;

/// Selections of at least this many entities are kept as a picture; smaller
/// ones cost less to draw than to keep.
pub(crate) const KEEP_FROM: usize = if cfg!(test) { 8 } else { 256 };

/// A number that is the same for the same set of ids and, but for a chance of
/// one in 2^64, differs for another. Cheap: one multiply for each id.
pub fn selection_hash(selection: &BTreeSet<EntityId>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64 ^ selection.len() as u64;
    for id in selection {
        h = (h ^ id.0).wrapping_mul(0x0000_0100_0000_01b3);
        h ^= h >> 29;
    }
    h
}

/// What a kept highlight was made for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SelectedKey {
    layer: LayerKey,
    hash: u64,
    /// The selection colour and its glow.
    colours: [Color32; 2],
}

impl SelectedKey {
    pub fn new(layer: LayerKey, hash: u64, colours: [Color32; 2]) -> Self {
        Self {
            layer,
            hash,
            colours,
        }
    }
}

pub(super) struct SelectedLayer {
    key: SelectedKey,
    piece: Piece,
}

impl LayerCache {
    pub fn has_selected(&self, key: &SelectedKey) -> bool {
        self.selected.as_ref().is_some_and(|s| &s.key == key)
    }

    /// Keep `piece` as the highlight for `key`, unless the cache would then
    /// be over `limit`; whether it was kept.
    pub fn set_selected(&mut self, key: SelectedKey, piece: Piece, limit: usize) -> bool {
        self.drop_selected();
        self.stats.selected_builds += 1;
        if self.stats.bytes + piece.bytes() > limit {
            return false;
        }
        self.stats.bytes += piece.bytes();
        self.selected = Some(SelectedLayer { key, piece });
        true
    }

    pub fn drop_selected(&mut self) {
        if let Some(old) = self.selected.take() {
            self.stats.bytes = self.stats.bytes.saturating_sub(old.piece.bytes());
        }
    }

    /// Draw the kept highlight; false when there is none.
    pub fn draw_selected(&self, painter: &Painter) -> bool {
        let Some(kept) = &self.selected else {
            return false;
        };
        for mesh in kept.piece.meshes() {
            painter.add(Shape::Mesh(mesh.clone()));
        }
        true
    }

    /// [`exclusions`](Self::exclusions) for a large selection, worked out
    /// again only when the selection (`hash`) or the hovered entity changes.
    pub fn exclusions_for(
        &mut self,
        (hash, hover): (u64, Option<EntityId>),
        highlighted: impl Iterator<Item = EntityId>,
        position: impl Fn(EntityId) -> Option<usize>,
    ) -> Arc<Exclusions> {
        if let Some((h, v, kept)) = &self.excluded {
            if (*h, *v) == (hash, hover) {
                return kept.clone();
            }
        }
        let made = Arc::new(self.exclusions(highlighted, position));
        self.excluded = Some((hash, hover, made.clone()));
        made
    }

    /// Whether a highlight is kept.
    #[cfg(debug_assertions)]
    pub fn has_any_selected(&self) -> bool {
        self.selected.is_some()
    }

    /// Whether the kept highlight is `fresh`.
    #[cfg(debug_assertions)]
    pub fn selected_matches(&self, fresh: &Piece) -> bool {
        self.selected
            .as_ref()
            .is_none_or(|s| s.piece.matches(fresh))
    }
}
