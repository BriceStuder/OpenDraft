//! Undoable insertion of a library block (see `Document::insert_from`): the
//! block, the blocks it contains, the layers and styles they use, and the
//! reference, in one step.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use opendraft_core::{
    BlockClash, BlockId, BringError, Document, LayoutId, LibraryPlacement, Placed, TransferWarning,
};

use crate::{Action, Changes};

/// What the first run did, for the caller to report (the action itself is in
/// the undo stack by then).
#[derive(Clone, Debug, Default)]
pub struct LibraryInsertReport {
    pub warnings: Vec<TransferWarning>,
    /// The name the block has in the drawing.
    pub block_name: String,
    /// References of the drawing that followed a replaced definition.
    pub rebound: usize,
    /// Why nothing was inserted.
    pub refused: Option<BringError>,
}

pub type LibraryInsertSink = Arc<Mutex<LibraryInsertReport>>;

pub struct InsertFromLibrary {
    source: Document,
    root: BlockId,
    choices: HashMap<BlockId, BlockClash>,
    placement: LibraryPlacement,
    /// Decided the first time the action runs: the space being edited then.
    layout: Option<Option<LayoutId>>,
    done: Option<Placed>,
    sink: Option<LibraryInsertSink>,
}

impl InsertFromLibrary {
    pub fn new(
        source: Document,
        root: BlockId,
        choices: HashMap<BlockId, BlockClash>,
        placement: LibraryPlacement,
    ) -> Self {
        Self {
            source,
            root,
            choices,
            placement,
            layout: None,
            done: None,
            sink: None,
        }
    }

    /// Put what the first run did in `sink`.
    pub fn reporting(mut self, sink: LibraryInsertSink) -> Self {
        self.sink = Some(sink);
        self
    }

    fn report(&self, report: LibraryInsertReport) {
        if let Some(Ok(mut sink)) = self.sink.as_ref().map(|s| s.lock()) {
            *sink = report;
        }
    }
}

impl Action for InsertFromLibrary {
    fn name(&self) -> String {
        "Insert from library".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        if let Some(done) = &self.done {
            doc.replace_placed(done);
            return;
        }
        let layout = *self.layout.get_or_insert(doc.active_layout());
        match doc.insert_from(
            &self.source,
            self.root,
            &self.choices,
            &self.placement,
            layout,
        ) {
            Ok(placed) => {
                self.report(LibraryInsertReport {
                    warnings: placed.brought.warnings.clone(),
                    block_name: placed.brought.block.name.clone(),
                    rebound: placed.brought.rebound,
                    refused: None,
                });
                self.done = Some(placed);
            }
            Err(why) => self.report(LibraryInsertReport {
                refused: Some(why),
                ..Default::default()
            }),
        }
    }

    fn revert(&mut self, doc: &mut Document) {
        if let Some(done) = &self.done {
            doc.unplace(done);
        }
    }

    fn changes(&self) -> Changes {
        let Some(done) = &self.done else {
            return Changes::default();
        };
        Changes {
            added: vec![done.object.id],
            changed: done.brought.changed(),
            layers: true,
            blocks: true,
            linetypes: true,
            text_styles: true,
            ..Changes::default()
        }
    }
}
