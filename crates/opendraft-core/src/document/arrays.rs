//! The associative arrays of the drawing (see [`crate::array_record`]).

use super::Document;
use crate::{ArrayRecord, EntityId};

impl Document {
    /// Every array record, in the order they were made.
    pub fn arrays(&self) -> &[ArrayRecord] {
        &self.arrays
    }

    /// The array a source or an item belongs to.
    pub fn array_holding(&self, id: EntityId) -> Option<&ArrayRecord> {
        self.arrays.iter().find(|a| a.holds(id))
    }

    /// An id for a new array record: higher than every one there.
    pub fn next_array_id(&self) -> u64 {
        self.arrays.iter().map(|a| a.id).max().map_or(1, |m| m + 1)
    }

    /// Replace the records, returning the old ones. For the undoable action
    /// and for loading a file.
    pub fn set_arrays(&mut self, arrays: Vec<ArrayRecord>) -> Vec<ArrayRecord> {
        std::mem::replace(&mut self.arrays, arrays)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArrayShape, Point};

    fn record(id: u64, sources: &[u64], items: &[u64]) -> ArrayRecord {
        ArrayRecord {
            id,
            sources: sources.iter().map(|i| EntityId(*i)).collect(),
            items: items.iter().map(|i| EntityId(*i)).collect(),
            shape: ArrayShape::Polar {
                center: Point::new(0.0, 0.0),
                items: 3,
                fill: 1.0,
                rotate: true,
            },
        }
    }

    #[test]
    fn a_source_or_an_item_finds_its_array_and_ids_climb() {
        let mut doc = Document::default();
        assert_eq!(doc.next_array_id(), 1);
        doc.set_arrays(vec![record(1, &[1], &[2, 3]), record(4, &[10], &[11])]);
        assert_eq!(doc.array_holding(EntityId(3)).map(|a| a.id), Some(1));
        assert_eq!(doc.array_holding(EntityId(10)).map(|a| a.id), Some(4));
        assert!(doc.array_holding(EntityId(99)).is_none());
        assert_eq!(doc.next_array_id(), 5);
        let old = doc.set_arrays(Vec::new());
        assert_eq!(old.len(), 2);
        assert!(doc.arrays().is_empty());
    }
}
