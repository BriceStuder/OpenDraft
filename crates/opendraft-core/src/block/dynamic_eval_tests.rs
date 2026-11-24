//! Evaluating a dynamic block: the value sets, the Move and Stretch actions,
//! the grips and their drags, and what a state that cannot be followed does.

use std::collections::HashSet;
use std::f64::consts::FRAC_PI_2;
use std::sync::Arc;

use super::tests::{near, numbered, p};
use super::*;
use crate::{Attributes, Bounds, Entity, GripKind, LayerId};

fn on_zero(entity: Entity) -> BlockEntity {
    BlockEntity::new(
        BlockObjectId(0),
        entity,
        Attributes::on_layer(LayerId::DEFAULT),
    )
}

fn parameter(set: DynamicValueSet) -> DynamicParameter {
    DynamicParameter {
        id: ParameterId(1),
        name: "Distance1".into(),
        label: "Distance".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Linear {
            start: p(0.0, 0.0),
            end: p(4.0, 0.0),
            value_set: set,
        },
    }
}

fn action(kind: DynamicActionKind) -> DynamicAction {
    DynamicAction {
        id: ActionId(1),
        name: "Action1".into(),
        parameter: ParameterId(1),
        kind,
    }
}

fn stretch(multiplier: f64, angle_offset: f64) -> DynamicActionKind {
    DynamicActionKind::Stretch {
        objects: vec![BlockObjectId(0), BlockObjectId(1)],
        frame: Bounds::from_points(p(3.0, -1.0), p(5.0, 3.0)),
        multiplier,
        angle_offset,
    }
}

/// A bar 4 long with a post at its right end and a fixed circle at the left.
fn bar(kind: DynamicActionKind, set: DynamicValueSet) -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(parameter(set));
    schema.actions.push(action(kind));
    schema.next_parameter_id = 2;
    schema.next_action_id = 2;
    Arc::new(BlockDef {
        id: BlockId(7),
        name: "Bar".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: numbered(vec![
            on_zero(Entity::Line {
                start: p(0.0, 0.0),
                end: p(4.0, 0.0),
            }),
            on_zero(Entity::Line {
                start: p(4.0, 0.0),
                end: p(4.0, 2.0),
            }),
            on_zero(Entity::Circle {
                center: p(0.0, 1.0),
                radius: 0.2,
            }),
        ]),
        dynamic: Some(Box::new(schema)),
    })
}

fn with_distance(block: &Arc<BlockDef>, v: f64) -> Insert {
    Insert::new(block.clone(), p(0.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(v))
        .unwrap()
}

fn lines(i: &Insert) -> Vec<(Point, Point)> {
    i.evaluated()
        .iter()
        .filter_map(|e| match e.entity {
            Entity::Line { start, end } => Some((start, end)),
            _ => None,
        })
        .collect()
}

use crate::Point;

#[test]
fn an_untouched_insert_borrows_the_definition() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let insert = Insert::new(block.clone(), p(0.0, 0.0));
    assert!(matches!(insert.evaluated(), std::borrow::Cow::Borrowed(_)));
    assert_eq!(insert.dynamic_value(ParameterId(1)), Some(4.0));
}

#[test]
fn a_stretch_moves_what_is_in_its_frame_and_keeps_the_rest() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let got = lines(&with_distance(&block, 6.0));
    assert!(near(got[0].0, p(0.0, 0.0)) && near(got[0].1, p(6.0, 0.0)));
    assert!(near(got[1].0, p(6.0, 0.0)) && near(got[1].1, p(6.0, 2.0)));
    let shorter = lines(&with_distance(&block, 3.0));
    assert!(near(shorter[0].1, p(3.0, 0.0)));
}

#[test]
fn objects_outside_the_selection_never_move() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let circle = with_distance(&block, 9.0).evaluated()[2].entity.clone();
    assert_eq!(circle, block.entities[2].entity);
}

#[test]
fn a_move_translates_whole_objects_by_the_change_times_the_multiplier() {
    let kind = DynamicActionKind::Move {
        objects: vec![BlockObjectId(1)],
        multiplier: 0.5,
        angle_offset: 0.0,
    };
    let block = bar(kind, DynamicValueSet::Any);
    let got = lines(&with_distance(&block, 6.0));
    assert!(near(got[0].1, p(4.0, 0.0)), "the bar is not selected");
    assert!(near(got[1].0, p(5.0, 0.0)) && near(got[1].1, p(5.0, 2.0)));
}

#[test]
fn the_angle_offset_turns_the_direction_of_the_change() {
    let kind = DynamicActionKind::Move {
        objects: vec![BlockObjectId(1)],
        multiplier: 1.0,
        angle_offset: FRAC_PI_2,
    };
    let block = bar(kind, DynamicValueSet::Any);
    let got = lines(&with_distance(&block, 5.0));
    assert!(near(got[1].0, p(4.0, 1.0)) && near(got[1].1, p(4.0, 3.0)));
}

