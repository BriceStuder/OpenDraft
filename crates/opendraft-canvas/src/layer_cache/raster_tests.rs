//! The rasteriser draws what the triangles say, once, in order, the same for
//! any number of threads.

use super::raster::rasterise;
use egui::epaint::{Mesh, Vertex};
use egui::{pos2, Color32, Rect, TextureId};
use std::sync::Arc;

fn quad(x0: f32, y0: f32, x1: f32, y1: f32, colour: Color32) -> Arc<Mesh> {
    let mut mesh = Mesh::default();
    mesh.add_colored_rect(Rect::from_min_max(pos2(x0, y0), pos2(x1, y1)), colour);
    Arc::new(mesh)
}

#[test]
fn a_rectangle_fills_the_pixels_it_covers_and_no_others() {
    let red = Color32::from_rgb(255, 0, 0);
    let done = rasterise(
        &[quad(2.0, 3.0, 6.0, 7.0, red)],
        [10, 10],
        pos2(0.0, 0.0),
        1.0,
        1,
    );
    for y in 0..10 {
        for x in 0..10 {
            let inside = (2..6).contains(&x) && (3..7).contains(&y);
            let want = if inside { [255, 0, 0, 255] } else { [0; 4] };
            assert_eq!(done.image.at(x, y), want, "pixel {x},{y}");
        }
    }
    assert!(done.leftover.is_empty());
}

#[test]
fn a_translucent_rectangle_is_blended_once_where_its_two_triangles_meet() {
    // On the diagonal a pixel centre is exactly on the shared edge.
    let glass = Color32::from_rgba_premultiplied(50, 0, 0, 100);
    let done = rasterise(
        &[quad(0.0, 0.0, 8.0, 8.0, glass)],
        [8, 8],
        pos2(0.0, 0.0),
        1.0,
        1,
    );
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(done.image.at(x, y), [50, 0, 0, 100], "pixel {x},{y}");
        }
    }
}

#[test]
fn later_triangles_go_over_earlier_ones() {
    let red = Color32::from_rgb(200, 0, 0);
    let glass = Color32::from_rgba_premultiplied(0, 0, 50, 100);
    let meshes = [
        quad(0.0, 0.0, 4.0, 4.0, red),
        quad(0.0, 0.0, 4.0, 4.0, glass),
    ];
    let done = rasterise(&meshes, [4, 4], pos2(0.0, 0.0), 1.0, 1);
    // src + dst * (1 - alpha): 0 + 200 * (155 / 255) = 121.6.
    assert_eq!(done.image.at(1, 1), [122, 0, 50, 255]);
}

#[test]
fn the_origin_and_scale_place_the_vertices() {
    let white = Color32::WHITE;
    // Points 10..12 from an origin of 8, at two pixels a point: pixels 4..8.
    let done = rasterise(
        &[quad(10.0, 10.0, 12.0, 12.0, white)],
        [12, 12],
        pos2(8.0, 8.0),
        2.0,
        1,
    );
    for y in 0..12 {
        for x in 0..12 {
            let inside = (4..8).contains(&x) && (4..8).contains(&y);
            assert_eq!(
                done.image.at(x, y)[3],
                if inside { 255 } else { 0 },
                "{x},{y}"
            );
        }
    }
}

#[test]
fn any_number_of_threads_gives_the_same_picture() {
    // Many overlapping translucent triangles across several tiles.
    let mut seed = 12345_u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1u32 << 24) as f32
    };
    let mut meshes = Vec::new();
    for _ in 0..300 {
        let (x, y) = (next() * 280.0, next() * 190.0);
        let colour = Color32::from_rgba_premultiplied(
            (next() * 60.0) as u8,
            (next() * 60.0) as u8,
            (next() * 60.0) as u8,
            80,
        );
        meshes.push(quad(x, y, x + next() * 60.0, y + next() * 60.0, colour));
    }
    let size = [300, 200];
    let one = rasterise(&meshes, size, pos2(0.0, 0.0), 1.0, 1);
    for threads in [2, 7, 16] {
        let many = rasterise(&meshes, size, pos2(0.0, 0.0), 1.0, threads);
        assert!(one.image == many.image, "{threads} threads differ");
    }
}

