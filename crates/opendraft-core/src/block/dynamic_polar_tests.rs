//! A polar parameter: a point at a distance and an angle from a base, the
//! value sets on each, the grip on the point, and the actions that follow it.

use std::collections::HashSet;
use std::f64::consts::{FRAC_PI_2, PI};
use std::sync::Arc;

use super::tests::{near, numbered, p};
use super::*;
use crate::{Attributes, Bounds, Entity, LayerId, Point};

fn object(entity: Entity) -> BlockEntity {
    BlockEntity::new(
        BlockObjectId(0),
        entity,
        Attributes::on_layer(LayerId::DEFAULT),
    )
}

fn polar(distances: DynamicValueSet, angles: DynamicValueSet) -> DynamicParameter {
    DynamicParameter {
        id: ParameterId(1),
        name: "Distance1".into(),
        label: "Distance".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Polar {
            base: p(0.0, 0.0),
            end: p(5.0, 0.0),
            distance_set: distances,
            angle_set: angles,
            angle_label: "Angle".into(),
        },
    }
}

fn action(id: u64, kind: DynamicActionKind) -> DynamicAction {
    DynamicAction {
        id: ActionId(id),
        name: format!("Action{id}"),
        parameter: ParameterId(1),
        kind,
    }
}

/// A line from the base to the end (object 0), and a circle at (2, 0)
/// (object 1), with the actions given.
fn arm(
    distances: DynamicValueSet,
    angles: DynamicValueSet,
    actions: Vec<DynamicActionKind>,
) -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(polar(distances, angles));
    for (i, kind) in actions.into_iter().enumerate() {
        schema.actions.push(action(i as u64 + 1, kind));
    }
    schema.next_parameter_id = 2;
    schema.next_action_id = schema.actions.len() as u64 + 1;
    Arc::new(BlockDef {
        id: BlockId(11),
        name: "Arm".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: numbered(vec![
            object(Entity::Line {
                start: p(0.0, 0.0),
                end: p(5.0, 0.0),
            }),
            object(Entity::Circle {
                center: p(2.0, 0.0),
                radius: 0.5,
            }),
        ]),
        dynamic: Some(Box::new(schema)),
    })
}

fn stretch() -> DynamicActionKind {
    DynamicActionKind::Stretch {
        objects: vec![BlockObjectId(0)],
        frame: Bounds::from_points(p(4.0, -1.0), p(6.0, 1.0)),
        multiplier: 1.0,
        angle_offset: 0.0,
    }
}

