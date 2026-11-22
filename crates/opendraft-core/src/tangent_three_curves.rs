//! CIRCLE Tan Tan Tan when an object is an ellipse or a spline. No formula
//! gives that circle, so it is found in steps. First, near where it was
//! picked, each ellipse or spline is stood in for by the circle that follows
//! it best there (its circle of curvature, or its tangent line where it runs
//! straight), and the exact solver gives the circles touching those. Then
//! each of them is corrected until it touches the real curves: for a centre
//! and radius, each object's touching point is the foot of the centre on it,
//! searched along the curve from the last one, and the centre and radius
//! move so that every foot is the radius away, on the same side as before
//! (Newton's method, three equations in three unknowns). A circle is kept
//! only when the steps settle with all three objects touched. As for lines
//! and circles, the one touching nearest the picks wins. An elliptical arc
//! counts as its whole ellipse; a spline only as itself, since it cannot be
//! continued.

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::tangent::{carrier, picked_curve, Carrier};
use crate::tangent_three::{contact, exact_circles};
use crate::{Ellipse, Entity, Point, Spline};

/// Most correction steps for the circle, and for each foot along a curve.
const STEPS: usize = 60;

fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

fn length(a: Point) -> f64 {
    a.x.hypot(a.y)
}

/// The unit vector a quarter turn left of `d`.
fn left(d: Point) -> Option<Point> {
    let len = length(d);
    (len > 0.0).then(|| Point::new(-d.y / len, d.x / len))
}

/// A line, circle, whole ellipse or spline, run along by a parameter.
enum Path<'a> {
    Line(Point, Point),
    Circle(Point, f64),
    Ellipse(Ellipse),
    Spline(&'a Spline),
}

/// Whether `entity` is an ellipse or a spline the circle can touch.
pub(crate) fn is_curve(entity: &Entity) -> bool {
    match entity {
        Entity::Ellipse(_) => true,
        Entity::Spline(s) => {
            let (a, b) = s.domain();
            b > a
        }
        _ => false,
    }
}

impl<'a> Path<'a> {
    fn of(entity: &'a Entity, pick: Point) -> Option<Self> {
        match entity {
            Entity::Ellipse(e) => Some(Path::Ellipse(Ellipse {
                start: 0.0,
                end: TAU,
                ..*e
            })),
            Entity::Spline(s) if is_curve(entity) => Some(Path::Spline(s)),
            _ => match carrier(&picked_curve(entity, pick)?)? {
                Carrier::Line(a, b) => Some(Path::Line(a, b)),
                Carrier::Circle(c, r) => Some(Path::Circle(c, r)),
            },
        }
    }

