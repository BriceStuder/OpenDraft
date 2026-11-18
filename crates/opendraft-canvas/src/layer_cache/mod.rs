//! The plain entities of a drawing as ready-made triangles, kept between
//! frames.
//!
//! Painting is immediate: every frame turns every visible entity into egui
//! shapes and egui into triangles, even when only the pointer moved. This
//! keeps the triangles of everything that is neither selected nor hovered and
//! draws those instead, until something they depend on changes. Selected and
//! hovered entities are drawn live on top. The layer is kept in chunks of
//! entities in draw order (`chunks.rs`); the chunk holding a highlighted
//! entity is drawn from a variant made without it, kept while the highlight
//! stays, so a highlight is drawn over nothing and hovering rebuilds one small
//! chunk, not the layer.
//!
//! What they depend on is the [`LayerKey`]: the drawing (its session and
//! revision, see `Session::draw_token`), the space, the layers frozen in the
//! viewport, the camera, the canvas, the colours and the settings that change
//! the picture. The cache is reused only while the key is equal, and is only
//! used at all when the app has given the view a drawing token, so a caller
//! that cannot say when the drawing changes draws as before.
//!
//! Audit of what plain painting reads (keep in step when painting changes):
//! the document (token), `camera` (key), `hidden` (key), `point_style` (key),
//! `CanvasStyle::entity` (key), the painter's clip (key), the hatch cache
//! (checks its own entries by content) and the picture cache of small text
//! (kept alive by [`LayerCache`], see `text_raster::hold_textures`).
//! Nothing else is read: pointer sizes and the other style colours are only
//! used for grips, snaps and highlights, which are drawn live.

mod capture;
mod chunks;
mod key;

#[cfg(debug_assertions)]
pub(crate) use capture::same_triangles;
pub(crate) use capture::{capture, tessellate};
pub(crate) use chunks::{LayerCache, Piece, CHUNK_OBJECTS};
pub(crate) use key::{Inputs, LayerKey};

/// Drawings with fewer objects than this are painted directly in Automatic
/// mode: building and keeping a layer costs more than it saves.
pub const AUTO_MIN_OBJECTS: usize = 5_000;
/// The most memory the kept triangles may take, unless the user sets another.
pub const DEFAULT_MEMORY_LIMIT: usize = 512 << 20;

/// Whether the drawing's picture is kept between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheMode {
    /// Kept for drawings of [`AUTO_MIN_OBJECTS`] or more.
    #[default]
    Automatic,
    /// Kept for every drawing.
    On,
    /// Never kept: every frame is painted from the drawing.
    Off,
}

/// How the cache behaves; set by the app from its settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheConfig {
    pub mode: CacheMode,
    /// Bytes of triangles that may be kept; a layer over it is not kept.
    pub memory_limit: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            mode: CacheMode::default(),
            memory_limit: DEFAULT_MEMORY_LIMIT,
        }
    }
}

impl CacheConfig {
    /// Whether a drawing of `objects` objects is painted through the cache.
    pub fn wants(&self, objects: usize) -> bool {
        match self.mode {
            CacheMode::Automatic => objects >= AUTO_MIN_OBJECTS,
            CacheMode::On => true,
            CacheMode::Off => false,
        }
    }
}

/// What the cache has done, for the performance readout and for tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Layers built, kept or not.
    pub builds: u64,
    /// Chunks built again without a selected or hovered entity.
    pub variant_builds: u64,
    /// Frames drawn from a kept layer.
    pub hits: u64,
    /// Bytes of triangles kept now.
    pub bytes: usize,
}
