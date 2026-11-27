//! What PEDIT keeps beside polylines: vertex tangents and the frame a Fit or
//! Spline curve was made from (see `opendraft_core::polyline_curve`). Only the
//! records that still describe their polyline are written, and the table is
//! left out when there are none. A record for an object that is not in the
//! file, or one whose numbers cannot be used, is dropped on reading.

use std::collections::HashSet;

use opendraft_core::pedit::PolyData;
use opendraft_core::{CurveFrame, Document, EntityId, Point, PolylineCurve};
use serde::{Deserialize, Serialize};

use super::limits;

#[derive(Serialize, Deserialize)]
struct ShapeDto {
    #[serde(deserialize_with = "limits::vertices")]
    points: Vec<[f64; 2]>,
    closed: bool,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "limits::vertices"
    )]
    bulges: Vec<f64>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "limits::vertices"
    )]
    widths: Vec<[f64; 2]>,
}

impl ShapeDto {
    fn of(shape: &PolyData) -> Self {
        Self {
            points: shape.points.iter().map(|p| [p.x, p.y]).collect(),
            closed: shape.closed,
            bulges: shape.bulges.clone(),
            widths: shape.widths.iter().map(|w| [w.0, w.1]).collect(),
        }
    }

    fn read(self) -> PolyData {
        PolyData {
            points: self.points.iter().map(|p| Point::new(p[0], p[1])).collect(),
            closed: self.closed,
            bulges: self.bulges,
            widths: self.widths.iter().map(|w| (w[0], w[1])).collect(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct FrameDto {
    shape: ShapeDto,
    #[serde(deserialize_with = "limits::vertices")]
    tangents: Vec<Option<f64>>,
}

#[derive(Serialize, Deserialize)]
struct CurveDto {
    id: u64,
    shape: ShapeDto,
    #[serde(deserialize_with = "limits::vertices")]
    tangents: Vec<Option<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frame: Option<FrameDto>,
}

/// The document's own field.
#[derive(Default, Serialize, Deserialize)]
pub(super) struct TableDto {
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "limits::table_rows"
    )]
    polyline_curves: Vec<CurveDto>,
}

impl TableDto {
    pub(super) fn of(document: &Document) -> Self {
        let records = document.polyline_curves().iter();
        Self {
            polyline_curves: records
                // Only what still describes its polyline.
                .filter(|r| document.polyline_curve(r.id) == Some(*r))
                .map(|r| CurveDto {
                    id: r.id.0,
                    shape: ShapeDto::of(&r.shape),
                    tangents: r.tangents.clone(),
                    frame: r.frame.as_ref().map(|f| FrameDto {
                        shape: ShapeDto::of(&f.shape),
                        tangents: f.tangents.clone(),
                    }),
                })
                .collect(),
        }
    }

    /// Install the records once the objects are in `document`: `known` are the
    /// ids of the file's objects.
    pub(super) fn load(self, document: &mut Document, known: &HashSet<u64>) {
        let records = self
            .polyline_curves
            .into_iter()
            .filter(|dto| known.contains(&dto.id))
            .map(|dto| PolylineCurve {
                id: EntityId(dto.id),
                shape: dto.shape.read(),
                tangents: dto.tangents,
                frame: dto.frame.map(|f| CurveFrame {
                    shape: f.shape.read(),
                    tangents: f.tangents,
                }),
            })
            .collect();
        document.set_polyline_curves(records);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::tests::roundtrip;
    use crate::native::NativeFormat;
    use crate::FileFormat;
    use opendraft_core::Entity;

    fn polyline(points: &[(f64, f64)]) -> Entity {
        Entity::Polyline {
            points: points.iter().map(|p| Point::new(p.0, p.1)).collect(),
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        }
    }

    fn with_record(document: &mut Document, id: EntityId) {
        let shape = PolyData::of(document.get(id).unwrap()).unwrap();
        let frame = CurveFrame {
            shape: PolyData {
                points: vec![Point::new(0.0, 0.0), Point::new(9.0, 0.0)],
                closed: false,
                bulges: Vec::new(),
                widths: Vec::new(),
            },
            tangents: vec![Some(0.25), None],
        };
        let n = shape.points.len();
        document.set_polyline_curve(
            id,
            Some(PolylineCurve {
                id,
                shape,
                tangents: vec![None; n],
                frame: Some(frame),
            }),
        );
    }

    #[test]
    fn the_records_round_trip_and_are_left_out_when_there_are_none() {
        let mut document = Document::default();
        let id = document.add(polyline(&[(0.0, 0.0), (3.0, 1.0), (9.0, 0.0)]));
        let plain = NativeFormat.write(&document).unwrap().bytes;
        assert!(!String::from_utf8(plain)
            .unwrap()
            .contains("polyline_curves"));
        with_record(&mut document, id);
        let loaded = roundtrip(&document);
        assert_eq!(loaded.polyline_curves(), document.polyline_curves());
        assert!(loaded.polyline_curve(id).is_some());
    }

    #[test]
    fn a_record_that_no_longer_describes_its_polyline_is_not_written() {
        let mut document = Document::default();
        let id = document.add(polyline(&[(0.0, 0.0), (3.0, 1.0), (9.0, 0.0)]));
        with_record(&mut document, id);
        document.replace(id, polyline(&[(0.0, 0.0), (3.0, 2.0), (9.0, 0.0)]));
        assert!(roundtrip(&document).polyline_curves().is_empty());
    }
}
