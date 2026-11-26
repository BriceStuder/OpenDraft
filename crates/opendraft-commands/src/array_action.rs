//! Undoable change of the drawing's array records (associative arrays).

use opendraft_core::{ArrayRecord, Document};

use crate::{Action, Changes};

/// Replace the drawing's array records. The objects the records name are
/// added and removed by the actions beside this one in the same step.
pub struct SetArrays {
    /// Applying and reverting both swap, so each leaves the other state ready.
    arrays: Vec<ArrayRecord>,
}

impl SetArrays {
    pub fn new(arrays: Vec<ArrayRecord>) -> Self {
        Self { arrays }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.arrays = doc.set_arrays(std::mem::take(&mut self.arrays));
    }
}

impl Action for SetArrays {
    fn name(&self) -> String {
        "Array".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::default()
    }
}

#[cfg(test)]
mod tests {
    use opendraft_core::{ArrayShape, EntityId, Point};

    use super::*;

    #[test]
    fn the_records_change_and_come_back() {
        let mut doc = Document::default();
        let record = ArrayRecord {
            id: 1,
            sources: vec![EntityId(1)],
            items: vec![EntityId(2)],
            shape: ArrayShape::Polar {
                center: Point::new(0.0, 0.0),
                items: 2,
                fill: 3.0,
                rotate: false,
            },
        };
        let mut action = SetArrays::new(vec![record.clone()]);
        action.apply(&mut doc);
        assert_eq!(doc.arrays(), [record]);
        action.revert(&mut doc);
        assert!(doc.arrays().is_empty());
        action.apply(&mut doc);
        assert_eq!(doc.arrays().len(), 1);
    }
}
