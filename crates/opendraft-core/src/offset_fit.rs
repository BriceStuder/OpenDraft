//! The parallel curve of a spline or an ellipse, for OFFSET. It is not a
//! curve of the same kind (the offset of a NURBS or of an ellipse is not one),
//! so a cubic spline is fitted through points of the true parallel curve and
//! refined until it stays within a tolerance of it, as AutoCAD makes a spline.
//!
//! The curve is given as a function of a parameter that returns the point and
//! its unit tangent. The offset point is the point moved along the left normal
//! (a negative distance goes to the right). A distance that is at least the
//! radius of curvature on the side it turns towards would fold the offset back
//! on itself (a cusp, then a loop): that is refused, not trimmed.

use crate::{Ellipse, Entity, KnotParameterization, Point, Spline};

/// Samples the fit starts from, over the whole curve.
const START: usize = 16;
/// The most samples it may use before giving up.
const MAX_SAMPLES: usize = 800;
/// The most refinement rounds.
const MAX_ROUNDS: usize = 40;
/// The offset folds when `distance × curvature` reaches this (1 is the cusp).
const FOLD: f64 = 0.98;

/// A point of the curve and its unit tangent at a parameter.
pub(crate) type At<'a> = &'a dyn Fn(f64) -> Option<(Point, Point)>;

fn parallel_point(at: At, u: f64, d: f64) -> Option<Point> {
    let (p, t) = at(u)?;
    Some(Point::new(p.x - t.y * d, p.y + t.x * d))
}

/// How fast the curve turns left at `u`, per unit of length (negative when it
/// turns right), from the tangents a hair either side.
fn curvature(at: At, u: f64, (a, b): (f64, f64)) -> Option<f64> {
    let h = (b - a) * 1e-5;
    let (u0, u1) = ((u - h).max(a), (u + h).min(b));
    let ((p0, t0), (p1, t1)) = (at(u0)?, at(u1)?);
    let ds = (p1.x - p0.x).hypot(p1.y - p0.y);
    if ds < 1e-14 {
        return None;
    }
    let turn = (t0.x * t1.y - t0.y * t1.x).atan2(t0.x * t1.x + t0.y * t1.y);
    Some(turn / ds)
}

/// The spline within `tolerance` of the curve moved `d` to the left over
/// `domain`, closed or open. `None` when the offset folds, a point cannot be
/// found, or it needs more samples than allowed.
pub(crate) fn offset_fit(
    at: At,
    domain: (f64, f64),
    closed: bool,
    d: f64,
    tolerance: f64,
) -> Option<Spline> {
    let (a, b) = domain;
    let span = b - a;
    if !(span > 0.0 && d.is_finite() && tolerance > 0.0) {
        return None;
    }
    let count = if closed { START } else { START + 1 };
    let mut us: Vec<f64> = (0..count)
        .map(|i| a + span * i as f64 / START as f64)
        .collect();
    let folds = |u: f64| curvature(at, u, domain).is_none_or(|k| d * k >= FOLD);
    for _ in 0..MAX_ROUNDS {
        if us.iter().any(|u| folds(*u)) {
            return None;
        }
        let points: Option<Vec<Point>> = us.iter().map(|u| parallel_point(at, *u, d)).collect();
        let points = points?;
        let fit = if closed {
            Spline::from_closed_fit_points(points, KnotParameterization::Chord)
        } else {
            Spline::from_fit_points(points, KnotParameterization::Chord)
        }
        .ok()?;
        // Between each two samples: is the curve where the true offset is?
        let mut worse = Vec::new();
        for i in 0..us.len() {
            let next = match us.get(i + 1) {
                Some(next) => *next,
                None if closed => b,
                None => break,
            };
            let mid = (us[i] + next) / 2.0;
            if folds(mid) {
                return None;
            }
            let truth = parallel_point(at, mid, d)?;
            if fit.distance_to(truth) > tolerance {
                worse.push(mid);
            }
        }
        if worse.is_empty() {
            return Some(fit);
        }
        us.extend(worse);
        us.sort_by(f64::total_cmp);
        if us.len() > MAX_SAMPLES {
            return None;
        }
    }
    None
}

/// Which side of a curve `toward` is on, given the nearest point on it and
/// the curve's tangent there: 1 left, -1 right, `None` on the curve.
fn side_of(nearest: Point, tangent: Point, toward: Point) -> Option<f64> {
    let side = tangent.x * (toward.y - nearest.y) - tangent.y * (toward.x - nearest.x);
    (side.abs() > 1e-9).then(|| side.signum())
}

