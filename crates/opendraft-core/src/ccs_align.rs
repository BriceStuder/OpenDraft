//! The coordinate system an object sets (the CCS command's Object option).
//! Following AutoCAD's rules where they fit 2D, with one choice of our own:
//! for a polyline or rectangle the segment nearest the pick is used (AutoCAD
//! always takes a polyline's first segment).

use crate::bulge;
use crate::ccs::Ccs;
use crate::{Entity, Point};

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// Origin at whichever end of `a`-`b` is nearer `pick`, X towards the other.
fn along(a: Point, b: Point, pick: Point) -> Option<Ccs> {
    if distance(a, pick) <= distance(b, pick) {
        Ccs::through(a, b)
    } else {
        Ccs::through(b, a)
    }
}

/// Origin at `center`, X towards `towards`.
fn from_center(center: Point, towards: Point) -> Option<Ccs> {
    Ccs::through(center, towards)
}

/// A segment of a path: straight from `a` to `b`, or an arc by `bulge`.
fn segment(a: Point, b: Point, bulge: f64, pick: Point) -> Option<Ccs> {
    match bulge::arc_of(a, b, bulge) {
        Some(arc) => {
            let end = if distance(a, pick) <= distance(b, pick) {
                a
            } else {
                b
            };
            from_center(arc.center, end)
        }
        None => along(a, b, pick),
    }
}

/// The segment of a path nearest `pick`.
fn nearest_segment(points: &[Point], bulges: &[f64], closed: bool, pick: Point) -> Option<Ccs> {
    let count = bulge::segment_count(points.len(), closed);
    let best = (0..count)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            let (near, _) = bulge::nearest(a, b, bulge::at(bulges, i), pick);
            (i, distance(near, pick))
        })
        .min_by(|x, y| x.1.total_cmp(&y.1))?;
    let i = best.0;
    let (a, b) = (points[i], points[(i + 1) % points.len()]);
    segment(a, b, bulge::at(bulges, i), pick)
}

/// The system `entity`, picked at `pick`, sets; none for an object that
/// sets no direction. `current` is the system in force, whose turn a point
/// object keeps.
pub fn aligned_to(entity: &Entity, pick: Point, current: Ccs) -> Option<Ccs> {
    let turned = |origin: Point, angle: f64| Some(Ccs::new(origin, angle));
    match entity {
        Entity::Line { start, end } => along(*start, *end, pick),
        Entity::Circle { center, .. } => from_center(*center, pick),
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => {
            let at = |a: f64| Point::new(center.x + radius * a.cos(), center.y + radius * a.sin());
            let (s, e) = (at(*start), at(*end));
            from_center(
                *center,
                if distance(s, pick) <= distance(e, pick) {
                    s
                } else {
                    e
                },
            )
        }
        Entity::Rectangle { min, max } => {
            let corners = [
                *min,
                Point::new(max.x, min.y),
                *max,
                Point::new(min.x, max.y),
            ];
            nearest_segment(&corners, &[], true, pick)
        }
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => nearest_segment(points, bulges, *closed, pick),
        Entity::Ellipse(e) => {
            let major = Point::new(e.center.x + e.major_axis.x, e.center.y + e.major_axis.y);
            let minor = Point::new(e.center.x - e.major_axis.x, e.center.y - e.major_axis.y);
            let end = if distance(major, pick) <= distance(minor, pick) {
                major
            } else {
                minor
            };
            from_center(e.center, end)
        }
        Entity::Text(t) => turned(t.placement.position, t.placement.rotation),
        Entity::AttributeDefinition(a) => turned(a.text.position, a.text.rotation),
        Entity::MText(m) => turned(m.insertion, m.direction.y.atan2(m.direction.x)),
        Entity::Insert(i) => turned(i.insertion, i.rotation),
        Entity::Point { at } => Some(current.moved_to(*at)),
        Entity::Xline { origin, direction } | Entity::Ray { origin, direction } => {
            turned(*origin, direction.y.atan2(direction.x))
        }
        Entity::Dimension(_)
        | Entity::Spline(_)
        | Entity::Hatch(_)
        | Entity::MLeader(_)
        | Entity::Leader(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn close(c: Option<Ccs>, origin: Point, angle: f64) -> bool {
        c.is_some_and(|c| {
            distance(c.origin, origin) < 1e-9
                && (c.angle - angle.rem_euclid(std::f64::consts::TAU)).abs() < 1e-9
        })
    }

    #[test]
    fn a_line_sets_its_nearer_end_and_runs_along_it() {
        let line = Entity::Line {
            start: p(0.0, 0.0),
            end: p(0.0, 10.0),
        };
        assert!(close(
            aligned_to(&line, p(0.0, 1.0), Ccs::WORLD),
            p(0.0, 0.0),
            FRAC_PI_2
        ));
        assert!(close(
            aligned_to(&line, p(0.0, 9.0), Ccs::WORLD),
            p(0.0, 10.0),
            -FRAC_PI_2
        ));
    }

    #[test]
    fn circles_and_arcs_set_their_centre() {
        let circle = Entity::Circle {
            center: p(5.0, 5.0),
            radius: 2.0,
        };
        assert!(close(
            aligned_to(&circle, p(5.0, 7.0), Ccs::WORLD),
            p(5.0, 5.0),
            FRAC_PI_2
        ));
        let arc = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 1.0,
            start: 0.0,
            end: FRAC_PI_2,
        };
        assert!(close(
            aligned_to(&arc, p(0.1, 0.9), Ccs::WORLD),
            p(0.0, 0.0),
            FRAC_PI_2
        ));
    }

    #[test]
    fn a_polyline_uses_the_segment_nearest_the_pick() {
        let path = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert!(close(
            aligned_to(&path, p(10.2, 8.0), Ccs::WORLD),
            p(10.0, 10.0),
            -FRAC_PI_2
        ));
        let rect = Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(4.0, 2.0),
        };
        assert!(close(
            aligned_to(&rect, p(3.5, 2.1), Ccs::WORLD),
            p(4.0, 2.0),
            PI
        ));
    }

    #[test]
    fn a_point_keeps_the_current_turn_and_a_spline_sets_nothing() {
        let current = Ccs::new(p(0.0, 0.0), 0.3);
        let point = Entity::Point { at: p(2.0, 3.0) };
        assert!(close(
            aligned_to(&point, p(2.0, 3.0), current),
            p(2.0, 3.0),
            0.3
        ));
        let ray = Entity::Ray {
            origin: p(1.0, 1.0),
            direction: p(0.0, -1.0),
        };
        assert!(close(
            aligned_to(&ray, p(1.0, 0.0), current),
            p(1.0, 1.0),
            -FRAC_PI_2
        ));
    }
}
