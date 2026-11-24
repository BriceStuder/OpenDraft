//! A visibility parameter: states, what each hides, the default, hiding in
//! an insert and what the schema check refuses.

use std::collections::HashSet;
use std::sync::Arc;

use super::tests::{numbered, p};
use super::*;
use crate::{Attributes, Entity, LayerId};

fn object(entity: Entity) -> BlockEntity {
    BlockEntity::new(
        BlockObjectId(0),
        entity,
        Attributes::on_layer(LayerId::DEFAULT),
    )
}

fn state(name: &str, hidden: &[u64]) -> VisibilityState {
    VisibilityState {
        name: name.into(),
        hidden: hidden.iter().map(|i| BlockObjectId(*i)).collect(),
    }
}

/// Object 0 is a line, 1 a circle, 2 a point. "Plain" hides the circle,
/// "Round" hides the line, "Both" hides nothing.
fn table() -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Visibility1".into(),
        label: "Visibility".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Visibility {
            position: p(0.0, 0.0),
            states: vec![
                state("Plain", &[1]),
                state("Round", &[0]),
                state("Both", &[]),
            ],
        },
    });
    schema.next_parameter_id = 2;
    Arc::new(BlockDef {
        id: BlockId(5),
        name: "Table".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: numbered(vec![
            object(Entity::Line {
                start: p(0.0, 0.0),
                end: p(1.0, 0.0),
            }),
            object(Entity::Circle {
                center: p(3.0, 3.0),
                radius: 1.0,
            }),
            object(Entity::Point { at: p(9.0, 9.0) }),
        ]),
        dynamic: Some(Box::new(schema)),
    })
}

fn ids(i: &Insert) -> Vec<u64> {
    i.evaluated().iter().map(|e| e.id.0).collect()
}

fn objects(block: &BlockDef) -> HashSet<BlockObjectId> {
    block.entities.iter().map(|e| e.id).collect()
}

#[test]
fn an_untouched_insert_shows_the_first_state() {
    let insert = Insert::new(table(), p(0.0, 0.0));
    assert_eq!(ids(&insert), [0, 2]);
    assert_eq!(insert.visibility_state().map(|s| s.1), Some("Plain"));
}

#[test]
fn another_state_hides_its_own_objects() {
    let insert = Insert::new(table(), p(0.0, 0.0))
        .with_visibility_state("Round")
        .unwrap();
    assert_eq!(ids(&insert), [1, 2]);
    let both = insert.with_visibility_state("both").unwrap();
    assert_eq!(ids(&both), [0, 1, 2]);
    assert_eq!(both.visibility_state().map(|s| s.1), Some("Both"));
}

#[test]
fn the_first_state_is_the_default_and_is_not_stored() {
    let insert = Insert::new(table(), p(0.0, 0.0));
    let round = insert.with_visibility_state("Round").unwrap();
    assert!(round.dynamic.is_some());
    assert!(round
        .with_visibility_state("plain")
        .unwrap()
        .dynamic
        .is_none());
    assert!(insert.with_visibility_state("Nowhere").is_none());
}

#[test]
fn the_value_api_takes_a_state_name_as_text() {
    let insert = Insert::new(table(), p(0.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Text("Round".into()))
        .unwrap();
    assert_eq!(ids(&insert), [1, 2]);
    let schema = insert.block.dynamic.as_ref().unwrap();
    assert!(schema.check_state(insert.dynamic.as_ref().unwrap()).is_ok());
    let mut bad = DynamicInstanceState::default();
    bad.set(ParameterId(1), DynamicValue::Text("Nowhere".into()));
    assert!(schema.check_state(&bad).is_err());
    // Other kinds of value are not a visibility's.
    assert!(insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(1.0))
        .is_none());
}

#[test]
fn a_state_that_names_no_state_draws_the_authored_objects() {
    let mut insert = Insert::new(table(), p(0.0, 0.0));
    let mut state = DynamicInstanceState::default();
    state.set(ParameterId(1), DynamicValue::Text("Gone".into()));
    insert.dynamic = Some(Box::new(state));
    assert_eq!(ids(&insert), [0, 1, 2]);
}

