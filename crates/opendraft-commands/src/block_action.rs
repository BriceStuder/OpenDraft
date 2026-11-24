//! Undoable changes to the block table.

use std::sync::Arc;

use opendraft_core::block::DynamicBlock;
use opendraft_core::{BlockDef, Document};

use crate::{Action, Changes};

/// Replace the whole block table (a block defined, redefined or purged).
/// Inserts that must follow a redefinition are changed by a
/// `ModifyEntities` in the same transaction.
pub struct SetBlocks {
    name: String,
    /// Applying and reverting both swap, so each leaves the other state ready.
    blocks: Vec<Arc<BlockDef>>,
}

impl SetBlocks {
    pub fn new(name: impl Into<String>, blocks: Vec<Arc<BlockDef>>) -> Self {
        Self {
            name: name.into(),
            blocks,
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.blocks = doc.set_blocks(std::mem::take(&mut self.blocks));
    }
}

impl Action for SetBlocks {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::blocks()
    }
}

/// Replace the dynamic schema a Block Editor drawing is authoring (`None`
/// clears it). The drawing's own block table is not touched: BSAVE turns the
/// schema into the block's.
pub struct SetBlockEditorDynamic {
    name: String,
    /// Applying and reverting both swap, so each leaves the other state ready.
    schema: Option<DynamicBlock>,
}

impl SetBlockEditorDynamic {
    pub fn new(name: impl Into<String>, schema: Option<DynamicBlock>) -> Self {
        Self {
            name: name.into(),
            schema,
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.schema = doc.set_block_editor_dynamic(self.schema.take());
    }
}

impl Action for SetBlockEditorDynamic {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::dynamic_blocks()
    }
}

#[cfg(test)]
mod tests {
    use opendraft_core::{BlockId, Point};

    use super::*;

    #[test]
    fn the_table_changes_and_comes_back() {
        let mut doc = Document::default();
        let block = Arc::new(BlockDef {
            unit: Default::default(),
            id: doc.allocate_block_id(),
            name: "Empty".into(),
            base: Point::new(0.0, 0.0),
            description: String::new(),
            entities: Vec::new(),
            dynamic: None,
        });
        let mut action = SetBlocks::new("Block", vec![block]);
        action.apply(&mut doc);
        assert_eq!(doc.blocks().len(), 1);
        action.revert(&mut doc);
        assert!(doc.blocks().is_empty());
        action.apply(&mut doc);
        assert_eq!(doc.blocks()[0].id, BlockId(1));
        assert_eq!(action.changes(), Changes::blocks());
        assert_eq!(doc.validate(), Ok(()));
    }

    #[test]
    fn the_editors_schema_changes_and_comes_back_without_touching_the_blocks() {
        let mut doc = Document::default();
        let mut schema = DynamicBlock::new();
        schema.next_parameter_id = 5;
        let mut action = SetBlockEditorDynamic::new("Parameter", Some(schema.clone()));
        action.apply(&mut doc);
        assert_eq!(doc.block_editor_dynamic(), Some(&schema));
        assert!(doc.blocks().is_empty());
        action.revert(&mut doc);
        assert_eq!(doc.block_editor_dynamic(), None);
        action.apply(&mut doc);
        assert_eq!(doc.block_editor_dynamic(), Some(&schema));
        assert_eq!(action.changes(), Changes::dynamic_blocks());
        assert_eq!(doc.validate(), Ok(()));
    }
}
