//! A measurement, not a check: `cargo test -p opendraft-core --release
//! dynamic_bench -- --ignored --nocapture` prints how long a drawing of
//! 1,000 references to a 100-object dynamic block takes to evaluate and to
//! make static, with every reference in a state of its own (the worst case).

use std::sync::Arc;
use std::time::Instant;

use super::tests::p;
use super::*;
use crate::{Attributes, Bounds, Document, Entity, LayerId};

fn block() -> Arc<BlockDef> {
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
            end: p(100.0, 0.0),
            value_set: DynamicValueSet::Any,
        },
    });
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Stretch1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Stretch {
            objects: (0..100).map(BlockObjectId).collect(),
            frame: Bounds::from_points(p(50.0, -1.0), p(101.0, 11.0)),
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    });
    schema.next_parameter_id = 2;
    schema.next_action_id = 2;
    Arc::new(BlockDef {
        id: BlockId(1),
        name: "Comb".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: (0..100)
            .map(|i| {
                BlockEntity::new(
                    BlockObjectId(i),
                    Entity::Line {
                        start: p(i as f64, 0.0),
                        end: p(i as f64, 10.0),
                    },
                    Attributes::on_layer(LayerId::DEFAULT),
                )
            })
            .collect(),
        dynamic: Some(Box::new(schema)),
    })
}

#[test]
#[ignore = "a measurement"]
fn measure_a_thousand_references_of_a_hundred_objects() {
    let block = block();
    let mut doc = Document::default();
    doc.push_block(block.clone());
    for n in 0..1000 {
        let insert = Insert::new(block.clone(), p(n as f64 * 200.0, 0.0))
            .with_dynamic_value(
                ParameterId(1),
                DynamicValue::Distance(100.0 + n as f64 * 0.01 + 0.5),
            )
            .unwrap();
        doc.add(Entity::Insert(Box::new(insert)));
    }
    let start = Instant::now();
    let bounds: Vec<_> = doc.objects().iter().map(|o| o.entity.bounds()).collect();
    println!(
        "evaluate and bound 1,000 references: {:?} ({})",
        start.elapsed(),
        bounds.len()
    );
    let start = Instant::now();
    let made = doc.materialized_static().expect("dynamic");
    println!(
        "make static: {:?} ({} blocks, {} objects in them)",
        start.elapsed(),
        made.document.blocks().len(),
        made.document
            .blocks()
            .iter()
            .map(|b| b.entities.len())
            .sum::<usize>()
    );
}