fn tolerance_for(entity: &Entity, d: f64) -> f64 {
    let b = entity.bounds();
    let size = (b.max.x - b.min.x).hypot(b.max.y - b.min.y);
    ((size + d.abs()) * 1e-5).max(1e-9)
}

impl Entity {
    /// The offset of an open or closed spline, `distance` away on the side
    /// `toward` is on, as a spline within a hundred-thousandth of the true
    /// parallel curve. `None` for a point on the curve, or a distance that
    /// would fold the offset back on itself.
    pub(crate) fn offset_spline(&self, distance: f64, toward: Point) -> Option<Entity> {
        let Entity::Spline(s) = self else {
            return None;
        };
        let (q, u) = s.nearest(toward);
        let d = distance * side_of(q, s.tangent_at(u)?, toward)?;
        self.offset_spline_left(d)
    }

    /// The offset of a spline `d` to the left of the way it runs (to the right
    /// when negative), fitted as for [`Self::offset_spline`].
    pub(crate) fn offset_spline_left(&self, d: f64) -> Option<Entity> {
        let Entity::Spline(s) = self else {
            return None;
        };
        let at = |u: f64| Some((s.point_at(u), s.tangent_at(u)?));
        let made = offset_fit(
            &at,
            s.domain(),
            s.closed || s.periodic,
            d,
            tolerance_for(self, d),
        )?;
        Some(Entity::Spline(Box::new(made)))
    }

