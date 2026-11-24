//! FILLET between two parallel lines, as AutoCAD does it: a half circle whose
//! diameter is the gap between the lines, whatever the current radius.
//!
//! The first line is the reference and is left as it is; the half circle
//! starts at its end nearest the point picked on it. The second line is cut
//! back or extended so that it ends level with that end, where the half
//! circle meets it.

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::corner::{CornerError, Joined};
use crate::{Entity, Point};

/// Lengths below this are nothing.
const EPS: f64 = 1e-9;

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

fn minus(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}

fn plus(a: Point, b: Point, times: f64) -> Point {
    Point::new(a.x + b.x * times, a.y + b.y * times)
}

/// The unit vector along a line, or `None` for a line of no length.
fn direction(start: Point, end: Point) -> Option<(Point, f64)> {
    let d = minus(end, start);
    let len = d.x.hypot(d.y);
    (len > EPS).then(|| (Point::new(d.x / len, d.y / len), len))
}

/// The half circle joining parallel lines `a` and `b`. `None` when they are
/// not two parallel lines (then the ordinary fillet applies), an error when
/// they lie on one line or no half circle can be drawn.
pub(crate) fn fillet_parallel(
    a: &Entity,
    b: &Entity,
    picked_a: Point,
) -> Option<Result<Joined, CornerError>> {
    let (Entity::Line { start: a0, end: a1 }, Entity::Line { start: b0, end: b1 }) = (a, b) else {
        return None;
    };
    let (u, len_a) = direction(*a0, *a1)?;
    let (v, _) = direction(*b0, *b1)?;
    // Not parallel: the lines meet somewhere.
    if (u.x * v.y - u.y * v.x).abs() > EPS {
        return None;
    }
    Some(semicircle(
        a.clone(),
        (*a0, *a1),
        u,
        len_a,
        (*b0, *b1),
        picked_a,
    ))
}

