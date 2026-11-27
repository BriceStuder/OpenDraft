//! PEDIT's curve options: Fit, Spline and Decurve for a polyline, vertex
//! tangents, and turning splines into polylines. The geometry is
//! `opendraft_core::pedit` (`curve.rs`, `arcs.rs`) and `polyline_curve`.
//!
//! Fit and Spline keep the polyline they were made from (and the tangents set
//! on it) in a record beside the object, so Decurve can bring the original
//! vertices back and a second curve starts again from them. The record applies
//! only while the polyline is as the curve left it.

use opendraft_commands::{Cancelled, Editor, NumberOptions, Options, SetPolylineCurve};
use opendraft_core::pedit::PolyData;
use opendraft_core::polyline_curve::{curved, decurved, normalized};
use opendraft_core::{CurveFrame, Entity, EntityId, PolylineCurve, Spline};
use opendraft_ui::key;

use crate::defaults::EditorDefaults;
use crate::pedit_cmd::store;

/// The record of `id` while it still describes the polyline.
pub(crate) fn record_of(ed: &Editor, id: EntityId) -> Result<Option<PolylineCurve>, Cancelled> {
    ed.read(move |doc| doc.polyline_curve(id).cloned())
}

/// Replace polyline `id` by `data` and give it a record with these tangents
/// and frame, as one undo step.
pub(crate) fn store_with_record(
    ed: &Editor,
    id: EntityId,
    data: PolyData,
    tangents: Vec<Option<f64>>,
    frame: Option<CurveFrame>,
) -> Result<(), Cancelled> {
    let shape = normalized(data.clone());
    let record = PolylineCurve {
        id,
        shape,
        tangents,
        frame,
    };
    let entity = data.into_entity();
    ed.transaction("Edit Polyline", move |tx| {
        tx.modify(id, move |_| entity);
        tx.perform(SetPolylineCurve::new(id, Some(record)));
        Ok(())
    })
}

/// Fit or Spline: replace the polyline by its curve. `true` when it changed.
pub(crate) fn curve(
    ed: &Editor,
    id: EntityId,
    data: &PolyData,
    fit: bool,
) -> Result<bool, Cancelled> {
    let record = record_of(ed, id)?;
    match curved(data, record.as_ref(), fit) {
        Some((made, frame)) => {
            let tangents = vec![None; made.points.len()];
            store_with_record(ed, id, made, tangents, Some(frame))?;
            Ok(true)
        }
        None => {
            ed.say(key!(
                "The polyline needs at least three vertices that are apart."
            ));
            Ok(false)
        }
    }
}

/// Decurve: the polyline Fit or Spline was made from when it is still as the
/// curve left it, otherwise every segment straight.
pub(crate) fn decurve(ed: &Editor, id: EntityId, data: &PolyData) -> Result<(), Cancelled> {
    let record = record_of(ed, id)?;
    let (made, tangents) = decurved(data, record.as_ref());
    if tangents.is_empty() {
        return store(ed, id, made);
    }
    store_with_record(ed, id, made, tangents, None)
}

/// Decurve for several polylines at once (PEDIT's Multiple), as one undo
/// step. `false` when there was nothing to change.
pub(crate) fn decurve_all(ed: &Editor, ids: &[EntityId]) -> Result<bool, Cancelled> {
    let ids = ids.to_vec();
    let edits = ed.read(move |doc| {
        ids.iter()
            .filter_map(|id| {
                let data = PolyData::of(doc.get(*id)?)?;
                let (made, tangents) = decurved(&data, doc.polyline_curve(*id));
                let record = (!tangents.is_empty()).then(|| PolylineCurve {
                    id: *id,
                    shape: normalized(made.clone()),
                    tangents,
                    frame: None,
                });
                Some((*id, made.into_entity(), record))
            })
            .collect::<Vec<_>>()
    })?;
    if edits.is_empty() {
        return Ok(false);
    }
    ed.transaction("Edit Polyline", move |tx| {
        for (id, entity, record) in edits {
            tx.modify(id, move |_| entity);
            tx.perform(SetPolylineCurve::new(id, record));
        }
        Ok(())
    })?;
    Ok(true)
}

/// Whether tangents can be set on `id`: not on a curve made by Fit or Spline,
/// whose vertices are not the ones to set them on (it says so when not).
pub(crate) fn can_set_tangent(ed: &Editor, id: EntityId) -> Result<bool, Cancelled> {
    let curve = record_of(ed, id)?.is_some_and(|r| r.frame.is_some());
    if curve {
        ed.say(key!(
            "This polyline is a curve: Decurve it to set tangents."
        ));
    }
    Ok(!curve)
}