    /// The offset of an ellipse or elliptical arc, as a spline (it is not an
    /// ellipse); see [`Self::offset_spline`].
    pub(crate) fn offset_ellipse(&self, distance: f64, toward: Point) -> Option<Entity> {
        let Entity::Ellipse(e) = self else {
            return None;
        };
        let unit = |e: &Ellipse, t: f64| {
            let d = e.tangent_at(t);
            let len = d.x.hypot(d.y);
            (len > 0.0).then(|| Point::new(d.x / len, d.y / len))
        };
        let (q, t) = e.nearest(toward);
        let d = distance * side_of(q, unit(e, t)?, toward)?;
        let at = |t: f64| Some((e.point_at(t), unit(e, t)?));
        let made = offset_fit(
            &at,
            (e.start, e.end),
            e.is_full(),
            d,
            tolerance_for(self, d),
        )?;
        Some(Entity::Spline(Box::new(made)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(r: f64) -> impl Fn(f64) -> Option<(Point, Point)> {
        move |t| {
            Some((
                Point::new(r * t.cos(), r * t.sin()),
                Point::new(-t.sin(), t.cos()),
            ))
        }
    }

    #[test]
    fn the_offset_of_a_circle_is_a_circle_within_the_tolerance() {
        let c = circle(10.0);
        let tau = std::f64::consts::TAU;
        // Left of a counter-clockwise circle is inside.
        let inner = offset_fit(&c, (0.0, tau), true, 3.0, 1e-5).unwrap();
        for i in 0..60 {
            let q = inner.point_at(
                inner.domain().0 + (inner.domain().1 - inner.domain().0) * f64::from(i) / 60.0,
            );
            assert!((q.x.hypot(q.y) - 7.0).abs() < 2e-5, "{q:?}");
        }
        assert!(inner.closed);
        let outer = offset_fit(&c, (0.0, tau), true, -3.0, 1e-5).unwrap();
        assert!((outer.length().unwrap() - tau * 13.0).abs() < 1e-3);
    }

    #[test]
    fn an_offset_that_would_fold_is_refused() {
        let c = circle(10.0);
        let tau = std::f64::consts::TAU;
        assert!(offset_fit(&c, (0.0, tau), true, 10.0, 1e-5).is_none());
        assert!(offset_fit(&c, (0.0, tau), true, 9.9, 1e-5).is_none());
        assert!(offset_fit(&c, (0.0, tau), true, 9.0, 1e-4).is_some());
        // Away from the centre there is no limit.
        assert!(offset_fit(&c, (0.0, tau), true, -50.0, 1e-4).is_some());
    }

    #[test]
    fn an_open_arc_gets_an_open_offset_that_starts_where_the_normal_does() {
        let c = circle(10.0);
        let fit = offset_fit(&c, (0.0, std::f64::consts::FRAC_PI_2), false, -2.0, 1e-5).unwrap();
        assert!(!fit.closed);
        let (s, e) = (fit.start_point(), fit.end_point());
        assert!((s.x - 12.0).abs() < 1e-9 && s.y.abs() < 1e-9);
        assert!(e.x.abs() < 1e-9 && (e.y - 12.0).abs() < 1e-9);
        let (a, b) = fit.domain();
        let mid = fit.point_at((a + b) / 2.0);
        assert!((mid.x.hypot(mid.y) - 12.0).abs() < 2e-5);
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn arch() -> Entity {
        Entity::Spline(Box::new(
            Spline::from_control_points(
                3,
                vec![p(0.0, 0.0), p(2.0, 6.0), p(8.0, 6.0), p(10.0, 0.0)],
            )
            .unwrap(),
        ))
    }

    #[test]
    fn a_splines_offset_is_the_given_distance_from_it_on_the_side_asked_for() {
        let original = arch();
        let Entity::Spline(base) = &original else {
            unreachable!()
        };
        for (toward, expect_above) in [(p(5.0, 9.0), true), (p(5.0, 1.0), false)] {
            let Some(Entity::Spline(off)) = original.offset(0.5, toward) else {
                panic!("an offset")
            };
            let (a, b) = off.domain();
            for i in 0..=40 {
                let q = off.point_at(a + (b - a) * f64::from(i) / 40.0);
                assert!((base.distance_to(q) - 0.5).abs() < 1e-3, "{q:?}");
            }
            // The crest is above the original's for the outer side.
            let crest = off
                .nearest(p(5.0, if expect_above { 20.0 } else { -20.0 }))
                .0;
            let base_crest = base.nearest(p(5.0, 20.0)).0;
            assert_eq!(crest.y > base_crest.y, expect_above, "{crest:?}");
        }
    }

    #[test]
    fn a_spline_offset_on_a_point_of_it_or_past_a_tight_curve_is_refused() {
        let original = arch();
        let Entity::Spline(base) = &original else {
            unreachable!()
        };
        assert!(original.offset(1.0, base.point_at(0.5)).is_none());
        // Inside the arch the offset has to fold long before this far.
        assert!(original.offset(30.0, p(5.0, 1.0)).is_none());
        assert!(original.offset(30.0, p(5.0, 80.0)).is_some());
    }

    #[test]
    fn a_closed_splines_offset_is_closed() {
        let ring = Entity::Spline(Box::new(
            Spline::from_closed_fit_points(
                vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 6.0), p(0.0, 6.0)],
                KnotParameterization::Chord,
            )
            .unwrap(),
        ));
        let Some(Entity::Spline(out)) = ring.offset(1.0, p(5.0, 20.0)) else {
            panic!("an offset")
        };
        assert!(out.closed);
        let Entity::Spline(base) = &ring else {
            unreachable!()
        };
        assert!(out.length().unwrap() > base.length().unwrap());
    }

    #[test]
    fn an_ellipse_is_offset_into_a_spline_that_is_the_true_parallel_curve() {
        let full = Ellipse::full(p(0.0, 0.0), p(6.0, 0.0), 0.5).unwrap();
        let ellipse = Entity::Ellipse(full);
        // Outwards: every point of the offset is 1 from the ellipse.
        let Some(Entity::Spline(out)) = ellipse.offset(1.0, p(20.0, 0.0)) else {
            panic!("an offset")
        };
        assert!(out.closed);
        let (a, b) = out.domain();
        for i in 0..60 {
            let q = out.point_at(a + (b - a) * f64::from(i) / 60.0);
            assert!((full.distance_to(q) - 1.0).abs() < 1e-3, "{q:?}");
        }
        // Inwards by 2 is fine (the tightest curve has radius b*b/a = 1.5),
        // but by 1.5 or more it would fold.
        assert!(ellipse.offset(1.0, p(0.0, 0.0)).is_some());
        assert!(ellipse.offset(1.5, p(0.0, 0.0)).is_none());
        // A point on the ellipse has no side.
        assert!(ellipse.offset(1.0, full.point_at(0.7)).is_none());
    }

    #[test]
    fn an_elliptical_arc_gives_an_open_spline() {
        let arc = Ellipse::new(p(0.0, 0.0), p(6.0, 0.0), 0.5, 0.2, 2.5).unwrap();
        let Some(Entity::Spline(out)) = Entity::Ellipse(arc).offset(0.5, p(20.0, 20.0)) else {
            panic!("an offset")
        };
        assert!(!out.closed);
        let (a, b) = out.domain();
        for i in 0..=30 {
            let q = out.point_at(a + (b - a) * f64::from(i) / 30.0);
            assert!((arc.distance_to(q) - 0.5).abs() < 1e-3, "{q:?}");
        }
    }
}
