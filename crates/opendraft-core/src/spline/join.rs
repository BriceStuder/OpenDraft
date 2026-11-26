//! Joining splines end to end: two splines of one degree that meet become one
//! spline through the shared point (the curve is the same, with the knot there
//! repeated so it may turn a corner, as it was).

use super::{Spline, DEFAULT_CONTROL_TOLERANCE, DEFAULT_KNOT_TOLERANCE};
use crate::Point;

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

impl Spline {
    /// The same curve run the other way.
    pub fn reversed(&self) -> Spline {
        let (a, b) = self.domain();
        let first = self.knots.first().copied().unwrap_or(a);
        let last = self.knots.last().copied().unwrap_or(b);
        let mut out = self.clone();
        out.control_points.reverse();
        if let Some(weights) = &mut out.weights {
            weights.reverse();
        }
        out.knots = self.knots.iter().rev().map(|k| first + last - k).collect();
        out.fit = None;
        out
    }

    /// This spline continued by `other`, when `other` starts where this one
    /// ends (within `tolerance`) and both are open and of one degree.
    fn followed_by(&self, other: &Spline, tolerance: f64) -> Option<Spline> {
        let p = usize::from(self.degree);
        if self.degree != other.degree
            || self.periodic
            || other.periodic
            || !self.well_formed()
            || !other.well_formed()
            || distance(self.end_point(), other.start_point()) > tolerance
        {
            return None;
        }
        let (a0, a1) = self.domain();
        let (b0, b1) = other.domain();
        if a1 <= a0 || b1 <= b0 {
            return None;
        }
        let mut points = self.control_points.clone();
        points.extend_from_slice(&other.control_points[1..]);
        // The other's knots, moved to start where this one ends.
        let shift = a1 - b0;
        let mut knots = self.knots[..self.knots.len() - (p + 1)].to_vec();
        knots.extend(std::iter::repeat_n(a1, p));
        knots.extend(other.knots[p + 1..].iter().map(|k| k + shift));
        let weights = if self.weights.is_some() || other.weights.is_some() {
            let mut w: Vec<f64> = (0..self.control_points.len())
                .map(|i| self.weight(i))
                .collect();
            // Scaling every weight of the other by a constant leaves its curve
            // as it is: do it so the shared point has one weight.
            let scale = self.weight(self.control_points.len() - 1) / other.weight(0);
            w.extend((1..other.control_points.len()).map(|i| other.weight(i) * scale));
            Some(w)
        } else {
            None
        };
        let joined = Spline {
            degree: self.degree,
            control_points: points,
            knots,
            weights,
            closed: false,
            periodic: false,
            knot_tolerance: DEFAULT_KNOT_TOLERANCE,
            control_point_tolerance: DEFAULT_CONTROL_TOLERANCE,
            fit: None,
        };
        joined.validate().ok().map(|()| joined)
    }

    /// The two splines as one, whichever way round they meet: this one first,
    /// or the other, either of them reversed if need be. `None` when they do
    /// not meet end to end (or differ in degree).
    pub fn joined(&self, other: &Spline, tolerance: f64) -> Option<Spline> {
        self.followed_by(other, tolerance)
            .or_else(|| self.followed_by(&other.reversed(), tolerance))
            .or_else(|| other.followed_by(self, tolerance))
            .or_else(|| self.reversed().followed_by(other, tolerance))
    }
}

#[cfg(test)]
#[path = "join_tests.rs"]
mod tests;
