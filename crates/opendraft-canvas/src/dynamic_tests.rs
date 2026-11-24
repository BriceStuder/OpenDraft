//! A stretched dynamic block reference is what picking and snapping see:
//! they work from the reference's picture, which follows its values.

use std::sync::Arc;

use opendraft_core::block::{
    ActionId, BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, DynamicParameter,
    DynamicParameterKind, DynamicValue, DynamicValueSet, ParameterId,
};
use opendraft_core::{
    Attributes, BlockDef, BlockEntity, BlockId, Bounds, Entity, Insert, LayerId, Point,
};

use crate::hit::distance_to_entity;
use crate::snap::{candidates, SnapKind};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A bar 4 long, stretched by its one parameter.
fn bar() -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(DynamicParameter {
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
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Stretch1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Stretch {
            objects: vec![BlockObjectId(0)],
            frame: Bounds::from_points(p(3.0, -1.0), p(5.0, 1.0)),
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    });
    schema.next_parameter_id = 2;
    schema.next_action_id = 2;
    Arc::new(BlockDef {
        id: BlockId(1),
        name: "Bar".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: vec![BlockEntity::new(
            BlockObjectId(0),
            Entity::Line {
                start: p(0.0, 0.0),
                end: p(4.0, 0.0),
            },
            Attributes::on_layer(LayerId::DEFAULT),
        )],
        dynamic: Some(Box::new(schema)),
    })
}

fn stretched() -> Entity {
    Entity::Insert(Box::new(
        Insert::new(bar(), p(10.0, 0.0))
            .with_dynamic_value(ParameterId(1), DynamicValue::Distance(9.0))
            .unwrap(),
    ))
}

#[test]
fn picking_reaches_the_stretched_end_and_not_where_it_was() {
    let e = stretched();
    assert!(
        distance_to_entity(&e, p(18.0, 0.0)) < 1e-9,
        "on the new end"
    );
    assert!(distance_to_entity(&e, p(18.0, 3.0)) > 2.9);
    let untouched = Entity::Insert(Box::new(Insert::new(bar(), p(10.0, 0.0))));
    assert!(
        distance_to_entity(&untouched, p(18.0, 0.0)) > 3.9,
        "off the old end"
    );
}

#[test]
fn snapping_finds_the_stretched_endpoint_and_middle() {
    let found = candidates(&stretched());
    let has = |kind: SnapKind, at: Point| {
        found.iter().any(|s| {
            s.kind == kind && (s.point.x - at.x).abs() < 1e-9 && (s.point.y - at.y).abs() < 1e-9
        })
    };
    assert!(has(SnapKind::Endpoint, p(19.0, 0.0)));
    assert!(has(SnapKind::Midpoint, p(14.5, 0.0)));
    assert!(has(SnapKind::Insertion, p(10.0, 0.0)));
    assert!(
        !has(SnapKind::Endpoint, p(14.0, 0.0)),
        "not the authored end"
    );
}
