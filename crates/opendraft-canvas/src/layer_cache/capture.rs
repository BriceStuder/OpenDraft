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

/// What egui makes triangles with this frame, read from the context once so
/// that triangles can be made on another thread exactly as egui would make
/// them here.
#[derive(Clone)]
pub(crate) struct Tess(Tessellator);

impl Tess {
    pub fn of(ctx: &Context) -> Self {
        let options = ctx.tessellation_options(|o| *o);
        let (size, discs) = ctx.fonts(|f| {
            let atlas = f.texture_atlas();
            let atlas = atlas.lock();
            (atlas.size(), atlas.prepared_discs())
        });
        Self(Tessellator::new(
            ctx.pixels_per_point(),
            options,
            size,
            discs,
        ))
    }
}

/// The triangles of `shapes`, made as egui would make them with `tess`.
pub(crate) fn tessellate(tess: &Tess, shapes: Vec<ClippedShape>) -> Vec<Mesh> {
    tess.0
        .clone()
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
            .all(|(x, y)| x.indices == y.indices && same_vertices(&x.vertices, &y.vertices))
}

/// Whether two vertex lists are the same bit for bit: a vertex at `NaN`, which
/// a degenerate shape can give, is not equal to itself by `==`.
#[cfg(debug_assertions)]
fn same_vertices(a: &[egui::epaint::Vertex], b: &[egui::epaint::Vertex]) -> bool {
    let bits = |v: &egui::epaint::Vertex| {
        [
            v.pos.x.to_bits(),
            v.pos.y.to_bits(),
            v.uv.x.to_bits(),
            v.uv.y.to_bits(),
        ]
    };
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| bits(x) == bits(y) && x.color == y.color)
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use egui::epaint::Vertex;
    use egui::{Color32, Pos2};

    #[test]
    fn a_mesh_with_a_nan_vertex_is_the_same_as_itself() {
        let vertex = Vertex {
            pos: Pos2::new(f32::NAN, f32::NAN),
            uv: Pos2::ZERO,
            color: Color32::WHITE,
        };
        let mut mesh = Mesh::default();
        mesh.vertices.push(vertex);
        mesh.indices.extend([0, 0, 0]);
        assert!(vertex.pos != vertex.pos, "NaN is not equal to itself");
        assert!(same_triangles(
            [&mesh].into_iter(),
            [&mesh.clone()].into_iter()
        ));
    }
}
