use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn cubic() -> Spline {
    Spline::from_control_points(
        3,
        vec![
            p(0.0, 0.0),
            p(2.0, 5.0),
            p(5.0, 6.0),
            p(8.0, 1.0),
            p(11.0, 4.0),
            p(14.0, 0.0),
        ],
    )
    .unwrap()
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).hypot(a.y - b.y) < 1e-9
}

/// Both pieces follow the whole curve, parameter for parameter.
fn follows(whole: &Spline, left: &Spline, right: &Spline, u: f64) {
    let (a, b) = left.domain();
    assert!((b - u).abs() < 1e-12 && (a - whole.domain().0).abs() < 1e-12);
    let (c, d) = right.domain();
    assert!((c - u).abs() < 1e-12 && (d - whole.domain().1).abs() < 1e-12);
    for step in 0..=20 {
        let t = a + (b - a) * f64::from(step) / 20.0;
        assert!(near(left.point_at(t), whole.point_at(t)), "left at {t}");
        let t = c + (d - c) * f64::from(step) / 20.0;
        assert!(near(right.point_at(t), whole.point_at(t)), "right at {t}");
    }
    assert!(near(left.end_point(), right.start_point()));
}

#[test]
fn a_cut_gives_two_splines_that_follow_the_curve_exactly() {
    let whole = cubic();
    for u in [0.13, 0.5, 0.77] {
        let (left, right) = whole.split_at_parameter(u).unwrap();
        follows(&whole, &left, &right, u);
        assert!(left.fit.is_none() && !left.closed);
    }
}

#[test]
fn a_cut_at_an_existing_knot_works_too() {
    let whole = cubic();
    let knot = whole.knots[whole.degree as usize + 1];
    let (left, right) = whole.split_at_parameter(knot).unwrap();
    follows(&whole, &left, &right, knot);
}

#[test]
fn a_rational_spline_is_cut_exactly_and_keeps_its_weights() {
    let whole = Spline::new_nurbs(
        2,
        vec![p(0.0, 0.0), p(4.0, 6.0), p(8.0, 0.0), p(12.0, 5.0)],
        vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
        Some(vec![1.0, 2.0, 0.5, 1.0]),
        false,
        false,
    )
    .unwrap();
    let (left, right) = whole.split_at_parameter(0.3).unwrap();
    follows(&whole, &left, &right, 0.3);
    assert!(left.weights.is_some());
}

#[test]
fn cuts_at_the_ends_or_of_a_periodic_spline_are_refused() {
    let whole = cubic();
    assert!(whole.split_at_parameter(0.0).is_none());
    assert!(whole.split_at_parameter(1.0).is_none());
    assert!(whole.split_at_parameter(2.0).is_none());
    let mut periodic = whole;
    periodic.periodic = true;
    assert!(periodic.split_at_parameter(0.5).is_none());
}

#[test]
fn a_piece_can_be_cut_again() {
    let whole = cubic();
    let (left, right) = whole.split_at_parameter(0.4).unwrap();
    let (a, b) = right.split_at_parameter(0.7).unwrap();
    follows(&right, &a, &b, 0.7);
    assert!(near(left.end_point(), a.start_point()));
}
