use std::f64::consts::{FRAC_PI_2, PI};

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn line(a: Point, b: Point) -> Entity {
    Entity::Line { start: a, end: b }
}

#[test]
fn a_line_runs_on_both_ways_as_an_infinite_line() {
    let edge = line(p(0.0, 0.0), p(10.0, 0.0));
    let implied = edge.with_implied_extension();
    assert_eq!(implied.len(), 1);
    assert!(matches!(implied[0], Entity::Xline { .. }));
    // A vertical object far past the line's end now meets it.
    let target = line(p(50.0, -5.0), p(50.0, 5.0));
    assert!(target.intersections(&edge).is_empty());
    let hits = target.intersections(&implied[0]);
    assert_eq!(hits.len(), 1);
    assert!((hits[0].x - 50.0).abs() < 1e-6 && hits[0].y.abs() < 1e-6);
}

#[test]
fn a_zero_length_line_stays_as_it_is() {
    let dot = line(p(1.0, 1.0), p(1.0, 1.0));
    assert_eq!(dot.with_implied_extension(), vec![dot]);
}

#[test]
fn an_arc_becomes_its_whole_circle() {
    let arc = Entity::Arc {
        center: p(0.0, 0.0),
        radius: 5.0,
        start: 0.0,
        end: FRAC_PI_2,
    };
    assert_eq!(
        arc.with_implied_extension(),
        vec![Entity::Circle {
            center: p(0.0, 0.0),
            radius: 5.0
        }]
    );
    // The point on the far side of the circle is now met.
    let across = line(p(-9.0, 0.0), p(-1.0, 0.0));
    assert!(across.intersections(&arc).is_empty());
    assert_eq!(
        across.intersections(&arc.with_implied_extension()[0]).len(),
        1
    );
}

#[test]
fn an_elliptical_arc_becomes_its_whole_ellipse() {
    let arc = Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, 0.0, PI).unwrap();
    let made = Entity::Ellipse(arc).with_implied_extension();
    assert_eq!(made.len(), 1);
    let Entity::Ellipse(full) = &made[0] else {
        panic!("{made:?}")
    };
    assert!(full.is_full());
    assert_eq!(full.center, arc.center);
}

#[test]
fn an_open_polyline_gains_a_ray_beyond_each_straight_end() {
    let pline = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    };
    let made = pline.with_implied_extension();
    assert_eq!(made.len(), 3);
    assert_eq!(made[0], pline);
    assert_eq!(
        made[1],
        Entity::Ray {
            origin: p(0.0, 0.0),
            direction: p(-1.0, 0.0)
        }
    );
    assert_eq!(
        made[2],
        Entity::Ray {
            origin: p(10.0, 10.0),
            direction: p(0.0, 1.0)
        }
    );
}

#[test]
fn an_arc_segment_at_the_end_of_a_polyline_gives_its_circle() {
    let pline = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
        closed: false,
        bulges: vec![0.0, 1.0, 0.0],
        widths: Vec::new(),
    };
    let made = pline.with_implied_extension();
    assert_eq!(made.len(), 3);
    // The last segment, (10,0) to (10,10) with bulge 1, is a half circle.
    let Entity::Circle { center, radius } = made[2] else {
        panic!("{:?}", made[2])
    };
    assert!((center.x - 10.0).abs() < 1e-9 && (center.y - 5.0).abs() < 1e-9);
    assert!((radius - 5.0).abs() < 1e-9);
    assert!(matches!(made[1], Entity::Ray { .. }));
}

#[test]
fn closed_shapes_and_the_rest_stay_as_they_are() {
    let circle = Entity::Circle {
        center: p(0.0, 0.0),
        radius: 2.0,
    };
    let rectangle = Entity::Rectangle {
        min: p(0.0, 0.0),
        max: p(1.0, 1.0),
    };
    let closed = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
        closed: true,
        bulges: Vec::new(),
        widths: Vec::new(),
    };
    for e in [circle, rectangle, closed] {
        assert_eq!(e.with_implied_extension(), vec![e]);
    }
}

#[test]
fn implied_edges_flattens_a_list() {
    let edges = vec![
        line(p(0.0, 0.0), p(1.0, 0.0)),
        Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        },
    ];
    let made = implied_edges(&edges);
    assert_eq!(made.len(), 2);
    assert!(matches!(made[0], Entity::Xline { .. }));
    assert_eq!(made[1], edges[1]);
}