fn with_end(block: &Arc<BlockDef>, end: Point) -> Insert {
    Insert::new(block.clone(), p(0.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Point(end))
        .unwrap()
}

fn line_end(i: &Insert) -> Point {
    i.evaluated()
        .iter()
        .find_map(|e| match e.entity {
            Entity::Line { end, .. } => Some(end),
            _ => None,
        })
        .unwrap()
}

fn circle(i: &Insert) -> (Point, f64) {
    i.evaluated()
        .iter()
        .find_map(|e| match e.entity {
            Entity::Circle { center, radius } => Some((center, radius)),
            _ => None,
        })
        .unwrap()
}

fn any() -> DynamicValueSet {
    DynamicValueSet::Any
}

#[test]
fn a_stretch_moves_what_is_in_its_frame_by_the_ends_displacement() {
    let block = arm(any(), any(), vec![stretch()]);
    assert!(near(line_end(&with_end(&block, p(3.0, 4.0))), p(3.0, 4.0)));
}

#[test]
fn a_rotate_turns_objects_about_the_base_by_the_change_of_angle() {
    let block = arm(
        any(),
        any(),
        vec![DynamicActionKind::Rotate {
            objects: vec![BlockObjectId(1)],
        }],
    );
    let (c, _) = circle(&with_end(&block, p(0.0, 5.0)));
    assert!(near(c, p(0.0, 2.0)), "{c:?}");
}

#[test]
fn a_scale_enlarges_by_the_ratio_of_distances_about_the_base() {
    let block = arm(
        any(),
        any(),
        vec![DynamicActionKind::Scale {
            objects: vec![BlockObjectId(1)],
        }],
    );
    let (c, r) = circle(&with_end(&block, p(0.0, 10.0)));
    assert!(near(c, p(4.0, 0.0)) && (r - 1.0).abs() < 1e-9, "{c:?} {r}");
}

#[test]
fn a_move_follows_the_displacement_and_the_untouched_insert_borrows_the_definition() {
    let block = arm(
        any(),
        any(),
        vec![DynamicActionKind::Move {
            objects: vec![BlockObjectId(1)],
            multiplier: 2.0,
            angle_offset: 0.0,
        }],
    );
    let (c, _) = circle(&with_end(&block, p(5.0, 1.0)));
    assert!(near(c, p(2.0, 2.0)), "{c:?}");
    assert!(matches!(
        Insert::new(block, p(0.0, 0.0)).evaluated(),
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn the_distance_and_the_angle_are_each_brought_into_their_own_set() {
    let step = 5f64.to_radians();
    let parameter = polar(
        DynamicValueSet::List(vec![2.0, 5.0, 8.0]),
        DynamicValueSet::Range {
            min: 0.0,
            max: 2.0 * PI,
            increment: Some(step),
        },
    );
    // Distance 6.4 is nearest 5; 47 degrees is nearest 45.
    let want = Point::new(
        5.0 * 45f64.to_radians().cos(),
        5.0 * 45f64.to_radians().sin(),
    );
    let end = Point::new(
        6.4 * 47f64.to_radians().cos(),
        6.4 * 47f64.to_radians().sin(),
    );
    let fitted = parameter.fit_end(end).unwrap();
    assert!(near(fitted, want), "{fitted:?} vs {want:?}");
    assert_eq!(parameter.fit_end(Point::new(f64::NAN, 0.0)), None);
    // A list takes a zero distance to its nearest entry; with no set it is refused.
    assert_eq!(parameter.fit_end(p(0.0, 0.0)), Some(p(2.0, 0.0)));
    assert_eq!(
        polar(any(), any()).fit_end(p(0.0, 0.0)),
        None,
        "no distance"
    );
}

#[test]
fn the_grip_is_at_the_end_and_dragging_it_sets_the_point() {
    let block = arm(any(), any(), vec![stretch()]);
    let insert = Insert::new(block, p(10.0, 0.0));
    let grips = insert.dynamic_grips();
    assert_eq!(grips.len(), 1);
    assert!(near(grips[0].1, p(15.0, 0.0)), "{:?}", grips[0].1);
    let moved = insert
        .with_dynamic_grip_moved(grips[0].0, p(10.0, 7.0))
        .unwrap();
    let (distance, angle) = moved.dynamic_pair(ParameterId(1)).unwrap();
    assert!((distance - 7.0).abs() < 1e-9 && (angle - FRAC_PI_2).abs() < 1e-9);
    assert!(near(moved.dynamic_grips()[0].1, p(10.0, 7.0)));
    // Back where it was: no state.
    let back = moved
        .with_dynamic_grip_moved(grips[0].0, p(15.0, 0.0))
        .unwrap();
    assert!(back.dynamic.is_none());
}

#[test]
fn the_polar_row_values_come_from_the_end() {
    let block = arm(any(), any(), vec![stretch()]);
    let insert = with_end(&block, p(3.0, 4.0));
    let (distance, angle) = insert.dynamic_pair(ParameterId(1)).unwrap();
    assert!((distance - 5.0).abs() < 1e-9);
    assert!((angle - (4.0f64).atan2(3.0)).abs() < 1e-9);
    // A distance is measured along the line as drawn.
    assert!((insert.dynamic_scale(ParameterId(1)).unwrap() - 1.0).abs() < 1e-9);
}

#[test]
fn the_check_pairs_polar_with_move_stretch_scale_and_rotate_but_not_flip() {
    let ids: HashSet<BlockObjectId> = [BlockObjectId(0), BlockObjectId(1)].into_iter().collect();
    for kind in [
        stretch(),
        DynamicActionKind::Scale {
            objects: vec![BlockObjectId(1)],
        },
        DynamicActionKind::Rotate {
            objects: vec![BlockObjectId(1)],
        },
    ] {
        let block = arm(any(), any(), vec![kind]);
        assert_eq!(block.dynamic.as_deref().unwrap().check(&ids), Ok(()));
    }
    let flip = arm(
        any(),
        any(),
        vec![DynamicActionKind::Flip {
            objects: vec![BlockObjectId(1)],
        }],
    );
    assert!(flip.dynamic.as_deref().unwrap().check(&ids).is_err());
    // Both value sets must be usable.
    let bad = arm(any(), DynamicValueSet::List(vec![]), vec![stretch()]);
    assert!(bad.dynamic.as_deref().unwrap().check(&ids).is_err());
}

#[test]
fn a_state_must_be_a_point_the_sets_allow_and_editing_moves_the_base() {
    let block = arm(
        DynamicValueSet::List(vec![5.0, 8.0]),
        any(),
        vec![stretch()],
    );
    let schema = block.dynamic.as_deref().unwrap();
    let mut state = DynamicInstanceState::default();
    state.set(ParameterId(1), DynamicValue::Point(p(0.0, 8.0)));
    assert_eq!(schema.check_state(&state), Ok(()));
    state.set(ParameterId(1), DynamicValue::Point(p(0.0, 7.0)));
    assert!(schema.check_state(&state).is_err());
    let fitted = schema.fit_state(&state).unwrap();
    let DynamicValue::Point(end) = fitted.values[0].value else {
        panic!("a point");
    };
    assert!(near(end, p(0.0, 8.0)), "{end:?}");
    state.set(ParameterId(1), DynamicValue::Distance(7.0));
    assert!(schema.check_state(&state).is_err());
    let DynamicParameterKind::Polar { base, end, .. } =
        &schema.translated(1.0, 2.0).parameters[0].kind
    else {
        panic!("polar");
    };
    assert!(near(*base, p(1.0, 2.0)) && near(*end, p(6.0, 2.0)));
}
