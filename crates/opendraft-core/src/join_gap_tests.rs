use std::f64::consts::{FRAC_PI_2, PI};

use super::*;
use crate::join;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn arc_between(start: f64, end: f64) -> Entity {
    Entity::Arc {
        center: p(0.0, 0.0),
        radius: 5.0,
        start,
        end,
    }
}

#[test]
fn arcs_of_one_circle_join_across_a_gap_counter_clockwise_from_the_first() {
    let items = [arc_between(0.0, FRAC_PI_2), arc_between(PI, PI + FRAC_PI_2)];
    let made = join(&items, 1e-6);
    assert_eq!(made.len(), 1);
    assert_eq!(made[0].used, vec![0, 1]);
    let Entity::Arc { start, end, .. } = made[0].entity else {
        panic!("{:?}", made[0].entity)
    };
    assert!(start.abs() < 1e-9);
    assert!((end - (PI + FRAC_PI_2)).abs() < 1e-9);
}

#[test]
fn the_source_decides_where_the_joined_arc_starts() {
    // The same two arcs the other way round: from the second's start, round
    // past the end of the first.
    let items = [arc_between(PI, PI + FRAC_PI_2), arc_between(0.0, FRAC_PI_2)];
    let made = join(&items, 1e-6);
    let Entity::Arc { start, end, .. } = made[0].entity else {
        panic!("{:?}", made[0].entity)
    };
    assert!((start - PI).abs() < 1e-9);
    assert!((end - FRAC_PI_2).abs() < 1e-9, "{end}");
}

#[test]
fn arcs_that_cover_the_circle_between_them_make_a_circle() {
    let items = [arc_between(0.0, PI + 0.5), arc_between(PI + 1.0, TAU - 0.1)];
    let made = join(&items, 1e-6);
    assert_eq!(made.len(), 1);
    assert!(matches!(made[0].entity, Entity::Arc { .. }));
    let items = [arc_between(0.0, PI), arc_between(PI, TAU - 1e-12)];
    assert!(matches!(
        join(&items, 1e-6)[0].entity,
        Entity::Circle { .. } | Entity::Arc { .. }
    ));
}

#[test]
fn arcs_that_meet_still_join_as_before_and_other_circles_do_not() {
    let meeting = [arc_between(FRAC_PI_2, PI), arc_between(0.0, FRAC_PI_2)];
    let made = join(&meeting, 1e-6);
    let Entity::Arc { start, end, .. } = made[0].entity else {
        panic!("{:?}", made[0].entity)
    };
    assert!(
        start.abs() < 1e-9 && (end - PI).abs() < 1e-9,
        "{start} {end}"
    );
    let other = Entity::Arc {
        center: p(1.0, 0.0),
        radius: 5.0,
        start: PI,
        end: PI + FRAC_PI_2,
    };
    assert!(join(&[arc_between(0.0, FRAC_PI_2), other], 1e-6).is_empty());
}

#[test]
fn a_fuzz_distance_joins_ends_that_miss_by_that_much_with_a_straight_link() {
    let a = Entity::Line {
        start: p(0.0, 0.0),
        end: p(10.0, 0.0),
    };
    let b = Entity::Line {
        start: p(10.0, 0.5),
        end: p(10.0, 10.0),
    };
    assert!(join(&[a.clone(), b.clone()], 1e-6).is_empty());
    let made = join(&[a, b], 1.0);
    assert_eq!(made.len(), 1);
    assert_eq!(
        made[0].entity,
        Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 0.5), p(10.0, 10.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        }
    );
}

#[test]
fn close_makes_an_arc_a_circle_and_closes_an_open_polyline() {
    assert_eq!(
        closed(&arc_between(0.0, 1.0)),
        Some(Entity::Circle {
            center: p(0.0, 0.0),
            radius: 5.0
        })
    );
    let open = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)],
        closed: false,
        bulges: vec![0.0, 0.5, 0.7],
        widths: Vec::new(),
    };
    let Some(Entity::Polyline {
        closed: true,
        points,
        bulges,
        ..
    }) = closed(&open)
    else {
        panic!()
    };
    assert_eq!(points.len(), 3);
    assert_eq!(bulges, vec![0.0, 0.5, 0.0]);
    assert_eq!(
        closed(&Entity::Line {
            start: p(0.0, 0.0),
            end: p(1.0, 0.0)
        }),
        None
    );
}

#[test]
fn close_makes_an_elliptical_arc_its_whole_ellipse() {
    let arc = Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, 0.0, PI).unwrap();
    let Some(Entity::Ellipse(full)) = closed(&Entity::Ellipse(arc)) else {
        panic!()
    };
    assert!(full.is_full());
}
