//! Circles of a given radius tangent to two objects when one is a spline
//! (CIRCLE, Ttr). As in `tangent_three_curves`: near its pick each object is
//! stood in for by a line or circle (a spline or ellipse by its circle of
//! curvature, or its tangent line), the exact Ttr solver gives the centres
//! for those, and each centre is corrected by Newton steps until it is the
//! radius from both real curves, on the side it started on (two equations
//! in the centre's two coordinates). A centre is kept only when the steps
//! settle with both objects touched, and the one touching nearest the picks
//! wins. A spline counts only as itself: it cannot be continued.

use crate::tangent::centres;
use crate::tangent_three::contact;
use crate::tangent_three_curves::{dot, left, length, sub, Path};
use crate::{Entity, Point};

/// Most correction steps.
const STEPS: usize = 60;

/// The seed centre corrected until a circle of `radius` there touches both
/// paths, starting each foot at `params`, with its touching points.
fn refine(
    paths: &[Path; 2],
    params: [f64; 2],
    seed: Point,
    radius: f64,
    size: f64,
) -> Option<(Point, [Point; 2])> {
    let mut centre = seed;
    let tolerance = 1e-9 * (size + radius);
    let mut ts = [0, 1].map(|i| paths[i].foot(centre, params[i]));
    let sides = [0, 1].map(|i| {
        let n = left(paths[i].d1(ts[i])).unwrap_or_default();
        if dot(sub(centre, paths[i].point(ts[i])), n) >= 0.0 {
            1.0
        } else {
            -1.0
        }
    });
    for _ in 0..STEPS {
        ts = [0, 1].map(|i| paths[i].foot(centre, ts[i]));
        let (na, nb) = (left(paths[0].d1(ts[0]))?, left(paths[1].d1(ts[1]))?);
        let miss = [0, 1].map(|i| {
            let n = if i == 0 { na } else { nb };
            sides[i] * radius - dot(sub(centre, paths[i].point(ts[i])), n)
        });
        if miss.iter().all(|m| m.abs() < tolerance) {
            break;
        }
        // na · d = miss[0], nb · d = miss[1].
        let det = na.x * nb.y - na.y * nb.x;
        if det.abs() < 1e-12 {
            return None;
        }
        let dx = (miss[0] * nb.y - miss[1] * na.y) / det;
        let dy = (na.x * miss[1] - nb.x * miss[0]) / det;
        centre = Point::new(centre.x + dx, centre.y + dy);
        if !(centre.x.is_finite() && centre.y.is_finite()) {
            return None;
        }
    }
    let tolerance = 1e-7 * (size + radius);
    let mut touches = [Point::default(); 2];
    for i in 0..2 {
        let (p, d1) = (paths[i].point(ts[i]), paths[i].d1(ts[i]));
        let off = sub(centre, p);
        let square = dot(off, d1).abs() / length(d1) < tolerance;
        if !square
            || (length(off) - radius).abs() > tolerance
            || paths[i].is_itself(centre, radius, tolerance)
        {
            return None;
        }
        touches[i] = p;
    }
    Some((centre, touches))
}

