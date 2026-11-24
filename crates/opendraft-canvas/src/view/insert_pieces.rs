//! What each block reference draws, worked out once per state of the drawing.
//! Expanding a reference (moving every entity of its block into place,
//! working out layers and colours) costs far more than drawing the result,
//! and a new picture after a pan or zoom would do it again for each one.

use std::collections::HashMap;
use std::sync::Arc;

use opendraft_core::{Document, EntityId, Object};

/// Pieces kept at most; past this a reference is expanded each time it is
/// drawn, so a drawing of huge blocks cannot fill the memory.
const MAX_KEPT_PIECES: usize = 300_000;

#[derive(Default)]
pub(crate) struct InsertPieces {
    /// What the drawing was when the pieces were made.
    token: Option<(u64, u64)>,
    kept: HashMap<EntityId, Arc<Vec<Object>>>,
    pieces: usize,
}

impl InsertPieces {
    /// The pieces of the block reference `object` (see
    /// `Document::insert_pieces`), `None` for any other object. `token` says
    /// the state of the drawing; with none, nothing is kept.
    pub fn of(
        &mut self,
        token: Option<(u64, u64)>,
        document: &Document,
        object: &Object,
    ) -> Option<Arc<Vec<Object>>> {
        let Some(token) = token else {
            return document.insert_pieces(object).map(Arc::new);
        };
        if self.token != Some(token) {
            self.token = Some(token);
            self.kept.clear();
            self.pieces = 0;
        }
        if let Some(kept) = self.kept.get(&object.id) {
            return Some(kept.clone());
        }
        let made = Arc::new(document.insert_pieces(object)?);
        if self.pieces + made.len() <= MAX_KEPT_PIECES {
            self.pieces += made.len();
            self.kept.insert(object.id, made.clone());
        }
        Some(made)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendraft_core::{
        Attributes, BlockDef, BlockEntity, BlockId, Entity, Insert, LayerId, Point,
    };

    fn a_block_in_a_drawing() -> (Document, Object) {
        let block = BlockDef {
            unit: Default::default(),
            id: BlockId(1),
            name: "Bar".into(),
            base: Point::new(0.0, 0.0),
            description: String::new(),
            entities: vec![BlockEntity {
                id: opendraft_core::BlockObjectId(0),
                entity: Entity::Line {
                    start: Point::new(0.0, 0.0),
                    end: Point::new(10.0, 0.0),
                },
                attrs: Attributes::on_layer(LayerId::DEFAULT),
            }],
            dynamic: None,
        };
        let mut doc = Document::default();
        let insert = Insert::new(Arc::new(block), Point::new(0.0, 20.0));
        let id = doc.add(Entity::Insert(Box::new(insert)));
        let object = doc.objects().iter().find(|o| o.id == id).cloned();
        (doc, object.expect("the insert was added"))
    }

    #[test]
    fn a_reference_is_expanded_once_for_one_state_of_the_drawing() {
        let (doc, object) = a_block_in_a_drawing();
        let mut kept = InsertPieces::default();
        let a = kept.of(Some((1, 0)), &doc, &object).expect("a reference");
        let b = kept.of(Some((1, 0)), &doc, &object).expect("a reference");
        assert!(Arc::ptr_eq(&a, &b), "the same pieces, not made again");
        assert_eq!(a.len(), 1);
        let c = kept.of(Some((1, 1)), &doc, &object).expect("a reference");
        assert!(!Arc::ptr_eq(&a, &c), "the drawing changed");
    }

    #[test]
    fn without_a_state_nothing_is_kept() {
        let (doc, object) = a_block_in_a_drawing();
        let mut kept = InsertPieces::default();
        let a = kept.of(None, &doc, &object).expect("a reference");
        let b = kept.of(None, &doc, &object).expect("a reference");
        assert!(!Arc::ptr_eq(&a, &b));
    }
}
