//! What several selected entities have in common: the name the panel gives
//! them, the rows they share and a shared value.

use opendraft_core::{Entity, EntityKind};
use opendraft_ui::key;

use super::{oblong, polyline, rows, value, Field, Row};

/// The name the panel gives the given entities of `kind`: "Rectangular
/// polyline" when every polyline among them traces a rectangle, "Oblong
/// polyline" when every one traces an oblong.
pub fn title(kind: EntityKind, entities: &[&Entity]) -> &'static str {
    let every = |is: fn(&Entity) -> bool| {
        kind == EntityKind::Polyline && entities.iter().filter(|e| e.kind() == kind).all(|e| is(e))
    };
    if every(|e| polyline::rectangle(e).is_some()) {
        key!("Rectangular polyline")
    } else if every(|e| oblong::slot(e).is_some()) {
        key!("Oblong polyline")
    } else {
        kind.title()
    }
}

/// Rows that every given entity has (same field and label), in the first
/// entity's order. Entities of different kinds currently share little, since
/// layer, colour and so on do not exist yet.
pub fn common_rows(entities: &[&Entity]) -> Vec<Row> {
    let Some((first, rest)) = entities.split_first() else {
        return Vec::new();
    };
    rows(first)
        .iter()
        .filter(|r| {
            rest.iter().all(|e| {
                rows(e)
                    .iter()
                    .any(|o| o.field == r.field && o.label == r.label)
            })
        })
        .copied()
        .collect()
}

/// A field's value across several entities: the shared value, or `None` when
/// they differ.
pub fn shared_value(entities: &[&Entity], field: Field) -> Option<f64> {
    let first = value(entities.first()?, field);
    entities
        .iter()
        .all(|e| (value(e, field) - first).abs() < 1e-9)
        .then_some(first)
}
