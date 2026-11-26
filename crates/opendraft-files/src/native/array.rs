//! Associative arrays: the records beside the objects (see
//! `opendraft_core::array_record`). Left out of the file when there are none.
//! The objects a record names are the file's own objects; a name that is not
//! one of them (an item erased since) is dropped on reading, and a record left
//! with no source is dropped.

use std::collections::HashSet;

use opendraft_core::{ArrayRecord, ArrayShape, Document, EntityId, Point};
use serde::{Deserialize, Serialize};

use super::ccs::CcsDto;
use super::entity::invalid;
use super::limits;
use crate::FileError;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ShapeDto {
    Rectangular {
        columns: usize,
        rows: usize,
        column_spacing: f64,
        row_spacing: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ccs: Option<CcsDto>,
    },
    Polar {
        center: [f64; 2],
        items: usize,
        fill: f64,
        rotate: bool,
    },
    Path {
        path: u64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        reversed: bool,
        items: usize,
        spacing: f64,
        align: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        divide: bool,
    },
}

#[derive(Serialize, Deserialize)]
struct ArrayDto {
    id: u64,
    #[serde(deserialize_with = "limits::objects")]
    sources: Vec<u64>,
    #[serde(deserialize_with = "limits::objects")]
    items: Vec<u64>,
    shape: ShapeDto,
}

/// The document's own field.
#[derive(Default, Serialize, Deserialize)]
pub(super) struct TableDto {
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "limits::table_rows"
    )]
    arrays: Vec<ArrayDto>,
}

impl TableDto {
    pub(super) fn of(document: &Document) -> Self {
        let ids = |ids: &[EntityId]| ids.iter().map(|id| id.0).collect();
        Self {
            arrays: document
                .arrays()
                .iter()
                .map(|a| ArrayDto {
                    id: a.id,
                    sources: ids(&a.sources),
                    items: ids(&a.items),
                    shape: match a.shape {
                        ArrayShape::Rectangular {
                            columns,
                            rows,
                            column_spacing,
                            row_spacing,
                            ccs,
                        } => ShapeDto::Rectangular {
                            columns,
                            rows,
                            column_spacing,
                            row_spacing,
                            ccs: CcsDto::unless_world(ccs),
                        },
                        ArrayShape::Polar {
                            center,
                            items,
                            fill,
                            rotate,
                        } => ShapeDto::Polar {
                            center: [center.x, center.y],
                            items,
                            fill,
                            rotate,
                        },
                        ArrayShape::Path {
                            path,
                            reversed,
                            items,
                            spacing,
                            align,
                            divide,
                        } => ShapeDto::Path {
                            path: path.0,
                            reversed,
                            items,
                            spacing,
                            align,
                            divide,
                        },
                    },
                })
                .collect(),
        }
    }

    /// Install the records once the objects are in `document`: `known` are the
    /// ids of the file's objects.
    pub(super) fn load(
        self,
        document: &mut Document,
        known: &HashSet<u64>,
    ) -> Result<(), FileError> {
        let mut records = Vec::new();
        let mut seen = HashSet::new();
        for dto in self.arrays {
            if !seen.insert(dto.id) {
                return invalid(format!("array {} appears twice", dto.id));
            }
            let alive = |ids: Vec<u64>| -> Vec<EntityId> {
                ids.into_iter()
                    .filter(|id| known.contains(id))
                    .map(EntityId)
                    .collect()
            };
            let (sources, items) = (alive(dto.sources), alive(dto.items));
            if sources.is_empty() {
                continue;
            }
            let shape = match dto.shape {
                ShapeDto::Rectangular {
                    columns,
                    rows,
                    column_spacing,
                    row_spacing,
                    ccs,
                } => ArrayShape::Rectangular {
                    columns,
                    rows,
                    column_spacing,
                    row_spacing,
                    ccs: ccs.map(CcsDto::read).transpose()?.unwrap_or_default(),
                },
                ShapeDto::Polar {
                    center,
                    items,
                    fill,
                    rotate,
                } => ArrayShape::Polar {
                    center: Point::new(center[0], center[1]),
                    items,
                    fill,
                    rotate,
                },
                ShapeDto::Path {
                    path,
                    reversed,
                    items,
                    spacing,
                    align,
                    divide,
                } => ArrayShape::Path {
                    path: EntityId(path),
                    reversed,
                    items,
                    spacing,
                    align,
                    divide,
                },
            };
            if !shape_is_sane(&shape) {
                return invalid(format!("array {} has settings that cannot be used", dto.id));
            }
            records.push(ArrayRecord {
                id: dto.id,
                sources,
                items,
                shape,
            });
        }
        document.set_arrays(records);
        Ok(())
    }
}

