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

#[test]
fn xy_and_point_parameters_survive_a_round_trip() {
    for kind in [
        DynamicParameterKind::Xy {
            base: p(0.0, 0.0),
            end: p(2.0, 1.0),
            x_set: DynamicValueSet::List(vec![1.0, 2.0, 4.0]),
            y_set: DynamicValueSet::Any,
            y_label: "Height".into(),
        },
        DynamicParameterKind::Point {
            position: p(2.0, 1.0),
        },
    ] {
        let mut s = DynamicBlock::new();
        s.parameters.push(DynamicParameter {
            id: ParameterId(1),
            name: "Distance1".into(),
            label: "Width".into(),
            description: String::new(),
            show: true,
            grips: 1,
            kind,
        });
        s.actions.push(DynamicAction {
            id: ActionId(1),
            name: "Move1".into(),
            parameter: ParameterId(1),
            kind: DynamicActionKind::Move {
                objects: vec![BlockObjectId(9)],
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
            .with_dynamic_value(ParameterId(1), DynamicValue::Point(p(4.0, 3.0)))
            .unwrap();
        doc.add(Entity::Insert(Box::new(moved)));
        let back = read(&write(&doc)).unwrap();
        assert_eq!(back.blocks(), doc.blocks());
        assert_eq!(back.objects(), doc.objects());
        assert_eq!(back.validate(), Ok(()));
    }
}

#[test]
fn a_visibility_parameter_its_states_and_a_chosen_state_survive_a_round_trip() {
    use opendraft_core::block::VisibilityState;
    let mut s = DynamicBlock::new();
    s.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Visibility1".into(),
        label: "Visibility".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Visibility {
            position: p(2.0, -1.0),
            states: vec![
                VisibilityState {
                    name: "Plain".into(),
                    hidden: vec![BlockObjectId(9)],
                },
                VisibilityState {
                    name: "Round".into(),
                    hidden: vec![BlockObjectId(7)],
                },
            ],
        },
    });
    s.next_parameter_id = 2;
    let mut doc = Document::default();
    let block = bar(&mut doc, Some(s));
    doc.push_block(block.clone());
    let chosen = Insert::new(block, p(0.0, 0.0))
        .with_visibility_state("Round")
        .unwrap();
    doc.add(Entity::Insert(Box::new(chosen)));
    let back = read(&write(&doc)).unwrap();
    assert_eq!(back.blocks(), doc.blocks());
    assert_eq!(back.objects(), doc.objects());
    assert_eq!(back.validate(), Ok(()));
}

#[test]
fn a_lookup_parameter_its_table_and_a_picked_row_survive_a_round_trip() {
    use opendraft_core::block::LookupRow;
    let mut s = DynamicBlock::new();
    s.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Distance1".into(),
        label: "Distance".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Linear {
            start: p(0.0, 0.0),
            end: p(4.0, 0.0),
            value_set: DynamicValueSet::Any,
        },
    });
    s.parameters.push(DynamicParameter {
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
                LookupRow {
                    label: "Short".into(),
                    values: vec![2.0],
                },
                LookupRow {
                    label: "Long".into(),
                    values: vec![6.0],
                },
            ],
        },
    });
    s.next_parameter_id = 3;
    let mut doc = Document::default();
    let block = bar(&mut doc, Some(s));
    doc.push_block(block.clone());
    let long = Insert::new(block, p(0.0, 0.0))
        .with_choice(ParameterId(2), 1)
        .unwrap();
    doc.add(Entity::Insert(Box::new(long)));
    let back = read(&write(&doc)).unwrap();
    assert_eq!(back.blocks(), doc.blocks());
    assert_eq!(back.objects(), doc.objects());
    assert_eq!(back.validate(), Ok(()));
}
