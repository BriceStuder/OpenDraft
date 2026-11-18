//! A software rasteriser for the triangles egui would draw: the kept layer as
//! an image. Drawing a frame from kept triangles costs egui copying them and the
//! GPU drawing them, about half a millisecond for each megabyte of triangles; an
//! image costs one rectangle however many objects it holds. The triangles are
//! the ones egui makes (with their soft edges, which are the anti-aliasing), so
//! the image looks like what the GPU would draw, and blending is the same:
//! premultiplied colours, "over", in gamma space.
//!
//! Only flat triangles can be drawn here (the vertices of a shape share one
//! point of egui's texture); the others (small text pictures) are given back to
//! be drawn as triangles over the image. The screen is cut into tiles that
//! worker threads take in turn; each tile keeps the order of the triangles, so
//! the result is the same for any number of threads.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use egui::epaint::{Mesh, Vertex};
use egui::{Color32, Pos2, TextureId};

/// Side of a tile, in pixels.
const TILE: usize = 64;

/// A picture of premultiplied RGBA pixels, row by row.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Image {
    pub size: [usize; 2],
    pub pixels: Vec<u8>,
}

impl Image {
    /// The colour of the pixel at `x`, `y` (premultiplied).
    #[cfg(test)]
    pub fn at(&self, x: usize, y: usize) -> [u8; 4] {
        let o = (y * self.size[0] + x) * 4;
        [
            self.pixels[o],
            self.pixels[o + 1],
            self.pixels[o + 2],
            self.pixels[o + 3],
        ]
    }
}

/// What rasterising gave.
pub(crate) struct Rastered {
    pub image: Image,
    /// Triangles that cannot be drawn here, to be drawn over the image.
    pub leftover: Vec<Arc<Mesh>>,
}

/// A flat triangle in pixels, with its corners' premultiplied colours.
struct Tri {
    p: [[f32; 2]; 3],
    c: [[f32; 4]; 3],
}

/// Rasterise `meshes` into a transparent image of `size` pixels. A vertex at
/// `pos` lands at `(pos - origin) * scale` (`origin` in points, `scale` the
/// pixels in a point).
pub(crate) fn rasterise(
    meshes: &[Arc<Mesh>],
    size: [usize; 2],
    origin: Pos2,
    scale: f32,
    threads: usize,
) -> Rastered {
    let mut tris = Vec::new();
    let mut leftover = Vec::new();
    for mesh in meshes {
        split(mesh, origin, scale, &mut tris, &mut leftover);
    }
    let [w, h] = size;
    let (tiles_x, tiles_y) = (w.div_ceil(TILE), h.div_ceil(TILE));
    let bins = bin(&tris, size, tiles_x, tiles_y);
    let next = AtomicUsize::new(0);
    let done: Vec<Mutex<Vec<u8>>> = (0..tiles_x * tiles_y)
        .map(|_| Mutex::new(Vec::new()))
        .collect();
    let work = || loop {
        let n = next.fetch_add(1, Ordering::Relaxed);
        if n >= done.len() {
            break;
        }
        let bounds = tile_bounds(n, tiles_x, size);
        let pixels = draw_tile(&tris, &bins[n], bounds);
        if let Ok(mut slot) = done[n].lock() {
            *slot = pixels;
        }
    };
    std::thread::scope(|s| {
        for _ in 1..threads.max(1) {
            s.spawn(work);
        }
        work();
    });
    let mut pixels = vec![0u8; w * h * 4];
    for (n, tile) in done.iter().enumerate() {
        let [x0, y0, x1, y1] = tile_bounds(n, tiles_x, size);
        let Ok(tile) = tile.lock() else { continue };
        for y in y0..y1 {
            let row = &tile[(y - y0) * (x1 - x0) * 4..(y - y0 + 1) * (x1 - x0) * 4];
            pixels[(y * w + x0) * 4..(y * w + x1) * 4].copy_from_slice(row);
        }
    }
    Rastered {
        image: Image { size, pixels },
        leftover,
    }
}

/// Sort the triangles of `mesh` into those drawn here and those left over.
fn split(
    mesh: &Mesh,
    origin: Pos2,
    scale: f32,
    tris: &mut Vec<Tri>,
    leftover: &mut Vec<Arc<Mesh>>,
) {
    let mut rest = Mesh::with_texture(mesh.texture_id);
    for t in mesh.indices.as_chunks::<3>().0 {
        let at = |k: usize| mesh.vertices.get(t[k] as usize);
        let (Some(a), Some(b), Some(c)) = (at(0), at(1), at(2)) else {
            continue;
        };
        let flat = mesh.texture_id == TextureId::Managed(0) && a.uv == b.uv && b.uv == c.uv;
        if !flat {
            let base = rest.vertices.len() as u32;
            rest.vertices.extend([*a, *b, *c]);
            rest.indices.extend([base, base + 1, base + 2]);
            continue;
        }
        let corner = |v: &Vertex| [(v.pos.x - origin.x) * scale, (v.pos.y - origin.y) * scale];
        let colour = |c: Color32| [c.r(), c.g(), c.b(), c.a()].map(f32::from);
        let tri = Tri {
            p: [corner(a), corner(b), corner(c)],
            c: [colour(a.color), colour(b.color), colour(c.color)],
        };
        // A vertex that is not a number (a degenerate shape) draws nothing.
        if tri.p.iter().flatten().all(|v| v.is_finite()) {
            tris.push(tri);
        }
    }
    if !rest.is_empty() {
        leftover.push(Arc::new(rest));
    }
}

