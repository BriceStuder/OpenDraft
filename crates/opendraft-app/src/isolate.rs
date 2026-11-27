//! Object isolation: ISOLATEOBJECTS hides every object but the ones chosen,
//! HIDEOBJECTS hides the ones chosen, and UNISOLATEOBJECTS shows everything
//! again. Hidden objects are neither drawn, picked, snapped to nor plotted,
//! but stay in the drawing; hiding is not an edit, so it is not an undo step
//! and does not mark the drawing changed. The hidden set is
//! `Document::hidden_objects`.

use opendraft_commands::{Cancelled, Command, Editor};
use opendraft_core::EntityId;
use opendraft_ui::key;

use crate::state::AppState;
use opendraft_commands::Wording;

/// The commands, for the command table.
pub fn commands() -> Vec<Command<AppState>> {
    vec![
        Command::new("isolate.objects", "Isolate Objects", |s: &mut AppState| {
            s.start_command("ISOLATEOBJECTS", isolate_objects)
        })
        .checked(|s| s.is_running("ISOLATEOBJECTS"))
        .transparent(),
        Command::new("isolate.hide", "Hide Objects", |s: &mut AppState| {
            s.start_command("HIDEOBJECTS", hide_objects)
        })
        .checked(|s| s.is_running("HIDEOBJECTS"))
        .transparent(),
        Command::new("isolate.end", "End Object Isolation", |s: &mut AppState| {
            s.start_command("UNISOLATEOBJECTS", end_isolation)
        })
        .checked(|s| s.is_running("UNISOLATEOBJECTS"))
        .transparent(),
    ]
}

/// What is hidden now, and what is shown in the space being edited.
fn state(ed: &Editor) -> Result<(Vec<EntityId>, Vec<EntityId>), Cancelled> {
    ed.read(|doc| {
        (
            doc.hidden_objects().to_vec(),
            doc.visible_objects().map(|o| o.id).collect(),
        )
    })
}

fn say_hidden(ed: &Editor, count: usize) {
    ed.say(
        Wording::new(key!("%{objects} hidden"), &[]).part("objects", crate::text::objects(count)),
    );
}

/// ISOLATEOBJECTS: hide everything shown but the chosen objects.
fn isolate_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let keep = ed.pickset_or_select(key!("Select objects to isolate"))?;
    let (mut hidden, shown) = state(ed)?;
    let others: Vec<EntityId> = shown.into_iter().filter(|id| !keep.contains(id)).collect();
    let count = others.len();
    hidden.extend(others);
    ed.set_hidden_objects(hidden)?;
    say_hidden(ed, count);
    Ok(())
}

/// HIDEOBJECTS: hide the chosen objects.
fn hide_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let chosen = ed.pickset_or_select(key!("Select objects to hide"))?;
    let (mut hidden, _) = state(ed)?;
    let count = chosen.len();
    hidden.extend(chosen.iter().copied());
    ed.set_hidden_objects(hidden)?;
    ed.deselect(chosen);
    say_hidden(ed, count);
    Ok(())
}

/// UNISOLATEOBJECTS: show everything again.
fn end_isolation(ed: &mut Editor) -> Result<(), Cancelled> {
    let (hidden, _) = state(ed)?;
    if hidden.is_empty() {
        ed.say(key!("No objects are hidden."));
        return Ok(());
    }
    let count = hidden.len();
    ed.set_hidden_objects(Vec::new())?;
    ed.say(
        Wording::new(key!("%{objects} shown"), &[]).part("objects", crate::text::objects(count)),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use opendraft_core::{Entity, Point};

    use super::*;

    fn drawing(n: usize) -> (AppState, Vec<EntityId>) {
        let mut state = AppState::new();
        for i in 0..n {
            state.add_entity(Entity::Line {
                start: Point::new(0.0, i as f64),
                end: Point::new(10.0, i as f64),
            });
        }
        let ids = state
            .session()
            .document()
            .objects()
            .iter()
            .map(|o| o.id)
            .collect();
        (state, ids)
    }

    fn shown(state: &AppState) -> Vec<EntityId> {
        state
            .session()
            .document()
            .visible_objects()
            .map(|o| o.id)
            .collect()
    }

    #[test]
    fn isolating_hides_everything_else_and_ending_shows_it_again() {
        let (mut state, ids) = drawing(4);
        state.select_ids(vec![ids[1]], false);
        state.start_command("ISOLATEOBJECTS", isolate_objects);
        state.settle_command();
        assert_eq!(shown(&state), [ids[1]]);
        // Hidden objects are still in the drawing, but cannot be picked.
        assert_eq!(state.session().document().objects().len(), 4);
        assert!(!state
            .session()
            .document()
            .is_selectable(state.session().document().object(ids[0]).unwrap()));
        state.start_command("UNISOLATEOBJECTS", end_isolation);
        state.settle_command();
        assert_eq!(shown(&state), ids);
    }

    #[test]
    fn hiding_hides_the_chosen_objects_and_isolating_after_adds_to_them() {
        let (mut state, ids) = drawing(4);
        state.select_ids(vec![ids[0]], false);
        state.start_command("HIDEOBJECTS", hide_objects);
        state.settle_command();
        assert_eq!(shown(&state), [ids[1], ids[2], ids[3]]);
        assert!(
            state.selection().is_empty(),
            "hidden objects are deselected"
        );
        state.select_ids(vec![ids[3]], false);
        state.start_command("ISOLATEOBJECTS", isolate_objects);
        state.settle_command();
        assert_eq!(shown(&state), [ids[3]]);
    }

    #[test]
    fn hiding_is_not_an_undo_step_and_does_not_dirty_the_drawing() {
        let (mut state, ids) = drawing(2);
        let modified = state.session().is_modified();
        state.select_ids(vec![ids[0]], false);
        state.start_command("HIDEOBJECTS", hide_objects);
        state.settle_command();
        assert_eq!(state.session().is_modified(), modified);
        state.undo();
        assert_eq!(
            state.session().document().objects().len(),
            1,
            "undo took the last line"
        );
        assert!(state.session().document().is_hidden(ids[0]));
    }

    #[test]
    fn ending_with_nothing_hidden_says_so() {
        let (mut state, _) = drawing(1);
        state.start_command("UNISOLATEOBJECTS", end_isolation);
        state.settle_command();
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("No objects are hidden.")));
    }
}
