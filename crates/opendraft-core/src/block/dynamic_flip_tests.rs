//! A flip parameter and the flip action: the mirror about its line, the
//! yes-or-no value, its grip and what the schema check refuses.

use std::collections::HashSet;
use std::sync::Arc;

use super::tests::{near, numbered, p};
use super::*;
use crate::{Attributes, Entity, LayerId, Point};

fn object(entity: Entity) -> BlockEntity {
    BlockEntity::new(
        BlockObjectId(0),
        entity,
        Attributes::on_layer(LayerId::DEFAULT),
    )
}

/// A line from (1, 1) to (3, 2) beside a fixed circle, with a vertical
/// mirror line through x = 0 that flips the line.
fn chair() -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Flip1".into(),
        label: "Flip state".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Flip {
            base: p(0.0, 0.0),
            end: p(0.0, 4.0),
        },
    });
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Flip1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Flip {
            objects: vec![BlockObjectId(0)],
        },
    });
    schema.next_parameter_id = 2;
    schema.next_action_id = 2;
    Arc::new(BlockDef {
        id: BlockId(9),
        name: "Chair".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: numbered(vec![
            object(Entity::Line {
                start: p(1.0, 1.0),
                end: p(3.0, 2.0),
            }),
            object(Entity::Circle {
                center: p(5.0, 5.0),
                radius: 0.5,
            }),
        ]),
        dynamic: Some(Box::new(schema)),
    })
}

fn flipped(block: &Arc<BlockDef>, yes: bool) -> Insert {
    Insert::new(block.clone(), p(0.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Bool(yes))
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

#[test]
fn a_flipped_insert_mirrors_the_objects_of_its_action_and_only_those() {
    let block = chair();
    let (start, end) = the_line(&flipped(&block, true));
    assert!(near(start, p(-1.0, 1.0)), "{start:?}");
    assert!(near(end, p(-3.0, 2.0)), "{end:?}");
    let circle = flipped(&block, true)
        .evaluated()
        .iter()
        .find_map(|e| match e.entity {
            Entity::Circle { center, .. } => Some(center),
            _ => None,
        })
        .unwrap();
    assert!(near(circle, p(5.0, 5.0)));
}

#[test]
fn not_flipped_is_the_default_and_is_not_stored() {
    let block = chair();
    let insert = flipped(&block, false);
    assert!(insert.dynamic.is_none());
    assert_eq!(insert.dynamic_value(ParameterId(1)), Some(0.0));
    assert!(flipped(&block, true).dynamic.is_some());
    // A distance is not a flip's kind of value.
    assert!(insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(1.0))
        .is_none());
}

#[test]
fn a_flip_is_a_yes_or_no_in_properties() {
    let block = chair();
    let insert = flipped(&block, true);
    let shown = insert.dynamic_value(ParameterId(1)).unwrap()
        * insert.dynamic_scale(ParameterId(1)).unwrap();
    assert_eq!(shown, 1.0);
    let parameter = &block.dynamic.as_deref().unwrap().parameters[0];
    assert!(parameter.is_flag());
    assert_eq!(parameter.normalize(0.2), Some(0.0));
    assert_eq!(parameter.normalize(0.7), Some(1.0));
    assert_eq!(parameter.normalize(f64::NAN), None);
}

#[test]
fn the_grip_is_at_the_middle_of_the_line_and_dragging_it_across_flips() {
    let block = chair();
    let insert = Insert::new(block, p(10.0, 0.0));
    let grips = insert.dynamic_grips();
    assert_eq!(grips.len(), 1);
    assert!(near(grips[0].1, p(10.0, 2.0)), "{:?}", grips[0].1);
    // Left of the line from (0, 0) up to (0, 4) is flipped.
    let left = insert
        .with_dynamic_grip_moved(grips[0].0, p(8.0, 2.0))
        .unwrap();
    assert_eq!(left.dynamic_value(ParameterId(1)), Some(1.0));
    assert!(left
        .with_dynamic_grip_moved(grips[0].0, p(12.0, 2.0))
        .is_some_and(|back| back.dynamic.is_none()));
    // Dropping it on the side it is already on changes nothing.
    assert!(insert
        .with_dynamic_grip_moved(grips[0].0, p(12.0, 2.0))
        .is_none());
}

#[test]
fn the_check_pairs_a_flip_action_with_a_flip_parameter_only() {
    let block = chair();
    let schema = block.dynamic.as_deref().unwrap();
    let ids: HashSet<BlockObjectId> = [BlockObjectId(0), BlockObjectId(1)].into_iter().collect();
    assert_eq!(schema.check(&ids), Ok(()));
    let mut on_distance = schema.clone();
    on_distance.parameters[0].kind = DynamicParameterKind::Linear {
        start: p(0.0, 0.0),
        end: p(1.0, 0.0),
        value_set: DynamicValueSet::Any,
    };
    assert!(on_distance.check(&ids).is_err());
    let mut no_line = schema.clone();
    no_line.parameters[0].kind = DynamicParameterKind::Flip {
        base: p(1.0, 1.0),
        end: p(1.0, 1.0),
    };
    assert!(no_line.check(&ids).is_err());
    let mut rotate = schema.clone();
    rotate.actions[0].kind = DynamicActionKind::Rotate {
        objects: vec![BlockObjectId(0)],
    };
    assert!(rotate.check(&ids).is_err());
}

#[test]
fn editing_the_block_moves_the_line_and_a_state_must_be_a_bool() {
    let block = chair();
    let schema = block.dynamic.as_deref().unwrap();
    let DynamicParameterKind::Flip { base, end } = &schema.translated(2.0, 1.0).parameters[0].kind
    else {
        panic!("a flip");
    };
    assert!(near(*base, p(2.0, 1.0)) && near(*end, p(2.0, 5.0)));
    let mut state = DynamicInstanceState::default();
    state.set(ParameterId(1), DynamicValue::Bool(true));
    assert_eq!(schema.check_state(&state), Ok(()));
    assert_eq!(schema.fit_state(&state), Some(state.clone()));
    state.set(ParameterId(1), DynamicValue::Angle(1.0));
    assert!(schema.check_state(&state).is_err());
}
