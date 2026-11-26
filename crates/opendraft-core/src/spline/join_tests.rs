use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: Point, b: Point) -> bool {
    distance(a, b) < 1e-9
}

fn whole() -> Spline {
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

#[test]
fn the_two_halves_of_a_cut_join_back_into_the_same_curve() {
    let original = whole();
    let (left, right) = original.split_at_parameter(0.4).unwrap();
    let joined = left.joined(&right, 1e-9).unwrap();
    let (a, b) = joined.domain();
    assert!((a - original.domain().0).abs() < 1e-12 && (b - original.domain().1).abs() < 1e-12);
    for step in 0..=40 {
        let t = a + (b - a) * f64::from(step) / 40.0;
        assert!(near(joined.point_at(t), original.point_at(t)), "at {t}");
    }
}

#[test]
fn the_order_and_direction_of_the_pieces_do_not_matter() {
    let original = whole();
    let (left, right) = original.split_at_parameter(0.6).unwrap();
    for joined in [
        right.joined(&left, 1e-9).unwrap(),
        left.joined(&right.reversed(), 1e-9).unwrap(),
        left.reversed().joined(&right, 1e-9).unwrap(),
    ] {
        // Whatever way it runs, it covers the same curve end to end.
        let (s, e) = (joined.start_point(), joined.end_point());
        let ends = [original.start_point(), original.end_point()];
        assert!(ends.iter().any(|q| near(*q, s)) && ends.iter().any(|q| near(*q, e)));
        let mid = joined.point_at((joined.domain().0 + joined.domain().1) / 2.0);
        assert!(original.distance_to(mid) < 1e-6);
    }
}

#[test]
fn a_rational_pair_joins_and_follows_both() {
    let one = Spline::new_nurbs(
        2,
        vec![p(0.0, 0.0), p(3.0, 4.0), p(6.0, 0.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        Some(vec![1.0, 2.0, 1.0]),
        false,
        false,
    )
    .unwrap();
    let two = Spline::new_nurbs(
        2,
        vec![p(6.0, 0.0), p(9.0, -4.0), p(12.0, 0.0)],
        vec![5.0, 5.0, 5.0, 7.0, 7.0, 7.0],
        Some(vec![3.0, 1.0, 1.0]),
        false,
        false,
    )
    .unwrap();
    let joined = one.joined(&two, 1e-9).unwrap();
    assert!(near(joined.start_point(), p(0.0, 0.0)) && near(joined.end_point(), p(12.0, 0.0)));
    assert_eq!(joined.control_points.len(), 5);
    // Both old curves lie on the new one.
    for step in 0..=10 {
        let t = f64::from(step) / 10.0;
        assert!(joined.distance_to(one.point_at(t)) < 1e-6);
        assert!(joined.distance_to(two.point_at(5.0 + 2.0 * t)) < 1e-6);
    }
}

#[test]
fn splines_that_do_not_meet_or_differ_in_degree_do_not_join() {
    let a = whole();
    let far = Spline::from_control_points(
        3,
        vec![p(20.0, 0.0), p(22.0, 3.0), p(24.0, 3.0), p(26.0, 0.0)],
    )
    .unwrap();
    assert!(a.joined(&far, 1e-9).is_none());
    let line = Spline::from_control_points(1, vec![p(14.0, 0.0), p(20.0, 0.0)]).unwrap();
    assert!(a.joined(&line, 1e-9).is_none());
}
