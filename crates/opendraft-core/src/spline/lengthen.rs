//! Making an open spline longer or shorter at one end, for LENGTHEN (and,
//! later, EXTEND).
//!
//! Shortening cuts the curve exactly (`split_at_parameter`). Lengthening
//! continues it as a straight tail along the end tangent, as AutoCAD's EXTEND
//! does for a spline; the tail is built into the frame, so the result is still
//! one spline that follows the old curve exactly and then runs straight on.

use super::Spline;
use crate::extend::End;
use crate::Point;

/// Less than this is no change.
const MIN: f64 = 1e-9;

impl Spline {
    /// With `end` moved along the curve by `delta` (negative shortens). `None`
    /// for a closed or periodic spline, one not clamped at the end being
    /// lengthened, or when nothing would be left.
    pub fn lengthened(&self, end: End, delta: f64) -> Option<Spline> {
        if !self.can_change_ends() || !delta.is_finite() {
            return None;
        }
        if delta.abs() <= MIN {
            return Some(self.clone());
        }
        if delta > 0.0 {
            self.with_tail(end, delta)
        } else {
            self.shortened(end, -delta)
        }
    }

    /// With `end` moved to where `p` is: cut back to the curve's nearest
    /// point, or, when `p` lies out beyond the end, continued straight to
    /// `p`'s projection on the end tangent.
    pub fn lengthened_to(&self, end: End, p: Point) -> Option<Spline> {
        if !self.can_change_ends() {
            return None;
        }
        let (a, b) = self.domain();
        let near = (b - a) * 1e-9;
        let (tip, outward) = match end {
            End::End => (self.end_point(), self.tangent_at(b)?),
            End::Start => {
                let t = self.tangent_at(a)?;
                (self.start_point(), Point::new(-t.x, -t.y))
            }
        };
        let along = (p.x - tip.x) * outward.x + (p.y - tip.y) * outward.y;
        let (_, u) = self.nearest(p);
        let at_end = match end {
            End::End => u >= b - near,
            End::Start => u <= a + near,
        };
        if along > MIN && at_end {
            return self.with_tail(end, along);
        }
        if u <= a + near || u >= b - near {
            return None;
        }
        let (first, second) = self.split_at_parameter(u)?;
        Some(match end {
            End::End => first,
            End::Start => second,
        })
    }

    fn can_change_ends(&self) -> bool {
        !self.closed && !self.periodic && self.well_formed()
    }

    /// Cut `cut` off `end`.
    fn shortened(&self, end: End, cut: f64) -> Option<Spline> {
        let total = self.length().ok()?;
        if cut >= total - MIN {
            return None;
        }
        match end {
            End::End => {
                let u = self.parameter_at_length(total - cut).ok()?;
                self.split_at_parameter(u).map(|(first, _)| first)
            }
            End::Start => {
                let u = self.parameter_at_length(cut).ok()?;
                self.split_at_parameter(u).map(|(_, second)| second)
            }
        }
    }

    /// A straight tail of `length` added at `end`.
    fn with_tail(&self, end: End, length: f64) -> Option<Spline> {
        match end {
            End::End => self.tail_at_end(length),
            End::Start => self.reversed().tail_at_end(length).map(|s| s.reversed()),
        }
    }