#[test]
fn what_cannot_be_drawn_flat_is_given_back() {
    let vertex = |x: f32, u: f32| Vertex {
        pos: pos2(x, 1.0),
        uv: pos2(u, 0.0),
        color: Color32::WHITE,
    };
    // Different texture points at the corners: a picture, not a flat shape.
    let mut glyphs = Mesh::default();
    glyphs
        .vertices
        .extend([vertex(0.0, 0.0), vertex(4.0, 0.5), vertex(2.0, 1.0)]);
    glyphs.indices.extend([0, 1, 2]);
    // A texture of its own.
    let mut picture = Mesh::with_texture(TextureId::Managed(7));
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    picture.add_rect_with_uv(
        Rect::from_min_max(pos2(0.0, 0.0), pos2(4.0, 4.0)),
        uv,
        Color32::WHITE,
    );
    let done = rasterise(
        &[Arc::new(glyphs), Arc::new(picture)],
        [8, 8],
        pos2(0.0, 0.0),
        1.0,
        1,
    );
    assert!(
        done.image.pixels.iter().all(|&b| b == 0),
        "nothing drawn flat"
    );
    let kept: usize = done.leftover.iter().map(|m| m.indices.len() / 3).sum();
    assert_eq!(kept, 3, "the triangle and the two of the rectangle");
}

#[test]
fn a_vertex_that_is_not_a_number_draws_nothing() {
    let mut mesh = Mesh::default();
    let at = |x: f32, y: f32| Vertex {
        pos: pos2(x, y),
        uv: pos2(0.0, 0.0),
        color: Color32::WHITE,
    };
    mesh.vertices
        .extend([at(f32::NAN, f32::NAN), at(5.0, 1.0), at(1.0, 5.0)]);
    mesh.indices.extend([0, 1, 2]);
    let done = rasterise(&[Arc::new(mesh)], [8, 8], pos2(0.0, 0.0), 1.0, 1);
    assert!(done.image.pixels.iter().all(|&b| b == 0));
}

#[test]
fn a_thin_triangle_reaching_far_off_screen_draws_only_where_it_is() {
    // A line a pixel thick that runs out to millions of pixels each way, as a
    // long line seen from close up does. Single precision edge maths at that
    // size draws rows that the triangle does not touch.
    let white = Color32::WHITE;
    let mut mesh = Mesh::default();
    let at = |x: f32, y: f32| Vertex {
        pos: pos2(x, y),
        uv: pos2(0.0, 0.0),
        color: white,
    };
    mesh.vertices.extend([
        at(-8_000_000.0, 50.0),
        at(8_000_000.0, 50.0),
        at(8_000_000.0, 51.0),
        at(-8_000_000.0, 51.0),
    ]);
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    let done = rasterise(&[Arc::new(mesh)], [120, 100], pos2(0.0, 0.0), 1.0, 1);
    for y in 0..100 {
        let filled = (0..120).filter(|&x| done.image.at(x, y)[3] > 0).count();
        let want = if y == 50 { 120 } else { 0 };
        assert_eq!(filled, want, "row {y}");
    }
}

#[test]
fn a_triangle_with_a_corner_far_off_screen_is_the_same_as_its_visible_part() {
    // Cut at the edge of the picture by hand, the same shape has the same pixels.
    let red = Color32::from_rgb(200, 0, 0);
    let make = |far: f32| {
        let at = |x: f32, y: f32| Vertex {
            pos: pos2(x, y),
            uv: pos2(0.0, 0.0),
            color: red,
        };
        let mut mesh = Mesh::default();
        mesh.vertices
            .extend([at(10.0, 10.0), at(far, 10.0), at(10.0, far)]);
        mesh.indices.extend([0, 1, 2]);
        rasterise(&[Arc::new(mesh)], [64, 64], pos2(0.0, 0.0), 1.0, 1).image
    };
    // The diagonal x + y = far + 10 is beyond the picture in both, so each
    // fills the whole quarter from the corner at (10, 10).
    assert!(make(1_000.0) == make(5_000_000.0));
}

#[test]
fn a_long_sliver_that_covers_no_pixel_centre_draws_nothing() {
    // Thinner than a thousandth of a pixel and a hundred thousand long, lying
    // between two rows of pixel centres: nothing of it is on a pixel, but
    // rounding in single precision once drew a streak along it.
    let white = Color32::WHITE;
    let at = |x: f32, y: f32| Vertex {
        pos: pos2(x, y),
        uv: pos2(0.0, 0.0),
        color: white,
    };
    let mut mesh = Mesh::default();
    mesh.vertices.extend([
        at(-50_000.0, 50.0),
        at(70_000.0, 50.0),
        at(10_000.0, 50.000004),
    ]);
    mesh.indices.extend([0, 1, 2]);
    let done = rasterise(&[Arc::new(mesh)], [120, 100], pos2(0.0, 0.0), 1.0, 1);
    assert!(
        done.image.pixels.iter().all(|&b| b == 0),
        "a streak was drawn"
    );
}
