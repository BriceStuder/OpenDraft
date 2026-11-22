//! Ortho: keeping a point on the horizontal or vertical line through a base
//! point. Pure geometry; the canvas decides when to apply it. In a custom
//! coordinate system "horizontal" is along its X axis (the `_turned` forms).

use crate::ccs::Ccs;
use crate::Point;

/// The direction a point is held to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Same y as the base; x is free.
    Horizontal,
    /// Same x as the base; y is free.
    Vertical,
}

/// The axis from `base` that `p` lies nearer to (horizontal when the offsets
/// tie).
pub fn axis_of(base: Point, p: Point) -> Axis {
    if (p.x - base.x).abs() >= (p.y - base.y).abs() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    }
}

/// `p` moved onto `axis` through `base`.
pub fn on_axis(base: Point, p: Point, axis: Axis) -> Point {
    match axis {
        Axis::Horizontal => Point::new(p.x, base.y),
        Axis::Vertical => Point::new(base.x, p.y),
    }
}

/// `p` moved onto whichever axis through `base` it is nearer to.
pub fn constrain(base: Point, p: Point) -> Point {
    on_axis(base, p, axis_of(base, p))
}

/// [`axis_of`] with the axes turned by `angle` (a custom coordinate
/// system's): horizontal is along its X axis.
pub fn axis_of_turned(base: Point, p: Point, angle: f64) -> Axis {
    if angle == 0.0 {
        return axis_of(base, p);
    }
    axis_of(Point::new(0.0, 0.0), Ccs::new(base, angle).from_world(p))
}

/// [`on_axis`] with the axes turned by `angle`. Unturned, it is exactly
/// [`on_axis`].
pub fn on_axis_turned(base: Point, p: Point, axis: Axis, angle: f64) -> Point {
    if angle == 0.0 {
        return on_axis(base, p, axis);
    }
    let frame = Ccs::new(base, angle);
    frame.to_world(on_axis(Point::new(0.0, 0.0), frame.from_world(p), axis))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_mostly_sideways_point_goes_horizontal() {
        assert_eq!(constrain(p(1.0, 1.0), p(10.0, 3.0)), p(10.0, 1.0));
    }

    #[test]
    fn a_mostly_upward_point_goes_vertical() {
        assert_eq!(constrain(p(1.0, 1.0), p(2.0, -9.0)), p(1.0, -9.0));
    }

    #[test]
    fn a_tie_goes_horizontal() {
        assert_eq!(axis_of(p(0.0, 0.0), p(5.0, -5.0)), Axis::Horizontal);
    }

    #[test]
    fn turned_axes_hold_along_the_system() {
        let angle = std::f64::consts::FRAC_PI_4;
        let base = p(1.0, 1.0);
        let cursor = p(5.0, 4.0);
        assert_eq!(axis_of_turned(base, cursor, angle), Axis::Horizontal);
        let held = on_axis_turned(base, cursor, Axis::Horizontal, angle);
        assert!((held.x - held.y).abs() < 1e-9, "on the 45 degree line");
        assert!((held.x - 4.5).abs() < 1e-9);
        let up = on_axis_turned(base, p(-3.0, 5.0), Axis::Vertical, angle);
        assert!(
            (up.x - 1.0 + up.y - 1.0).abs() < 1e-9,
            "on the 135 degree line"
        );
        assert_eq!(
            on_axis_turned(base, cursor, Axis::Vertical, 0.0),
            p(1.0, 4.0)
        );
    }

    #[test]
    fn a_point_on_the_base_stays_there() {
        assert_eq!(constrain(p(3.0, 4.0), p(3.0, 4.0)), p(3.0, 4.0));
    }
}
