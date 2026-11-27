//! The tangents and curve frames PEDIT keeps beside polylines (see
//! [`crate::polyline_curve`]).

use super::Document;
use crate::pedit::PolyData;
use crate::{EntityId, PolylineCurve};

impl Document {
    /// Every record, whether or not it still applies.
    pub fn polyline_curves(&self) -> &[PolylineCurve] {
        &self.polyline_curves
    }

    /// The record of polyline `id`, if there is one and the polyline is still
    /// as it left it.
    pub fn polyline_curve(&self, id: EntityId) -> Option<&PolylineCurve> {
        let record = self.polyline_curves.iter().find(|r| r.id == id)?;
        let current = PolyData::of(self.get(id)?)?;
        record.applies_to(&current).then_some(record)
    }

    /// Replace the record of polyline `id` (or take it away with `None`) and
    /// return the old one. A record that cannot be used is not kept.
    pub fn set_polyline_curve(
        &mut self,
        id: EntityId,
        record: Option<PolylineCurve>,
    ) -> Option<PolylineCurve> {
        let old = self
            .polyline_curves
            .iter()
            .position(|r| r.id == id)
            .map(|at| self.polyline_curves.remove(at));
        if let Some(record) = record.filter(|r| r.id == id && r.is_valid() && !r.is_empty()) {
            self.polyline_curves.push(record);
        }
        old
    }

    /// Replace all the records, for loading a file.
    pub fn set_polyline_curves(&mut self, records: Vec<PolylineCurve>) {
        self.polyline_curves = records
            .into_iter()
            .filter(|r| r.is_valid() && !r.is_empty())
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Entity, Point};

    fn open(points: &[(f64, f64)]) -> Entity {
        Entity::Polyline {
            points: points.iter().map(|p| Point::new(p.0, p.1)).collect(),
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        }
    }

    #[test]
    fn a_record_is_found_while_the_polyline_is_as_it_was_and_not_after_an_edit() {
        let mut doc = Document::default();
        let id = doc.add(open(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)]));
        let shape = PolyData::of(doc.get(id).unwrap()).unwrap();
        let record = PolylineCurve {
            id,
            shape,
            tangents: vec![Some(0.0), None, None],
            frame: None,
        };
        assert!(doc.set_polyline_curve(id, Some(record.clone())).is_none());
        assert_eq!(doc.polyline_curve(id), Some(&record));
        // Edited since: no longer found (and the old record is still held
        // until the next change).
        doc.replace(id, open(&[(0.0, 0.0), (5.0, 1.0), (5.0, 5.0)]));
        assert!(doc.polyline_curve(id).is_none());
        // Replacing returns the old one; taking away leaves none.
        assert_eq!(doc.set_polyline_curve(id, None), Some(record));
        assert!(doc.polyline_curves().is_empty());
    }

    #[test]
    fn an_empty_or_unusable_record_is_not_kept() {
        let mut doc = Document::default();
        let id = doc.add(open(&[(0.0, 0.0), (5.0, 0.0)]));
        let shape = PolyData::of(doc.get(id).unwrap()).unwrap();
        let empty = PolylineCurve {
            id,
            shape: shape.clone(),
            tangents: vec![None, None],
            frame: None,
        };
        doc.set_polyline_curve(id, Some(empty));
        let wrong_length = PolylineCurve {
            id,
            shape,
            tangents: vec![Some(1.0)],
            frame: None,
        };
        doc.set_polyline_curve(id, Some(wrong_length));
        assert!(doc.polyline_curves().is_empty());
    }
}
