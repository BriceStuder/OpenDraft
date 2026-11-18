//! The kept layer as an image (see `raster.rs`): once the triangles of a new
//! layer are made they are drawn into an image, and the triangles are let go.
//! A frame is then one rectangle however many objects the drawing has, and a
//! pan or zoom moves the rectangle. What cannot be drawn into an image (small
//! text pictures) stays as triangles, drawn over it.

use std::sync::Arc;

use egui::emath::TSTransform;
use egui::epaint::Mesh;
use egui::{
    pos2, Color32, ColorImage, Context, Painter, Pos2, Rect, Shape, TextureHandle, TextureOptions,
    Vec2,
};

use super::chunks::{LayerCache, Piece};
use super::raster::rasterise;
use super::LayerKey;

/// Where an image lands and how fine it is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Geometry {
    /// The part of the screen the picture is of, in points.
    pub canvas: Rect,
    /// Pixels in a point.
    pub scale: f32,
}

impl Geometry {
    /// The corner (in points, on a pixel) and size in pixels of the image, so
    /// that each of its pixels is a pixel of the screen.
    fn placed(self) -> (Pos2, [usize; 2]) {
        let corner = (self.canvas.min.to_vec2() * self.scale).round() / self.scale;
        let pixels = (self.canvas.size() * self.scale).ceil();
        (
            corner.to_pos2(),
            [pixels.x.max(1.0) as usize, pixels.y.max(1.0) as usize],
        )
    }
}

/// The image of a layer.
pub(super) struct Bitmap {
    texture: TextureHandle,
    /// Where it is drawn, in points.
    rect: Rect,
    /// Triangles to draw over it, and the textures they use.
    leftover: Vec<Arc<Mesh>>,
    _textures: Vec<TextureHandle>,
    /// The pixels, to check against a fresh picture.
    #[cfg(debug_assertions)]
    image: super::raster::Image,
}

/// Worker threads for drawing an image: the cores there are, but not more
/// than eight (past that, tiles are too few to share).
fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get().min(8))
}

impl LayerCache {
    /// Keep the layer for `key` as an image, made from `pieces` (one for each
    /// chunk); the triangles are not kept.
    pub fn store_image(
        &mut self,
        ctx: &Context,
        name: &str,
        key: LayerKey,
        pieces: Vec<Piece>,
        geometry: Geometry,
    ) {
        let bitmap = make(ctx, name, &pieces, geometry);
        self.stats.builds += 1;
        self.stats.bytes = bitmap.bytes();
        self.key = Some(key);
        self.layer = Some(Vec::new());
        self.selected = None;
        self.excluded = None;
        self.bitmap = Some(bitmap);
    }

    /// Note a frame drawn from the kept layer.
    pub fn note_hit(&mut self) {
        self.stats.hits += 1;
    }

    /// Whether the layer is kept as an image.
    pub fn has_bitmap(&self) -> bool {
        self.bitmap.is_some()
    }

    /// Draw the image, moved by `by` if it is, then the triangles over it;
    /// false when there is none.
    pub fn draw_bitmap(&self, painter: &Painter, by: Option<TSTransform>) -> bool {
        let Some(bitmap) = &self.bitmap else {
            return false;
        };
        bitmap.draw(painter, by);
        true
    }

    /// Whether the image is the one a fresh picture of `pieces` gives.
    #[cfg(debug_assertions)]
    pub fn bitmap_matches(&self, pieces: &[Piece], geometry: Geometry) -> bool {
        self.bitmap
            .as_ref()
            .is_some_and(|kept| kept.matches(pieces, geometry))
    }
}

impl Bitmap {
    pub(super) fn bytes(&self) -> usize {
        let [w, h] = self.texture.size();
        w * h * 4
    }

    /// Draw the image, moved by `by` if it is, then the triangles over it.
    pub(super) fn draw(&self, painter: &Painter, by: Option<TSTransform>) {
        let rect = match by {
            Some(by) => Rect::from_min_max(by * self.rect.min, by * self.rect.max),
            None => self.rect,
        };
        let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(self.texture.id(), rect, whole, Color32::WHITE);
        match by {
            Some(by) => super::draw_moved(painter, &self.leftover, by),
            None => {
                for mesh in &self.leftover {
                    painter.add(Shape::Mesh(mesh.clone()));
                }
            }
        }
    }

    /// Whether this is the image a fresh picture of `pieces` gives.
    #[cfg(debug_assertions)]
    pub(super) fn matches(&self, pieces: &[Piece], geometry: Geometry) -> bool {
        let (origin, size) = geometry.placed();
        let meshes = meshes_of(pieces);
        let fresh = rasterise(&meshes, size, origin, geometry.scale, threads());
        self.image == fresh.image
            && super::same_triangles(
                self.leftover.iter().map(|m| &**m),
                fresh.leftover.iter().map(|m| &**m),
            )
    }
}

fn meshes_of(pieces: &[Piece]) -> Vec<Arc<Mesh>> {
    pieces
        .iter()
        .flat_map(|p| p.meshes().iter().cloned())
        .collect()
}

pub(super) fn make(ctx: &Context, name: &str, pieces: &[Piece], geometry: Geometry) -> Bitmap {
    let (origin, size) = geometry.placed();
    let done = rasterise(&meshes_of(pieces), size, origin, geometry.scale, threads());
    let picture = ColorImage::from_rgba_premultiplied(size, &done.image.pixels);
    // Linear, so that an image drawn moved or scaled is smooth; drawn where it
    // was made it is exact, each pixel a pixel of the screen.
    let texture = ctx.load_texture(name, picture, TextureOptions::LINEAR);
    let extent = Vec2::new(size[0] as f32, size[1] as f32) / geometry.scale;
    Bitmap {
        texture,
        rect: Rect::from_min_size(origin, extent),
        leftover: done.leftover,
        _textures: pieces
            .iter()
            .flat_map(|p| p.textures().iter().cloned())
            .collect(),
        #[cfg(debug_assertions)]
        image: done.image,
    }
}
