//! Saving and reading the rotation, flip, scale and polar parameters and
//! actions.

use opendraft_core::block::{
    ActionId, BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, DynamicParameter,
    DynamicParameterKind, DynamicValue, DynamicValueSet, ParameterId,
};
use opendraft_core::{Bounds, Document, Entity, Insert};

use super::tests::{bar, p, read, schema, write};

#[test]
fn a_rotation_parameter_and_its_action_and_value_survive_a_round_trip() {
    let mut s = DynamicBlock::new();
    s.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Angle1".into(),
        label: "Angle".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Rotation {
            base: p(4.0, 0.0),
            radius: 1.5,
            value_set: DynamicValueSet::List(vec![0.0, std::f64::consts::FRAC_PI_2]),
        },
    });
    s.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Rotate1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Rotate {
            objects: vec![BlockObjectId(9)],
        },
    });
    s.next_parameter_id = 2;
    s.next_action_id = 2;
    let mut doc = Document::default();
    let block = bar(&mut doc, Some(s));
    doc.push_block(block.clone());
    let turned = Insert::new(block, p(0.0, 0.0))
        .with_dynamic_value(
            ParameterId(1),
            DynamicValue::Angle(std::f64::consts::FRAC_PI_2),
        )
        .unwrap();
    doc.add(Entity::Insert(Box::new(turned)));
    let back = read(&write(&doc)).unwrap();
    assert_eq!(back.blocks(), doc.blocks());
    assert_eq!(back.objects(), doc.objects());
    assert_eq!(back.validate(), Ok(()));
}

#[test]
fn a_flip_parameter_and_action_and_a_flipped_value_survive_a_round_trip() {
    let mut s = DynamicBlock::new();
    s.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Flip1".into(),
        label: "Flip".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Flip {
            base: p(0.0, 0.0),
            end: p(0.0, 3.0),
        },
    });
    s.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Flip1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Flip {
            objects: vec![BlockObjectId(7)],
        },
    });
    s.next_parameter_id = 2;
    s.next_action_id = 2;
    let mut doc = Document::default();
    let block = bar(&mut doc, Some(s));
    doc.push_block(block.clone());
    let flipped = Insert::new(block, p(0.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Bool(true))
        .unwrap();
    doc.add(Entity::Insert(Box::new(flipped)));
    let back = read(&write(&doc)).unwrap();
    assert_eq!(back.blocks(), doc.blocks());
    assert_eq!(back.objects(), doc.objects());
    assert_eq!(back.validate(), Ok(()));
}

#[test]
fn a_scale_action_survives_a_round_trip() {
    let mut s = schema(DynamicValueSet::Any);
    s.actions.push(DynamicAction {
        id: ActionId(3),
        name: "Scale1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Scale {
            objects: vec![BlockObjectId(9)],
        },
    });
    s.next_action_id = 4;
    let mut doc = Document::default();
    let block = bar(&mut doc, Some(s));
    doc.push_block(block.clone());
    doc.add(Entity::Insert(Box::new(Insert::new(block, p(0.0, 0.0)))));
    let back = read(&write(&doc)).unwrap();
    assert_eq!(back.blocks(), doc.blocks());
    assert_eq!(back.validate(), Ok(()));
}

#[test]
fn a_polar_parameter_with_its_sets_and_a_value_survive_a_round_trip() {
    let mut s = DynamicBlock::new();
    s.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Distance1".into(),
        label: "Reach".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Polar {
            base: p(0.0, 0.0),
            end: p(4.0, 0.0),
            distance_set: DynamicValueSet::List(vec![2.0, 4.0, 6.0]),
            angle_set: DynamicValueSet::Range {
                min: 0.0,
                max: std::f64::consts::TAU,
                increment: Some(0.0872664625997165),
            },
            angle_label: "Swing".into(),
        },
    });
    s.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Stretch1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Stretch {
            objects: vec![BlockObjectId(9)],
            frame: Bounds::from_points(p(3.0, -1.0), p(5.0, 3.0)),
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    });
    s.next_parameter_id = 2;
    s.next_action_id = 2;
    let mut doc = Document::default();
    let block = bar(&mut doc, Some(s));
    doc.push_block(block.clone());
    let moved = Insert::new(block, p(0.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Point(p(0.0, 6.0)))
        .unwrap();
    doc.add(Entity::Insert(Box::new(moved)));
    let back = read(&write(&doc)).unwrap();
    assert_eq!(back.blocks(), doc.blocks());
    assert_eq!(back.objects(), doc.objects());
    assert_eq!(back.validate(), Ok(()));
}
