//! An XY parameter (a point at a horizontal and a vertical distance from a
//! base) and a point parameter (a location): the value sets, the grip, and
//! the move and stretch actions that follow them.

use std::collections::HashSet;
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

fn xy(x_set: DynamicValueSet, y_set: DynamicValueSet) -> DynamicParameterKind {
    DynamicParameterKind::Xy {
        base: p(0.0, 0.0),
        end: p(2.0, 1.0),
        x_set,
        y_set,
        y_label: "Y".into(),
    }
}

/// A circle at (2, 1) (object 0) and a line from (0, 0) to (2, 1) (object 1),
/// a move of the circle and a stretch of the line's end, both following the
/// parameter.
fn block(kind: DynamicParameterKind) -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Distance1".into(),
        label: "X".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind,
    });
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Move1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Move {
            objects: vec![BlockObjectId(0)],
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    });
    schema.actions.push(DynamicAction {
        id: ActionId(2),
        name: "Stretch1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Stretch {
            objects: vec![BlockObjectId(1)],
            frame: Bounds::from_points(p(1.5, 0.5), p(2.5, 1.5)),
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    });
    schema.next_parameter_id = 2;
    schema.next_action_id = 3;
    Arc::new(BlockDef {
        id: BlockId(12),
        name: "Pointer".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: numbered(vec![
            object(Entity::Circle {
                center: p(2.0, 1.0),
                radius: 0.2,
            }),
            object(Entity::Line {
                start: p(0.0, 0.0),
                end: p(2.0, 1.0),
            }),
        ]),
        dynamic: Some(Box::new(schema)),
    })
}

fn at(block: &Arc<BlockDef>, end: Point) -> Insert {
    Insert::new(block.clone(), p(0.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Point(end))
        .unwrap()
}

fn centre_and_end(i: &Insert) -> (Point, Point) {
    let mut centre = p(0.0, 0.0);
    let mut end = p(0.0, 0.0);
    for e in i.evaluated().iter() {
        match e.entity {
            Entity::Circle { center, .. } => centre = center,
            Entity::Line { end: e, .. } => end = e,
            _ => {}
        }
    }
    (centre, end)
}

#[test]
fn an_xy_end_moves_what_follows_it_by_the_displacement() {
    let b = block(xy(DynamicValueSet::Any, DynamicValueSet::Any));
    let (centre, end) = centre_and_end(&at(&b, p(5.0, 4.0)));
    assert!(
        near(centre, p(5.0, 4.0)) && near(end, p(5.0, 4.0)),
        "{centre:?} {end:?}"
    );
}

#[test]
fn each_xy_distance_is_brought_into_its_own_set() {
    let parameter = DynamicParameter {
        id: ParameterId(1),
        name: "D".into(),
        label: "X".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: xy(
            DynamicValueSet::List(vec![1.0, 2.0, 4.0]),
            DynamicValueSet::Range {
                min: 0.0,
                max: 3.0,
                increment: Some(1.0),
            },
        ),
    };
    // x 3.1 is nearest 4; y 1.4 snaps to 1.
    assert!(near(parameter.fit_end(p(3.1, 1.4)).unwrap(), p(4.0, 1.0)));
    assert_eq!(parameter.parts_of(p(4.0, 1.0)), Some((4.0, 1.0)));
    assert_eq!(parameter.point_of(4.0, 1.0), Some(p(4.0, 1.0)));
    assert!(parameter.sets_valid());
}

#[test]
fn the_grip_is_on_the_point_and_dragging_it_sets_it() {
    let b = block(xy(DynamicValueSet::Any, DynamicValueSet::Any));
    let insert = Insert::new(b, p(10.0, 0.0));
    let grips = insert.dynamic_grips();
    assert!(near(grips[0].1, p(12.0, 1.0)), "{:?}", grips[0].1);
    let moved = insert
        .with_dynamic_grip_moved(grips[0].0, p(13.0, 5.0))
        .unwrap();
    assert_eq!(moved.dynamic_pair(ParameterId(1)), Some((3.0, 5.0)));
    assert!((moved.dynamic_scale(ParameterId(1)).unwrap() - 1.0).abs() < 1e-9);
}

#[test]
fn a_point_parameter_has_a_location_a_move_follows() {
    let b = block(DynamicParameterKind::Point {
        position: p(2.0, 1.0),
    });
    let (centre, _) = centre_and_end(&at(&b, p(3.0, 3.0)));
    assert!(near(centre, p(3.0, 3.0)), "{centre:?}");
    let insert = Insert::new(b.clone(), p(0.0, 0.0));
    assert_eq!(insert.dynamic_pair(ParameterId(1)), Some((2.0, 1.0)));
    assert!(insert.dynamic_grips().len() == 1);
    // At its drawn place there is no state.
    assert!(at(&b, p(2.0, 1.0)).dynamic.is_none());
}

#[test]
fn xy_and_point_take_move_and_stretch_but_not_scale_rotate_or_flip() {
    let ids: HashSet<BlockObjectId> = [BlockObjectId(0), BlockObjectId(1)].into_iter().collect();
    for kind in [
        xy(DynamicValueSet::Any, DynamicValueSet::Any),
        DynamicParameterKind::Point {
            position: p(2.0, 1.0),
        },
    ] {
        let b = block(kind);
        assert_eq!(b.dynamic.as_deref().unwrap().check(&ids), Ok(()));
        for action in [
            DynamicActionKind::Scale {
                objects: vec![BlockObjectId(0)],
            },
            DynamicActionKind::Rotate {
                objects: vec![BlockObjectId(0)],
            },
            DynamicActionKind::Flip {
                objects: vec![BlockObjectId(0)],
            },
        ] {
            let mut schema = b.dynamic.as_deref().unwrap().clone();
            schema.actions[0].kind = action;
            assert!(schema.check(&ids).is_err());
        }
    }
    let bad = block(xy(DynamicValueSet::List(vec![]), DynamicValueSet::Any));
    assert!(bad.dynamic.as_deref().unwrap().check(&ids).is_err());
}

#[test]
fn editing_the_block_moves_the_base_and_a_state_must_fit() {
    let b = block(xy(
        DynamicValueSet::List(vec![1.0, 3.0]),
        DynamicValueSet::Any,
    ));
    let schema = b.dynamic.as_deref().unwrap();
    let DynamicParameterKind::Xy { base, end, .. } =
        &schema.translated(1.0, 1.0).parameters[0].kind
    else {
        panic!("xy");
    };
    assert!(near(*base, p(1.0, 1.0)) && near(*end, p(3.0, 2.0)));
    let mut state = DynamicInstanceState::default();
    state.set(ParameterId(1), DynamicValue::Point(p(2.0, 5.0)));
    assert!(
        schema.check_state(&state).is_err(),
        "x 2 is not in the list"
    );
    let fitted = schema.fit_state(&state).unwrap();
    let DynamicValue::Point(end) = fitted.values[0].value else {
        panic!("a point");
    };
    assert!(near(end, p(1.0, 5.0)) || near(end, p(3.0, 5.0)), "{end:?}");
}
