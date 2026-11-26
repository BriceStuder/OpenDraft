//! Whether two objects are the same shape in the same place.

use crate::{arc, bulge, Entity, Point};

pub(super) fn near(a: Point, b: Point, tol: f64) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= tol
}

pub(super) fn same(a: &Entity, b: &Entity, tol: f64) -> bool {
    match (a, b) {
        (Entity::Line { start: s1, end: e1 }, Entity::Line { start: s2, end: e2 }) => {
            (near(*s1, *s2, tol) && near(*e1, *e2, tol))
                || (near(*s1, *e2, tol) && near(*e1, *s2, tol))
        }
        (
            Entity::Circle {
                center: c1,
                radius: r1,
            },
            Entity::Circle {
                center: c2,
                radius: r2,
            },
        ) => near(*c1, *c2, tol) && (r1 - r2).abs() <= tol,
        (
            Entity::Arc {
                center: c1,
                radius: r1,
                start: s1,
                end: e1,
            },
            Entity::Arc {
                center: c2,
                radius: r2,
                start: s2,
                end: e2,
            },
        ) => {
            // Equal ends on one circle also mean one sweep: an arc runs
            // counter-clockwise from its start to its end.
            near(*c1, *c2, tol)
                && (r1 - r2).abs() <= tol
                && near(
                    arc::point_at(*c1, *r1, *s1),
                    arc::point_at(*c2, *r2, *s2),
                    tol,
                )
                && near(
                    arc::point_at(*c1, *r1, *e1),
                    arc::point_at(*c2, *r2, *e2),
                    tol,
                )
        }
        (Entity::Rectangle { min: a1, max: b1 }, Entity::Rectangle { min: a2, max: b2 }) => {
            near(*a1, *a2, tol) && near(*b1, *b2, tol)
        }
        (Entity::Point { at: p }, Entity::Point { at: q }) => near(*p, *q, tol),
        (Entity::Polyline { .. }, Entity::Polyline { .. }) => same_polyline(a, b, tol),
        _ => a == b,
    }
}

/// Whether the loose line or arc `loose` lies wholly on one segment of the
/// polyline `shape`: a line on a straight segment, an arc on an arc segment of
/// the same circle, each within `tol`. Such an object adds nothing the
/// polyline does not already draw.
pub(super) fn lies_on_polyline(loose: &Entity, shape: &Entity, tol: f64) -> bool {
    let Entity::Polyline {
        points,
        closed,
        bulges,
        ..
    } = shape
    else {
        return false;
    };
    let n = points.len();
    (0..bulge::segment_count(n, *closed)).any(|i| {
        let (a, b, k) = (points[i], points[(i + 1) % n], bulge::at(bulges, i));
        match (loose, bulge::arc_of(a, b, k)) {
            (Entity::Line { start, end }, None) => {
                let on = |p: Point| bulge::nearest(a, b, 0.0, p).0;
                near(on(*start), *start, tol) && near(on(*end), *end, tol)
            }
            (
                Entity::Arc {
                    center,
                    radius,
                    start,
                    end,
                },
                Some(seg),
            ) => {
                // The same circle, and both ends and the middle on the segment.
                let on = |p: Point| bulge::nearest(a, b, k, p).0;
                near(seg.center, *center, tol)
                    && (seg.radius - radius).abs() <= tol
                    && [
                        arc::point_at(*center, *radius, *start),
                        arc::point_at(*center, *radius, *end),
                        arc::midpoint(*center, *radius, *start, *end),
                    ]
                    .iter()
                    .all(|p| near(on(*p), *p, tol))
            }
            _ => false,
        }
    })
}

/// One polyline as its points and, for the segment leaving each, its bulge
/// and its width at both ends.
struct Path<'a> {
    closed: bool,
    bulges: &'a [f64],
    widths: &'a [(f64, f64)],
}

impl Path<'_> {
    fn bulge(&self, i: usize) -> f64 {
        bulge::at(self.bulges, i)
    }

    fn width(&self, i: usize) -> (f64, f64) {
        self.widths.get(i).copied().unwrap_or((0.0, 0.0))
    }

    /// Segment `i` here against segment `j` there, the other way round when
    /// `flip`: a flipped segment turns the opposite way and tapers backwards.
    fn segment_matches(&self, i: usize, other: &Path, j: usize, flip: bool, tol: f64) -> bool {
        let (w, v) = (self.width(i), other.width(j));
        let (b, c) = (self.bulge(i), other.bulge(j));
        if flip {
            (b + c).abs() <= tol && (w.0 - v.1).abs() <= tol && (w.1 - v.0).abs() <= tol
        } else {
            (b - c).abs() <= tol && (w.0 - v.0).abs() <= tol && (w.1 - v.1).abs() <= tol
        }
    }
}

/// The most points of a closed polyline tried against every start point.
const ROTATIONS: usize = 256;

fn same_polyline(a: &Entity, b: &Entity, tol: f64) -> bool {
    let (
        Entity::Polyline {
            points: p1,
            closed: c1,
            bulges: g1,
            widths: w1,
        },
        Entity::Polyline {
            points: p2,
            closed: c2,
            bulges: g2,
            widths: w2,
        },
    ) = (a, b)
    else {
        return false;
    };
    if c1 != c2 || p1.len() != p2.len() {
        return false;
    }
    let n = p1.len();
    let first = Path {
        closed: *c1,
        bulges: g1,
        widths: w1,
    };
    let second = Path {
        closed: *c2,
        bulges: g2,
        widths: w2,
    };
    // Segments that really exist: the closing one only on a closed polyline.
    let segments = if first.closed { n } else { n.saturating_sub(1) };
    if !first.closed {
        let forward = (0..n).all(|i| near(p1[i], p2[i], tol))
            && (0..segments).all(|i| first.segment_matches(i, &second, i, false, tol));
        let backward = (0..n).all(|i| near(p1[i], p2[n - 1 - i], tol))
            && (0..segments)
                .all(|i| first.segment_matches(i, &second, segments - 1 - i, true, tol));
        return forward || backward;
    }
    // Closed: any start point, either way round.
    let starts = if n <= ROTATIONS { n } else { 1 };
    (0..starts).any(|r| {
        let forward = (0..n).all(|i| near(p1[i], p2[(i + r) % n], tol))
            && (0..n).all(|i| first.segment_matches(i, &second, (i + r) % n, false, tol));
        let backward = (0..n).all(|i| near(p1[i], p2[(r + n - i) % n], tol))
            && (0..n)
                .all(|i| first.segment_matches(i, &second, (r + 2 * n - i - 1) % n, true, tol));
        forward || backward
    })
}
