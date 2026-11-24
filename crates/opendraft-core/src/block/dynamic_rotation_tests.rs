//! A rotation parameter and the rotate action: the turn it makes about its
//! base point, the angle value sets (turns count), its grip and what the
//! schema check refuses.

use std::collections::HashSet;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

use super::tests::{near, numbered, p};
use super::*;
use crate::{Attributes, Entity, GripKind, LayerId, Point};

fn object(entity: Entity) -> BlockEntity {
    BlockEntity::new(
        BlockObjectId(0),
        entity,
        Attributes::on_layer(LayerId::DEFAULT),
    )
}

fn rotation(set: DynamicValueSet) -> DynamicParameter {
    DynamicParameter {
        id: ParameterId(1),
        name: "Angle1".into(),
        label: "Angle".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Rotation {
            base: p(1.0, 0.0),
            radius: 2.0,
            value_set: set,
        },
    }
}

/// A line from (1, 0) to (3, 0) and a fixed circle, with a rotation about
/// (1, 0) that turns the line.
fn door(set: DynamicValueSet) -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(rotation(set));
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Rotate1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Rotate {
            objects: vec![BlockObjectId(0)],
        },
    });
    schema.next_parameter_id = 2;
    schema.next_action_id = 2;
    Arc::new(BlockDef {
        id: BlockId(8),
        name: "Door".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: numbered(vec![
            object(Entity::Line {
                start: p(1.0, 0.0),
                end: p(3.0, 0.0),
            }),
            object(Entity::Circle {
                center: p(0.0, 0.0),
                radius: 0.5,
            }),
        ]),
        dynamic: Some(Box::new(schema)),
    })
}

fn turned(block: &Arc<BlockDef>, angle: f64) -> Insert {
    Insert::new(block.clone(), p(0.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Angle(angle))
        .unwrap()
}

fn the_line(i: &Insert) -> (Point, Point) {
    i.evaluated()
        .iter()
        .find_map(|e| match e.entity {
            Entity::Line { start, end } => Some((start, end)),
            _ => None,
        })
        .unwrap()
}

fn ids() -> HashSet<BlockObjectId> {
    [BlockObjectId(0), BlockObjectId(1)].into_iter().collect()
}

#[test]
fn the_rotate_action_turns_its_objects_about_the_base_and_leaves_the_rest() {
    let block = door(DynamicValueSet::Any);
    let insert = turned(&block, FRAC_PI_2);
    let (start, end) = the_line(&insert);
    assert!(near(start, p(1.0, 0.0)), "{start:?}");
    assert!(near(end, p(1.0, 2.0)), "{end:?}");
    let circle = insert
        .evaluated()
        .iter()
        .find_map(|e| match e.entity {
            Entity::Circle { center, .. } => Some(center),
            _ => None,
        })
        .unwrap();
    assert!(near(circle, p(0.0, 0.0)));
}

#[test]
fn an_untouched_insert_has_no_turn_and_borrows_the_definition() {
    let block = door(DynamicValueSet::Any);
    let insert = Insert::new(block.clone(), p(0.0, 0.0));
    assert!(matches!(insert.evaluated(), std::borrow::Cow::Borrowed(_)));
    assert_eq!(insert.dynamic_value(ParameterId(1)), Some(0.0));
    // A turn of nothing is not stored.
    assert!(turned(&block, 0.0).dynamic.is_none());
}

#[test]
fn a_value_of_the_wrong_kind_is_refused() {
    let block = door(DynamicValueSet::Any);
    let insert = Insert::new(block, p(0.0, 0.0));
    assert!(insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(1.0))
        .is_none());
}

#[test]
fn with_no_set_an_angle_is_brought_into_one_turn() {
    let parameter = rotation(DynamicValueSet::Any);
    assert!((parameter.normalize(TAU + 0.5).unwrap() - 0.5).abs() < 1e-12);
    assert!((parameter.normalize(-FRAC_PI_2).unwrap() - 3.0 * FRAC_PI_2).abs() < 1e-12);
    assert_eq!(parameter.normalize(f64::NAN), None);
}

#[test]
fn in_a_list_the_nearest_angle_is_the_one_a_shorter_turn_away() {
    let list = DynamicValueSet::List(vec![0.0, FRAC_PI_2, PI, 3.0 * FRAC_PI_2]);
    let parameter = rotation(list);
    // 359 degrees is next to 0, not next to 270.
    let value = parameter.normalize(359f64.to_radians()).unwrap();
    assert_eq!(value, 0.0);
    assert_eq!(parameter.normalize(100f64.to_radians()), Some(FRAC_PI_2));
}

