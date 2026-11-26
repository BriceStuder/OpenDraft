//! OVERKILL: a loose line or arc lying on a polyline segment.

use super::*;
use crate::Point;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn item(entity: Entity) -> Candidate {
    Candidate {
        entity,
        group: 0,
        protected: false,
    }
}

fn shape() -> Entity {
    Entity::Polyline {
        points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

fn line(a: Point, b: Point) -> Entity {
    Entity::Line { start: a, end: b }
}

#[test]
fn a_line_lying_on_a_polyline_segment_is_removed_whichever_comes_first() {
    let loose = line(p(2.0, 0.0), p(7.0, 0.0));
    let out = overkill(&[item(shape()), item(loose.clone())], &Settings::default()).unwrap();
    assert_eq!(out.removed, [1]);
    let out = overkill(&[item(loose), item(shape())], &Settings::default()).unwrap();
    assert_eq!(out.removed, [0]);
}

#[test]
fn a_line_that_leaves_the_segment_or_a_protected_one_stays() {
    let past = line(p(2.0, 0.0), p(12.0, 0.0));
    let out = overkill(&[item(shape()), item(past)], &Settings::default()).unwrap();
    assert!(out.removed.is_empty());
    let beside = line(p(2.0, 1.0), p(7.0, 1.0));
    let out = overkill(&[item(shape()), item(beside)], &Settings::default()).unwrap();
    assert!(out.removed.is_empty());
    let on = line(p(2.0, 0.0), p(7.0, 0.0));
    let mut kept = item(on);
    kept.protected = true;
    let out = overkill(&[item(shape()), kept], &Settings::default()).unwrap();
    assert!(out.removed.is_empty());
}

#[test]
fn an_arc_on_an_arc_segment_of_a_polyline_is_removed() {
    // A half circle from (0, 0) to (4, 0), then a straight leg.
    let polyline = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 5.0)],
        closed: false,
        bulges: vec![1.0, 0.0, 0.0],
        widths: Vec::new(),
    };
    // The quarter of that circle (centre (2, 0), radius 2) lying under it.
    let arc = Entity::Arc {
        center: p(2.0, 0.0),
        radius: 2.0,
        start: std::f64::consts::PI,
        end: 1.5 * std::f64::consts::PI,
    };
    let out = overkill(&[item(polyline), item(arc)], &Settings::default()).unwrap();
    assert_eq!(out.removed, [1]);
}
