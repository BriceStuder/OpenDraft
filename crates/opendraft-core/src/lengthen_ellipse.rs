//! Length and lengthening of elliptical arcs, for LENGTHEN. An ellipse has no
//! closed-form arc length, so lengths are integrated along the curve and an
//! end is found by bisection; both are accurate to far better than a drawing
//! needs.

use std::f64::consts::TAU;

use crate::extend::End;
use crate::{Ellipse, Point};

/// Less than this is no arc.
const MIN: f64 = 1e-9;

impl Ellipse {
    /// The length along the curve between parameters `from` and `to` (`to`
    /// above `from`), by Simpson's rule: finer for a longer stretch.
    pub fn length_between(&self, from: f64, to: f64) -> f64 {
        let span = to - from;
        if span <= 0.0 {
            return 0.0;
        }
        // An even number of steps, 256 for a sliver up to 2048 for a turn.
        let steps = 2 * (128 + (span / TAU * 896.0).ceil() as usize);
        let h = span / steps as f64;
        let speed = |t: f64| {
            let d: Point = self.tangent_at(t);
            d.x.hypot(d.y)
        };
        let mut sum = speed(from) + speed(to);
        for i in 1..steps {
            let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
            sum += weight * speed(from + h * i as f64);
        }
        sum * h / 3.0
    }

    /// The length of the arc, or the perimeter of a full ellipse.
    pub fn arc_length(&self) -> f64 {
        self.length_between(self.start, self.end)
    }

    /// The arc with `end` moved along the curve by `delta` (negative
    /// shortens); `None` when nothing, or the whole ellipse or more, would be
    /// left.
    pub fn lengthened(&self, end: End, delta: f64) -> Option<Ellipse> {
        if self.is_full() {
            return None;
        }
        let want = self.arc_length() + delta;
        let whole = self.length_between(0.0, TAU);
        if want.is_nan() || want <= MIN || want >= whole - MIN {
            return None;
        }
        // The sweep from the end that stays put that has length `want`.
        let reaches = |sweep: f64| match end {
            End::End => self.length_between(self.start, self.start + sweep),
            End::Start => self.length_between(self.end - sweep, self.end),
        };
        let (mut low, mut high) = (0.0, TAU);
        for _ in 0..60 {
            let mid = 0.5 * (low + high);
            if reaches(mid) < want {
                low = mid;
            } else {
                high = mid;
            }
        }
        self.with_sweep(end, 0.5 * (low + high))
    }

    /// The arc with `end` moved to where `p` is along the curve (its
    /// parameter, by the nearest direction from the centre).
    pub fn lengthened_to(&self, end: End, p: Point) -> Option<Ellipse> {
        if self.is_full() {
            return None;
        }
        let t = self.parameter_of(p);
        let sweep = match end {
            End::End => (t - self.start).rem_euclid(TAU),
            End::Start => (self.end - t).rem_euclid(TAU),
        };
        self.with_sweep(end, sweep)
    }

    /// The arc keeping the end that does not move and turning through
    /// `sweep` from it.
    fn with_sweep(&self, end: End, sweep: f64) -> Option<Ellipse> {
        if sweep <= MIN || sweep >= TAU - MIN {
            return None;
        }
        let (start, stop) = match end {
            End::End => (self.start, self.start + sweep),
            End::Start => (self.end - sweep, self.end),
        };
        Ellipse::new(self.center, self.major_axis, self.radius_ratio, start, stop).ok()
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_circle_shaped_ellipse_has_the_length_of_a_circle() {
        let e = Ellipse::new(p(0.0, 0.0), p(3.0, 0.0), 1.0, 0.0, FRAC_PI_2).unwrap();
        assert!((e.arc_length() - 3.0 * FRAC_PI_2).abs() < 1e-9);
        let whole = Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        // Ramanujan's estimate of the perimeter of a 4 by 2 ellipse.
        assert!((whole.arc_length() - 19.376_89).abs() < 1e-3);
    }

    #[test]
    fn lengthening_either_end_adds_that_much_along_the_curve() {
        let e = Ellipse::new(p(1.0, 2.0), p(4.0, 0.0), 0.5, 0.3, 2.0).unwrap();
        let before = e.arc_length();
        for end in [End::Start, End::End] {
            let longer = e.lengthened(end, 1.5).unwrap();
            assert!((longer.arc_length() - before - 1.5).abs() < 1e-7);
            let shorter = e.lengthened(end, -0.5).unwrap();
            assert!((shorter.arc_length() - before + 0.5).abs() < 1e-7);
        }
        // The end that is not moved stays where it was.
        let longer = e.lengthened(End::End, 1.0).unwrap();
        assert!((longer.start - e.start).abs() < 1e-9);
        let longer = e.lengthened(End::Start, 1.0).unwrap();
        assert!((longer.end - e.end).abs() < 1e-9);
    }

    #[test]
    fn too_much_or_too_little_is_refused() {
        let e = Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, 0.0, PI).unwrap();
        assert!(e.lengthened(End::End, 100.0).is_none());
        assert!(e.lengthened(End::End, -100.0).is_none());
        let whole = Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        assert!(whole.lengthened(End::End, 1.0).is_none());
    }

    #[test]
    fn dragging_an_end_puts_it_over_the_point() {
        let e = Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, 0.0, FRAC_PI_2).unwrap();
        let longer = e.lengthened_to(End::End, p(-4.0, 0.1)).unwrap();
        assert!((longer.end - PI).abs() < 0.1, "{}", longer.end);
        assert!(longer.start.abs() < 1e-9);
    }
}
