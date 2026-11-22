//! Circles tangent to three objects (CIRCLE, Tan Tan Tan): Apollonius'
//! problem. A circle of centre `c` and radius `r` touches a line when `c` is
//! `r` from it on one side or the other, and touches a circle of radius `R`
//! when `c` is `R + r` from its centre (outside) or `|R - r|` (inside). For
//! one choice of sides those are three equations in `(cx, cy, r)`. A line's
//! is linear. A circle's is `cx² + cy² - r²` plus linear terms, the same
//! squared part for every circle, so the difference of two circles' is
//! linear too. That leaves two linear equations, whose answers lie along a
//! line in `(cx, cy, r)`, and one circle's own equation, a quadratic along
//! it; three lines give three linear equations. As in Ttr, objects count as
//! the whole line or circle they lie on, and of the possible circles the one
//! whose touching points lie nearest the picks wins.

use crate::tangent::{carrier, picked_curve, Carrier};
use crate::{Entity, Point};

/// Below this (in coordinates scaled to the picks' spread) a value is zero.
const EPS: f64 = 1e-9;

type V3 = [f64; 3];

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn scaled(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn sum(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// `row · (cx, cy, r) = rhs`.
struct Linear {
    row: V3,
    rhs: f64,
}

/// A circle's equation: `cx² + cy² - r² + lin · (cx, cy, r) + k = 0`.
struct Round {
    lin: V3,
    k: f64,
}

/// The equation for touching `carrier` on side `side` (±1): for a line, the
/// side of it the centre is on; for a circle, +1 outside and -1 inside.
fn equation(carrier: Carrier, side: f64) -> Result<Linear, Round> {
    match carrier {
        Carrier::Line(a, b) => {
            let len = distance(a, b);
            let (nx, ny) = (-(b.y - a.y) / len, (b.x - a.x) / len);
            Ok(Linear {
                row: [nx, ny, -side],
                rhs: nx * a.x + ny * a.y,
            })
        }
        Carrier::Circle(c, radius) => Err(Round {
            lin: [-2.0 * c.x, -2.0 * c.y, -2.0 * side * radius],
            k: c.x * c.x + c.y * c.y - radius * radius,
        }),
    }
}

/// The solutions `(cx, cy, r)` of three equations, `r` of either sign.
fn solve(equations: Vec<Result<Linear, Round>>) -> Vec<V3> {
    let mut lines = Vec::new();
    let mut first: Option<Round> = None;
    for eq in equations {
        match (eq, &first) {
            (Ok(line), _) => lines.push(line),
            (Err(round), None) => first = Some(round),
            (Err(round), Some(f)) => lines.push(Linear {
                row: sum(round.lin, scaled(f.lin, -1.0)),
                rhs: f.k - round.k,
            }),
        }
    }
    match (first, lines.as_slice()) {
        (None, [a, b, c]) => {
            let det = dot(a.row, cross(b.row, c.row));
            if det.abs() < EPS {
                return Vec::new();
            }
            let x = sum(
                sum(
                    scaled(cross(b.row, c.row), a.rhs),
                    scaled(cross(c.row, a.row), b.rhs),
                ),
                scaled(cross(a.row, b.row), c.rhs),
            );
            vec![scaled(x, 1.0 / det)]
        }
        (Some(round), [a, b]) => along_line(&round, a, b),
        _ => Vec::new(),
    }
}

/// Where the line of answers of `a` and `b` meets the circle equation.
fn along_line(round: &Round, a: &Linear, b: &Linear) -> Vec<V3> {
    let normal = cross(a.row, b.row);
    let len = dot(normal, normal).sqrt();
    if len < EPS {
        return Vec::new();
    }
    let dir = scaled(normal, 1.0 / len);
    // The point of the line nearest the origin: a mix of the two rows.
    let (aa, ab, bb) = (dot(a.row, a.row), dot(a.row, b.row), dot(b.row, b.row));
    let det = aa * bb - ab * ab;
    let ya = (a.rhs * bb - b.rhs * ab) / det;
    let yb = (b.rhs * aa - a.rhs * ab) / det;
    let base = sum(scaled(a.row, ya), scaled(b.row, yb));
    // Minkowski square: cx² + cy² - r².
    let square = |u: V3, v: V3| u[0] * v[0] + u[1] * v[1] - u[2] * v[2];
    let qa = square(dir, dir);
    let qb = 2.0 * square(base, dir) + dot(round.lin, dir);
    let qc = square(base, base) + dot(round.lin, base) + round.k;
    let roots = if qa.abs() < EPS {
        if qb.abs() < EPS {
            Vec::new()
        } else {
            vec![-qc / qb]
        }
    } else {
        let disc = qb * qb - 4.0 * qa * qc;
        if disc < -EPS * (qb * qb + (4.0 * qa * qc).abs()) {
            Vec::new()
        } else {
            let root = disc.max(0.0).sqrt();
            vec![(-qb + root) / (2.0 * qa), (-qb - root) / (2.0 * qa)]
        }
    };
    roots
        .into_iter()
        .map(|t| sum(base, scaled(dir, t)))
        .collect()
}

/// `carrier` moved by `-origin` and shrunk by `scale`.
fn normalised(carrier: Carrier, origin: Point, scale: f64) -> Carrier {
    let n = |p: Point| Point::new((p.x - origin.x) / scale, (p.y - origin.y) / scale);
    match carrier {
        Carrier::Line(a, b) => Carrier::Line(n(a), n(b)),
        Carrier::Circle(c, r) => Carrier::Circle(n(c), r / scale),
    }
}

/// Where the circle of `centre` and `radius` touches `carrier`, or `None`
/// when it does not touch it at one point (it is the carrier's own circle).
pub(crate) fn contact(carrier: Carrier, centre: Point, radius: f64) -> Option<Point> {
    let tolerance = 1e-6 * (1.0 + radius);
    match carrier {
        Carrier::Line(a, b) => {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let t = ((centre.x - a.x) * dx + (centre.y - a.y) * dy) / (dx * dx + dy * dy);
            let foot = Point::new(a.x + t * dx, a.y + t * dy);
            ((distance(foot, centre) - radius).abs() < tolerance).then_some(foot)
        }
        Carrier::Circle(c, r) => {
            let d = distance(c, centre);
            if d < tolerance {
                return None;
            }
            let (ux, uy) = ((centre.x - c.x) / d, (centre.y - c.y) / d);
            // The near side, or the far side when the circle holds the carrier.
            [1.0, -1.0]
                .into_iter()
                .map(|s| Point::new(c.x + s * ux * r, c.y + s * uy * r))
                .find(|p| (distance(*p, centre) - radius).abs() < tolerance)
        }
    }
}

/// Whether CIRCLE Tan Tan Tan can touch `entity` where it was picked: its
/// picked part is a line, a circle or an arc (a polyline's segment counts),
/// or it is an ellipse, an elliptical arc or a spline.
pub fn touchable_by_tangent_circle(entity: &Entity, pick: Point) -> bool {
    exact_carrier(entity, pick).is_some() || crate::tangent_three_curves::is_curve(entity)
}

/// The line or circle the picked part of `entity` lies on.
fn exact_carrier(entity: &Entity, pick: Point) -> Option<Carrier> {
    carrier(&picked_curve(entity, pick)?)
}

/// The circle (centre, radius) touching the three `objects`, each given with
/// the point it was picked at. Of the possible circles it is the one whose
/// touching points lie nearest the picks. `None` when no circle touches all
/// three, or an object cannot be touched. Lines, circles and arcs are solved
/// exactly; with an ellipse or a spline, see `tangent_three_curves`.
pub fn circle_tangent_to_three(objects: [(&Entity, Point); 3]) -> Option<(Point, f64)> {
    let picks = objects.map(|(_, pick)| pick);
    let exact = objects.map(|(entity, pick)| exact_carrier(entity, pick));
    match exact {
        [Some(a), Some(b), Some(c)] => exact_circles([a, b, c], picks)
            .first()
            .map(|&(_, centre, radius)| (centre, radius)),
        _ => crate::tangent_three_curves::circle_touching(objects),
    }
}

/// Every circle touching the three carriers (with a single touching point
/// on each), as (score, centre, radius), the score being how far its
/// touching points lie from the picks, best first.
pub(crate) fn exact_circles(carriers: [Carrier; 3], picks: [Point; 3]) -> Vec<(f64, Point, f64)> {
    // Solve near the origin at about unit size, so tolerances hold far out.
    let origin = picks[0];
    let scale = picks
        .iter()
        .map(|pick| distance(*pick, origin))
        .fold(0.0, f64::max)
        .max(EPS);
    let local = carriers.map(|c| normalised(c, origin, scale));
    let mut found = Vec::new();
    for sides in 0..8 {
        let side = |i: usize| if sides >> i & 1 == 0 { 1.0 } else { -1.0 };
        let equations = (0..3).map(|i| equation(local[i], side(i))).collect();
        for [x, y, r] in solve(equations) {
            if r <= EPS {
                continue;
            }
            let centre = Point::new(origin.x + x * scale, origin.y + y * scale);
            let radius = r * scale;
            let score = carriers.iter().zip(picks).try_fold(0.0, |sum, (c, pick)| {
                Some(sum + distance(contact(*c, centre, radius)?, pick))
            });
            if let Some(score) = score {
                found.push((score, centre, radius));
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn assert_circle(found: Option<(Point, f64)>, centre: Point, radius: f64) {
        let (c, r) = found.expect("a circle");
        assert!(
            distance(c, centre) < 1e-6 && (r - radius).abs() < 1e-6,
            "got {c:?} r {r}, expected {centre:?} r {radius}"
        );
    }

    /// The right triangle (0,0), (4,0), (0,3): sides 4, 3 and 5.
    fn triangle(shift: f64) -> [Entity; 3] {
        let s = |x: f64, y: f64| (x + shift, y + shift);
        [
            line(s(0.0, 0.0), s(4.0, 0.0)),
            line(s(0.0, 0.0), s(0.0, 3.0)),
            line(s(4.0, 0.0), s(0.0, 3.0)),
        ]
    }

    #[test]
    fn three_lines_give_the_inscribed_circle_when_picked_on_the_sides() {
        let [a, b, c] = triangle(0.0);
        let found =
            circle_tangent_to_three([(&a, p(2.0, 0.0)), (&b, p(0.0, 1.5)), (&c, p(2.0, 1.5))]);
        assert_circle(found, p(1.0, 1.0), 1.0);
    }

    #[test]
    fn three_lines_give_an_outside_circle_when_picked_beyond_the_corners() {
        // The excircle across the long side touches the legs' extensions at
        // (6, 0) and (0, 6), and the long side at (2.4, 1.2).
        let [a, b, c] = triangle(0.0);
        let found =
            circle_tangent_to_three([(&a, p(6.0, 0.0)), (&b, p(0.0, 6.0)), (&c, p(2.4, 1.2))]);
        assert_circle(found, p(6.0, 6.0), 6.0);
    }

    #[test]
    fn far_from_the_origin_the_circle_is_as_exact() {
        let k = 1.0e6;
        let [a, b, c] = triangle(k);
        let found = circle_tangent_to_three([
            (&a, p(k + 2.0, k)),
            (&b, p(k, k + 1.5)),
            (&c, p(k + 2.0, k + 1.5)),
        ]);
        assert_circle(found, p(k + 1.0, k + 1.0), 1.0);
    }

    #[test]
    fn a_polylines_sides_count_as_three_lines() {
        let outline = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(0.0, 3.0)],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        let found = circle_tangent_to_three([
            (&outline, p(2.0, 0.0)),
            (&outline, p(0.0, 1.5)),
            (&outline, p(2.0, 1.5)),
        ]);
        assert_circle(found, p(1.0, 1.0), 1.0);
    }

    #[test]
    fn three_circles_give_the_circle_between_or_round_them() {
        // Radius 2 at the corners of a triangle of side 10: its centre is
        // 10/√3 from each corner.
        let h = 10.0 * 3.0_f64.sqrt() / 2.0;
        let (a, b, c) = (
            circle(0.0, 0.0, 2.0),
            circle(10.0, 0.0, 2.0),
            circle(5.0, h, 2.0),
        );
        let middle = p(5.0, h / 3.0);
        let far = 10.0 / 3.0_f64.sqrt();
        let towards = |x: f64, y: f64, s: f64| {
            let d = distance(p(x, y), middle);
            p(
                x + s * 2.0 * (middle.x - x) / d,
                y + s * 2.0 * (middle.y - y) / d,
            )
        };
        let between = circle_tangent_to_three([
            (&a, towards(0.0, 0.0, 1.0)),
            (&b, towards(10.0, 0.0, 1.0)),
            (&c, towards(5.0, h, 1.0)),
        ]);
        assert_circle(between, middle, far - 2.0);
        let round = circle_tangent_to_three([
            (&a, towards(0.0, 0.0, -1.0)),
            (&b, towards(10.0, 0.0, -1.0)),
            (&c, towards(5.0, h, -1.0)),
        ]);
        assert_circle(round, middle, far + 2.0);
    }

    #[test]
    fn a_line_and_two_circles_give_the_circle_resting_between_them() {
        // Two wheels of radius 2 on the ground, 6 apart: the circle between
        // them on the ground has 9 + (r - 2)² = (r + 2)², so r = 9/8.
        let ground = line((-10.0, 0.0), (10.0, 0.0));
        let (left, right) = (circle(-3.0, 2.0, 2.0), circle(3.0, 2.0, 2.0));
        let found = circle_tangent_to_three([
            (&ground, p(0.0, 0.0)),
            (&left, p(-1.0, 1.5)),
            (&right, p(1.0, 1.5)),
        ]);
        assert_circle(found, p(0.0, 1.125), 1.125);
    }

    #[test]
    fn an_arc_counts_as_its_whole_circle() {
        // The upper half of the right wheel, picked near its lower half.
        let ground = line((-10.0, 0.0), (10.0, 0.0));
        let left = circle(-3.0, 2.0, 2.0);
        let right = Entity::Arc {
            center: p(3.0, 2.0),
            radius: 2.0,
            start: 0.0,
            end: std::f64::consts::PI,
        };
        let found = circle_tangent_to_three([
            (&ground, p(0.0, 0.0)),
            (&left, p(-1.0, 1.5)),
            (&right, p(1.0, 1.5)),
        ]);
        assert_circle(found, p(0.0, 1.125), 1.125);
    }

    #[test]
    fn a_picked_circle_is_never_the_answer_itself() {
        // The circle already touches both axes at (1, 0) and (0, 1); picked
        // there it would score best, but it is not a new circle.
        let (x, y, ring) = (
            line((0.0, 0.0), (5.0, 0.0)),
            line((0.0, 0.0), (0.0, 5.0)),
            circle(1.0, 1.0, 1.0),
        );
        if let Some((centre, radius)) =
            circle_tangent_to_three([(&x, p(1.0, 0.0)), (&y, p(0.0, 1.0)), (&ring, p(2.0, 1.0))])
        {
            assert!(distance(centre, p(1.0, 1.0)) > 1e-3 || (radius - 1.0).abs() > 1e-3);
        }
        // Picked in the corner: the small circle there, r = 3 - 2√2.
        let small = 3.0 - 2.0 * 2.0_f64.sqrt();
        let corner = 1.0 - 0.5_f64.sqrt();
        let found = circle_tangent_to_three([
            (&x, p(small, 0.0)),
            (&y, p(0.0, small)),
            (&ring, p(corner, corner)),
        ]);
        assert_circle(found, p(small, small), small);
    }

    #[test]
    fn no_circle_touches_three_parallel_lines_or_one_line_thrice() {
        let (a, b, c) = (
            line((0.0, 0.0), (9.0, 0.0)),
            line((0.0, 2.0), (9.0, 2.0)),
            line((0.0, 5.0), (9.0, 5.0)),
        );
        assert!(
            circle_tangent_to_three([(&a, p(1.0, 0.0)), (&b, p(1.0, 2.0)), (&c, p(1.0, 5.0))])
                .is_none()
        );
        assert!(
            circle_tangent_to_three([(&a, p(1.0, 0.0)), (&a, p(4.0, 0.0)), (&a, p(8.0, 0.0))])
                .is_none()
        );
    }

    #[test]
    fn two_parallel_lines_and_a_crossing_one_give_a_circle_between_them() {
        let (a, b, c) = (
            line((0.0, 0.0), (9.0, 0.0)),
            line((0.0, 2.0), (9.0, 2.0)),
            line((4.0, -1.0), (4.0, 3.0)),
        );
        let found =
            circle_tangent_to_three([(&a, p(5.0, 0.0)), (&b, p(5.0, 2.0)), (&c, p(4.0, 1.0))]);
        assert_circle(found, p(5.0, 1.0), 1.0);
    }

    #[test]
    fn lines_circles_arcs_and_ellipses_can_be_touched_but_not_points() {
        assert!(touchable_by_tangent_circle(
            &line((0.0, 0.0), (1.0, 0.0)),
            p(0.5, 0.0)
        ));
        assert!(touchable_by_tangent_circle(
            &circle(0.0, 0.0, 1.0),
            p(1.0, 0.0)
        ));
        let ellipse = Entity::Ellipse(crate::Ellipse::full(p(0.0, 0.0), p(2.0, 0.0), 0.5).unwrap());
        assert!(touchable_by_tangent_circle(&ellipse, p(2.0, 0.0)));
        let point = Entity::Point { at: p(0.0, 0.0) };
        assert!(!touchable_by_tangent_circle(&point, p(0.0, 0.0)));
    }
}
