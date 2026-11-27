//! FILLET where one or both objects are splines: a line, an arc, a circle or
//! another spline on the other side.
//!
//! The arc's centre is a point `radius` from both curves. It is looked for
//! where the offset curves of the two meet (each offset on both sides; a
//! spline's offset is fitted, see `offset_fit`), then made exact by Newton's
//! method on the true distances to the two curves, so the arc touches both. Of
//! the centres found, the one whose touching points are nearest the points
//! picked is used, as for lines. A spline is cut exactly at its touching point
//! (`split_at_parameter`) and keeps the part picked. A spline is not lengthened
//! to meet the arc: where the arc would touch beyond its end there is no fit.

use crate::corner::{CornerError, Joined};
use crate::fillet::{joint_arc, keep_part};
use crate::{Entity, Point, Spline};

/// Lengths below this are nothing.
const EPS: f64 = 1e-9;

fn dist(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// The point of `e`'s whole carrier (the line or circle it lies on, not just
/// the part drawn; a spline as drawn) nearest `p`.
fn nearest_on(e: &Entity, p: Point) -> Option<Point> {
    match e {
        Entity::Line { start, end } => {
            let len = dist(*start, *end);
            if len <= EPS {
                return None;
            }
            let (ux, uy) = ((end.x - start.x) / len, (end.y - start.y) / len);
            let t = (p.x - start.x) * ux + (p.y - start.y) * uy;
            Some(Point::new(start.x + ux * t, start.y + uy * t))
        }
        Entity::Arc { center, radius, .. } | Entity::Circle { center, radius } => {
            let len = dist(*center, p);
            (len > EPS && *radius > EPS).then(|| {
                Point::new(
                    center.x + (p.x - center.x) / len * radius,
                    center.y + (p.y - center.y) / len * radius,
                )
            })
        }
        Entity::Spline(s) => Some(s.nearest(p).0),
        _ => None,
    }
}

/// Curves a circle of `radius` could be centred on to touch `e`: its offsets on
/// both sides, with a line and an arc made whole so that an extension counts.
fn offsets(e: &Entity, radius: f64, reach: f64) -> Vec<Entity> {
    match e {
        Entity::Line { start, end } => {
            let len = dist(*start, *end);
            if len <= EPS {
                return Vec::new();
            }
            let (ux, uy) = ((end.x - start.x) / len, (end.y - start.y) / len);
            [radius, -radius]
                .into_iter()
                .map(|d| {
                    let (ox, oy) = (start.x - uy * d, start.y + ux * d);
                    Entity::Line {
                        start: Point::new(ox - ux * reach, oy - uy * reach),
                        end: Point::new(ox + ux * reach, oy + uy * reach),
                    }
                })
                .collect()
        }
        Entity::Arc {
            center, radius: r, ..
        }
        | Entity::Circle { center, radius: r } => {
            let mut out = vec![Entity::Circle {
                center: *center,
                radius: r + radius,
            }];
            if (r - radius).abs() > EPS {
                out.push(Entity::Circle {
                    center: *center,
                    radius: (r - radius).abs(),
                });
            }
            out
        }
        Entity::Spline(s) => [radius, -radius]
            .into_iter()
            .filter_map(|d| {
                e.offset_spline_left(d)
                    .filter(|_| !s.closed || d.is_finite())
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The centre near `guess` that is exactly `radius` from both `a` and `b`,
/// by Newton's method on the two distances.
fn refine(a: &Entity, b: &Entity, radius: f64, guess: Point, scale: f64) -> Option<Point> {
    let mut c = guess;
    for _ in 0..30 {
        let (qa, qb) = (nearest_on(a, c)?, nearest_on(b, c)?);
        let (da, db) = (dist(c, qa), dist(c, qb));
        let (fa, fb) = (da - radius, db - radius);
        if fa.abs().max(fb.abs()) <= 1e-12 * scale.max(1.0) {
            return Some(c);
        }
        if da <= EPS || db <= EPS {
            return None;
        }
        // Solve [n_a; n_b] · step = -(fa, fb), n being the unit vector from the
        // nearest point to the centre.
        let (na, nb) = (
            Point::new((c.x - qa.x) / da, (c.y - qa.y) / da),
            Point::new((c.x - qb.x) / db, (c.y - qb.y) / db),
        );
        let det = na.x * nb.y - na.y * nb.x;
        if det.abs() < 1e-9 {
            return None;
        }
        let (rx, ry) = (-fa, -fb);
        c = Point::new(
            c.x + (rx * nb.y - na.y * ry) / det,
            c.y + (na.x * ry - rx * nb.x) / det,
        );
    }
    let (qa, qb) = (nearest_on(a, c)?, nearest_on(b, c)?);
    let off = (dist(c, qa) - radius)
        .abs()
        .max((dist(c, qb) - radius).abs());
    (off <= 1e-7 * scale.max(1.0)).then_some(c)
}

/// The part of a spline that stays when it is cut at `at`, on the side of
/// `picked`.
fn spline_keep(s: &Spline, at: Point, picked: Point) -> Result<Entity, CornerError> {
    if s.closed || s.periodic {
        return Err(CornerError::Unsupported);
    }
    let (_, u) = s.nearest(at);
    let (_, picked_at) = s.nearest(picked);
    let (first, second) = s.split_at_parameter(u).ok_or(CornerError::TooLong)?;
    Ok(Entity::Spline(Box::new(if picked_at >= u {
        second
    } else {
        first
    })))
}

fn keep(e: &Entity, at: Point, picked: Point) -> Result<Entity, CornerError> {
    match e {
        Entity::Spline(s) => spline_keep(s, at, picked),
        other => keep_part(other, at, picked),
    }
}

fn supported(e: &Entity) -> bool {
    matches!(
        e,
        Entity::Line { .. } | Entity::Arc { .. } | Entity::Circle { .. } | Entity::Spline(_)
    )
}

/// Round the corner of `a` and `b`, at least one a spline, with an arc of
/// `radius`; with a radius of zero the two meet at a point instead.
pub(crate) fn fillet_spline(
    a: &Entity,
    b: &Entity,
    radius: f64,
    picked_a: Point,
    picked_b: Point,
) -> Result<Joined, CornerError> {
    if !(supported(a) && supported(b)) {
        return Err(CornerError::Unsupported);
    }
    let (ba, bb) = (a.bounds(), b.bounds());
    let both = ba.union(bb);
    let scale = (both.max.x - both.min.x).hypot(both.max.y - both.min.y) + radius;
    let reach = 20.0 * scale.max(1.0);
    let round = radius.is_finite() && radius > EPS;
    // (point on a, point on b, centre) of each way the corner can be made.
    let mut options: Vec<(Point, Point, Point)> = Vec::new();
    if round {
        for oa in offsets(a, radius, reach) {
            for ob in offsets(b, radius, reach) {
                for guess in oa.intersections(&ob) {
                    let Some(c) = refine(a, b, radius, guess, scale) else {
                        continue;
                    };
                    if let (Some(ta), Some(tb)) = (nearest_on(a, c), nearest_on(b, c)) {
                        if dist(ta, tb) > EPS
                            && !options.iter().any(|o| dist(o.2, c) < 1e-7 * scale)
                        {
                            options.push((ta, tb, c));
                        }
                    }
                }
            }
        }
    } else {
        let whole = |e: &Entity| match e {
            Entity::Line { start, end } => {
                let len = dist(*start, *end);
                let (ux, uy) = (
                    (end.x - start.x) / len.max(EPS),
                    (end.y - start.y) / len.max(EPS),
                );
                Entity::Line {
                    start: Point::new(start.x - ux * reach, start.y - uy * reach),
                    end: Point::new(end.x + ux * reach, end.y + uy * reach),
                }
            }
            other => other.clone(),
        };
        for corner in whole(a).intersections(&whole(b)) {
            options.push((corner, corner, corner));
        }
    }
    let score = |o: &(Point, Point, Point)| dist(o.0, picked_a) + dist(o.1, picked_b);
    let (ta, tb, center) = options
        .into_iter()
        .min_by(|x, y| score(x).total_cmp(&score(y)))
        .ok_or(CornerError::NoFit)?;
    Ok(Joined {
        a: keep(a, ta, picked_a)?,
        b: keep(b, tb, picked_b)?,
        joint: round.then(|| joint_arc(center, radius, ta, tb)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    /// An arch from (0, 0) over a crest near y = 4.5 to (10, 0).
    fn arch() -> Entity {
        Entity::Spline(Box::new(
            Spline::from_control_points(
                3,
                vec![p(0.0, 0.0), p(2.0, 6.0), p(8.0, 6.0), p(10.0, 0.0)],
            )
            .unwrap(),
        ))
    }

    fn spline_of(e: &Entity) -> &Spline {
        match e {
            Entity::Spline(s) => s,
            other => panic!("a spline, got {other:?}"),
        }
    }

    fn arc_of(e: &Option<Entity>) -> (Point, f64) {
        match e {
            Some(Entity::Arc { center, radius, .. }) => (*center, *radius),
            other => panic!("an arc, got {other:?}"),
        }
    }

    #[test]
    fn a_spline_and_a_line_are_joined_by_an_arc_that_touches_both() {
        // A vertical line cutting the arch's left flank, picked on the parts
        // to keep: the arch to its right and the wall above.
        let wall = line(p(3.0, -2.0), p(3.0, 12.0));
        let done = fillet_spline(&arch(), &wall, 1.0, p(7.0, 3.9), p(3.0, 9.0)).unwrap();
        let (centre, radius) = arc_of(&done.joint);
        assert!((radius - 1.0).abs() < 1e-12);
        // The arc touches the spline and the wall exactly.
        let Entity::Spline(base) = arch() else {
            unreachable!()
        };
        assert!((base.distance_to(centre) - 1.0).abs() < 1e-8);
        assert!(((centre.x - 3.0).abs() - 1.0).abs() < 1e-8);
        // The line keeps the upper part, ending at the arc's foot on it.
        let Entity::Line { start, end } = done.b else {
            panic!("a line")
        };
        assert!(start.y > 4.0 || end.y > 4.0);
        // The spline keeps the right-hand part, from the arc's foot on it.
        let kept = spline_of(&done.a);
        assert!(kept.start_point().x > 3.0 || kept.end_point().x > 3.0);
        assert!(
            (kept.end_point().x - 10.0).abs() < 1e-9 || (kept.start_point().x - 10.0).abs() < 1e-9
        );
        let foot = Entity::Spline(Box::new(kept.clone()))
            .nearest_point(centre)
            .unwrap();
        assert!((dist(foot, centre) - 1.0).abs() < 1e-8);
    }

    #[test]
    fn two_splines_are_joined_and_a_radius_of_zero_makes_a_sharp_corner() {
        // Two arches crossing at (5, y).
        let other = Entity::Spline(Box::new(
            Spline::from_control_points(
                3,
                vec![p(10.0, 0.0), p(8.0, 6.0), p(2.0, 6.0), p(0.0, 0.0)],
            )
            .unwrap(),
        ));
        let sharp = fillet_spline(&arch(), &other, 0.0, p(1.0, 1.0), p(9.0, 1.0));
        // The same arch twice (reversed) lies on itself: there is no corner.
        assert!(sharp.is_err() || sharp.unwrap().joint.is_none());
        // A cross of two different curves.
        let cross = Entity::Spline(Box::new(
            Spline::from_control_points(
                3,
                vec![p(2.0, 6.0), p(4.0, 2.0), p(6.0, 2.0), p(8.0, 6.0)],
            )
            .unwrap(),
        ));
        let sharp = fillet_spline(&arch(), &cross, 0.0, p(1.0, 3.0), p(3.0, 4.0)).unwrap();
        assert!(sharp.joint.is_none());
        let end_of = |e: &Entity| {
            let s = spline_of(e);
            [s.start_point(), s.end_point()]
        };
        // Both are cut at one shared point.
        let shared = end_of(&sharp.a)
            .iter()
            .any(|p| end_of(&sharp.b).iter().any(|q| dist(*p, *q) < 1e-3));
        assert!(shared, "{:?} {:?}", end_of(&sharp.a), end_of(&sharp.b));
    }

    #[test]
    fn a_spline_and_a_circle_get_an_arc_between_them_and_the_circle_is_whole() {
        // The circle's foot is 1.1 above the crest: an arc of radius 0.6 fits
        // beside the crest and touches both.
        let circle = Entity::Circle {
            center: p(5.0, 7.6),
            radius: 2.0,
        };
        let done = fillet_spline(&arch(), &circle, 0.6, p(7.0, 3.9), p(6.8, 6.7)).unwrap();
        assert_eq!(done.b, circle);
        assert!(done.joint.is_some());
    }

    #[test]
    fn what_cannot_fit_is_said() {
        // Parallel to the arch's crest, far away: nothing to round.
        let far = line(p(-50.0, 40.0), p(50.0, 40.0));
        assert_eq!(
            fillet_spline(&arch(), &far, 1.0, p(5.0, 4.0), p(5.0, 40.0)).err(),
            Some(CornerError::NoFit)
        );
        let text = Entity::Point { at: p(0.0, 0.0) };
        assert_eq!(
            fillet_spline(&arch(), &text, 1.0, p(5.0, 4.0), p(0.0, 0.0)).err(),
            Some(CornerError::Unsupported)
        );
    }
}
