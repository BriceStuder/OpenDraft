//! The plain entities of a drawing as ready-made triangles, kept between
//! frames.
//!
//! Painting is immediate: every frame turns every visible entity into egui
//! shapes and egui into triangles, even when only the pointer moved. This
//! keeps the triangles of everything that is neither selected nor hovered and
//! draws those instead, until something they depend on changes. Selected and
//! hovered entities are drawn live on top.
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
mod key;

use std::sync::Arc;

use egui::epaint::Mesh;
use egui::{Painter, Shape, TextureHandle};

pub(crate) use capture::{capture, same_triangles, tessellate};
pub(crate) use key::{Inputs, LayerKey};

/// Drawings with fewer objects than this are painted directly in Automatic
/// mode: building and keeping a layer costs more than it saves.
pub const AUTO_MIN_OBJECTS: usize = 5_000;
/// The most memory the kept triangles may take, unless the user sets another.
pub const DEFAULT_MEMORY_LIMIT: usize = 512 << 20;

/// Whether the drawing's picture is kept between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    /// Frames drawn from a kept layer.
    pub hits: u64,
    /// Bytes of triangles kept now.
    pub bytes: usize,
}

struct Kept {
    meshes: Vec<Arc<Mesh>>,
    /// The textures the meshes point at, so they outlive the picture cache.
    _textures: Vec<TextureHandle>,
}

/// The kept layer, if there is one for `key`.
#[derive(Default)]
pub(crate) struct LayerCache {
    key: Option<LayerKey>,
    /// `None` with a key: it was too big to keep, so do not try again.
    kept: Option<Kept>,
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
        self.kept = None;
        self.stats.bytes = 0;
    }

    /// Keep `meshes` as the layer for `key`, unless they are over `limit`.
    pub fn store(
        &mut self,
        key: LayerKey,
        meshes: Vec<Mesh>,
        textures: Vec<TextureHandle>,
        limit: usize,
    ) {
        let bytes: usize = meshes
            .iter()
            .map(|m| {
                m.vertices.len() * std::mem::size_of::<egui::epaint::Vertex>()
                    + m.indices.len() * std::mem::size_of::<u32>()
            })
            .sum();
        self.stats.builds += 1;
        self.key = Some(key);
        if bytes > limit {
            self.kept = None;
            self.stats.bytes = 0;
            return;
        }
        self.stats.bytes = bytes;
        self.kept = Some(Kept {
            meshes: meshes.into_iter().map(Arc::new).collect(),
            _textures: textures,
        });
    }

    /// Draw the kept layer; false when there is none (too big to keep).
    pub fn draw(&mut self, painter: &Painter) -> bool {
        let Some(kept) = &self.kept else {
            return false;
        };
        self.stats.hits += 1;
        for mesh in &kept.meshes {
            painter.add(Shape::Mesh(mesh.clone()));
        }
        true
    }

    /// Whether the kept meshes are what `fresh` is.
    pub fn matches(&self, fresh: &[Mesh]) -> bool {
        match &self.kept {
            Some(kept) => same_triangles(kept.meshes.iter().map(|m| &**m), fresh.iter()),
            None => true,
        }
    }
}
