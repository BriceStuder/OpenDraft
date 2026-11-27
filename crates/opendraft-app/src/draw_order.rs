//! DRAWORDER: change which objects are drawn over which. The four choices
//! (Front, Back, Above objects, Under objects) are commands of their own for
//! the context menu, and DRAWORDER asks which. The change itself is
//! `ReorderObjects` in `opendraft_commands`.

use opendraft_commands::{
    Answer, Cancelled, Command, Editor, Options, Picked, Place, ReorderObjects,
};
use opendraft_ui::key;

use crate::state::AppState;
use opendraft_commands::Wording;

/// The commands, for the command table.
pub fn commands() -> Vec<Command<AppState>> {
    vec![
        Command::new("modify.draw_order", "Draw Order", |s: &mut AppState| {
            s.start_command("DRAWORDER", |ed| draw_order(ed, None))
        })
        .checked(|s| s.is_running("DRAWORDER"))
        .transparent(),
        choice!("modify.draw_front", "Bring to Front", Choice::Front),
        choice!("modify.draw_back", "Send to Back", Choice::Back),
        choice!("modify.draw_above", "Bring Above Objects", Choice::Above),
        choice!("modify.draw_under", "Send Under Objects", Choice::Under),
    ]
}

/// A command that does one choice of DRAWORDER without asking which.
macro_rules! choice {
    ($id:literal, $label:literal, $how:expr) => {
        Command::new($id, $label, |s: &mut AppState| {
            s.start_command("DRAWORDER", |ed| draw_order(ed, Some($how)))
        })
        .checked(|s| s.is_running("DRAWORDER"))
        .transparent()
    };
}
use choice;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Front,
    Back,
    Above,
    Under,
}

/// DRAWORDER: select the objects (or use those selected), then choose where
/// they go: over or under a reference object, or to the front or back of all.
/// One undo step.
fn draw_order(ed: &mut Editor, asked: Option<Choice>) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select(key!("Select objects"))?;
    let how = match asked {
        Some(how) => how,
        None => {
            let options = Options::<String>::new(key!("Enter object ordering option"))
                .keyword("Above objects", "A")
                .keyword("Under objects", "U")
                .keyword("Front", "F")
                .keyword("Back", "B")
                .default("Back".to_owned());
            match ed.get_keyword(options)?.as_str() {
                "Above objects" => Choice::Above,
                "Under objects" => Choice::Under,
                "Front" => Choice::Front,
                _ => Choice::Back,
            }
        }
    };
    let place = match how {
        Choice::Front => Place::Front,
        Choice::Back => Place::Back,
        Choice::Above | Choice::Under => {
            let options = Options::<Picked>::new(key!("Select reference object"));
            let Answer::Value(Picked { id, .. }) = ed.ask_object(options)? else {
                return Ok(());
            };
            if how == Choice::Above {
                Place::Above(id)
            } else {
                Place::Under(id)
            }
        }
    };
    let count = ids.len();
    ed.transaction("Draw Order", move |tx| {
        tx.perform(ReorderObjects::new(ids, place));
        Ok(())
    })?;
    ed.say(
        Wording::new(key!("Draw order changed for %{objects}"), &[])
            .part("objects", crate::text::objects(count)),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use opendraft_core::{Entity, EntityId, Point};

    use super::*;

    fn drawing(n: usize) -> (AppState, Vec<EntityId>) {
        let mut state = AppState::new();
        for i in 0..n {
            state.add_entity(Entity::Point {
                at: Point::new(i as f64, 0.0),
            });
        }
        let ids = order(&state);
        (state, ids)
    }

    fn order(state: &AppState) -> Vec<EntityId> {
        state
            .session()
            .document()
            .objects()
            .iter()
            .map(|o| o.id)
            .collect()
    }

    #[test]
    fn the_selection_goes_to_the_front_in_one_undo_step() {
        let (mut state, ids) = drawing(4);
        state.select_ids(vec![ids[0], ids[2]], false);
        state.start_command("DRAWORDER", |ed| draw_order(ed, Some(Choice::Front)));
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(order(&state), [ids[1], ids[3], ids[0], ids[2]]);
        state.undo();
        assert_eq!(order(&state), ids);
    }

    #[test]
    fn drawing_order_asks_which_and_back_is_the_default() {
        let (mut state, ids) = drawing(3);
        state.select_ids(vec![ids[2]], false);
        state.start_command("DRAWORDER", |ed| draw_order(ed, None));
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Enter object ordering option"));
        state.cmd_submit("");
        state.settle_command();
        assert_eq!(order(&state), [ids[2], ids[0], ids[1]]);
    }

    #[test]
    fn above_asks_for_a_reference_object() {
        let (mut state, ids) = drawing(4);
        state.select_ids(vec![ids[0]], false);
        state.start_command("DRAWORDER", |ed| draw_order(ed, Some(Choice::Above)));
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select reference object"));
        state.answer_object(Point::new(2.0, 0.0), Some(ids[2]), false);
        state.settle_command();
        assert_eq!(order(&state), [ids[1], ids[2], ids[0], ids[3]]);
    }
}