#[test]
fn a_range_of_angles_snaps_to_its_increment() {
    let parameter = rotation(DynamicValueSet::Range {
        min: 0.0,
        max: PI,
        increment: Some(FRAC_PI_2),
    });
    assert_eq!(parameter.normalize(1.0), Some(FRAC_PI_2));
    assert_eq!(parameter.normalize(4.0), Some(PI));
}

#[test]
fn the_grip_follows_the_turn_and_a_drag_sets_the_angle() {
    let block = door(DynamicValueSet::Any);
    let insert = Insert::new(block.clone(), p(10.0, 0.0));
    let grips = insert.dynamic_grips();
    assert_eq!(grips.len(), 1);
    // Base (1, 0) plus the radius 2 along +x, then the insert's offset.
    assert!(near(grips[0].1, p(13.0, 0.0)), "{:?}", grips[0].1);
    let moved = insert
        .with_dynamic_grip_moved(grips[0].0, p(11.0, 5.0))
        .unwrap();
    let angle = moved.dynamic_value(ParameterId(1)).unwrap();
    assert!((angle - FRAC_PI_2).abs() < 1e-9, "{angle}");
    let at = moved.dynamic_grips()[0].1;
    assert!(near(at, p(11.0, 2.0)), "{at:?}");
    assert!(matches!(
        Entity::Insert(Box::new(insert)).grips()[1].kind,
        GripKind::Dynamic(_)
    ));
}

#[test]
fn the_rotation_is_shown_in_degrees() {
    let block = door(DynamicValueSet::Any);
    let insert = turned(&block, FRAC_PI_2);
    let shown = insert.dynamic_value(ParameterId(1)).unwrap()
        * insert.dynamic_scale(ParameterId(1)).unwrap();
    assert!((shown - 90.0).abs() < 1e-9);
}

#[test]
fn a_schema_with_a_rotation_checks_and_the_wrong_pairs_are_refused() {
    let block = door(DynamicValueSet::Any);
    let schema = block.dynamic.as_deref().unwrap();
    assert_eq!(schema.check(&ids()), Ok(()));

    let mut no_radius = schema.clone();
    if let DynamicParameterKind::Rotation { radius, .. } = &mut no_radius.parameters[0].kind {
        *radius = 0.0;
    }
    assert!(no_radius.check(&ids()).is_err());

    // A move cannot follow an angle, and a rotate cannot follow a distance.
    let mut move_on_angle = schema.clone();
    move_on_angle.actions[0].kind = DynamicActionKind::Move {
        objects: vec![BlockObjectId(0)],
        multiplier: 1.0,
        angle_offset: 0.0,
    };
    assert!(move_on_angle.check(&ids()).is_err());
    let mut rotate_on_distance = schema.clone();
    rotate_on_distance.parameters[0].kind = DynamicParameterKind::Linear {
        start: p(0.0, 0.0),
        end: p(1.0, 0.0),
        value_set: DynamicValueSet::Any,
    };
    assert!(rotate_on_distance.check(&ids()).is_err());
}

#[test]
fn a_state_is_checked_against_the_kind_of_its_parameter() {
    let block = door(DynamicValueSet::List(vec![0.0, PI]));
    let schema = block.dynamic.as_deref().unwrap();
    let mut state = DynamicInstanceState::default();
    state.set(ParameterId(1), DynamicValue::Angle(PI));
    assert_eq!(schema.check_state(&state), Ok(()));
    state.set(ParameterId(1), DynamicValue::Distance(PI));
    assert!(schema.check_state(&state).is_err());
    state.set(ParameterId(1), DynamicValue::Angle(1.0));
    assert!(schema.check_state(&state).is_err());
}

#[test]
fn editing_the_block_moves_the_base_and_drops_deleted_objects() {
    let block = door(DynamicValueSet::Any);
    let schema = block.dynamic.as_deref().unwrap();
    let shifted = schema.translated(5.0, 1.0);
    let DynamicParameterKind::Rotation { base, .. } = &shifted.parameters[0].kind else {
        panic!("a rotation");
    };
    assert!(near(*base, p(6.0, 1.0)));
    let pruned = schema.pruned(|id| id != BlockObjectId(0));
    assert!(pruned.schema.actions.is_empty());
    assert_eq!(pruned.removed, ["Rotate1"]);
}

#[test]
fn fitting_a_state_keeps_an_angle_and_drops_a_distance() {
    let block = door(DynamicValueSet::Any);
    let schema = block.dynamic.as_deref().unwrap();
    let mut state = DynamicInstanceState::default();
    state.set(ParameterId(1), DynamicValue::Angle(TAU + 1.0));
    let fitted = schema.fit_state(&state).unwrap();
    assert!(matches!(
        fitted.values[0].value,
        DynamicValue::Angle(a) if (a - 1.0).abs() < 1e-12
    ));
    state.set(ParameterId(1), DynamicValue::Distance(1.0));
    assert!(schema.fit_state(&state).is_none());
}