/// For each tile, the triangles that touch it, in drawing order.
fn bin(tris: &[Tri], [w, h]: [usize; 2], tiles_x: usize, tiles_y: usize) -> Vec<Vec<u32>> {
    let mut bins = vec![Vec::new(); tiles_x * tiles_y];
    for (i, t) in tris.iter().enumerate() {
        let (lo, hi) = extent(t);
        let (x0, x1) = (lo[0].max(0.0), hi[0].min(w as f32 - 1.0));
        let (y0, y1) = (lo[1].max(0.0), hi[1].min(h as f32 - 1.0));
        if x0 > x1 || y0 > y1 {
            continue;
        }
        for ty in (y0 as usize / TILE)..=(y1 as usize / TILE) {
            for tx in (x0 as usize / TILE)..=(x1 as usize / TILE) {
                bins[ty * tiles_x + tx].push(i as u32);
            }
        }
    }
    bins
}

/// The corners of the box around a triangle.
fn extent(t: &Tri) -> ([f32; 2], [f32; 2]) {
    let mut lo = [f32::MAX; 2];
    let mut hi = [f32::MIN; 2];
    for p in &t.p {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

/// `[x0, y0, x1, y1]` of tile `n`.
fn tile_bounds(n: usize, tiles_x: usize, [w, h]: [usize; 2]) -> [usize; 4] {
    let (x0, y0) = ((n % tiles_x) * TILE, (n / tiles_x) * TILE);
    [x0, y0, (x0 + TILE).min(w), (y0 + TILE).min(h)]
}

/// The pixels of one tile: its triangles, in order, blended over nothing.
fn draw_tile(tris: &[Tri], list: &[u32], [x0, y0, x1, y1]: [usize; 4]) -> Vec<u8> {
    let width = x1 - x0;
    let mut pixels = vec![0u8; width * (y1 - y0) * 4];
    for &i in list {
        let t = &tris[i as usize];
        let (lo, hi) = extent(t);
        let from_x = lo[0].max(x0 as f32).floor() as usize;
        let to_x = (hi[0].min(x1 as f32 - 1.0)).floor() as usize;
        let from_y = lo[1].max(y0 as f32).floor() as usize;
        let to_y = (hi[1].min(y1 as f32 - 1.0)).floor() as usize;
        // Wind every triangle the same way, so one rule for edges serves all.
        let [a, mut b, mut c] = t.p;
        let [ca, mut cb, mut cc] = t.c;
        let mut area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if area < 0.0 {
            std::mem::swap(&mut b, &mut c);
            std::mem::swap(&mut cb, &mut cc);
            area = -area;
        }
        if area < 1e-9 {
            continue;
        }
        let inv = 1.0 / area;
        for y in from_y..=to_y {
            for x in from_x..=to_x {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let e0 = edge(b, c, p);
                let e1 = edge(c, a, p);
                let e2 = edge(a, b, p);
                if !(inside(e0, b, c) && inside(e1, c, a) && inside(e2, a, b)) {
                    continue;
                }
                let (w0, w1, w2) = (e0 * inv, e1 * inv, e2 * inv);
                let at = ((y - y0) * width + (x - x0)) * 4;
                let src: [f32; 4] = std::array::from_fn(|k| w0 * ca[k] + w1 * cb[k] + w2 * cc[k]);
                let keep = 1.0 - src[3] / 255.0;
                for k in 0..4 {
                    let over = src[k] + f32::from(pixels[at + k]) * keep;
                    pixels[at + k] = over.round().min(255.0) as u8;
                }
            }
        }
    }
    pixels
}

/// Twice the area of the triangle `p`, `q` and `r`, signed: positive when `r`
/// is on the inner side of the edge `p` to `q`.
fn edge(p: [f32; 2], q: [f32; 2], r: [f32; 2]) -> f32 {
    (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
}

/// Whether a pixel centre with edge value `e` is inside an edge from `p` to
/// `q`: on the edge it counts for only one of the two triangles that share it,
/// by the top-left rule, so a translucent edge is blended once.
fn inside(e: f32, p: [f32; 2], q: [f32; 2]) -> bool {
    if e != 0.0 {
        return e > 0.0;
    }
    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
    dy < 0.0 || (dy == 0.0 && dx > 0.0)
}