#[test]
fn visibility_beside_an_action_hides_after_the_action() {
    let mut def = (*table()).clone();
    let all = objects(&def);
    let schema = def.dynamic.as_mut().unwrap();
    schema.parameters.push(DynamicParameter {
        id: ParameterId(2),
        name: "Distance1".into(),
        label: "Distance".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Linear {
            start: p(0.0, 0.0),
            end: p(2.0, 0.0),
            value_set: DynamicValueSet::Any,
        },
    });
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Move1".into(),
        parameter: ParameterId(2),
        kind: DynamicActionKind::Move {
            objects: vec![BlockObjectId(2)],
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    });
    schema.next_parameter_id = 3;
    schema.next_action_id = 2;
    assert!(schema.check(&all).is_ok());
    let insert = Insert::new(Arc::new(def), p(0.0, 0.0))
        .with_dynamic_value(ParameterId(2), DynamicValue::Distance(5.0))
        .unwrap();
    assert_eq!(ids(&insert), [0, 2]);
    let moved = insert.evaluated();
    assert!(matches!(moved[1].entity, Entity::Point { at } if (at.x - 12.0).abs() < 1e-9));
}

#[test]
fn the_check_refuses_a_visibility_that_cannot_work() {
    let block = table();
    let all = objects(&block);
    let check = |edit: &dyn Fn(&mut DynamicBlock)| {
        let mut schema = (**block.dynamic.as_ref().unwrap()).clone();
        edit(&mut schema);
        schema.check(&all)
    };
    let states = |s: &mut DynamicBlock, f: &dyn Fn(&mut Vec<VisibilityState>)| {
        if let DynamicParameterKind::Visibility { states, .. } = &mut s.parameters[0].kind {
            f(states);
        }
    };
    assert!(check(&|_| {}).is_ok());
    assert!(check(&|s| states(s, &|v| v.clear())).is_err());
    assert!(check(&|s| states(s, &|v| v[1].name = "plain".into())).is_err());
    assert!(check(&|s| states(s, &|v| v[1].name = "  ".into())).is_err());
    assert!(check(&|s| states(s, &|v| v[0].hidden = vec![BlockObjectId(7)])).is_err());
    assert!(check(&|s| states(s, &|v| v[0].hidden =
        vec![BlockObjectId(2), BlockObjectId(1)]))
    .is_err());
    // A second visibility parameter.
    assert!(check(&|s| {
        let mut again = s.parameters[0].clone();
        again.id = ParameterId(2);
        again.name = "Visibility2".into();
        s.parameters.push(again);
        s.next_parameter_id = 3;
    })
    .is_err());
    // No action follows a visibility.
    assert!(check(&|s| s.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Move1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Move {
            objects: vec![BlockObjectId(0)],
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    }))
    .is_err());
}

#[test]
fn deleting_an_object_takes_it_off_every_state() {
    let block = table();
    let pruned = block
        .dynamic
        .as_ref()
        .unwrap()
        .pruned(|o| o != BlockObjectId(1));
    let states = pruned.schema.parameters[0].visibility_states().unwrap();
    assert!(states[0].hidden.is_empty());
    assert_eq!(states[1].hidden, [BlockObjectId(0)]);
}

#[test]
fn moving_the_block_moves_the_grip_position() {
    let moved = table().dynamic.as_ref().unwrap().translated(2.0, 3.0);
    let DynamicParameterKind::Visibility { position, .. } = &moved.parameters[0].kind else {
        panic!()
    };
    assert!((position.x - 2.0).abs() < 1e-9 && (position.y - 3.0).abs() < 1e-9);
}

#[test]
fn a_stored_state_survives_a_schema_edit_only_if_it_still_exists() {
    let block = table();
    let round = Insert::new(block.clone(), p(0.0, 0.0))
        .with_visibility_state("Round")
        .unwrap();
    let schema = block.dynamic.as_ref().unwrap();
    assert!(schema.fit_state(round.dynamic.as_ref().unwrap()).is_some());
    let mut gone = DynamicInstanceState::default();
    gone.set(ParameterId(1), DynamicValue::Text("Gone".into()));
    assert!(schema.fit_state(&gone).is_none());
}
