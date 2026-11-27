use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn data(points: &[Point], closed: bool) -> PolyData {
    PolyData {
        points: points.to_vec(),
        closed,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

fn near(a: Point, b: Point) -> bool {
    distance(a, b) < 1e-9
}

#[test]
fn a_fit_keeps_the_vertices_and_puts_a_point_between_each_pair() {
    let open = data(
        &[p(0.0, 0.0), p(4.0, 3.0), p(8.0, 0.0), p(12.0, 3.0)],
        false,
    );
    let fit = open.fitted().unwrap();
    assert_eq!(fit.points.len(), 7);
    for (k, original) in open.points.iter().enumerate() {
        assert!(near(fit.points[2 * k], *original));
    }
    assert_eq!(fit.bulges.len(), 7);
    let closed = data(&[p(0.0, 0.0), p(6.0, 0.0), p(6.0, 6.0), p(0.0, 6.0)], true);
    let fit = closed.fitted().unwrap();
    assert_eq!(fit.points.len(), 8);
    assert!(fit.closed);
}

#[test]
fn the_two_arcs_of_a_segment_meet_smoothly_and_so_do_neighbouring_segments() {
    let fit = data(
        &[p(0.0, 0.0), p(4.0, 3.0), p(8.0, 0.0), p(12.0, 3.0)],
        false,
    )
    .fitted()
    .unwrap();
    let n = fit.points.len();
    // Direction a curve runs in at the start of a segment and at its end.
    let start = |i: usize| {
        let (a, b) = (fit.points[i], fit.points[i + 1]);
        let back = bulge::direction_at_end(b, a, -fit.bulges[i]);
        Point::new(-back.x, -back.y)
    };
    let end = |i: usize| {
        let (a, b) = (fit.points[i], fit.points[i + 1]);
        bulge::direction_at_end(a, b, fit.bulges[i])
    };
    let same = |a: Point, b: Point| (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6;
    for i in 0..n - 2 {
        assert!(
            same(end(i), start(i + 1)),
            "no kink at point {} ({:?} then {:?})",
            i + 1,
            end(i),
            start(i + 1)
        );
    }
}

#[test]
fn collinear_vertices_fit_to_a_straight_line_and_fewer_than_three_do_not_fit() {
    let line = data(&[p(0.0, 0.0), p(2.0, 0.0), p(6.0, 0.0)], false);
    let fit = line.fitted().unwrap();
    assert!(fit.bulges.iter().all(|k| k.abs() < 1e-9));
    assert!(data(&[p(0.0, 0.0), p(1.0, 1.0)], false).fitted().is_none());
}

#[test]
fn a_spline_reaches_the_ends_when_open_and_loops_inside_the_frame_when_closed() {
    let open = data(
        &[p(0.0, 0.0), p(4.0, 6.0), p(8.0, 0.0), p(12.0, 6.0)],
        false,
    );
    let curve = open.splined().unwrap();
    assert!(near(curve.points[0], p(0.0, 0.0)));
    assert!(near(*curve.points.last().unwrap(), p(12.0, 6.0)));
    assert!(curve.bulges.is_empty() && !curve.closed);
    // It passes inside the corners it does not touch.
    assert!(curve
        .points
        .iter()
        .all(|q| q.y >= -1e-9 && q.y <= 6.0 + 1e-9));
    let square = data(&[p(0.0, 0.0), p(6.0, 0.0), p(6.0, 6.0), p(0.0, 6.0)], true);
    let curve = square.splined().unwrap();
    assert!(curve.closed);
    assert_eq!(curve.points.len(), 4 * STEPS);
    assert!(curve
        .points
        .iter()
        .all(|q| (0.0..=6.0).contains(&q.x) && (0.0..=6.0).contains(&q.y)));
    assert!(data(&[p(0.0, 0.0), p(1.0, 1.0)], false).splined().is_none());
}

#[test]
fn a_spline_becomes_a_polyline_along_it() {
    let spline =
        Spline::from_control_points(3, vec![p(0.0, 0.0), p(2.0, 3.0), p(5.0, 3.0), p(7.0, 0.0)])
            .unwrap();
    let line = PolyData::from_spline(&spline, 0.01).unwrap();
    assert!(near(line.points[0], spline.start_point()));
    assert!(line.points.len() > 4 && !line.closed);
}

#[test]
fn a_tangent_set_at_a_vertex_bends_the_fit_to_leave_along_it() {
    // Three points in a line fit to a straight line...
    let line = data(&[p(0.0, 0.0), p(5.0, 0.0), p(10.0, 0.0)], false);
    assert!(!bulge::has_arcs(&line.fitted().unwrap().bulges));
    // ...until the middle one is told to run straight up.
    let set = [None, Some(std::f64::consts::FRAC_PI_2), None];
    let bent = line.fitted_with(&set).unwrap();
    assert!(bulge::has_arcs(&bent.bulges));
    // The curve passes through the same vertices, and at the middle one the
    // first arc arrives and the next leaves along the set direction.
    assert!(near(bent.points[2], p(5.0, 0.0)));
    let (a, m, b) = (bent.points[1], bent.points[2], bent.points[3]);
    let arriving = (
        bulge::point_at(a, m, bent.bulges[1], 1.0),
        bulge::point_at(a, m, bent.bulges[1], 0.999_999),
    );
    let leaving = (
        bulge::point_at(m, b, bent.bulges[2], 0.000_001),
        bulge::point_at(m, b, bent.bulges[2], 0.0),
    );
    for (to, from) in [arriving, leaving] {
        let d = distance(from, to);
        let (dx, dy) = ((to.x - from.x) / d, (to.y - from.y) / d);
        assert!(dx.abs() < 1e-3 && dy.abs() > 0.999, "{dx} {dy}");
    }
    // A tangent nobody set leaves the chord's direction as it was.
    assert_eq!(line.fitted_with(&[None, None, None]), line.fitted());
}
