//! Lines, arcs and open polylines as splines of a given degree, exactly, so
//! that JOIN can add them to a spline.
//!
//! A line is a Bézier piece with its control points in a line. An arc is cut
//! into rational quadratic Bézier pieces of at most a quarter turn (weights
//! `1, cos(θ/2), 1`), which are raised to the degree wanted. Raising the
//! degree of a rational Bézier piece is done on its homogeneous control
//! points, `(w·x, w·y, w)`, where it is the same as for a plain one. The pieces
//! are chained at their ends, so the result is the same curve.

use std::f64::consts::{FRAC_PI_2, TAU};

use super::Spline;
use crate::{arc, bulge, Entity, Point};

/// A control point with its weight folded in: `(w·x, w·y, w)`.
type Homogeneous = [f64; 3];

/// Raise the degree of a Bézier piece by one.
fn elevate(piece: &[Homogeneous]) -> Vec<Homogeneous> {
    let n = piece.len() - 1;
    let mut out = Vec::with_capacity(n + 2);
    out.push(piece[0]);
    for i in 1..=n {
        let a = i as f64 / (n + 1) as f64;
        let (before, here) = (piece[i - 1], piece[i]);
        out.push([
            a * before[0] + (1.0 - a) * here[0],
            a * before[1] + (1.0 - a) * here[1],
            a * before[2] + (1.0 - a) * here[2],
        ]);
    }
    out.push(piece[n]);
    out
}

/// One Bézier piece of `degree` over the parameter range `0..span`.
fn piece(mut frame: Vec<Homogeneous>, degree: u8, span: f64) -> Option<Spline> {
    while frame.len() <= usize::from(degree) {
        frame = elevate(&frame);
    }
    if frame.len() != usize::from(degree) + 1 {
        return None;
    }
    let points = frame
        .iter()
        .map(|h| Point::new(h[0] / h[2], h[1] / h[2]))
        .collect();
    let weights: Vec<f64> = frame.iter().map(|h| h[2]).collect();
    let mut knots = vec![0.0; usize::from(degree) + 1];
    knots.extend(std::iter::repeat_n(span, usize::from(degree) + 1));
    Spline::new_nurbs(degree, points, knots, Some(weights), false, false).ok()
}

fn homogeneous(p: Point, w: f64) -> Homogeneous {
    [p.x * w, p.y * w, w]
}

fn straight(a: Point, b: Point, degree: u8) -> Option<Spline> {
    let span = (b.x - a.x).hypot(b.y - a.y);
    (span > 1e-12).then_some(())?;
    piece(vec![homogeneous(a, 1.0), homogeneous(b, 1.0)], degree, span)
}

/// The arc about `center` from angle `start` counter-clockwise to `end`, as
/// pieces of a quarter turn at most, chained.
fn arc_as_spline(
    center: Point,
    radius: f64,
    start: f64,
    end: f64,
    degree: u8,
    tolerance: f64,
) -> Option<Spline> {
    if degree < 2 || radius <= 0.0 {
        return None;
    }
    let sweep = arc::sweep(start, end);
    if sweep <= 1e-12 || sweep >= TAU - 1e-9 {
        return None;
    }
    let pieces = (sweep / FRAC_PI_2).ceil().max(1.0) as usize;
    let step = sweep / pieces as f64;
    let on = |t: f64, r: f64| Point::new(center.x + r * t.cos(), center.y + r * t.sin());
    let mut run: Option<Spline> = None;
    for k in 0..pieces {
        let (t0, t1) = (start + step * k as f64, start + step * (k + 1) as f64);
        let w = (step / 2.0).cos();
        let frame = vec![
            homogeneous(on(t0, radius), 1.0),
            homogeneous(on((t0 + t1) / 2.0, radius / w), w),
            homogeneous(on(t1, radius), 1.0),
        ];
        let next = piece(frame, degree, radius * step)?;
        run = Some(match run {
            None => next,
            Some(so_far) => so_far.followed_by(&next, tolerance)?,
        });
    }
    run
}

impl Spline {
    /// This line, arc or open polyline as a spline of `degree`: the same curve,
    /// exactly. `None` for anything else, for a polyline with arc segments
    /// below degree 2, and for what has no length.
    pub fn from_entity(entity: &Entity, degree: u8) -> Option<Spline> {
        if !(1..=super::MAX_DEGREE).contains(&degree) {
            return None;
        }
        match entity {
            Entity::Line { start, end } => straight(*start, *end, degree),
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => arc_as_spline(
                *center,
                *radius,
                *start,
                *end,
                degree,
                tolerance_of(*center),
            ),
            Entity::Polyline {
                points,
                closed: false,
                bulges,
                ..
            } if points.len() >= 2 => {
                let tolerance = points.iter().map(|p| tolerance_of(*p)).fold(0.0, f64::max);
                let mut run: Option<Spline> = None;
                for i in 0..points.len() - 1 {
                    let (a, b) = (points[i], points[i + 1]);
                    let next = match bulge::arc_of(a, b, bulge::at(bulges, i)) {
                        None => straight(a, b, degree)?,
                        Some(s) => {
                            arc_as_spline(s.center, s.radius, s.start, s.end, degree, tolerance)
                                .map(|spline| if s.forward { spline } else { spline.reversed() })?
                        }
                    };
                    run = Some(match run {
                        None => next,
                        Some(so_far) => so_far.followed_by(&next, tolerance)?,
                    });
                }
                run
            }
            _ => None,
        }
    }
}

