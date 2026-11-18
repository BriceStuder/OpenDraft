//! Painting into a scratch layer, so the shapes can be kept instead of drawn,
//! and turning them into triangles once.

use egui::epaint::{ClippedShape, Mesh, Primitive, Tessellator};
use egui::{Context, Id, LayerId, Order, Painter, Rect, TextureHandle};

use crate::view::text_raster;

/// What `paint` draws on the painter it is given, taken out of egui's frame
/// instead of going into it, with the textures that drawing used.
pub(crate) fn capture(
    ctx: &Context,
    clip: Rect,
    paint: impl FnOnce(&Painter),
) -> (Vec<ClippedShape>, Vec<TextureHandle>) {
    let layer = LayerId::new(Order::Background, Id::new("opendraft_layer_capture"));
    let painter = Painter::new(ctx.clone(), layer, clip);
    text_raster::hold_textures();
    paint(&painter);
    let textures = text_raster::release_textures();
    let list = ctx.graphics_mut(|g| g.get_mut(layer).map(std::mem::take));
    let shapes = list
        .map(|l| l.all_entries().cloned().collect())
        .unwrap_or_default();
    (shapes, textures)
}

/// The triangles of `shapes`, made as egui would make them this frame.
pub(crate) fn tessellate(ctx: &Context, shapes: Vec<ClippedShape>) -> Vec<Mesh> {
    let options = ctx.tessellation_options(|o| *o);
    let (size, discs) = ctx.fonts(|f| {
        let atlas = f.texture_atlas();
        let atlas = atlas.lock();
        (atlas.size(), atlas.prepared_discs())
    });
    Tessellator::new(ctx.pixels_per_point(), options, size, discs)
        .tessellate_shapes(shapes)
        .into_iter()
        .filter_map(|p| match p.primitive {
            Primitive::Mesh(mesh) => Some(mesh),
            Primitive::Callback(_) => None,
        })
        .collect()
}

/// Whether two sets of meshes draw the same triangles (debug builds check the
/// kept layer with it). Texture ids are not compared: a text picture made
/// again gets a new one.
#[cfg(debug_assertions)]
pub(crate) fn same_triangles<'a>(
    a: impl ExactSizeIterator<Item = &'a Mesh>,
    b: impl ExactSizeIterator<Item = &'a Mesh>,
) -> bool {
    a.len() == b.len()
        && a.zip(b)
            .all(|(x, y)| x.indices == y.indices && x.vertices == y.vertices)
}
