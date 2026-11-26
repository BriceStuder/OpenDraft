use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn pline(points: &[Point], closed: bool, bulges: &[f64], widths: &[(f64, f64)]) -> Entity {
    Entity::Polyline {
        points: points.to_vec(),
        closed,
        bulges: bulges.to_vec(),
        widths: widths.to_vec(),
    }
}

fn parts(e: &Entity) -> (Vec<Point>, Vec<f64>, Vec<(f64, f64)>) {
    match e {
        Entity::Polyline {
            points,
            bulges,
            widths,
            ..
        } => (points.clone(), bulges.clone(), widths.clone()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_grips_of_a_polyline_are_told_apart() {
    let open = pline(
        &[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)],
        false,
        &[0.0, 1.0, 0.0],
        &[],
    );
    assert_eq!(open.vertex_grip(1), Some(VertexGrip::Vertex(1)));
    assert_eq!(
        open.vertex_grip(3),
        Some(VertexGrip::Segment {
            index: 0,
            arc: false
        })
    );
    assert_eq!(
        open.vertex_grip(4),
        Some(VertexGrip::Segment {
            index: 1,
            arc: true
        })
    );
    assert_eq!(open.vertex_grip(5), None);
    let line = Entity::Line {
        start: p(0.0, 0.0),
        end: p(1.0, 0.0),
    };
    assert_eq!(line.vertex_grip(0), None);
}

#[test]
fn a_vertex_added_on_a_segment_splits_it_and_its_width() {
    let open = pline(
        &[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
        false,
        &[],
        &[(2.0, 4.0), (1.0, 1.0), (0.0, 0.0)],
    );
    // From the middle grip of the first segment (grip 3).
    let (points, bulges, widths) = parts(&open.with_vertex_added(3, p(5.0, 3.0)).unwrap());
    assert_eq!(
        points,
        [p(0.0, 0.0), p(5.0, 3.0), p(10.0, 0.0), p(10.0, 10.0)]
    );
    assert!(bulges.is_empty());
    assert_eq!(widths, [(2.0, 3.0), (3.0, 4.0), (1.0, 1.0), (0.0, 0.0)]);
    // From a vertex grip, it goes on the segment that starts there.
    let (points, ..) = parts(&open.with_vertex_added(1, p(10.0, 5.0)).unwrap());
    assert_eq!(points[2], p(10.0, 5.0));
    // From the last vertex of an open polyline, it goes on beyond the end.
    let (points, ..) = parts(&open.with_vertex_added(2, p(10.0, 20.0)).unwrap());
    assert_eq!(points.len(), 4);
    assert_eq!(points[3], p(10.0, 20.0));
}

#[test]
fn a_vertex_added_on_an_arc_goes_on_the_arc_and_splits_it_in_two() {
    // A half circle from (0, 0) to (4, 0), bulging below.
    let arc = pline(
        &[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 5.0)],
        false,
        &[1.0, 0.0, 0.0],
        &[],
    );
    let (points, bulges, _) = parts(&arc.with_vertex_added(3, p(2.0, -5.0)).unwrap());
    assert!((points[1].x - 2.0).abs() < 1e-9 && (points[1].y + 2.0).abs() < 1e-9);
    // Each half is a quarter circle: bulge tan(90 / 4 degrees).
    let quarter = std::f64::consts::FRAC_PI_8.tan();
    assert!((bulges[0] - quarter).abs() < 1e-9 && (bulges[1] - quarter).abs() < 1e-9);
}

#[test]
fn a_vertex_added_on_the_closing_segment_of_a_closed_polyline() {
    let square = pline(
        &[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)],
        true,
        &[],
        &[],
    );
    // The closing segment is segment 3, whose middle grip is 4 + 3.
    let (points, ..) = parts(&square.with_vertex_added(7, p(-1.0, 2.0)).unwrap());
    assert_eq!(points.len(), 5);
    assert_eq!(points[4], p(-1.0, 2.0));
}

#[test]
fn a_vertex_removed_joins_its_neighbours_with_a_straight_segment() {
    let open = pline(
        &[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(8.0, 4.0)],
        false,
        &[0.0, 1.0, 0.0, 0.0],
        &[(1.0, 2.0), (2.0, 3.0), (3.0, 4.0), (0.0, 0.0)],
    );
    let (points, bulges, widths) = parts(&open.with_vertex_removed(2).unwrap());
    assert_eq!(points, [p(0.0, 0.0), p(4.0, 0.0), p(8.0, 4.0)]);
    assert!(
        bulges.is_empty(),
        "the arc that ended at the vertex is gone"
    );
    assert_eq!(widths, [(1.0, 2.0), (2.0, 4.0), (0.0, 0.0)]);
    // The ends can go too.
    let (points, ..) = parts(&open.with_vertex_removed(0).unwrap());
    assert_eq!(points.len(), 3);
    let (points, _, widths) = parts(&open.with_vertex_removed(3).unwrap());
    assert_eq!(points, [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)]);
    assert_eq!(widths[2], (0.0, 0.0));
}

#[test]
fn too_few_vertices_are_never_left() {
    let two = pline(&[p(0.0, 0.0), p(1.0, 0.0)], false, &[], &[]);
    assert!(two.with_vertex_removed(0).is_none());
    let triangle = pline(&[p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)], true, &[], &[]);
    assert!(triangle.with_vertex_removed(1).is_none());
    let square = pline(
        &[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)],
        true,
        &[],
        &[],
    );
    let (points, ..) = parts(&square.with_vertex_removed(0).unwrap());
    assert_eq!(points, [p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)]);
}

#[test]
fn a_segment_turns_into_an_arc_through_a_point_and_back() {
    let open = pline(&[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)], false, &[], &[]);
    let bent = open.with_segment_arc(0, p(2.0, -2.0)).unwrap();
    let (_, bulges, _) = parts(&bent);
    // A half circle from (0, 0) to (4, 0) passing below: a bulge of 1.
    assert!((bulges[0] - 1.0).abs() < 1e-9, "{bulges:?}");
    assert_eq!(
        bent.vertex_grip(3),
        Some(VertexGrip::Segment {
            index: 0,
            arc: true
        })
    );
    let (_, bulges, _) = parts(&bent.with_segment_straight(0).unwrap());
    assert!(bulges.is_empty());
    // Three points in a line make no arc.
    assert!(open.with_segment_arc(0, p(2.0, 0.0)).is_none());
}
