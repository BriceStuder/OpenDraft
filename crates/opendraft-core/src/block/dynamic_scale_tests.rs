//! The scale action: it enlarges its objects about the start of its linear
//! parameter by the value over the value as drawn.

use std::collections::HashSet;
use std::sync::Arc;

use super::tests::{near, numbered, p};
use super::*;
use crate::{Attributes, Entity, LayerId};

fn object(entity: Entity) -> BlockEntity {
    BlockEntity::new(
        BlockObjectId(0),
        entity,
        Attributes::on_layer(LayerId::DEFAULT),
    )
}

/// A fixed line along the parameter (0, 0) to (4, 0), and a circle at (2, 1)
/// that the scale action enlarges.
fn gauge(set: DynamicValueSet) -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Distance1".into(),
        label: "Size".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Linear {
            start: p(0.0, 0.0),
            end: p(4.0, 0.0),
            value_set: set,
        },
    });
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Scale1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Scale {
            objects: vec![BlockObjectId(1)],
        },
    });
    schema.next_parameter_id = 2;
    schema.next_action_id = 2;
    Arc::new(BlockDef {
        id: BlockId(10),
        name: "Gauge".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: numbered(vec![
            object(Entity::Line {
                start: p(0.0, 0.0),
                end: p(4.0, 0.0),
            }),
            object(Entity::Circle {
                center: p(2.0, 1.0),
                radius: 0.5,
            }),
        ]),
        dynamic: Some(Box::new(schema)),
    })
}

fn sized(block: &Arc<BlockDef>, v: f64) -> Insert {
    Insert::new(block.clone(), p(0.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(v))
        .unwrap()
}

fn the_circle(i: &Insert) -> (Point, f64) {
    i.evaluated()
        .iter()
        .find_map(|e| match e.entity {
            Entity::Circle { center, radius } => Some((center, radius)),
            _ => None,
        })
        .unwrap()
}

use crate::Point;

#[test]
fn the_value_over_its_default_is_the_factor_about_the_start() {
    let block = gauge(DynamicValueSet::Any);
    let (c, r) = the_circle(&sized(&block, 8.0));
    assert!(near(c, p(4.0, 2.0)) && (r - 1.0).abs() < 1e-9, "{c:?} {r}");
    let (c, r) = the_circle(&sized(&block, 1.0));
    assert!(
        near(c, p(0.5, 0.25)) && (r - 0.125).abs() < 1e-9,
        "{c:?} {r}"
    );
    // The line is not in the action and stays.
    let line = sized(&block, 8.0)
        .evaluated()
        .iter()
        .find_map(|e| match e.entity {
            Entity::Line { end, .. } => Some(end),
            _ => None,
        })
        .unwrap();
    assert!(near(line, p(4.0, 0.0)));
}

#[test]
fn a_scale_follows_a_linear_parameter_only_and_is_one_with_the_default() {
    let block = gauge(DynamicValueSet::Any);
    let schema = block.dynamic.as_deref().unwrap();
    let ids: HashSet<BlockObjectId> = [BlockObjectId(0), BlockObjectId(1)].into_iter().collect();
    assert_eq!(schema.check(&ids), Ok(()));
    let mut on_angle = schema.clone();
    on_angle.parameters[0].kind = DynamicParameterKind::Rotation {
        base: p(0.0, 0.0),
        radius: 1.0,
        value_set: DynamicValueSet::Any,
    };
    assert!(on_angle.check(&ids).is_err());
    assert!(matches!(
        Insert::new(block.clone(), p(0.0, 0.0)).evaluated(),
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn scaling_works_with_a_value_list_and_a_pruned_schema() {
    let block = gauge(DynamicValueSet::List(vec![2.0, 4.0, 8.0]));
    let (_, r) = the_circle(&sized(&block, 2.0));
    assert!((r - 0.25).abs() < 1e-9);
    let schema = block.dynamic.as_deref().unwrap();
    let pruned = schema.pruned(|id| id != BlockObjectId(1));
    assert_eq!(pruned.removed, ["Scale1"]);
}
