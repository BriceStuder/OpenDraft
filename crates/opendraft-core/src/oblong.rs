//! The oblong (a slot): two half circles joined by two straight sides, drawn
//! as one closed polyline from the centres of its half circles and its width.

use crate::{Entity, Point};

/// The oblong whose half circles are centred on `a` and `b`, `width` across
/// (twice their radius): a closed polyline of four vertices, two straight
/// sides and two half circles, drawn counter-clockwise from the side on the
/// right of `a` to `b`. `None` when the centres meet or the width is not
/// positive.
pub fn outline(a: Point, b: Point, width: f64) -> Option<Entity> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx.hypot(dy);
    let radius = width / 2.0;
    if !(length > 0.0 && radius > 0.0 && radius.is_finite()) {
        return None;
    }
    // Across the oblong, to the left of a -> b, as long as the radius.
    let n = (-dy / length * radius, dx / length * radius);
    let off = |p: Point, side: f64| Point::new(p.x + n.0 * side, p.y + n.1 * side);
    Some(Entity::Polyline {
        points: vec![off(a, -1.0), off(b, -1.0), off(b, 1.0), off(a, 1.0)],
        closed: true,
        bulges: vec![0.0, 1.0, 0.0, 1.0],
        widths: Vec::new(),
    })
}

/// The width of the oblong centred on `a` and `b` whose side passes through
/// `at`: twice its distance from the line through the centres (0 when they
/// meet).
pub fn width_through(a: Point, b: Point, at: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx.hypot(dy);
    if length.is_nan() || length <= 0.0 {
        return 0.0;
    }
    2.0 * (dx * (at.y - a.y) - dy * (at.x - a.x)).abs() / length
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bulge;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn an_oblong_is_two_half_circles_joined_by_straight_sides() {
        let Some(Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        }) = outline(p(0.0, 0.0), p(10.0, 0.0), 4.0)
        else {
            panic!("an oblong")
        };
        assert!(closed);
        assert_eq!(
            points,
            [p(0.0, -2.0), p(10.0, -2.0), p(10.0, 2.0), p(0.0, 2.0)]
        );
        assert_eq!(bulges, [0.0, 1.0, 0.0, 1.0]);
        let length = bulge::path_length(&points, &bulges, true);
        let pi = std::f64::consts::PI;
        assert!((length - (20.0 + 4.0 * pi)).abs() < 1e-9);
        let area = bulge::path_area(&points, &bulges);
        assert!((area.abs() - (40.0 + 4.0 * pi)).abs() < 1e-9);
    }

    #[test]
    fn an_oblong_lies_along_its_centres_at_any_angle() {
        let Some(Entity::Polyline { points, .. }) = outline(p(1.0, 1.0), p(1.0, 7.0), 2.0) else {
            panic!("an oblong")
        };
        assert_eq!(points, [p(2.0, 1.0), p(2.0, 7.0), p(0.0, 7.0), p(0.0, 1.0)]);
    }

    #[test]
    fn centres_that_meet_or_no_width_give_no_oblong() {
        assert_eq!(outline(p(1.0, 1.0), p(1.0, 1.0), 2.0), None);
        assert_eq!(outline(p(0.0, 0.0), p(5.0, 0.0), 0.0), None);
        assert_eq!(outline(p(0.0, 0.0), p(5.0, 0.0), -1.0), None);
    }

    #[test]
    fn the_width_through_a_point_is_twice_its_distance_from_the_centres_line() {
        let (a, b) = (p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(width_through(a, b, p(3.0, 1.5)), 3.0);
        assert_eq!(width_through(a, b, p(30.0, -2.0)), 4.0);
        assert_eq!(width_through(a, a, p(3.0, 1.5)), 0.0);
    }
}