    fn point(&self, t: f64) -> Point {
        match self {
            Path::Line(a, b) => Point::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)),
            Path::Circle(c, r) => Point::new(c.x + r * t.cos(), c.y + r * t.sin()),
            Path::Ellipse(e) => e.point_at(t),
            Path::Spline(s) => s.point_at(t),
        }
    }

    /// The first derivative along the parameter.
    fn d1(&self, t: f64) -> Point {
        match self {
            Path::Line(a, b) => sub(*b, *a),
            Path::Circle(_, r) => Point::new(-r * t.sin(), r * t.cos()),
            Path::Ellipse(e) => e.tangent_at(t),
            Path::Spline(s) => s.derivative_at(t),
        }
    }

    /// The second derivative (for a spline, from its first, numerically).
    fn d2(&self, t: f64) -> Point {
        match self {
            Path::Line(..) => Point::default(),
            Path::Circle(c, _) | Path::Ellipse(Ellipse { center: c, .. }) => sub(*c, self.point(t)),
            Path::Spline(s) => {
                let (a, b) = s.domain();
                let h = (b - a) * 1e-6;
                let (lo, hi) = ((t - h).max(a), (t + h).min(b));
                let (dl, dh) = (s.derivative_at(lo), s.derivative_at(hi));
                Point::new((dh.x - dl.x) / (hi - lo), (dh.y - dl.y) / (hi - lo))
            }
        }
    }

    /// `t` brought onto the path: a spline stops at its ends, or wraps round
    /// when it is closed.
    fn keep(&self, t: f64) -> f64 {
        let Path::Spline(s) = self else { return t };
        let (a, b) = s.domain();
        let bounds = s.bounds();
        let size = length(sub(bounds.max, bounds.min));
        let closed = length(sub(s.start_point(), s.end_point())) <= 1e-9 * (1.0 + size);
        if closed {
            a + (t - a).rem_euclid(b - a)
        } else {
            t.clamp(a, b)
        }
    }

    /// The longest step along the parameter while looking for a foot.
    fn max_step(&self) -> f64 {
        match self {
            Path::Line(..) => f64::INFINITY,
            Path::Circle(..) | Path::Ellipse(_) => 0.5,
            Path::Spline(s) => {
                let (a, b) = s.domain();
                (b - a) * 0.1
            }
        }
    }

    /// The parameter of the point nearest `p`.
    fn nearest(&self, p: Point) -> f64 {
        match self {
            Path::Line(a, b) => {
                let d = sub(*b, *a);
                dot(sub(p, *a), d) / dot(d, d)
            }
            Path::Circle(c, _) => (p.y - c.y).atan2(p.x - c.x),
            Path::Ellipse(e) => e.nearest(p).1,
            Path::Spline(s) => s.nearest(p).1,
        }
    }

    /// The foot of `centre` on the path, searched from parameter `t`: where
    /// the line from the curve to `centre` is square to the curve.
    fn foot(&self, centre: Point, mut t: f64) -> f64 {
        for _ in 0..STEPS {
            let (off, d1, d2) = (sub(centre, self.point(t)), self.d1(t), self.d2(t));
            let speed = dot(d1, d1);
            if speed <= 0.0 {
                break;
            }
            let slope = speed - dot(off, d2);
            let step = if slope.abs() > 1e-9 * speed {
                slope
            } else {
                speed
            };
            let step = (dot(off, d1) / step).clamp(-self.max_step(), self.max_step());
            let next = self.keep(t + step);
            let settled = (next - t).abs() <= 1e-15 * (1.0 + t.abs());
            t = next;
            if settled {
                break;
            }
        }
        t
    }

    /// What stands in for the path near `t` in the exact solver: the path
    /// itself for a line or circle, else its circle of curvature, or its
    /// tangent line (`straight`, or where it hardly bends).
    fn stand_in(&self, t: f64, size: f64, straight: bool) -> Option<Carrier> {
        match self {
            Path::Line(a, b) => Some(Carrier::Line(*a, *b)),
            Path::Circle(c, r) => Some(Carrier::Circle(*c, *r)),
            Path::Ellipse(_) | Path::Spline(_) => {
                let (p, d1, d2) = (self.point(t), self.d1(t), self.d2(t));
                let normal = left(d1)?;
                let bend = (d1.x * d2.y - d1.y * d2.x) / dot(d1, d1).powf(1.5);
                if straight || bend.abs() * size < 1e-6 {
                    return Some(Carrier::Line(p, Point::new(p.x + d1.x, p.y + d1.y)));
                }
                let centre = Point::new(p.x + normal.x / bend, p.y + normal.y / bend);
                Some(Carrier::Circle(centre, 1.0 / bend.abs()))
            }
        }
    }

    /// Whether the circle is the path itself (a circle, or an ellipse with
    /// equal axes).
    fn is_itself(&self, centre: Point, radius: f64, tolerance: f64) -> bool {
        let round = matches!(self, Path::Circle(..))
            || matches!(self, Path::Ellipse(e) if (1.0 - e.radius_ratio).abs() < 1e-9);
        round
            && (0..4).all(|k| {
                let p = self.point(f64::from(k) * FRAC_PI_2);
                (length(sub(p, centre)) - radius).abs() < tolerance
            })
    }
}

