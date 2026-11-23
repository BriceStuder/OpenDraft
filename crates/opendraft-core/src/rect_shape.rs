//! Recognising a rectangle in a polyline, whichever command drew it and at
//! whatever angle, so that its sides and angle can be read and changed.

use crate::Point;

/// A rectangle read from a polyline's corners: the first vertex, the
/// direction and length of the first side, the length of the second, and
/// which way the corners turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Point,
    /// Direction of the first side (first vertex to second), in radians.
    pub angle: f64,
    /// Length of the first side.
    pub width: f64,
    /// Length of the second side.
    pub height: f64,
    /// Whether the corners turn counter-clockwise.
    pub left: bool,
}

impl Frame {
    /// The four corners in drawing order, starting at `origin`.
    pub fn corners(&self) -> [Point; 4] {
        let u = (self.angle.cos(), self.angle.sin());
        let v = if self.left { (-u.1, u.0) } else { (u.1, -u.0) };
        let at = |a: f64, b: f64| {
            Point::new(
                self.origin.x + u.0 * a + v.0 * b,
                self.origin.y + u.1 * a + v.1 * b,
            )
        };
        [
            at(0.0, 0.0),
            at(self.width, 0.0),
            at(self.width, self.height),
            at(0.0, self.height),
        ]
    }
}

/// The rectangle a polyline traces, if it is one: four straight sides at
/// right angles, either closed or with a fifth vertex back on the first.
/// The small errors a rotation leaves are tolerated, in proportion to the
/// size of the sides and of the coordinates.
pub fn of_polyline(points: &[Point], bulges: &[f64], closed: bool) -> Option<Frame> {
    if bulges.iter().any(|b| *b != 0.0) {
        return None;
    }
    let (p, back) = match points {
        [a, b, c, d] if closed => ([*a, *b, *c, *d], None),
        [a, b, c, d, e] => ([*a, *b, *c, *d], Some(*e)),
        _ => return None,
    };
    let (a, b) = (sub(p[1], p[0]), sub(p[2], p[1]));
    let (width, height) = (a.0.hypot(a.1), b.0.hypot(b.1));
    let size = p
        .iter()
        .map(|q| q.x.abs().max(q.y.abs()))
        .fold(0.0, f64::max);
    let tol = 1e-9 * width.max(height) + 1e-12 * size;
    let gap = |q: Point, r: Point| (q.x - r.x).hypot(q.y - r.y);
    let fourth = Point::new(p[0].x + b.0, p[0].y + b.1);
    let square = width > tol
        && height > tol
        && (a.0 * b.0 + a.1 * b.1).abs() / height <= tol
        && gap(p[3], fourth) <= tol
        && back.is_none_or(|e| gap(e, p[0]) <= tol);
    square.then(|| Frame {
        origin: p[0],
        angle: a.1.atan2(a.0),
        width,
        height,
        left: a.0 * b.1 - a.1 * b.0 > 0.0,
    })
}

fn sub(a: Point, b: Point) -> (f64, f64) {
    (a.x - b.x, a.y - b.y)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-9
    }

    #[test]
    fn a_closed_four_sided_polyline_at_right_angles_is_a_rectangle() {
        let points = [p(1.0, 2.0), p(11.0, 2.0), p(11.0, 6.0), p(1.0, 6.0)];
        let frame = of_polyline(&points, &[], true).unwrap();
        assert_eq!(frame.origin, p(1.0, 2.0));
        assert_eq!((frame.width, frame.height, frame.angle), (10.0, 4.0, 0.0));
        assert!(frame.left);
        assert_eq!(frame.corners(), points);
    }

    #[test]
    fn the_first_side_gives_the_width_and_the_angle() {
        // Drawn up the short side first, clockwise.
        let points = [p(0.0, 0.0), p(0.0, 4.0), p(10.0, 4.0), p(10.0, 0.0)];
        let frame = of_polyline(&points, &[0.0; 4], true).unwrap();
        assert_eq!((frame.width, frame.height), (4.0, 10.0));
        assert!((frame.angle - FRAC_PI_2).abs() < 1e-12);
        assert!(!frame.left);
        let back = frame.corners();
        assert!(back.iter().zip(points).all(|(a, b)| near(*a, b)));
    }

    #[test]
    fn a_rectangle_turned_by_any_angle_is_still_recognised() {
        let turned = Frame {
            origin: p(1234.5, -987.25),
            angle: 0.3,
            width: 7.0,
            height: 3.0,
            left: true,
        };
        let frame = of_polyline(&turned.corners(), &[], true).unwrap();
        assert!((frame.width - 7.0).abs() < 1e-9 && (frame.height - 3.0).abs() < 1e-9);
        assert!((frame.angle - 0.3).abs() < 1e-12);
    }

    #[test]
    fn a_fifth_vertex_back_on_the_first_also_closes_it() {
        let mut points = vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 2.0), p(0.0, 2.0)];
        assert_eq!(of_polyline(&points, &[], false), None, "three sides");
        points.push(p(0.0, 0.0));
        assert!(of_polyline(&points, &[], false).is_some());
        points[4] = p(0.0, 1.0);
        assert_eq!(of_polyline(&points, &[], false), None, "left open");
    }

    #[test]
    fn other_shapes_are_not_rectangles() {
        let quad = |d: Point| [p(0.0, 0.0), p(5.0, 0.0), p(5.0, 2.0), d];
        assert_eq!(
            of_polyline(&quad(p(0.5, 2.0)), &[], true),
            None,
            "a trapezium"
        );
        let rhombus = [p(0.0, 0.0), p(5.0, 0.0), p(6.0, 2.0), p(1.0, 2.0)];
        assert_eq!(of_polyline(&rhombus, &[], true), None, "a parallelogram");
        let arc = [0.0, 0.4, 0.0, 0.0];
        assert_eq!(
            of_polyline(&quad(p(0.0, 2.0)), &arc, true),
            None,
            "an arc side"
        );
        let flat = [p(0.0, 0.0), p(5.0, 0.0), p(5.0, 0.0), p(0.0, 0.0)];
        assert_eq!(of_polyline(&flat, &[], true), None, "no height");
        let six = [p(0.0, 0.0); 6];
        assert_eq!(of_polyline(&six, &[], true), None);
    }

    #[test]
    fn a_corner_slightly_off_square_is_not_a_rectangle() {
        let points = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 4.0), p(1e-6, 4.0)];
        assert_eq!(of_polyline(&points, &[], true), None);
    }
}
