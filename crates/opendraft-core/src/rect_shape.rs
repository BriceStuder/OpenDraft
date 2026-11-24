//! Recognising a rectangle in a polyline, whichever command drew it and at
//! whatever angle, so that its sides, angle and corners can be read and
//! changed. Its corners may be square, or all four rounded or bevelled alike
//! (as RECTANGLE's Fillet and Chamfer make them).

use std::f64::consts::FRAC_PI_8;

use crate::Point;

/// How the four corners of a recognised rectangle are cut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cut {
    Square,
    /// Rounded by quarter circles of this radius.
    Round(f64),
    /// Bevelled by this distance along both sides.
    Bevel(f64),
}

impl Cut {
    /// How far back from the sharp corner the cut starts (0 when square).
    pub fn size(self) -> f64 {
        match self {
            Cut::Square => 0.0,
            Cut::Round(d) | Cut::Bevel(d) => d,
        }
    }
}

/// A rectangle read from a polyline: the sharp corner its first side starts
/// from, the direction and length of the first side, the length of the
/// second, which way the corners turn and how they are cut. The lengths run
/// to the sharp corners, also when the corners are cut away.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Point,
    /// Direction of the first side, in radians.
    pub angle: f64,
    /// Length of the first side.
    pub width: f64,
    /// Length of the second side.
    pub height: f64,
    /// Whether the corners turn counter-clockwise.
    pub left: bool,
    pub cut: Cut,
    /// With cut corners, whether the polyline starts with a side rather than
    /// with a corner (RECTANGLE starts with a corner).
    pub side_first: bool,
}

impl Frame {
    /// The four sharp corners in drawing order, starting at `origin`.
    pub fn corners(&self) -> [Point; 4] {
        let (u, v) = self.axes();
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

    /// Whether the sides are long enough for the cut corners, each keeping a
    /// straight part.
    pub fn fits(&self) -> bool {
        let d = self.cut.size();
        let cut_ok = self.cut == Cut::Square || d > 0.0;
        cut_ok && self.width > 2.0 * d && self.height > 2.0 * d
    }

    /// The polyline's vertices in drawing order: the four corners, or with
    /// cut corners the eight ends of the sides.
    pub fn vertices(&self) -> Vec<Point> {
        let corners = self.corners();
        let d = self.cut.size();
        if self.cut == Cut::Square {
            return corners.to_vec();
        }
        let (u, v) = self.axes();
        let along = [u, v, (-u.0, -u.1), (-v.0, -v.1)];
        let mut out = Vec::with_capacity(8);
        for (k, c) in corners.iter().enumerate() {
            let (before, after) = (along[(k + 3) % 4], along[k]);
            out.push(Point::new(c.x - before.0 * d, c.y - before.1 * d));
            out.push(Point::new(c.x + after.0 * d, c.y + after.1 * d));
        }
        if self.side_first {
            out.rotate_left(1);
        }
        out
    }

    /// The directions of the first and second sides.
    fn axes(&self) -> ((f64, f64), (f64, f64)) {
        let u = (self.angle.cos(), self.angle.sin());
        let v = if self.left { (-u.1, u.0) } else { (u.1, -u.0) };
        (u, v)
    }
}

/// The rectangle a polyline traces, if it is one: four straight sides at
/// right angles, either joined directly or by four corners alike (quarter
/// circles of one radius, or bevels the same distance along both sides),
/// closed or with a last vertex back on the first. The small errors a
/// rotation leaves are tolerated, in proportion to the size of the shape and
/// of the coordinates.
pub fn of_polyline(points: &[Point], bulges: &[f64], closed: bool) -> Option<Frame> {
    let first = *points.first()?;
    let reach = points.iter().map(|q| gap(*q, first)).fold(0.0, f64::max);
    let size = points
        .iter()
        .map(|q| q.x.abs().max(q.y.abs()))
        .fold(0.0, f64::max);
    let tol = 1e-9 * reach + 1e-12 * size;
    let ring = match points {
        [.., last] if matches!(points.len(), 5 | 9) && gap(*last, first) <= tol => {
            &points[..points.len() - 1]
        }
        _ if closed && matches!(points.len(), 4 | 8) => points,
        _ => return None,
    };
    let bulge = |i: usize| bulges.get(i).copied().unwrap_or(0.0);
    if ring.len() == 4 {
        return read(ring, &bulge, 0, tol);
    }
    // RECTANGLE starts with a corner; another command may start with a side.
    read(ring, &bulge, 1, tol).or_else(|| read(ring, &bulge, 0, tol))
}

/// The rectangle `ring` traces with its first side as segment `start`.
fn read(ring: &[Point], bulge: &dyn Fn(usize) -> f64, start: usize, tol: f64) -> Option<Frame> {
    let n = ring.len();
    let step = n / 4;
    let side = |k: usize| {
        let i = (start + step * k) % n;
        (i, ring[i], ring[(i + 1) % n])
    };
    let vector = |k: usize| {
        let (_, a, b) = side(k);
        sub(b, a)
    };
    let (s0, s1) = (vector(0), vector(1));
    let (len0, len1) = (s0.0.hypot(s0.1), s1.0.hypot(s1.1));
    if len0 <= tol || len1 <= tol {
        return None;
    }
    let u = (s0.0 / len0, s0.1 / len0);
    let left = s0.0 * s1.1 - s0.1 * s1.0 > 0.0;
    let v = if left { (-u.1, u.0) } else { (u.1, -u.0) };
    let along = [u, v, (-u.0, -u.1), (-v.0, -v.1)];
    let close = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1) <= tol;
    let sides_ok = dot(s1, u).abs() <= tol
        && close(vector(2), (-s0.0, -s0.1))
        && close(vector(3), (-s1.0, -s1.1))
        && (0..4).all(|k| bulge(side(k).0) == 0.0);
    if !sides_ok {
        return None;
    }
    let cut = if step == 1 {
        Cut::Square
    } else {
        corner_cut(&side, n, &along, bulge, left, tol)?
    };
    let d = cut.size();
    let (_, start_point, _) = side(0);
    Some(Frame {
        origin: Point::new(start_point.x - u.0 * d, start_point.y - u.1 * d),
        angle: u.1.atan2(u.0),
        width: len0 + 2.0 * d,
        height: len1 + 2.0 * d,
        left,
        cut,
        side_first: start == 0,
    })
}

