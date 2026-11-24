//! How much a block expands to, and the cap that stops a block graph that
//! fans out.

use std::sync::Arc;

use super::tests::{p, square_block};
use super::*;

/// `levels` blocks above `leaf`, each inserting the one below twice.
fn doubling_chain(leaf: Arc<BlockDef>, levels: u64) -> Arc<BlockDef> {
    let mut below = leaf;
    for id in 1..=levels {
        let insert = |n| BlockEntity {
            id: crate::BlockObjectId(n),
            entity: Entity::Insert(Box::new(Insert::new(below.clone(), p(0.0, 0.0)))),
            attrs: Attributes::on_layer(LayerId::DEFAULT),
        };
        below = Arc::new(BlockDef {
            unit: Default::default(),
            id: BlockId(id + 10),
            name: format!("level{id}"),
            base: p(0.0, 0.0),
            description: String::new(),
            entities: vec![insert(0), insert(1)],
            dynamic: None,
        });
    }
    below
}

#[test]
fn expansion_size_counts_nested_objects_once_per_definition() {
    let mut memo = std::collections::HashMap::new();
    assert_eq!(square_block().expansion_size(&mut memo), 2);
    // Each level holds two inserts (2) and two copies of the level below.
    let chain = doubling_chain(square_block(), 3);
    assert_eq!(
        chain.expansion_size(&mut memo),
        2 + 2 * (2 + 2 * (2 + 2 * 2))
    );
    let huge = doubling_chain(square_block(), 60);
    assert!(huge.expansion_size(&mut memo) > MAX_EXPANSION);
}

#[test]
fn a_block_that_fans_out_is_cut_short_not_expanded_forever() {
    // An empty leaf draws nothing, so only the budget stops the walk.
    let empty = Arc::new(BlockDef {
        unit: Default::default(),
        id: BlockId(1),
        name: "Empty".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        entities: Vec::new(),
        dynamic: None,
    });
    let insert = Insert::new(doubling_chain(empty, 50), p(0.0, 0.0));
    assert!(insert.picture().is_empty());
    // One under the budget is expanded in full.
    let pieces = Insert::new(doubling_chain(square_block(), 12), p(0.0, 0.0)).picture();
    assert_eq!(pieces.len(), 2 << 12);
}
