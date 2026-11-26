//! Cutting a spline in two at a parameter, exactly: the knot is inserted
//! until the curve passes through a control point there (Boehm's algorithm in
//! homogeneous coordinates), and the frame and knots are divided at that
//! point. Both pieces are the same curve as before, over their own parts of
//! the parameter range.

use super::{Spline, DEFAULT_CONTROL_TOLERANCE, DEFAULT_KNOT_TOLERANCE};
use crate::Point;

/// A control point with its weight folded in: `(w·x, w·y, w)`.
type Homogeneous = [f64; 3];

impl Spline {
    /// The two splines the curve makes when cut at parameter `u`, which must
    /// lie inside the domain (not at its ends). `None` for a periodic spline
    /// (its frame wraps round and has no ends to cut at), a spline that is not
    /// well formed, or a cut at an end.
    pub fn split_at_parameter(&self, u: f64) -> Option<(Spline, Spline)> {
        let (a, b) = self.domain();
        let span = b - a;
        if self.periodic || !self.well_formed() || span <= 0.0 {
            return None;
        }
        let near = span * 1e-9;
        if !(u > a + near && u < b - near) {
            return None;
        }
        let p = usize::from(self.degree);
        let mut knots = self.knots.clone();
        let mut frame: Vec<Homogeneous> = (0..self.control_points.len())
            .map(|i| {
                let q = self.control_points[i];
                let w = self.weight(i);
                [q.x * w, q.y * w, w]
            })
            .collect();
        // A knot already at `u` counts towards the p copies needed.
        let mut count = knots.iter().filter(|k| (**k - u).abs() <= near).count();
        // Snap those to `u` exactly so the pieces meet.
        for k in knots.iter_mut().filter(|k| (**k - u).abs() <= near) {
            *k = u;
        }
        while count < p {
            insert_knot(&mut knots, &mut frame, p, u);
            count += 1;
        }
        // The first copy of u: the curve passes through the point before it.
        let first = knots.iter().position(|k| *k == u)?;
        if first == 0 || first + p > knots.len() {
            return None;
        }
        let split = first - 1;
        let mut left_knots = knots[..first].to_vec();
        left_knots.extend(std::iter::repeat_n(u, p + 1));
        let mut right_knots: Vec<f64> = std::iter::repeat_n(u, p + 1).collect();
        right_knots.extend_from_slice(&knots[first + p..]);
        let left = self.piece(&frame[..=split], left_knots)?;
        let right = self.piece(&frame[split..], right_knots)?;
        Some((left, right))
    }

    /// A spline of this one's degree over `frame` and `knots`, without the
    /// fit data (the frame decides the curve) and open.
    fn piece(&self, frame: &[Homogeneous], knots: Vec<f64>) -> Option<Spline> {
        let points: Vec<Point> = frame
            .iter()
            .map(|h| Point::new(h[0] / h[2], h[1] / h[2]))
            .collect();
        let weights: Vec<f64> = frame.iter().map(|h| h[2]).collect();
        let rational = weights.iter().any(|w| (*w - 1.0).abs() > 1e-12);
        let spline = Spline {
            degree: self.degree,
            control_points: points,
            knots,
            weights: rational.then_some(weights),
            closed: false,
            periodic: false,
            knot_tolerance: DEFAULT_KNOT_TOLERANCE,
            control_point_tolerance: DEFAULT_CONTROL_TOLERANCE,
            fit: None,
        };
        spline.validate().ok().map(|()| spline)
    }
}

/// Insert the knot `u` once (A5.1 of The NURBS Book): the span it falls in
/// gets one more control point.
fn insert_knot(knots: &mut Vec<f64>, frame: &mut Vec<Homogeneous>, p: usize, u: f64) {
    // The span: knots[k] <= u < knots[k + 1].
    let k = knots
        .windows(2)
        .rposition(|w| w[0] <= u && u < w[1])
        .unwrap_or(p);
    let mut next: Vec<Homogeneous> = Vec::with_capacity(frame.len() + 1);
    for i in 0..=frame.len() {
        if i + p <= k {
            next.push(frame[i]);
        } else if i > k {
            next.push(frame[i - 1]);
        } else {
            let alpha = (u - knots[i]) / (knots[i + p] - knots[i]);
            let (before, here) = (frame[i - 1], frame[i]);
            next.push([
                alpha * here[0] + (1.0 - alpha) * before[0],
                alpha * here[1] + (1.0 - alpha) * before[1],
                alpha * here[2] + (1.0 - alpha) * before[2],
            ]);
        }
    }
    knots.insert(k + 1, u);
    *frame = next;
}

#[cfg(test)]
#[path = "cut_tests.rs"]
mod tests;

impl Spline {
    /// The curve cut where it is nearest each of `points`, in order along the
    /// curve; the whole curve when nothing is cut (a periodic spline, cuts
    /// at its ends, or no points).
    pub fn split_at_points(&self, points: &[Point]) -> Vec<Spline> {
        let mut params: Vec<f64> = points.iter().map(|p| self.nearest(*p).1).collect();
        params.sort_by(f64::total_cmp);
        let (a, b) = self.domain();
        let near = (b - a) * 1e-9;
        params.dedup_by(|x, y| (*x - *y).abs() <= near);
        let mut pieces = Vec::new();
        let mut rest = self.clone();
        for u in params {
            match rest.split_at_parameter(u) {
                Some((first, second)) => {
                    pieces.push(first);
                    rest = second;
                }
                None => continue,
            }
        }
        pieces.push(rest);
        pieces
    }

    /// What is left when the part between the two points nearest `first` and
    /// `second` is removed: the pieces before and after (one when a point is
    /// at an end), or the curve cut in two where the points coincide. `None`
    /// when it cannot be cut (a periodic spline).
    pub fn break_between(&self, first: Point, second: Point) -> Option<Vec<Spline>> {
        let (a, b) = self.domain();
        let near = (b - a) * 1e-9;
        let (mut lo, mut hi) = (self.nearest(first).1, self.nearest(second).1);
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        if self.periodic || !self.well_formed() {
            return None;
        }
        if hi - lo <= near {
            let pieces = self.split_at_points(&[first]);
            return (pieces.len() > 1).then_some(pieces);
        }
        let mut kept = Vec::new();
        let mut rest = self.clone();
        if lo > a + near {
            let (before, after) = rest.split_at_parameter(lo)?;
            kept.push(before);
            rest = after;
        }
        if hi < b - near {
            let (_, after) = rest.split_at_parameter(hi)?;
            kept.push(after);
        }
        Some(kept)
    }
}
