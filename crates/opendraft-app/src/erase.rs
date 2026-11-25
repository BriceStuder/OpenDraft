//! ERASE: remove objects. Like the other editing commands it takes the
//! selection when there is one, and otherwise asks for objects until Enter.

use opendraft_commands::{Cancelled, Command, Editor};
use opendraft_ui::key;

use crate::state::AppState;
use opendraft_commands::Wording;

/// The ERASE command for the command table. A viewport picked on the sheet
/// is erased at once; otherwise the selection, or the objects picked at
/// key!("Select objects").
pub fn command() -> Command<AppState> {
    Command::new("modify.erase", "Erase", |s: &mut AppState| {
        if !s.erase_viewport() {
            s.start_command("ERASE", erase_objects);
        }
    })
    .key("Delete")
    .checked(|s| s.is_running("ERASE"))
}

/// Whether a keyboard shortcut may fire now. Delete only erases what is
/// already selected, as in AutoCAD: with nothing selected it does nothing,
/// where typing E asks for objects.
pub fn may_fire(id: &str, s: &AppState) -> bool {
    match id {
        "view.dynamic_input" => false,
        "modify.erase" => !s.selection().is_empty() || s.viewport_picked(),
        _ => true,
    }
}

fn erase_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select(key!("Select objects"))?;
    let count = ids.len();
    ed.transaction("Erase", move |tx| {
        tx.remove(ids);
        Ok(())
    })?;
    ed.say(
        Wording::new(key!("Erased %{objects}"), &[]).part("objects", crate::text::objects(count)),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::commands::build_registry;
    use crate::state::AppState;
    use opendraft_core::{Entity, Point};

    fn line(y: f64) -> Entity {
        Entity::Line {
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
        }
    }

    fn drawing(n: usize) -> AppState {
        let mut state = AppState::new();
        for i in 0..n {
            state.perform(opendraft_commands::AddEntity::new(line(i as f64)));
        }
        state
    }

    #[test]
    fn erase_without_a_selection_asks_for_objects_until_enter() {
        let registry = build_registry();
        let mut state = drawing(3);
        assert!(registry.is_enabled("modify.erase", &state));
        registry.execute("modify.erase", &mut state).unwrap();
        state.settle_command();
        assert!(state.command_running());
        assert!(state.cmd_prompt().text.starts_with("Select objects"));
        let doc = state.session().document();
        let (first, third) = (doc.objects()[0].id, doc.objects()[2].id);
        state.select_ids(vec![first, third], false);
        state.answer_enter();
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(state.session().document().len(), 1);
        assert_eq!(
            state.cmdline.log.last().map(String::as_str),
            Some("Erased 2 objects")
        );
        // One undo step brings both back.
        registry.execute("edit.undo", &mut state).unwrap();
        assert_eq!(state.session().document().len(), 3);
    }

    #[test]
    fn erase_with_a_selection_removes_it_at_once() {
        let registry = build_registry();
        let mut state = drawing(2);
        registry.execute("edit.select_all", &mut state).unwrap();
        registry.execute("modify.erase", &mut state).unwrap();
        state.settle_command();
        assert!(!state.command_running());
        assert!(state.session().document().is_empty());
    }

    #[test]
    fn enter_with_nothing_chosen_keeps_asking() {
        let registry = build_registry();
        let mut state = drawing(1);
        registry.execute("modify.erase", &mut state).unwrap();
        state.settle_command();
        state.answer_enter();
        state.settle_command();
        assert!(state.command_running(), "as MOVE and COPY do");
        assert_eq!(state.session().document().len(), 1);
    }

    #[test]
    fn the_delete_key_needs_a_selection() {
        let mut state = drawing(1);
        assert!(!super::may_fire("modify.erase", &state));
        assert!(!super::may_fire("view.dynamic_input", &state));
        assert!(super::may_fire("edit.undo", &state));
        state.select_all();
        assert!(super::may_fire("modify.erase", &state));
    }
}
