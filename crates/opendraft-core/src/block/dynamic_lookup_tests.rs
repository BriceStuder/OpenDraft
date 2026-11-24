//! A lookup parameter: picking a row sets its inputs, the row shown is the
//! one the inputs match, and the check refuses a table that cannot work.

use std::sync::Arc;

use super::tests::{numbered, p};
use super::*;
use crate::{Attributes, Entity, LayerId};

fn row(label: &str, values: &[f64]) -> LookupRow {
    LookupRow {
        label: label.into(),
        values: values.to_vec(),
    }
}

/// A bar 4 long with a post at its end, stretched by a distance whose value
/// set is 2 to 8 by 2, and a lookup of sizes over that distance.
fn sized() -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Distance1".into(),
        label: "Length".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Linear {
            start: p(0.0, 0.0),
            end: p(4.0, 0.0),
            value_set: DynamicValueSet::Range {
                min: 2.0,
                max: 8.0,
                increment: Some(2.0),
            },
        },
    });
    schema.parameters.push(DynamicParameter {
        id: ParameterId(2),
        name: "Lookup1".into(),
        label: "Size".into(),
        description: String::new(),
        show: true,
        grips: 0,
        kind: DynamicParameterKind::Lookup {
            position: p(0.0, 1.0),
            inputs: vec![ParameterId(1)],
            rows: vec![
                row("Small", &[2.0]),
                row("Medium", &[4.0]),
                row("Large", &[8.0]),
            ],
        },
    });
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Stretch1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Stretch {
            objects: vec![BlockObjectId(1)],
            frame: crate::Bounds::from_points(p(3.0, -1.0), p(5.0, 3.0)),
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    });
    schema.next_parameter_id = 3;
    schema.next_action_id = 2;
    let line = |a: Point, b: Point| {
        BlockEntity::new(
            BlockObjectId(0),
            Entity::Line { start: a, end: b },
            Attributes::on_layer(LayerId::DEFAULT),
        )
    };
    Arc::new(BlockDef {
        id: BlockId(4),
        name: "Sized".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: numbered(vec![
            line(p(0.0, 0.0), p(4.0, 0.0)),
            line(p(4.0, 0.0), p(4.0, 2.0)),
        ]),
        dynamic: Some(Box::new(schema)),
    })
}

fn shown(i: &Insert) -> String {
    let (names, at) = i.choice(ParameterId(2)).unwrap();
    names[at].clone()
}

#[test]
fn an_untouched_insert_shows_the_row_its_default_inputs_match() {
    assert_eq!(shown(&Insert::new(sized(), p(0.0, 0.0))), "Medium");
}

#[test]
fn picking_a_row_sets_the_inputs_and_the_picture_follows() {
    let insert = Insert::new(sized(), p(0.0, 0.0));
    let large = insert.with_choice(ParameterId(2), 2).unwrap();
    assert_eq!(large.dynamic_value(ParameterId(1)), Some(8.0));
    assert_eq!(shown(&large), "Large");
    assert!((large.bounds().max.x - 8.0).abs() < 1e-9);
    // Picking the row already shown changes nothing; the default is not stored.
    assert!(large.with_choice(ParameterId(2), 2).is_none());
    let back = large.with_choice(ParameterId(2), 1).unwrap();
    assert!(back.dynamic.is_none());
}

#[test]
fn inputs_that_match_no_row_show_custom_and_custom_sets_nothing() {
    let insert = Insert::new(sized(), p(0.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(6.0))
        .unwrap();
    let (names, at) = insert.choice(ParameterId(2)).unwrap();
    assert_eq!(names, ["Small", "Medium", "Large", "Custom"]);
    assert_eq!(at, 3);
    assert!(insert.with_choice(ParameterId(2), 3).is_none());
    assert!(insert.with_choice(ParameterId(2), 9).is_none());
}

#[test]
fn a_lookup_stores_nothing_in_an_insert() {
    let block = sized();
    let schema = block.dynamic.as_ref().unwrap();
    let mut state = DynamicInstanceState::default();
    state.set(ParameterId(2), DynamicValue::Text("Large".into()));
    assert!(schema.check_state(&state).is_err());
    assert!(Insert::new(block, p(0.0, 0.0))
        .with_dynamic_value(ParameterId(2), DynamicValue::Text("Large".into()))
        .is_none());
}

#[test]
fn the_check_refuses_a_table_that_cannot_work() {
    let block = sized();
    let all = block.entities.iter().map(|e| e.id).collect();
    let check = |edit: &dyn Fn(&mut Vec<ParameterId>, &mut Vec<LookupRow>)| {
        let mut schema = (**block.dynamic.as_ref().unwrap()).clone();
        if let DynamicParameterKind::Lookup { inputs, rows, .. } = &mut schema.parameters[1].kind {
            edit(inputs, rows);
        }
        schema.check(&all)
    };
    assert!(check(&|_, _| {}).is_ok());
    assert!(check(&|i, _| i.clear()).is_err());
    assert!(check(&|i, _| i.push(ParameterId(2))).is_err());
    assert!(check(&|i, _| i[0] = ParameterId(9)).is_err());
    assert!(check(&|_, r| r.clear()).is_err());
    assert!(check(&|_, r| r[1].label = "small".into()).is_err());
    assert!(check(&|_, r| r[1].label = "custom".into()).is_err());
    assert!(check(&|_, r| r[1].label = " ".into()).is_err());
    assert!(check(&|_, r| r[0].values.clear()).is_err());
    // 3 is not in the input's set (2, 4, 6, 8); two rows may not share values.
    assert!(check(&|_, r| r[0].values = vec![3.0]).is_err());
    assert!(check(&|_, r| r[0].values = vec![4.0]).is_err());
}
