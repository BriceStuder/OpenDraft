//! PEDIT's Fit and Spline: a polyline turned into a smooth curve, and a
//! spline turned into a polyline.
//!
//! Fit makes the curve through the vertices from arcs, two to a segment (a
//! biarc), so the result is still a polyline with bulges. Spline makes the
//! uniform cubic B-spline the vertices are the frame of, as many short
//! straight segments. Neither keeps the original vertices: Decurve straightens
//! what is there, it does not bring the frame back.

use super::{distance, PolyData};
use crate::{bulge, Point, Spline};

/// How many straight segments a span of the frame is cut into for Spline.
const STEPS: usize = 8;

fn unit(a: Point, b: Point) -> Option<Point> {
    let d = distance(a, b);
    (d > 1e-12).then(|| Point::new((b.x - a.x) / d, (b.y - a.y) / d))
}

impl PolyData {
    /// The curve through every vertex made of arcs (two to a segment), smooth
    /// at the vertices where the polyline is: the tangent at a vertex runs
    /// along the chord from the one before to the one after. `None` for
    /// fewer than three vertices when open (a line stays a line) or for
    /// coincident vertices.
    pub fn fitted(&self) -> Option<PolyData> {
        let n = self.points.len();
        let segments = bulge::segment_count(n, self.closed);
        if n < 3 || segments == 0 {
            return None;
        }
        let at = |i: usize| self.points[i % n];
        // The direction the curve runs through each vertex.
        let mut tangents: Vec<Point> = Vec::with_capacity(n);
        for i in 0..n {
            let (before, after) = if self.closed {
                (at(i + n - 1), at(i + 1))
            } else if i == 0 {
                (at(0), at(1))
            } else if i == n - 1 {
                (at(n - 2), at(n - 1))
            } else {
                (at(i - 1), at(i + 1))
            };
            tangents.push(unit(before, after)?);
        }
        let mut points = Vec::new();
        let mut bulges = Vec::new();
        for i in 0..segments {
            let (a, b) = (at(i), at(i + 1));
            let (ta, tb) = (tangents[i], tangents[(i + 1) % n]);
            let (join, first, second) = biarc(a, ta, b, tb)?;
            points.extend([a, join]);
            bulges.extend([first, second]);
        }
        if !self.closed {
            points.push(at(n - 1));
            bulges.push(0.0);
        }
        Some(PolyData {
            points,
            closed: self.closed,
            bulges,
            widths: Vec::new(),
        })
    }

    /// The uniform cubic B-spline with the vertices as its frame, as short
    /// straight segments: starting at the first vertex and ending at the last
    /// when open, a smooth loop inside the frame when closed. `None` for
    /// fewer than three vertices.
    pub fn splined(&self) -> Option<PolyData> {
        let n = self.points.len();
        if n < 3 {
            return None;
        }
        // The frame, with the ends tripled when open so the curve reaches them.
        let frame: Vec<Point> = if self.closed {
            (0..n + 3).map(|i| self.points[i % n]).collect()
        } else {
            let mut f = vec![self.points[0]; 2];
            f.extend(self.points.iter().copied());
            f.extend([self.points[n - 1]; 2]);
            f
        };
        let spans = frame.len() - 3;
        let mut points = Vec::with_capacity(spans * STEPS + 1);
        for span in 0..spans {
            for step in 0..STEPS {
                let t = step as f64 / STEPS as f64;
                points.push(b_spline(&frame[span..span + 4], t));
            }
        }
        if !self.closed {
            points.push(self.points[n - 1]);
        }
        Some(PolyData {
            points,
            closed: self.closed,
            bulges: Vec::new(),
            widths: Vec::new(),
        })
    }

    /// A polyline of straight segments along the spline, within `tolerance`
    /// of it; closed when the spline is.
    pub fn from_spline(spline: &Spline, tolerance: f64) -> Option<PolyData> {
        let mut points = spline.flatten(tolerance).ok()?;
        let closed = points.len() > 2
            && distance(points[0], points[points.len() - 1]) <= tolerance.max(1e-9);
        if closed {
            points.pop();
        }
        (points.len() >= 2).then_some(PolyData {
            points,
            closed,
            bulges: Vec::new(),
            widths: Vec::new(),
        })
    }
}

/// A point of the uniform cubic B-spline over four frame points, `t` in 0..1.
fn b_spline(p: &[Point], t: f64) -> Point {
    let (t2, t3) = (t * t, t * t * t);
    let w = [
        (1.0 - t).powi(3) / 6.0,
        (3.0 * t3 - 6.0 * t2 + 4.0) / 6.0,
        (-3.0 * t3 + 3.0 * t2 + 3.0 * t + 1.0) / 6.0,
        t3 / 6.0,
    ];
    Point::new(
        (0..4).map(|k| w[k] * p[k].x).sum(),
        (0..4).map(|k| w[k] * p[k].y).sum(),
    )
}

/// The two arcs from `a` (leaving along `ta`) to `b` (arriving along `tb`),
/// meeting smoothly: the point they meet at and the bulge of each. The
/// meeting point is half way between the points `d` along each tangent where
/// `d` makes those two points `2 d` apart, so the arcs are about the same size.
fn biarc(a: Point, ta: Point, b: Point, tb: Point) -> Option<(Point, f64, f64)> {
    let v = Point::new(b.x - a.x, b.y - a.y);
    if v.x.hypot(v.y) <= 1e-12 {
        return None;
    }
    let w = Point::new(ta.x + tb.x, ta.y + tb.y);
    let vw = v.x * w.x + v.y * w.y;
    let quad = w.x * w.x + w.y * w.y - 4.0;
    // d² (|w|² - 4) - 2 d (v·w) + |v|² = 0, the positive root.
    let d = if quad.abs() < 1e-12 {
        (v.x * v.x + v.y * v.y) / (2.0 * vw)
    } else {
        let disc = vw * vw - quad * (v.x * v.x + v.y * v.y);
        if disc < 0.0 {
            return None;
        }
        let root = disc.sqrt();
        [(vw + root) / quad, (vw - root) / quad]
            .into_iter()
            .filter(|d| *d > 1e-12)
            .fold(f64::INFINITY, f64::min)
    };
    if !d.is_finite() {
        return None;
    }
    let p1 = Point::new(a.x + ta.x * d, a.y + ta.y * d);
    let p2 = Point::new(b.x - tb.x * d, b.y - tb.y * d);
    let join = Point::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
    // The second arc leaves `join` as the first arrives, and reaches `b`
    // along `tb`: seen from `b`, backwards, it leaves along `-tb`.
    let first = bulge::tangent(a, ta, join).unwrap_or(0.0);
    let second = bulge::tangent(b, Point::new(-tb.x, -tb.y), join).map_or(0.0, |k| -k);
    Some((join, first, second))
}

#[cfg(test)]
#[path = "curve_tests.rs"]
mod tests;
