//! A spline turned into a polyline of arcs and short straight pieces
//! (`PLINECONVERTMODE` 1), for PEDIT. The spline is cut into many short
//! straight pieces first; then as many neighbouring points as possible are
//! covered by one arc that keeps them all within the tolerance.

use std::f64::consts::PI;

use super::{distance, PolyData};
use crate::{bulge, Point, Spline};

/// The most an arc may turn through: a half circle.
const MAX_SWEEP: f64 = PI;

/// The circle through three points: its centre and radius, `None` when they
/// are in a line.
fn circle_through(a: Point, b: Point, c: Point) -> Option<(Point, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (
        a.x * a.x + a.y * a.y,
        b.x * b.x + b.y * b.y,
        c.x * c.x + c.y * c.y,
    );
    let centre = Point::new(
        (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    );
    Some((centre, distance(centre, a)))
}

/// The bulge of the arc from `points[from]` to `points[to]` that keeps every
/// point between within `tolerance` of it, or `None` when no such arc turns
/// through no more than half a circle.
fn arc_over(points: &[Point], from: usize, to: usize, tolerance: f64) -> Option<f64> {
    let (a, b) = (points[from], points[to]);
    let m = points[(from + to) / 2];
    let (centre, radius) = circle_through(a, m, b)?;
    if !radius.is_finite() || radius > 1e9 {
        return None;
    }
    let ccw = (m.x - a.x) * (b.y - a.y) - (m.y - a.y) * (b.x - a.x) > 0.0;
    let angle = |p: Point| (p.y - centre.y).atan2(p.x - centre.x);
    let along = |p: Point| {
        let turn = if ccw {
            angle(p) - angle(a)
        } else {
            angle(a) - angle(p)
        };
        turn.rem_euclid(2.0 * PI)
    };
    let sweep = along(b);
    if sweep > MAX_SWEEP + 1e-9 {
        return None;
    }
    let mut last = 0.0;
    for p in &points[from..=to] {
        // Close to the circle, and travelling the same way round.
        if (distance(centre, *p) - radius).abs() > tolerance {
            return None;
        }
        let here = along(*p);
        if here + 1e-9 < last || here > sweep + 1e-9 {
            return None;
        }
        last = here;
    }
    Some(bulge::from_sweep(sweep, ccw))
}

impl PolyData {
    /// A polyline of arcs (and straight pieces where the spline is straight)
    /// along the spline, within `tolerance` of it; closed when the spline is.
    pub fn from_spline_arcs(spline: &Spline, tolerance: f64) -> Option<PolyData> {
        let mut dense = spline.flatten(tolerance / 20.0).ok()?;
        let closed =
            dense.len() > 2 && distance(dense[0], dense[dense.len() - 1]) <= tolerance.max(1e-9);
        if closed {
            dense.pop();
        }
        if dense.len() < 2 {
            return None;
        }
        let last = if closed { dense.len() } else { dense.len() - 1 };
        let at = |i: usize| dense[i % dense.len()];
        // The same points again, so an arc may run past the end of a closed one.
        let wrapped: Vec<Point> = (0..=last + dense.len()).map(at).collect();
        let (mut points, mut bulges) = (Vec::new(), Vec::new());
        let mut i = 0;
        while i < last {
            let (mut best, mut curve) = (i + 1, 0.0);
            let mut j = i + 2;
            while j <= last.min(i + dense.len() - 1) {
                match arc_over(&wrapped, i, j, tolerance) {
                    Some(b) => (best, curve) = (j, b),
                    None => break,
                }
                j += 1;
            }
            points.push(at(i));
            bulges.push(curve);
            i = best;
        }
        if !closed {
            points.push(at(last));
            bulges.push(0.0);
        }
        (points.len() >= 2).then_some(PolyData {
            points,
            closed,
            bulges,
            widths: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn arch() -> Spline {
        Spline::from_control_points(
            3,
            vec![
                p(0.0, 0.0),
                p(2.0, 4.0),
                p(6.0, 4.0),
                p(8.0, 0.0),
                p(10.0, -2.0),
            ],
        )
        .unwrap()
    }

    #[test]
    fn the_arcs_follow_the_spline_within_the_tolerance_in_far_fewer_pieces() {
        let spline = arch();
        let tolerance = 0.01;
        let arcs = PolyData::from_spline_arcs(&spline, tolerance).unwrap();
        let straight = PolyData::from_spline(&spline, tolerance).unwrap();
        assert!(
            arcs.points.len() * 2 < straight.points.len(),
            "{} vs {}",
            arcs.points.len(),
            straight.points.len()
        );
        assert!(bulge::has_arcs(&arcs.bulges));
        // Vertices and the middle of every arc are on the spline.
        let n = arcs.points.len();
        for i in 0..n {
            assert!(
                spline.distance_to(arcs.points[i]) <= tolerance * 2.0,
                "vertex {i}"
            );
            if i + 1 < n {
                let mid = bulge::point_at(arcs.points[i], arcs.points[i + 1], arcs.bulges[i], 0.5);
                assert!(spline.distance_to(mid) <= tolerance * 2.0, "arc {i}");
            }
        }
        assert!(distance(arcs.points[0], spline.start_point()) < 1e-9);
        assert!(distance(arcs.points[n - 1], spline.end_point()) < 1e-9);
    }

    #[test]
    fn a_straight_spline_stays_straight_pieces() {
        let line =
            Spline::from_control_points(1, vec![p(0.0, 0.0), p(5.0, 0.0), p(10.0, 0.0)]).unwrap();
        let made = PolyData::from_spline_arcs(&line, 0.01).unwrap();
        assert!(!bulge::has_arcs(&made.bulges));
        assert!(distance(made.points[made.points.len() - 1], p(10.0, 0.0)) < 1e-9);
    }

    #[test]
    fn a_closed_spline_gives_a_closed_polyline_of_arcs() {
        let ring = Spline::new_nurbs(
            3,
            vec![
                p(0.0, 0.0),
                p(4.0, 4.0),
                p(8.0, 0.0),
                p(4.0, -4.0),
                p(0.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
            None,
            true,
            false,
        )
        .unwrap();
        let made = PolyData::from_spline_arcs(&ring, 0.01).unwrap();
        assert!(made.closed);
        assert!(bulge::has_arcs(&made.bulges));
    }
}