/// How close two ends must be to be one point, for coordinates this size.
fn tolerance_of(p: Point) -> f64 {
    1e-9 * (1.0 + p.x.abs().max(p.y.abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn on_curve(spline: &Spline, check: impl Fn(Point) -> f64, tolerance: f64) {
        let (a, b) = spline.domain();
        for i in 0..=40 {
            let q = spline.point_at(a + (b - a) * f64::from(i) / 40.0);
            assert!(check(q).abs() < tolerance, "{q:?} off by {}", check(q));
        }
    }

    #[test]
    fn a_line_is_a_spline_of_any_degree_and_keeps_its_ends_and_length() {
        for degree in 1..=5 {
            let s = Spline::from_entity(
                &Entity::Line {
                    start: p(1.0, 2.0),
                    end: p(7.0, 10.0),
                },
                degree,
            )
            .unwrap();
            assert_eq!(s.degree, degree);
            assert!(
                (s.start_point().x - 1.0).abs() < 1e-12 && (s.end_point().y - 10.0).abs() < 1e-12
            );
            assert!((s.length().unwrap() - 10.0).abs() < 1e-9);
            // Every point is on the line 4x - 3y + 2 = 0.
            on_curve(&s, |q| (4.0 * q.x - 3.0 * q.y + 2.0) / 5.0, 1e-9);
        }
    }

    #[test]
    fn an_arc_is_a_rational_spline_on_its_circle_at_any_degree_from_two() {
        let centre = p(3.0, -1.0);
        let entity = Entity::Arc {
            center: centre,
            radius: 5.0,
            start: 0.3,
            end: 0.3 + 4.0,
        };
        assert!(Spline::from_entity(&entity, 1).is_none());
        for degree in 2..=5 {
            let s = Spline::from_entity(&entity, degree).unwrap();
            assert!(s.is_rational());
            on_curve(&s, |q| (q.x - centre.x).hypot(q.y - centre.y) - 5.0, 1e-9);
            assert!((s.length().unwrap() - 20.0).abs() < 1e-6, "degree {degree}");
            let (a, _) = (s.start_point(), s.end_point());
            assert!((a.x - (centre.x + 5.0 * 0.3_f64.cos())).abs() < 1e-9);
        }
    }

    #[test]
    fn a_polyline_with_straight_and_arc_segments_is_one_spline_through_its_vertices() {
        let poly = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)],
            closed: false,
            // The middle segment bulges right, as a half circle.
            bulges: vec![0.0, 1.0, 0.0, 0.0],
            widths: Vec::new(),
        };
        let s = Spline::from_entity(&poly, 3).unwrap();
        assert!((s.start_point().x).abs() < 1e-9 && (s.end_point().y - 4.0).abs() < 1e-9);
        let expected = 4.0 + std::f64::consts::PI * 2.0 + 4.0;
        assert!((s.length().unwrap() - expected).abs() < 1e-6);
        // The half circle is about (4, 2), on the right.
        let right = s.nearest(p(6.0, 2.0)).0;
        assert!((right.x - 6.0).abs() < 1e-6 && (right.y - 2.0).abs() < 1e-6);
        assert!(
            Spline::from_entity(&poly, 1).is_none(),
            "arcs need degree 2"
        );
    }

    #[test]
    fn a_clockwise_arc_segment_is_traversed_the_right_way() {
        let poly = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0)],
            closed: false,
            bulges: vec![-1.0, 0.0],
            widths: Vec::new(),
        };
        let s = Spline::from_entity(&poly, 2).unwrap();
        // A clockwise half circle from (0,0) to (4,0) runs over the top.
        let top = s.nearest(p(2.0, 5.0)).0;
        assert!((top.y - 2.0).abs() < 1e-6, "{top:?}");
        assert!((s.start_point().x).abs() < 1e-9 && (s.end_point().x - 4.0).abs() < 1e-9);
    }

    #[test]
    fn closed_things_and_other_kinds_are_not_converted() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        assert!(Spline::from_entity(&circle, 3).is_none());
        let closed = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        assert!(Spline::from_entity(&closed, 3).is_none());
        assert!(Spline::from_entity(&Entity::Point { at: p(0.0, 0.0) }, 3).is_none());
    }
}