#[test]
fn a_value_equal_to_the_default_is_not_stored() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let stretched = with_distance(&block, 6.0);
    let back = stretched
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(4.0))
        .unwrap();
    assert_eq!(back.dynamic, None);
    assert_eq!(back, Insert::new(block, p(0.0, 0.0)));
}

#[test]
fn a_value_edit_is_refused_for_an_unknown_parameter_a_wrong_type_or_a_value_not_finite() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let insert = Insert::new(block, p(0.0, 0.0));
    let d = DynamicValue::Distance(1.0);
    assert!(insert
        .with_dynamic_value(ParameterId(9), d.clone())
        .is_none());
    assert!(insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Angle(1.0))
        .is_none());
    assert!(insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(f64::NAN))
        .is_none());
    let plain = Insert::new(super::tests::square_block(), p(0.0, 0.0));
    assert!(plain.with_dynamic_value(ParameterId(1), d).is_none());
}

#[test]
fn a_state_that_cannot_be_followed_draws_the_authored_block() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let mut insert = Insert::new(block.clone(), p(0.0, 0.0));
    let mut state = DynamicInstanceState::default();
    state.set(ParameterId(1), DynamicValue::Text("x".into()));
    insert.dynamic = Some(Box::new(state));
    assert_eq!(
        evaluate(&block, insert.dynamic.as_deref()),
        Err(DynamicBlockError::BadValue(ParameterId(1)))
    );
    assert_eq!(&*insert.evaluated(), &block.entities[..]);
}

#[test]
fn an_action_that_cannot_move_its_object_is_an_error() {
    let kind = DynamicActionKind::Move {
        objects: vec![BlockObjectId(5)],
        multiplier: 1.0,
        angle_offset: 0.0,
    };
    let block = bar(kind, DynamicValueSet::Any);
    let insert = with_distance(&block, 6.0);
    assert_eq!(
        evaluate(&block, insert.dynamic.as_deref()),
        Err(DynamicBlockError::MissingObject(BlockObjectId(5)))
    );
}

#[test]
fn the_picture_and_bounds_follow_the_values() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let b = with_distance(&block, 6.0).bounds();
    assert!(near(b.max, p(6.0, 2.0)));
    let plain = Insert::new(block, p(0.0, 0.0)).bounds();
    assert!(near(plain.max, p(4.0, 2.0)));
}

#[test]
fn the_grip_sits_at_the_value_along_the_line_through_the_insert_transform() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let mut insert = with_distance(&block, 6.0);
    insert.insertion = p(10.0, 10.0);
    insert.scale_x = -2.0;
    insert.rotation = FRAC_PI_2;
    let (grip, at) = insert.dynamic_grips()[0];
    assert_eq!(grip.index, 0);
    // x = 6, mirrored and doubled: -12, then turned a quarter: (0, -12).
    assert!(near(at, p(10.0, -2.0)), "{at:?}");
    let entity = Entity::Insert(Box::new(insert));
    assert_eq!(entity.grips().len(), 2);
    assert_eq!(entity.grips()[1].kind, GripKind::Dynamic(grip));
}

#[test]
fn dragging_the_grip_sets_the_value_through_the_inverse_transform() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let mut insert = Insert::new(block, p(10.0, 10.0));
    insert.scale_x = -2.0;
    insert.rotation = FRAC_PI_2;
    let (grip, _) = insert.dynamic_grips()[0];
    let moved = insert.with_dynamic_grip_moved(grip, p(10.0, -2.0)).unwrap();
    assert!((moved.dynamic_value(ParameterId(1)).unwrap() - 6.0).abs() < 1e-9);
    // The same through the entity's own grip drag, as the grip editor does.
    let entity = Entity::Insert(Box::new(insert));
    let Some(Entity::Insert(done)) = entity.with_grip_moved(1, p(10.0, -2.0)) else {
        panic!("the drag was refused");
    };
    assert_eq!(*done, moved);
}

#[test]
fn a_drag_is_brought_into_the_value_set() {
    let block = bar(
        stretch(1.0, 0.0),
        DynamicValueSet::List(vec![2.0, 4.0, 8.0]),
    );
    let insert = Insert::new(block, p(0.0, 0.0));
    let (grip, _) = insert.dynamic_grips()[0];
    let moved = insert.with_dynamic_grip_moved(grip, p(7.1, 5.0)).unwrap();
    assert_eq!(moved.dynamic_value(ParameterId(1)), Some(8.0));
}