/// Counts that can be laid out and numbers that are numbers.
fn shape_is_sane(shape: &ArrayShape) -> bool {
    let count = |n: usize| (1..=opendraft_core::array::MAX_ITEMS).contains(&n);
    match *shape {
        ArrayShape::Rectangular {
            columns,
            rows,
            column_spacing,
            row_spacing,
            ..
        } => count(columns) && count(rows) && column_spacing.is_finite() && row_spacing.is_finite(),
        ArrayShape::Polar {
            center,
            items,
            fill,
            ..
        } => count(items) && fill.is_finite() && center.x.is_finite() && center.y.is_finite(),
        ArrayShape::Path { items, spacing, .. } => count(items) && spacing.is_finite(),
    }
}

#[cfg(test)]
mod tests {
    use opendraft_core::{Attributes, Entity, LayerId, Object};

    use super::*;
    use crate::native::tests::roundtrip;
    use crate::native::NativeFormat;
    use crate::FileFormat;

    fn with_objects(count: u64) -> Document {
        let mut document = Document::default();
        for i in 1..=count {
            document.push(Object::new(
                EntityId(i),
                Entity::Point {
                    at: Point::new(i as f64, 0.0),
                },
                Attributes::on_layer(LayerId::DEFAULT),
                None,
            ));
        }
        document.reserve_ids(count + 1, 1);
        document
    }

    fn polar() -> ArrayRecord {
        ArrayRecord {
            id: 3,
            sources: vec![EntityId(1)],
            items: vec![EntityId(2), EntityId(3)],
            shape: ArrayShape::Polar {
                center: Point::new(5.0, -1.0),
                items: 3,
                fill: 2.5,
                rotate: false,
            },
        }
    }

    #[test]
    fn no_arrays_leave_no_trace_in_the_file() {
        let text =
            String::from_utf8(NativeFormat.write(&Document::default()).unwrap().bytes).unwrap();
        assert!(!text.contains("arrays"));
    }

    #[test]
    fn every_kind_of_array_round_trips() {
        let mut document = with_objects(8);
        let rect = ArrayRecord {
            id: 1,
            sources: vec![EntityId(4)],
            items: vec![EntityId(5)],
            shape: ArrayShape::Rectangular {
                columns: 2,
                rows: 1,
                column_spacing: 3.5,
                row_spacing: -2.0,
                ccs: opendraft_core::Ccs::new(Point::new(1.0, 2.0), 0.5),
            },
        };
        let path = ArrayRecord {
            id: 4,
            sources: vec![EntityId(6)],
            items: vec![EntityId(7)],
            shape: ArrayShape::Path {
                path: EntityId(8),
                reversed: true,
                items: 2,
                spacing: 4.0,
                align: true,
                divide: true,
            },
        };
        document.set_arrays(vec![rect, polar(), path]);
        let loaded = roundtrip(&document);
        assert_eq!(loaded.arrays(), document.arrays());
    }

    #[test]
    fn items_that_are_gone_are_dropped_and_so_is_an_array_with_no_source() {
        let mut document = with_objects(3);
        let mut lost = polar();
        lost.items.push(EntityId(99));
        let orphan = ArrayRecord {
            id: 9,
            sources: vec![EntityId(98)],
            ..polar()
        };
        document.set_arrays(vec![lost, orphan]);
        let loaded = roundtrip(&document);
        assert_eq!(loaded.arrays().len(), 1);
        assert_eq!(loaded.arrays()[0].items, [EntityId(2), EntityId(3)]);
    }
}
