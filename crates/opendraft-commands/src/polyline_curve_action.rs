//! Undoable change of what PEDIT keeps beside a polyline (its vertex tangents
//! and curve frame).

use opendraft_core::{Document, EntityId, PolylineCurve};

use crate::{Action, Changes};

/// Replace (or remove) the record of one polyline. It is made beside the
/// action that changes the polyline, in the same undo step.
pub struct SetPolylineCurve {
    id: EntityId,
    /// Applying and reverting both swap, so each leaves the other state ready.
    record: Option<PolylineCurve>,
}

impl SetPolylineCurve {
    pub fn new(id: EntityId, record: Option<PolylineCurve>) -> Self {
        Self { id, record }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.record = doc.set_polyline_curve(self.id, self.record.take());
    }
}

impl Action for SetPolylineCurve {
    fn name(&self) -> String {
        "Edit Polyline".to_owned()
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
    use opendraft_core::pedit::PolyData;
    use opendraft_core::{Entity, Point};

    use super::*;

    #[test]
    fn the_record_is_set_and_comes_back() {
        let mut doc = Document::default();
        let id = doc.add(Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(4.0, 0.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        });
        let shape = PolyData::of(doc.get(id).unwrap()).unwrap();
        let record = PolylineCurve {
            id,
            shape,
            tangents: vec![Some(0.5), None],
            frame: None,
        };
        let mut action = SetPolylineCurve::new(id, Some(record.clone()));
        action.apply(&mut doc);
        assert_eq!(doc.polyline_curve(id), Some(&record));
        action.revert(&mut doc);
        assert!(doc.polyline_curves().is_empty());
        action.apply(&mut doc);
        assert_eq!(doc.polyline_curve(id), Some(&record));
    }
}