/// How the four corners between the sides are cut, if all alike: the step
/// from the end of side `k` to the start of the next goes the same distance
/// back along both, straight or as a quarter circle turning with the sides.
fn corner_cut(
    side: &dyn Fn(usize) -> (usize, Point, Point),
    n: usize,
    along: &[(f64, f64); 4],
    bulge: &dyn Fn(usize) -> f64,
    left: bool,
    tol: f64,
) -> Option<Cut> {
    let (corner0, _, end0) = side(0);
    let (_, start1, _) = side(1);
    let d = dot(sub(start1, end0), along[0]);
    let shape = bulge((corner0 + 1) % n);
    for k in 0..4 {
        let (i, _, end) = side(k);
        let (_, start, _) = side((k + 1) % 4);
        let step = sub(start, end);
        let same = (dot(step, along[k]) - d).abs() <= tol
            && (dot(step, along[(k + 1) % 4]) - d).abs() <= tol
            && bulge((i + 1) % n) == shape;
        if !same {
            return None;
        }
    }
    let quarter = FRAC_PI_8.tan();
    let turn = if left { quarter } else { -quarter };
    if d <= tol {
        None
    } else if shape == 0.0 {
        Some(Cut::Bevel(d))
    } else if (shape - turn).abs() <= 1e-9 {
        Some(Cut::Round(d))
    } else {
        None
    }
}