/// The centre of the circle of `radius` touching the two `objects` (each
/// with the point it was picked at), one of them a spline: the one touching
/// nearest the picks, or `None`.
pub(crate) fn tangent_circle_spline(objects: [(&Entity, Point); 2], radius: f64) -> Option<Point> {
    let [(a, pa), (b, pb)] = objects;
    let paths = [Path::of(a, pa)?, Path::of(b, pb)?];
    let picks = [pa, pb];
    let at_picks = [0, 1].map(|i| paths[i].nearest(picks[i]));
    let size = length(sub(pa, pb)).max(1e-9);
    let middle = Point::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0);
    let mut best: Option<(f64, Point)> = None;
    for straight in [false, true] {
        let stand_ins = [0, 1].map(|i| paths[i].stand_in(at_picks[i], size, straight));
        let [Some(sa), Some(sb)] = stand_ins else {
            continue;
        };
        for seed in centres(sa, sb, radius, middle) {
            let (Some(ta), Some(tb)) = (contact(sa, seed, radius), contact(sb, seed, radius))
            else {
                continue;
            };
            let params = [paths[0].nearest(ta), paths[1].nearest(tb)];
            let Some((centre, touches)) = refine(&paths, params, seed, radius, size) else {
                continue;
            };
            let score = length(sub(touches[0], pa)) + length(sub(touches[1], pb));
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, centre));
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|(_, centre)| centre)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tangent_circle, Ellipse, Spline};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Line {
            start: p(a.0, a.1),
            end: p(b.0, b.1),
        }
    }

    fn curve(points: &[(f64, f64)]) -> Spline {
        let points = points.iter().map(|&(x, y)| p(x, y)).collect();
        Spline::from_control_points(3, points).unwrap()
    }

    fn spline(points: &[(f64, f64)]) -> Entity {
        Entity::Spline(Box::new(curve(points)))
    }

    fn near(a: Point, b: Point) -> bool {
        length(sub(a, b)) < 1e-6
    }

    #[test]
    fn a_straight_spline_gives_the_lines_answer() {
        let x = spline(&[(0.0, 0.0), (3.0, 0.0), (7.0, 0.0), (10.0, 0.0)]);
        let y = line((0.0, -10.0), (0.0, 10.0));
        let at = |pa, pb| tangent_circle(&x, pa, &y, pb, 1.0).unwrap();
        assert!(near(at(p(5.0, 0.0), p(0.0, 5.0)), p(1.0, 1.0)));
        assert!(near(at(p(5.0, 0.0), p(0.0, -5.0)), p(1.0, -1.0)));
    }

    #[test]
    fn a_straight_spline_and_a_circle_touch_the_nearer_side() {
        // As `a_line_and_a_circle_touch_the_nearer_side` in `tangent`.
        let ground = spline(&[(-10.0, 0.0), (-4.0, 0.0), (4.0, 0.0), (10.0, 0.0)]);
        let ring = Entity::Circle {
            center: p(0.0, 3.0),
            radius: 2.0,
        };
        let centre = tangent_circle(&ground, p(-6.0, 0.0), &ring, p(-2.0, 3.0), 1.0).unwrap();
        assert!((centre.y - 1.0).abs() < 1e-6);
        assert!((length(sub(centre, p(0.0, 3.0))) - 3.0).abs() < 1e-6);
        assert!(centre.x < 0.0);
    }

    #[test]
    fn a_circle_touches_a_bent_spline_and_a_line() {
        let bent = curve(&[(5.0, -1.0), (7.0, 1.0), (3.0, 3.0), (5.0, 5.0)]);
        let (a, b) = bent.domain();
        let pick = bent.point_at((a + b) / 2.0);
        let s = Entity::Spline(Box::new(bent.clone()));
        let floor = line((-5.0, 0.0), (9.0, 0.0));
        let centre = tangent_circle(&s, pick, &floor, p(2.0, 0.0), 1.5).unwrap();
        assert!((centre.y - 1.5).abs() < 1e-6);
        let (touch, _) = bent.nearest(centre);
        assert!((length(sub(touch, centre)) - 1.5).abs() < 1e-6);
        assert!(centre.x < pick.x, "on the side it was picked from");
    }

    #[test]
    fn a_circle_touches_a_spline_and_an_ellipse() {
        let e = Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        let ellipse = Entity::Ellipse(e);
        let wave = curve(&[(6.0, -4.0), (8.0, -1.0), (6.0, 1.0), (8.0, 4.0)]);
        let s = Entity::Spline(Box::new(wave.clone()));
        // The gap between them is about 3 wide, so a circle of radius 2 sits
        // in it, a little off the axis.
        let centre = tangent_circle(&ellipse, p(4.0, 0.0), &s, p(7.0, 0.0), 2.0).unwrap();
        assert!((e.distance_to(centre) - 2.0).abs() < 1e-6);
        let (touch, _) = wave.nearest(centre);
        assert!((length(sub(touch, centre)) - 2.0).abs() < 1e-6);
        assert!(centre.x > 4.0 && centre.x < touch.x, "between them");
    }

    #[test]
    fn no_circle_when_the_radius_cannot_reach_both() {
        let a = spline(&[(0.0, 0.0), (3.0, 0.0), (7.0, 0.0), (10.0, 0.0)]);
        let b = line((0.0, 10.0), (10.0, 10.0));
        assert!(tangent_circle(&a, p(5.0, 0.0), &b, p(5.0, 10.0), 1.0).is_none());
        let fits = tangent_circle(&a, p(5.0, 0.0), &b, p(5.0, 10.0), 5.0).unwrap();
        assert!((fits.y - 5.0).abs() < 1e-6);
    }

    #[test]
    fn a_spline_is_not_continued_past_its_end() {
        // The spline stops at x = 4; with the vertical line at x = 10 the
        // circle of radius 1 would rest on it at x = 9.
        let short = spline(&[(0.0, 0.0), (1.0, 0.0), (3.0, 0.0), (4.0, 0.0)]);
        let wall = line((10.0, -5.0), (10.0, 5.0));
        assert!(tangent_circle(&short, p(2.0, 0.0), &wall, p(10.0, 1.0), 1.0).is_none());
    }
    #[test]
    fn a_straight_spline_always_gives_what_its_line_gives() {
        // Fixed pseudo-random rods and wheels; the rod also as a spline.
        let mut seed = 2024_u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64 * 20.0 - 10.0
        };
        let mut compared = 0;
        for _ in 0..60 {
            let (ax, ay, bx, by) = (next(), next(), next(), next());
            let (cx, cy, r) = (next(), next(), next().abs() / 2.0 + 0.5);
            let radius = next().abs() / 2.0 + 0.5;
            let rod = line((ax, ay), (bx, by));
            let along = |t: f64| (ax + t * (bx - ax), ay + t * (by - ay));
            let rod_spline = spline(&[along(0.0), along(0.3), along(0.6), along(1.0)]);
            let wheel = Entity::Circle {
                center: p(cx, cy),
                radius: r,
            };
            let (pick_rod, pick_wheel) = (p(along(0.5).0, along(0.5).1), p(cx, cy + r));
            let exact = tangent_circle(&rod, pick_rod, &wheel, pick_wheel, radius);
            let stepped = tangent_circle(&rod_spline, pick_rod, &wheel, pick_wheel, radius);
            if let Some(centre) = exact {
                // The line runs on past the rod's ends; the spline does not.
                let foot = Path::Line(p(ax, ay), p(bx, by)).nearest(centre);
                if (0.0..=1.0).contains(&foot) {
                    let got = stepped.expect("a circle");
                    assert!(near(got, centre), "got {got:?}, expected {centre:?}");
                    compared += 1;
                }
            }
        }
        assert!(compared > 20, "only {compared} cases compared");
    }
}