fn semicircle(
    a: Entity,
    (a0, a1): (Point, Point),
    u: Point,
    len_a: f64,
    (b0, b1): (Point, Point),
    picked_a: Point,
) -> Result<Joined, CornerError> {
    // Sideways from line a to line b (signed gap times the left normal).
    let left = Point::new(-u.y, u.x);
    let gap = dot(minus(b0, a0), left);
    if gap.abs() <= EPS * (1.0 + len_a) {
        // On one line: no gap to bridge.
        return Err(CornerError::Collinear);
    }
    // The end of line a nearest the pick, and the way out past it.
    let picked_end = dot(minus(picked_a, a0), u) >= len_a / 2.0;
    let (tip, out) = if picked_end {
        (a1, u)
    } else {
        (a0, Point::new(-u.x, -u.y))
    };
    let center = plus(tip, left, gap / 2.0);
    let landing = plus(tip, left, gap);
    // Line b keeps the end farther back from the half circle; the other end
    // moves to where the half circle lands.
    let back = |p: Point| dot(minus(p, landing), out);
    let keeps_start = back(b0) <= back(b1);
    let far = if keeps_start { b0 } else { b1 };
    if back(far) >= -EPS {
        return Err(CornerError::PastTheEnd);
    }
    let new_b = if keeps_start {
        Entity::Line {
            start: far,
            end: landing,
        }
    } else {
        Entity::Line {
            start: landing,
            end: far,
        }
    };
    // Counter-clockwise through the point farthest out.
    let bulge = out.y.atan2(out.x);
    let joint = Entity::Arc {
        center,
        radius: gap.abs() / 2.0,
        start: (bulge - FRAC_PI_2).rem_euclid(TAU),
        end: (bulge + FRAC_PI_2).rem_euclid(TAU),
    };
    Ok(Joined {
        a,
        b: new_b,
        joint: Some(joint),
    })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-9
    }

    fn has_ends(e: &Entity, a: Point, b: Point) -> bool {
        matches!(e, Entity::Line { start, end } if near(*start, a) && near(*end, b))
    }

    fn arc_of(done: &Joined) -> (Point, f64, f64, f64) {
        match &done.joint {
            Some(Entity::Arc {
                center,
                radius,
                start,
                end,
            }) => (*center, *radius, *start, *end),
            other => panic!("expected an arc, got {other:?}"),
        }
    }

    fn angle_near(a: f64, b: f64) -> bool {
        let d = (a - b).rem_euclid(TAU);
        d < 1e-9 || TAU - d < 1e-9
    }

    #[test]
    fn a_half_circle_spans_the_gap_at_the_end_picked_on_the_first_line() {
        // As in the user's drawing: line 1 on top, line 2 below and longer
        // on the left; picked near their left ends.
        let top = line(p(0.0, 10.0), p(20.0, 10.0));
        let bottom = line(p(-8.0, 0.0), p(25.0, 0.0));
        let done = fillet_parallel(&top, &bottom, p(2.0, 10.0))
            .unwrap()
            .unwrap();
        assert_eq!(done.a, top, "the first line is left as it is");
        assert!(
            has_ends(&done.b, p(0.0, 0.0), p(25.0, 0.0)),
            "the second line is cut back level with the first: {:?}",
            done.b
        );
        let (center, radius, start, end) = arc_of(&done);
        assert!(near(center, p(0.0, 5.0)) && (radius - 5.0).abs() < 1e-9);
        // It bulges to the left: from the top (π/2) round to the bottom.
        assert!(angle_near(start, FRAC_PI_2) && angle_near(end, 3.0 * FRAC_PI_2));
    }

    #[test]
    fn the_second_line_is_extended_when_it_is_short() {
        let top = line(p(0.0, 10.0), p(20.0, 10.0));
        let bottom = line(p(5.0, 0.0), p(15.0, 0.0));
        let done = fillet_parallel(&top, &bottom, p(18.0, 10.0))
            .unwrap()
            .unwrap();
        assert!(has_ends(&done.b, p(5.0, 0.0), p(20.0, 0.0)), "{:?}", done.b);
        let (center, _, start, end) = arc_of(&done);
        assert!(near(center, p(20.0, 5.0)));
        // Bulges to the right: from the bottom (-π/2) round to the top.
        assert!(angle_near(start, -FRAC_PI_2) && angle_near(end, FRAC_PI_2));
    }

    #[test]
    fn the_radius_is_half_the_gap_and_the_second_line_keeps_its_direction() {
        // Lines drawn in opposite directions, gap of 3, at an angle.
        let u = p((PI / 6.0).cos(), (PI / 6.0).sin());
        let left = p(-u.y, u.x);
        let at = |t: f64, side: f64| p(u.x * t + left.x * side, u.y * t + left.y * side);
        let a = line(at(0.0, 0.0), at(10.0, 0.0));
        let b = line(at(12.0, 3.0), at(-4.0, 3.0));
        let done = fillet_parallel(&a, &b, at(9.0, 0.0)).unwrap().unwrap();
        let (center, radius, ..) = arc_of(&done);
        assert!((radius - 1.5).abs() < 1e-9 && near(center, at(10.0, 1.5)));
        assert!(
            has_ends(&done.b, at(10.0, 3.0), at(-4.0, 3.0)),
            "{:?}",
            done.b
        );
    }

    #[test]
    fn lines_that_meet_are_left_to_the_ordinary_fillet() {
        let a = line(p(0.0, 0.0), p(10.0, 0.0));
        let b = line(p(0.0, 0.0), p(0.0, 10.0));
        assert!(fillet_parallel(&a, &b, p(5.0, 0.0)).is_none());
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        assert!(fillet_parallel(&a, &circle, p(5.0, 0.0)).is_none());
    }

    #[test]
    fn lines_on_one_line_or_past_the_end_are_refused() {
        let a = line(p(0.0, 0.0), p(10.0, 0.0));
        let same_line = line(p(12.0, 0.0), p(20.0, 0.0));
        assert_eq!(
            fillet_parallel(&a, &same_line, p(9.0, 0.0)),
            Some(Err(CornerError::Collinear))
        );
        // The second line lies wholly beyond the end picked on the first.
        let beyond = line(p(12.0, 4.0), p(20.0, 4.0));
        assert_eq!(
            fillet_parallel(&a, &beyond, p(9.0, 0.0)),
            Some(Err(CornerError::PastTheEnd))
        );
    }
}