fn gap(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn sub(a: Point, b: Point) -> (f64, f64) {
    (a.x - b.x, a.y - b.y)
}

fn dot(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;
    use crate::rect::{self, Corners};
    use crate::Entity;

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
        assert_eq!(frame.cut, Cut::Square);
        assert_eq!(frame.corners(), points);
        assert_eq!(frame.vertices(), points);
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
            cut: Cut::Square,
            side_first: true,
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

    fn drawn(angle: f64, dx: f64, dy: f64, corners: Corners) -> (Vec<Point>, Vec<f64>) {
        let at = p(-40.0, 25.0);
        match rect::outline(at, angle, dx, dy, corners, 0.0) {
            Entity::Polyline { points, bulges, .. } => (points, bulges),
            e => panic!("a polyline, got {e:?}"),
        }
    }

    #[test]
    fn corners_rounded_or_bevelled_by_rectangle_are_recognised_and_redrawn() {
        for (dx, dy) in [(10.0, 4.0), (10.0, -4.0)] {
            for (corners, cut) in [
                (Corners::Fillet(1.5), Cut::Round(1.5)),
                (Corners::Chamfer(1.5, 1.5), Cut::Bevel(1.5)),
            ] {
                let (points, bulges) = drawn(0.7, dx, dy, corners);
                let frame = of_polyline(&points, &bulges, true).unwrap();
                let kind = std::mem::discriminant(&frame.cut);
                assert_eq!(kind, std::mem::discriminant(&cut));
                assert!((frame.cut.size() - cut.size()).abs() < 1e-9);
                assert!(near(frame.origin, p(-40.0, 25.0)), "the sharp corner");
                assert!((frame.width - 10.0).abs() < 1e-9);
                assert!((frame.height - 4.0).abs() < 1e-9);
                assert!((frame.angle - 0.7).abs() < 1e-12);
                assert!(!frame.side_first);
                let back = frame.vertices();
                assert!(back.iter().zip(&points).all(|(a, b)| near(*a, *b)));
            }
        }
    }

    #[test]
    fn a_polyline_may_start_with_a_side_and_end_back_on_its_first_vertex() {
        let (mut points, mut bulges) = drawn(0.0, 10.0, 4.0, Corners::Fillet(1.0));
        points.rotate_left(1);
        bulges.rotate_left(1);
        points.push(points[0]);
        bulges.push(0.0);
        let frame = of_polyline(&points, &bulges, false).unwrap();
        assert!(frame.side_first);
        assert_eq!(frame.cut, Cut::Round(1.0));
        assert!(near(frame.origin, p(-40.0, 25.0)));
        let back = frame.vertices();
        assert!(back.iter().zip(&points).all(|(a, b)| near(*a, *b)));
    }

    #[test]
    fn corners_that_differ_are_not_recognised() {
        let (points, bulges) = drawn(0.0, 10.0, 4.0, Corners::Chamfer(1.0, 1.5));
        assert_eq!(of_polyline(&points, &bulges, true), None, "unequal bevel");
        let (points, mut bulges) = drawn(0.0, 10.0, 4.0, Corners::Fillet(1.0));
        bulges[2] = 0.0;
        assert_eq!(of_polyline(&points, &bulges, true), None, "one cut corner");
        let (points, mut bulges) = drawn(0.0, 10.0, 4.0, Corners::Fillet(1.0));
        bulges.iter_mut().for_each(|b| *b = -*b);
        assert_eq!(of_polyline(&points, &bulges, true), None, "arcs bulging in");
        let (mut points, bulges) = drawn(0.0, 10.0, 4.0, Corners::Fillet(1.0));
        points[3].x += 0.5;
        assert_eq!(of_polyline(&points, &bulges, true), None, "one radius off");
    }

    #[test]
    fn cut_corners_must_leave_a_straight_part_of_each_side() {
        let (points, bulges) = drawn(0.0, 10.0, 4.0, Corners::Fillet(1.0));
        let mut frame = of_polyline(&points, &bulges, true).unwrap();
        assert!(frame.fits());
        frame.cut = Cut::Round(2.0);
        assert!(!frame.fits(), "the short sides would be all arc");
        frame.cut = Cut::Bevel(0.0);
        assert!(!frame.fits());
    }
}