    /// The tail goes on the last point with `p` new control points in a line.
    /// The curve's end is a clamped knot of `p + 1` copies; one copy goes, so
    /// the join is a knot of `p` copies. That keeps the old curve exactly (a
    /// knot of `p` copies only repeats the control point the curve passes
    /// through) and lets the tail start at the last control point.
    fn tail_at_end(&self, length: f64) -> Option<Spline> {
        let p = usize::from(self.degree);
        let n = self.control_points.len() - 1;
        let (a, b) = self.domain();
        // The last `p + 1` knots are the end of the domain.
        let clamped = self.knots[n + 1..]
            .iter()
            .all(|k| (*k - b).abs() <= self.knot_tolerance);
        let last = *self.control_points.last()?;
        let total = self.length().ok()?;
        let span = (b - a) * length / total;
        if !clamped || span <= self.knot_tolerance * 10.0 || !span.is_finite() {
            return None;
        }
        let t = self.tangent_at(b)?;
        let mut knots = self.knots.clone();
        knots.pop();
        knots.extend(std::iter::repeat_n(b + span, p + 1));
        let mut control_points = self.control_points.clone();
        for j in 1..=p {
            let d = length * j as f64 / p as f64;
            control_points.push(Point::new(last.x + t.x * d, last.y + t.y * d));
        }
        let weights = self.weights.as_ref().map(|w| {
            let mut w = w.clone();
            let end = w[n];
            w.extend(std::iter::repeat_n(end, p));
            w
        });
        let spline = Spline {
            degree: self.degree,
            control_points,
            knots,
            weights,
            closed: false,
            periodic: false,
            knot_tolerance: self.knot_tolerance,
            control_point_tolerance: self.control_point_tolerance,
            fit: None,
        };
        spline.validate().ok().map(|()| spline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn arch(degree: u8) -> Spline {
        Spline::from_control_points(
            degree,
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

    fn near(a: Point, b: Point, tol: f64) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < tol
    }

    #[test]
    fn a_tail_keeps_the_old_curve_and_runs_straight_along_the_end_tangent() {
        for degree in 1..=4 {
            let old = arch(degree);
            let (a, b) = old.domain();
            let longer = old.lengthened(End::End, 5.0).unwrap();
            let (c, d) = longer.domain();
            assert_eq!(c, a);
            assert!(d > b);
            for i in 0..=20 {
                let u = a + (b - a) * f64::from(i) / 20.0;
                assert!(
                    near(longer.point_at(u), old.point_at(u), 1e-9),
                    "degree {degree} u {u}"
                );
            }
            let gained = longer.length().unwrap() - old.length().unwrap();
            assert!((gained - 5.0).abs() < 1e-6, "degree {degree}: {gained}");
            let t = old.tangent_at(b).unwrap();
            let end = old.end_point();
            let want = p(end.x + t.x * 5.0, end.y + t.y * 5.0);
            assert!(near(longer.end_point(), want, 1e-9), "degree {degree}");
            // Straight: the middle of the tail is on the line.
            let mid = longer.point_at((b + d) / 2.0);
            let off = (mid.x - end.x) * t.y - (mid.y - end.y) * t.x;
            assert!(off.abs() < 1e-9, "degree {degree}");
        }
    }

    #[test]
    fn the_start_is_lengthened_the_same_way() {
        let old = arch(3);
        let longer = old.lengthened(End::Start, 2.0).unwrap();
        let (a, _) = old.domain();
        let t = old.tangent_at(a).unwrap();
        let start = old.start_point();
        let want = p(start.x - t.x * 2.0, start.y - t.y * 2.0);
        assert!(near(longer.start_point(), want, 1e-9));
        assert!(near(longer.end_point(), old.end_point(), 1e-9));
        assert!((longer.length().unwrap() - old.length().unwrap() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn shortening_cuts_the_curve_by_that_much_and_keeps_its_shape() {
        let old = arch(3);
        let total = old.length().unwrap();
        for end in [End::End, End::Start] {
            let shorter = old.lengthened(end, -3.0).unwrap();
            assert!((shorter.length().unwrap() - (total - 3.0)).abs() < 1e-6);
            // Every point of the short one is on the old curve.
            let (a, b) = shorter.domain();
            for i in 0..=10 {
                let q = shorter.point_at(a + (b - a) * f64::from(i) / 10.0);
                assert!(old.distance_to(q) < 1e-6);
            }
        }
        assert!(old.lengthened(End::End, -total).is_none());
        assert!(old.lengthened(End::End, -total - 1.0).is_none());
    }

    #[test]
    fn lengthening_then_shortening_comes_back_to_the_curve() {
        let old = arch(3);
        let back = old
            .lengthened(End::End, 4.0)
            .and_then(|s| s.lengthened(End::End, -4.0))
            .unwrap();
        assert!((back.length().unwrap() - old.length().unwrap()).abs() < 1e-6);
        assert!(near(back.end_point(), old.end_point(), 1e-6));
    }

    #[test]
    fn a_rational_spline_keeps_its_weights_through_a_tail() {
        let old = Spline::new_nurbs(
            2,
            vec![p(0.0, 0.0), p(3.0, 4.0), p(6.0, 0.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            Some(vec![1.0, 0.7, 1.0]),
            false,
            false,
        )
        .unwrap();
        let longer = old.lengthened(End::End, 3.0).unwrap();
        assert!(longer.is_rational());
        let (a, b) = old.domain();
        for i in 0..=10 {
            let u = a + (b - a) * f64::from(i) / 10.0;
            assert!(near(longer.point_at(u), old.point_at(u), 1e-9));
        }
        assert!((longer.length().unwrap() - old.length().unwrap() - 3.0).abs() < 1e-6);
    }

    #[test]
    fn dynamic_cuts_back_to_the_nearest_point_or_runs_straight_past_the_end() {
        let old = arch(3);
        // Near the middle of the curve: cut there.
        let mid = old.point_at(0.5);
        let cut = old.lengthened_to(End::End, p(mid.x, mid.y + 0.2)).unwrap();
        assert!(cut.length().unwrap() < old.length().unwrap());
        assert!(near(cut.end_point(), mid, 0.3));
        // Out past the end along its tangent: continue to the projection.
        let t = old.tangent_at(old.domain().1).unwrap();
        let end = old.end_point();
        let beyond = p(end.x + t.x * 6.0, end.y + t.y * 6.0);
        let longer = old.lengthened_to(End::End, beyond).unwrap();
        assert!(near(longer.end_point(), beyond, 1e-6));
    }

    #[test]
    fn closed_and_periodic_splines_are_not_lengthened() {
        let mut closed = arch(3);
        closed.closed = true;
        assert!(closed.lengthened(End::End, 1.0).is_none());
        assert!(closed.lengthened_to(End::End, p(0.0, 0.0)).is_none());
    }

    #[test]
    fn a_spline_with_a_tail_can_be_cut_back_at_either_end() {
        let old = arch(3);
        let total = old.length().unwrap();
        let tailed = old.lengthened(End::End, 3.0).unwrap();
        let from_start = tailed.lengthened(End::Start, -2.0).unwrap();
        assert!((from_start.length().unwrap() - (total + 1.0)).abs() < 1e-6);
        let from_end = tailed.lengthened(End::End, -2.0).unwrap();
        assert!((from_end.length().unwrap() - (total + 1.0)).abs() < 1e-6);
        // Cutting through the join into the old curve.
        let into_old = tailed.lengthened(End::End, -5.0).unwrap();
        assert!((into_old.length().unwrap() - (total - 2.0)).abs() < 1e-6);
    }
}