/// Solves `rows · x = rhs` for three rows, or `None` when they are flat.
fn solve3(rows: [[f64; 3]; 3], rhs: [f64; 3]) -> Option<[f64; 3]> {
    let [a, b, c] = rows;
    let cross = |u: [f64; 3], v: [f64; 3]| {
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    };
    let (bc, ca, ab) = (cross(b, c), cross(c, a), cross(a, b));
    let det = a[0] * bc[0] + a[1] * bc[1] + a[2] * bc[2];
    if det.abs() < 1e-12 {
        return None;
    }
    Some([0, 1, 2].map(|i| (rhs[0] * bc[i] + rhs[1] * ca[i] + rhs[2] * ab[i]) / det))
}

/// The seed circle corrected until it touches the three paths, starting
/// each foot at `params` (where the seed touches the stand-ins), with its
/// touching points; `None` when the steps do not settle on a true touching
/// circle.
fn refine(
    paths: &[Path; 3],
    params: [f64; 3],
    seed: (Point, f64),
    size: f64,
) -> Option<(Point, f64, [Point; 3])> {
    let (mut centre, mut radius) = seed;
    let tolerance = 1e-9 * (size + radius);
    let mut ts = [0, 1, 2].map(|i| paths[i].foot(centre, params[i]));
    // Which side of each path the centre stays on.
    let sides = [0, 1, 2].map(|i| {
        let n = left(paths[i].d1(ts[i])).unwrap_or_default();
        if dot(sub(centre, paths[i].point(ts[i])), n) >= 0.0 {
            1.0
        } else {
            -1.0
        }
    });
    for _ in 0..STEPS {
        ts = [0, 1, 2].map(|i| paths[i].foot(centre, ts[i]));
        let mut rows = [[0.0; 3]; 3];
        let mut miss = [0.0; 3];
        for i in 0..3 {
            let n = left(paths[i].d1(ts[i]))?;
            rows[i] = [n.x, n.y, -sides[i]];
            miss[i] = sides[i] * radius - dot(sub(centre, paths[i].point(ts[i])), n);
        }
        if miss.iter().all(|m| m.abs() < tolerance) {
            break;
        }
        let [dx, dy, dr] = solve3(rows, miss)?;
        centre = Point::new(centre.x + dx, centre.y + dy);
        radius += dr;
        if !(radius > 0.0 && radius.is_finite()) {
            return None;
        }
    }
    let tolerance = 1e-7 * (size + radius);
    let mut touches = [Point::default(); 3];
    for i in 0..3 {
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
    Some((centre, radius, touches))
}

/// The circle (centre, radius) touching the three `objects` when one is an
/// ellipse or a spline: the one touching nearest the picks.
pub(crate) fn circle_touching(objects: [(&Entity, Point); 3]) -> Option<(Point, f64)> {
    let [(a, pa), (b, pb), (c, pc)] = objects;
    let paths = [Path::of(a, pa)?, Path::of(b, pb)?, Path::of(c, pc)?];
    let picks = [pa, pb, pc];
    let at_picks = [0, 1, 2].map(|i| paths[i].nearest(picks[i]));
    let size = picks
        .iter()
        .map(|p| length(sub(*p, pa)))
        .fold(0.0, f64::max)
        .max(1e-9);
    let mut best: Option<(f64, Point, f64)> = None;
    for straight in [false, true] {
        let stand_ins = [0, 1, 2].map(|i| paths[i].stand_in(at_picks[i], size, straight));
        let [Some(sa), Some(sb), Some(sc)] = stand_ins else {
            continue;
        };
        let stand_ins = [sa, sb, sc];
        for (_, centre, radius) in exact_circles(stand_ins, picks) {
            let touching = stand_ins.map(|s| contact(s, centre, radius));
            let [Some(ta), Some(tb), Some(tc)] = touching else {
                continue;
            };
            let points = [ta, tb, tc];
            let params = [0, 1, 2].map(|i| paths[i].nearest(points[i]));
            let Some((centre, radius, touches)) = refine(&paths, params, (centre, radius), size)
            else {
                continue;
            };
            let score: f64 = (0..3).map(|i| length(sub(touches[i], picks[i]))).sum();
            if best.is_none_or(|(s, ..)| score < s) {
                best = Some((score, centre, radius));
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|(_, centre, radius)| (centre, radius))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circle_tangent_to_three;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Line {
            start: p(a.0, a.1),
            end: p(b.0, b.1),
        }
    }

    fn circle(x: f64, y: f64, radius: f64) -> Entity {
        Entity::Circle {
            center: p(x, y),
            radius,
        }
    }

    fn ellipse(centre: Point, major: Point, ratio: f64) -> Entity {
        Entity::Ellipse(Ellipse::full(centre, major, ratio).unwrap())
    }

    fn spline(points: &[(f64, f64)]) -> Entity {
        let points = points.iter().map(|&(x, y)| p(x, y)).collect();
        Entity::Spline(Box::new(Spline::from_control_points(3, points).unwrap()))
    }

    fn assert_circle(found: Option<(Point, f64)>, centre: Point, radius: f64) {
        let (c, r) = found.expect("a circle");
        assert!(
            length(sub(c, centre)) < 1e-6 && (r - radius).abs() < 1e-6,
            "got {c:?} r {r}, expected {centre:?} r {radius}"
        );
    }

    /// Two wheels of radius 2 on the ground, 6 apart: the circle between
    /// them on the ground has radius 9/8 (see `tangent_three`).
    fn between_wheels(ground: &Entity, left: &Entity) -> Option<(Point, f64)> {
        let right = circle(3.0, 2.0, 2.0);
        circle_tangent_to_three([
            (ground, p(0.0, 0.0)),
            (left, p(-1.0, 1.5)),
            (&right, p(1.0, 1.5)),
        ])
    }

    #[test]
    fn an_ellipse_with_equal_axes_gives_the_circles_answer() {
        let ground = line((-10.0, 0.0), (10.0, 0.0));
        let wheel = ellipse(p(-3.0, 2.0), p(0.0, 2.0), 1.0);
        assert_circle(between_wheels(&ground, &wheel), p(0.0, 1.125), 1.125);
    }

    #[test]
    fn a_straight_spline_gives_the_lines_answer() {
        let ground = spline(&[(-10.0, 0.0), (-3.0, 0.0), (4.0, 0.0), (10.0, 0.0)]);
        let wheel = circle(-3.0, 2.0, 2.0);
        assert_circle(between_wheels(&ground, &wheel), p(0.0, 1.125), 1.125);
    }

    #[test]
    fn a_circle_between_two_lines_rests_on_the_ellipses_end() {
        // The ellipse's end at (4, 0) bends with radius b²/a = 1, so from
        // (7, 0) it is the nearest point, 3 away.
        let e = ellipse(p(0.0, 0.0), p(4.0, 0.0), 0.5);
        let (top, bottom) = (line((0.0, 3.0), (9.0, 3.0)), line((0.0, -3.0), (9.0, -3.0)));
        let found = circle_tangent_to_three([
            (&top, p(7.0, 3.0)),
            (&bottom, p(7.0, -3.0)),
            (&e, p(4.0, 0.0)),
        ]);
        assert_circle(found, p(7.0, 0.0), 3.0);
    }

    #[test]
    fn an_elliptical_arc_counts_as_its_whole_ellipse() {
        // Only the part from 45° to 270° is drawn; the circle touches the
        // missing end at (4, 0).
        let whole = Ellipse::full(p(0.0, 0.0), p(4.0, 0.0), 0.5).unwrap();
        let arc = Entity::Ellipse(Ellipse {
            start: FRAC_PI_2 / 2.0,
            end: 3.0 * FRAC_PI_2,
            ..whole
        });
        let (top, bottom) = (line((0.0, 3.0), (9.0, 3.0)), line((0.0, -3.0), (9.0, -3.0)));
        let pick = whole.point_at(FRAC_PI_2 / 2.0);
        let found =
            circle_tangent_to_three([(&top, p(7.0, 3.0)), (&bottom, p(7.0, -3.0)), (&arc, pick)]);
        assert_circle(found, p(7.0, 0.0), 3.0);
    }

    #[test]
    fn a_circle_between_two_lines_touches_a_bent_spline() {
        let points = vec![p(5.0, -1.0), p(7.0, 1.0), p(3.0, 3.0), p(5.0, 5.0)];
        let curve = Spline::from_control_points(3, points).unwrap();
        let s = Entity::Spline(Box::new(curve.clone()));
        let (a, b) = curve.domain();
        let pick = curve.point_at((a + b) / 2.0);
        let (floor, roof) = (line((-5.0, 0.0), (9.0, 0.0)), line((-5.0, 4.0), (9.0, 4.0)));
        let (centre, radius) =
            circle_tangent_to_three([(&floor, p(2.0, 0.0)), (&roof, p(2.0, 4.0)), (&s, pick)])
                .unwrap();
        assert!((radius - 2.0).abs() < 1e-6 && (centre.y - 2.0).abs() < 1e-6);
        let (near, _) = curve.nearest(centre);
        assert!((length(sub(near, centre)) - 2.0).abs() < 1e-6);
        assert!(centre.x < pick.x, "on the side it was picked from");
    }

    #[test]
    fn a_spline_is_not_continued_past_its_end() {
        // The ground stops at x = -5, so the circle between the wheels,
        // which would rest on it at x = 0, is not an answer.
        let ground = spline(&[(-10.0, 0.0), (-8.0, 0.0), (-7.0, 0.0), (-5.0, 0.0)]);
        let wheel = circle(-3.0, 2.0, 2.0);
        if let Some((centre, radius)) = between_wheels(&ground, &wheel) {
            assert!(length(sub(centre, p(0.0, 1.125))) > 1e-3 || (radius - 1.125).abs() > 1e-3);
        }
    }

    #[test]
    fn a_round_ellipse_is_never_the_answer_itself() {
        let (x, y) = (line((0.0, 0.0), (5.0, 0.0)), line((0.0, 0.0), (0.0, 5.0)));
        let ring = ellipse(p(1.0, 1.0), p(1.0, 0.0), 1.0);
        if let Some((centre, radius)) =
            circle_tangent_to_three([(&x, p(1.0, 0.0)), (&y, p(0.0, 1.0)), (&ring, p(2.0, 1.0))])
        {
            assert!(length(sub(centre, p(1.0, 1.0))) > 1e-3 || (radius - 1.0).abs() > 1e-3);
        }
    }

    #[test]
    fn far_from_the_origin_an_ellipse_is_as_exact() {
        let k = 1.0e6;
        let e = ellipse(p(k, k), p(4.0, 0.0), 0.5);
        let (top, bottom) = (
            line((k, k + 3.0), (k + 9.0, k + 3.0)),
            line((k, k - 3.0), (k + 9.0, k - 3.0)),
        );
        let found = circle_tangent_to_three([
            (&top, p(k + 7.0, k + 3.0)),
            (&bottom, p(k + 7.0, k - 3.0)),
            (&e, p(k + 4.0, k)),
        ]);
        assert_circle(found, p(k + 7.0, k), 3.0);
    }
    #[test]
    fn a_round_ellipse_always_gives_what_its_circle_gives() {
        // Fixed pseudo-random wheels and lines; picks on each object.
        let mut seed = 12345_u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64 * 20.0 - 10.0
        };
        let mut compared = 0;
        for _ in 0..60 {
            let (cx, cy, r) = (next(), next(), next().abs() / 2.0 + 0.5);
            let (ax, ay, bx, by) = (next(), next(), next(), next());
            let (dx, dy, er) = (next(), next(), next().abs() / 2.0 + 0.5);
            let wheel = circle(cx, cy, r);
            let rod = line((ax, ay), (bx, by));
            let round = circle(dx, dy, er);
            let as_ellipse = ellipse(p(dx, dy), p(er, 0.0), 1.0);
            let picks = [
                p(cx + r, cy),
                p((ax + bx) / 2.0, (ay + by) / 2.0),
                p(dx, dy + er),
            ];
            let exact =
                circle_tangent_to_three([(&wheel, picks[0]), (&rod, picks[1]), (&round, picks[2])]);
            let stepped = circle_tangent_to_three([
                (&wheel, picks[0]),
                (&rod, picks[1]),
                (&as_ellipse, picks[2]),
            ]);
            if let Some((centre, radius)) = exact {
                assert_circle(stepped, centre, radius);
                compared += 1;
            }
        }
        assert!(compared > 30, "only {compared} cases had a circle");
    }
}