#[test]
fn a_drag_that_changes_nothing_or_is_not_finite_is_refused() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let insert = Insert::new(block, p(0.0, 0.0));
    let (grip, _) = insert.dynamic_grips()[0];
    assert!(insert.with_dynamic_grip_moved(grip, p(4.0, 9.0)).is_none());
    assert!(insert
        .with_dynamic_grip_moved(grip, p(f64::NAN, 0.0))
        .is_none());
    let reserved = DynamicGrip { index: 1, ..grip };
    assert!(insert
        .with_dynamic_grip_moved(reserved, p(9.0, 0.0))
        .is_none());
}

#[test]
fn a_parameter_with_no_grips_shows_none() {
    let block = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let mut def = (*block).clone();
    if let Some(s) = def.dynamic.as_mut() {
        s.parameters[0].grips = 0;
    }
    assert!(Insert::new(Arc::new(def), p(0.0, 0.0))
        .dynamic_grips()
        .is_empty());
}

#[test]
fn the_schema_check_refuses_what_evaluation_could_not_follow() {
    let objects: HashSet<BlockObjectId> = (0..3).map(BlockObjectId).collect();
    let good = bar(stretch(1.0, 0.0), DynamicValueSet::Any);
    let schema = (**good.dynamic.as_ref().unwrap()).clone();
    assert_eq!(schema.check(&objects), Ok(()));
    let mut cases: Vec<(&str, DynamicBlock)> = Vec::new();
    let mut s = schema.clone();
    s.parameters[0].kind = DynamicParameterKind::Linear {
        start: p(1.0, 1.0),
        end: p(1.0, 1.0),
        value_set: DynamicValueSet::Any,
    };
    cases.push(("no length", s));
    let mut s = schema.clone();
    s.parameters[0].grips = 3;
    cases.push(("grips", s));
    let mut s = schema.clone();
    s.parameters[0].kind = DynamicParameterKind::Linear {
        start: p(0.0, 0.0),
        end: p(4.0, 0.0),
        value_set: DynamicValueSet::List(vec![]),
    };
    cases.push(("value set", s));
    for (name, multiplier, offset) in [
        ("zero multiplier", 0.0, 0.0),
        ("nan multiplier", f64::NAN, 0.0),
        ("nan angle", 1.0, f64::NAN),
    ] {
        let mut s = schema.clone();
        s.actions[0].kind = stretch(multiplier, offset);
        cases.push((name, s));
    }
    let mut s = schema.clone();
    s.actions[0].kind = DynamicActionKind::Stretch {
        objects: vec![BlockObjectId(0)],
        frame: Bounds::from_points(p(1.0, 1.0), p(1.0, 5.0)),
        multiplier: 1.0,
        angle_offset: 0.0,
    };
    cases.push(("flat frame", s));
    for (name, s) in cases {
        assert!(s.check(&objects).is_err(), "{name} was accepted");
    }
}

#[test]
fn any_finite_transform_and_value_evaluates_to_finite_geometry() {
    let block = bar(stretch(1.5, 0.3), DynamicValueSet::Any);
    let scales = [1.0, -1.0, 0.001, 250.0];
    let values = [-1e6, -4.0, 0.0, 4.0, 1e-9, 77.7, 1e6];
    for &sx in &scales {
        for &sy in &scales {
            for &value in &values {
                let mut insert = with_distance(&block, value);
                insert.scale_x = sx;
                insert.scale_y = sy;
                insert.rotation = value;
                for e in insert.picture() {
                    let b = e.bounds();
                    assert!(
                        [b.min.x, b.min.y, b.max.x, b.max.y]
                            .iter()
                            .all(|v| v.is_finite()),
                        "{sx} {sy} {value}"
                    );
                }
                let grips = Entity::Insert(Box::new(insert)).grips();
                assert!(grips
                    .iter()
                    .all(|g| g.at.x.is_finite() && g.at.y.is_finite()));
            }
        }
    }
}

#[test]
fn validate_refuses_a_value_the_schema_does_not_allow() {
    use crate::{Document, DocumentError};
    let block = bar(
        stretch(1.0, 0.0),
        DynamicValueSet::List(vec![2.0, 4.0, 8.0]),
    );
    let check = |v: f64| {
        let mut doc = Document::default();
        doc.push_block(block.clone());
        let insert = Insert::new(block.clone(), p(0.0, 0.0));
        let mut state = DynamicInstanceState::default();
        state.set(ParameterId(1), DynamicValue::Distance(v));
        let insert = Insert {
            dynamic: Some(Box::new(state)),
            ..insert
        };
        doc.add(Entity::Insert(Box::new(insert)));
        doc.validate()
    };
    assert_eq!(check(8.0), Ok(()));
    assert!(matches!(
        check(5.0),
        Err(DocumentError::BadDynamicState(..))
    ));
}