/// Edit vertex, Tangent: set the direction (radians) the curve of Fit runs
/// along at vertex `v` (see [`can_set_tangent`]).
pub(crate) fn set_tangent(
    ed: &Editor,
    id: EntityId,
    data: &PolyData,
    v: usize,
    angle: f64,
) -> Result<bool, Cancelled> {
    let record = record_of(ed, id)?.filter(|r| r.frame.is_none());
    let mut tangents = record.map_or_else(|| vec![None; data.points.len()], |r| r.tangents);
    let Some(slot) = tangents.get_mut(v) else {
        return Ok(false);
    };
    *slot = Some(angle);
    let record = PolylineCurve {
        id,
        shape: normalized(data.clone()),
        tangents,
        frame: None,
    };
    ed.transaction("Edit Polyline", move |tx| {
        tx.perform(SetPolylineCurve::new(id, Some(record)));
        Ok(())
    })?;
    Ok(true)
}

/// Replace polyline `id` by `data` after an edit that leaves the vertices
/// where they are (moving one, a segment's width), keeping the tangents set on
/// it. Without any, it is a plain `store`.
pub(crate) fn store_keeping_tangents(
    ed: &Editor,
    id: EntityId,
    old: &PolyData,
    data: PolyData,
) -> Result<(), Cancelled> {
    match record_of(ed, id)? {
        Some(r)
            if r.frame.is_none() && r.applies_to(old) && r.tangents.len() == data.points.len() =>
        {
            store_with_record(ed, id, data, r.tangents, None)
        }
        _ => store(ed, id, data),
    }
}

/// Ask how closely a spline is followed: 0 to 99, the higher the closer (10
/// is a thousandth of its size and each ten more is ten times closer).
fn ask_precision(ed: &Editor) -> Result<f64, Cancelled> {
    Ok(ed
        .get_number(
            NumberOptions::new(key!("Specify precision for polyline conversion (0 to 99)"))
                .default(10.0),
        )?
        .clamp(0.0, 99.0))
}

/// The polyline for `spline`: arcs when `PLINECONVERTMODE` is 1, else
/// straight pieces.
fn polyline_of(spline: &Spline, precision: f64, arcs: bool) -> Option<PolyData> {
    let size = {
        let b = spline.bounds();
        (b.max.x - b.min.x).hypot(b.max.y - b.min.y)
    };
    let tolerance = (size * 10f64.powf(-2.0 - precision / 10.0)).max(1e-9);
    if arcs {
        PolyData::from_spline_arcs(spline, tolerance)
    } else {
        PolyData::from_spline(spline, tolerance)
    }
}

fn spline_of(ed: &Editor, id: EntityId) -> Result<Option<Spline>, Cancelled> {
    ed.read(move |doc| match doc.get(id) {
        Some(Entity::Spline(s)) => Some((**s).clone()),
        _ => None,
    })
}

/// A spline picked for PEDIT: offer to turn it into a polyline (without
/// asking when PEDITACCEPT is 1), by the precision asked for (0 to 99: the
/// higher, the closer to the curve) and into arcs or straight pieces as
/// `PLINECONVERTMODE` says. `true` when the spline became a polyline.
pub(crate) fn convert_spline(ed: &Editor, id: EntityId) -> Result<bool, Cancelled> {
    if !ed.defaults().pedit.get().accept {
        let answer = ed.get_keyword(
            Options::new(key!(
                "Object selected is not a polyline. Do you want to turn it into one?"
            ))
            .keyword("Yes", "Y")
            .keyword("No", "N")
            .default("Yes".to_owned()),
        )?;
        if answer == "No" {
            return Ok(false);
        }
    }
    let precision = ask_precision(ed)?;
    Ok(!convert_splines(ed, &[id], precision)?.is_empty())
}

/// Turn the splines `ids` into polylines (one undo step) at `precision`;
/// returns the ones that became polylines, and says about those that could
/// not.
pub(crate) fn convert_splines(
    ed: &Editor,
    ids: &[EntityId],
    precision: f64,
) -> Result<Vec<EntityId>, Cancelled> {
    let arcs = ed.defaults().pedit.get().convert_arcs;
    let mut made = Vec::new();
    for id in ids {
        let Some(spline) = spline_of(ed, *id)? else {
            continue;
        };
        match polyline_of(&spline, precision, arcs) {
            Some(data) => made.push((*id, data.into_entity())),
            None => ed.say(key!("The spline could not be turned into a polyline.")),
        }
    }
    let done: Vec<EntityId> = made.iter().map(|(id, _)| *id).collect();
    if made.is_empty() {
        return Ok(done);
    }
    ed.transaction("Edit Polyline", move |tx| {
        for (id, entity) in made {
            tx.modify(id, move |_| entity);
        }
        Ok(())
    })?;
    Ok(done)
}

/// PEDIT's Multiple with splines among the objects: ask the precision once
/// and turn them all into polylines. Returns the ids that are now polylines.
pub(crate) fn convert_many_splines(
    ed: &Editor,
    ids: &[EntityId],
) -> Result<Vec<EntityId>, Cancelled> {
    let precision = ask_precision(ed)?;
    convert_splines(ed, ids, precision)
}
