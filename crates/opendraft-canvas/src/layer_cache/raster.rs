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
    let [w, h] = size;
    let (tiles_x, tiles_y) = (w.div_ceil(TILE), h.div_ceil(TILE));
    // Each thread readies and bins a run of the meshes; a tile then draws the
    // runs in order, so the order of the triangles is kept.
    let runs: Vec<Run> = std::thread::scope(|s| {
        let ready = |part: &[Arc<Mesh>]| {
            let mut run = Run::default();
            for mesh in part {
                split(mesh, origin, scale, &mut run.tris, &mut run.leftover);
            }
            run.bins = bin(&run.tris, size, tiles_x, tiles_y);
            run
        };
        let parts = parts(meshes, threads.max(1));
        let others: Vec<_> = (parts.iter().skip(1))
            .map(|&p| s.spawn(move || ready(p)))
            .collect();
        let first = parts.first().map(|p| ready(p)).unwrap_or_default();
        // A thread that panicked has drawn nothing (`Run::default`).
        let joined = others.into_iter().map(|h| h.join().unwrap_or_default());
        std::iter::once(first).chain(joined).collect()
    });
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
        let mut pixels = vec![0u8; (bounds[2] - bounds[0]) * (bounds[3] - bounds[1]) * 4];
        // An empty run (no meshes, or a thread that failed) has no bins.
        for run in &runs {
            let list = run.bins.get(n).map_or(&[][..], Vec::as_slice);
            draw_tile(&mut pixels, &run.tris, list, bounds);
        }
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
        leftover: runs.into_iter().flat_map(|r| r.leftover).collect(),
    }
}

/// What one thread readied of a run of meshes: its flat triangles, the
/// triangles that touch each tile, and what cannot be drawn here.
#[derive(Default)]
struct Run {
    tris: Vec<Tri>,
    bins: Vec<Vec<u32>>,
    leftover: Vec<Arc<Mesh>>,
}

/// `meshes` cut into at most `n` runs, in order, of about as many triangles.
fn parts(meshes: &[Arc<Mesh>], n: usize) -> Vec<&[Arc<Mesh>]> {
    let total: usize = meshes.iter().map(|m| m.indices.len()).sum();
    let share = total.div_ceil(n).max(1);
    let (mut out, mut from, mut held) = (Vec::new(), 0, 0);
    for (i, mesh) in meshes.iter().enumerate() {
        held += mesh.indices.len();
        if held >= share {
            out.push(&meshes[from..=i]);
            (from, held) = (i + 1, 0);
        }
    }
    if from < meshes.len() {
        out.push(&meshes[from..]);
    }
    out
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

/// Blend the triangles `list` of `tris`, in order, over the pixels of a tile.
fn draw_tile(pixels: &mut [u8], tris: &[Tri], list: &[u32], [x0, y0, x1, y1]: [usize; 4]) {
    let width = x1 - x0;
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
            let Some((first, last)) = span(&[a, b, c], y as f32 + 0.5, from_x, to_x) else {
                continue;
            };
            for x in first..=last {
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
}

/// The pixels of a row, `from` to `to`, that may be inside the triangle `t`
/// (wound as `draw_tile` winds it) at height `y`: where each edge crosses the
/// row, widened by [`SPAN_MARGIN`] so that rounding never leaves out a pixel
/// the exact test would take. A thin slanted triangle, which most lines are,
/// covers little of its box, so this saves testing most of it.
fn span(t: &[[f32; 2]; 3], y: f32, from: usize, to: usize) -> Option<(usize, usize)> {
    let (mut lo, mut hi) = (from as f32, to as f32);
    for (p, q) in [(t[1], t[2]), (t[2], t[0]), (t[0], t[1])] {
        let dy = q[1] - p[1];
        if dy == 0.0 {
            continue;
        }
        // The pixel whose centre the edge crosses: inside is before it when
        // the edge goes down the screen, after it when it goes up.
        let cross = p[0] + (q[0] - p[0]) * (y - p[1]) / dy - 0.5;
        if dy > 0.0 {
            hi = hi.min(cross.floor() + SPAN_MARGIN);
        } else {
            lo = lo.max(cross.ceil() - SPAN_MARGIN);
        }
    }
    // A crossing that is not a number (a near-flat edge) narrows nothing.
    let (lo, hi) = (lo.max(from as f32), hi.min(to as f32));
    (lo <= hi).then_some((lo as usize, hi as usize))
}

/// Pixels added at each end of a row's span; the exact test decides them.
const SPAN_MARGIN: f32 = 2.0;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny deterministic generator, so a failure can be run again.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> f32 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 40) as f32 / (1u64 << 24) as f32
        }
    }

    #[test]
    fn nothing_to_draw_is_a_clear_picture() {
        for threads in [1, 4] {
            let done = rasterise(&[], [70, 70], Pos2::ZERO, 1.0, threads);
            assert!(done.image.pixels.iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn the_runs_keep_every_mesh_in_order_and_what_is_left_over_too() {
        let picture = |n: u32| {
            let mut mesh = Mesh::with_texture(TextureId::Managed(u64::from(n) + 1));
            let uv = egui::Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0));
            let at = egui::Rect::from_min_max(Pos2::ZERO, egui::pos2(4.0, 4.0));
            mesh.add_rect_with_uv(at, uv, Color32::WHITE);
            Arc::new(mesh)
        };
        let meshes: Vec<_> = (0..23).map(picture).collect();
        for n in [1, 2, 5, 23, 40] {
            let runs = parts(&meshes, n);
            assert!(runs.len() <= n);
            let back: Vec<_> = runs.concat();
            assert_eq!(back.len(), meshes.len());
            let done = rasterise(&meshes, [8, 8], Pos2::ZERO, 1.0, n);
            let ids: Vec<_> = done.leftover.iter().map(|m| m.texture_id).collect();
            let want: Vec<_> = meshes.iter().map(|m| m.texture_id).collect();
            assert_eq!(ids, want, "{n} threads");
        }
    }

    #[test]
    fn a_row_span_holds_every_pixel_the_exact_test_takes() {
        let mut rng = Lcg(3);
        // Small, long and thin, and reaching millions of pixels off the tile.
        for reach in [8.0, 300.0, 50_000.0, 8_000_000.0] {
            for n in 0..3_000 {
                let mut corner = || [(rng.next() - 0.3) * reach, (rng.next() - 0.3) * reach];
                let [a, mut b, mut c] = [corner(), corner(), corner()];
                // Every other one a sliver, as the sides of a line are.
                if n % 2 == 0 {
                    c = [b[0] + rng.next() * 1.5, b[1] - rng.next() * 1.5];
                }
                if edge(a, b, c) < 0.0 {
                    std::mem::swap(&mut b, &mut c);
                }
                for y in 0..64 {
                    let py = y as f32 + 0.5;
                    let span = span(&[a, b, c], py, 0, 63);
                    for x in 0..64 {
                        let p = [x as f32 + 0.5, py];
                        let taken = inside(edge(b, c, p), b, c)
                            && inside(edge(c, a, p), c, a)
                            && inside(edge(a, b, p), a, b);
                        let spanned = span.is_some_and(|(lo, hi)| (lo..=hi).contains(&x));
                        assert!(!taken || spanned, "{a:?} {b:?} {c:?} pixel {x},{y}");
                    }
                }
            }
        }
    }
}
