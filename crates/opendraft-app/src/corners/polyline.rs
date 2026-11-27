//! The polyline cases of FILLET and CHAMFER: a line joined to a polyline's end
//! segment, one corner of a polyline, and the Polyline option.

use opendraft_commands::{Answer, Cancelled, Editor, Options, Wording};
use opendraft_core::{join_to_end_segment, round_polyline, Chamfer, Entity, Round};
use opendraft_ui::key;

use super::{apply, Kind, Memory, Picked, Plan};

/// A line or arc and the end segment of an open polyline become one
/// polyline. `None` when the two are not that pair.
pub(super) fn join_line_to_end(
    m: &Memory,
    kind: Kind,
    [a, b]: [&Entity; 2],
    [first, second]: [Picked; 2],
) -> Option<Result<Plan, Option<String>>> {
    let open = |e: &Entity| matches!(e, Entity::Polyline { closed: false, .. });
    let plain = |e: &Entity| matches!(e, Entity::Line { .. } | Entity::Arc { .. });
    let (line, polyline, line_pick, polyline_pick, polyline_first) = if open(a) && plain(b) {
        (b, a, second, first, true)
    } else if open(b) && plain(a) {
        (a, b, first, second, false)
    } else {
        return None;
    };
    let sharp = second.shift;
    let how = match kind {
        Kind::Fillet => Round::Fillet(if sharp { 0.0 } else { m.radius }),
        Kind::Chamfer if sharp => Round::Chamfer(Chamfer::Distances(0.0, 0.0)),
        Kind::Chamfer => Round::Chamfer(m.chamfer()),
    };
    Some(
        join_to_end_segment(line, polyline, how, line_pick.at, polyline_pick.at)
            .map(|done| Plan::Merge {
                polyline_first,
                merged: Box::new(done.merged),
                joint: done.joint,
            })
            .map_err(|e| Some(e.to_string())),
    )
}

pub(super) fn corner_polyline(
    path: &opendraft_core::Path,
    how: Round,
    only: Option<usize>,
) -> Result<Plan, Option<String>> {
    let done = round_polyline(path, how, only).map_err(|e| Some(e.to_string()))?;
    Ok(Plan::Replace(done.pieces))
}

/// The Polyline option: pick one polyline and round every corner that fits.
/// Returns whether something was done.
pub(super) fn round_whole_polyline(ed: &Editor, kind: Kind) -> Result<bool, Cancelled> {
    let picked = match ed.ask_object(Options::<Picked>::new(key!("Select polyline"))) {
        Ok(Answer::Value(picked)) => picked,
        Err(Cancelled) | Ok(_) => return Ok(false),
    };
    let id = picked.id;
    let read = ed.read(move |doc| doc.object(id).map(|o| (o.entity.clone(), o.attrs.clone())))?;
    let Some((entity, attrs)) = read else {
        return Ok(false);
    };
    let Some(path) = entity.polyline_path() else {
        ed.say(key!("That is not a polyline."));
        return Ok(false);
    };
    let m = Memory::read(ed)?;
    let how = match kind {
        Kind::Fillet => Round::Fillet(m.radius),
        Kind::Chamfer => Round::Chamfer(m.chamfer()),
    };
    let done = match round_polyline(&path, how, None) {
        Ok(done) => done,
        Err(e) => {
            ed.say(e.to_string());
            return Ok(false);
        }
    };
    if done.done == 0 {
        ed.say(key!("No corner could be done."));
        return Ok(false);
    }
    let corners = |count| Wording::count(count, key!("%{count} corner"), key!("%{count} corners"));
    let said = if done.skipped > 0 {
        Wording::new(
            key!("%{done} done, %{skipped} left sharp (segments too short)."),
            &[],
        )
        .part("skipped", corners(done.skipped))
    } else {
        Wording::new(key!("%{done} done."), &[])
    };
    ed.say(said.part("done", corners(done.done)));
    apply(ed, kind, id, id, attrs, Plan::Replace(done.pieces))
}
