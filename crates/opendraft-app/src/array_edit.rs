//! ARRAYEDIT: change an associative array as a whole. Pick any object of it,
//! its source or one of its items, and the same options as when it was made
//! are offered, starting from the settings it has; on Enter the items are laid
//! out again (the old items are replaced, the sources stay), as one undo step.
//! The settings are the array's record (`opendraft_core::ArrayRecord`).

use opendraft_commands::{Answer, Cancelled, Editor, Options, Picked};
use opendraft_core::ArrayShape;
use opendraft_ui::key;

use crate::array_cmd::{polar_with, rect_with};
use crate::array_path::path_with;

/// ARRAYEDIT: pick an object of an array (or have one selected).
pub fn array_edit(ed: &mut Editor) -> Result<(), Cancelled> {
    let id = match ed.pickset() {
        [one] => *one,
        _ => {
            let options = Options::<Picked>::new(key!("Select array")).allow_empty();
            match ed.ask_object(options)? {
                Answer::Value(Picked { id, .. }) => id,
                _ => return Ok(()),
            }
        }
    };
    let found = ed.read(move |doc| {
        doc.array_holding(id).cloned().map(|record| {
            let alive = record.sources.iter().any(|s| doc.object(*s).is_some());
            (record, alive)
        })
    })?;
    let Some((record, alive)) = found else {
        ed.say(key!("That object is not part of an associative array."));
        return Ok(());
    };
    if !alive {
        ed.say(key!("The source objects of the array are gone."));
        return Ok(());
    }
    let sources = record.sources.clone();
    match record.shape {
        ArrayShape::Rectangular { .. } => rect_with(ed, sources, Some(record)),
        ArrayShape::Polar { .. } => polar_with(ed, sources, Some(record)),
        ArrayShape::Path { .. } => path_with(ed, sources, Some(record)),
    }
}

#[cfg(test)]
mod tests {
    use opendraft_commands::Reply;
    use opendraft_core::{Entity, EntityId, Point};

    use super::*;
    use crate::array_cmd::array_rect;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn square() -> Entity {
        Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(2.0, 2.0),
        }
    }

    fn ids(state: &AppState) -> Vec<EntityId> {
        state
            .session()
            .document()
            .objects()
            .iter()
            .map(|o| o.id)
            .collect()
    }

    fn reply(state: &mut AppState, replies: &[Reply]) {
        for r in replies {
            state.reply(r.clone());
            state.settle_command();
        }
    }

    /// A square arrayed 3 across and 2 up, 5 apart.
    fn arrayed() -> AppState {
        let mut state = AppState::new();
        state.add_entity(square());
        state.select_all();
        state.start_command("ARRAYRECT", array_rect);
        state.settle_command();
        reply(
            &mut state,
            &[
                Reply::Keyword("COUnt".into()),
                Reply::Number(3.0),
                Reply::Number(2.0),
                Reply::Keyword("Spacing".into()),
                Reply::Number(5.0),
                Reply::Number(5.0),
                Reply::Enter,
            ],
        );
        state
    }

    #[test]
    fn making_an_array_records_it_in_one_undo_step() {
        let mut state = arrayed();
        let now = ids(&state);
        assert_eq!(now.len(), 6);
        let doc = state.session().document();
        assert_eq!(doc.arrays().len(), 1);
        let record = &doc.arrays()[0];
        assert_eq!(record.sources, [now[0]]);
        assert_eq!(record.items.len(), 5);
        assert!(matches!(
            record.shape,
            ArrayShape::Rectangular {
                columns: 3,
                rows: 2,
                ..
            }
        ));
        state.undo();
        assert!(state.session().document().arrays().is_empty());
        assert_eq!(ids(&state).len(), 1);
        state.redo();
        assert_eq!(state.session().document().arrays().len(), 1);
    }

    #[test]
    fn editing_lays_the_items_out_again_and_keeps_the_sources() {
        let mut state = arrayed();
        let source = ids(&state)[0];
        // Pick one of the copies: the whole array comes up.
        let item = state.session().document().arrays()[0].items[2];
        state.start_command("ARRAYEDIT", array_edit);
        state.settle_command();
        state.answer_object(p(0.0, 0.0), Some(item), false);
        state.settle_command();
        // Starting from 3 by 2: make it 2 across and 1 up.
        reply(
            &mut state,
            &[
                Reply::Keyword("COUnt".into()),
                Reply::Number(2.0),
                Reply::Number(1.0),
                Reply::Enter,
            ],
        );
        assert!(!state.command_running());
        let now = ids(&state);
        assert_eq!(now.len(), 2, "the source and one item");
        assert!(now.contains(&source));
        let record = &state.session().document().arrays()[0];
        assert!(matches!(
            record.shape,
            ArrayShape::Rectangular {
                columns: 2,
                rows: 1,
                ..
            }
        ));
        assert_eq!(record.items.len(), 1);
        // One undo step brings the old layout back, record and all.
        state.undo();
        assert_eq!(ids(&state).len(), 6);
        assert!(matches!(
            state.session().document().arrays()[0].shape,
            ArrayShape::Rectangular {
                columns: 3,
                rows: 2,
                ..
            }
        ));
    }

    #[test]
    fn an_object_outside_any_array_is_refused_and_so_is_one_whose_sources_are_gone() {
        let mut state = AppState::new();
        state.add_entity(square());
        state.start_command("ARRAYEDIT", array_edit);
        state.settle_command();
        let id = ids(&state)[0];
        state.answer_object(p(0.0, 0.0), Some(id), false);
        state.settle_command();
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("not part of an associative array")));
        let mut state = arrayed();
        let source = ids(&state)[0];
        let item = state.session().document().arrays()[0].items[0];
        state.perform(opendraft_commands::RemoveEntities::new(vec![source]));
        state.start_command("ARRAYEDIT", array_edit);
        state.settle_command();
        state.answer_object(p(0.0, 0.0), Some(item), false);
        state.settle_command();
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("source objects of the array are gone")));
    }

    #[test]
    fn with_associativity_off_no_record_is_made() {
        let mut state = AppState::new();
        state.defaults().array_plain.change(|plain| *plain = true);
        state.add_entity(square());
        state.select_all();
        state.start_command("ARRAYRECT", array_rect);
        state.settle_command();
        reply(&mut state, &[Reply::Enter]);
        assert!(ids(&state).len() > 1);
        assert!(state.session().document().arrays().is_empty());
    }

    #[test]
    fn a_polar_array_is_edited_from_its_own_centre_without_asking_for_it_again() {
        let mut state = AppState::new();
        state.add_entity(square());
        state.select_all();
        state.start_command("ARRAYPOLAR", crate::array_cmd::array_polar);
        state.settle_command();
        state.answer_point(p(10.0, 0.0));
        state.settle_command();
        reply(&mut state, &[Reply::Enter]);
        assert_eq!(ids(&state).len(), 6);
        let item = state.session().document().arrays()[0].items[0];
        state.start_command("ARRAYEDIT", array_edit);
        state.settle_command();
        state.answer_object(p(0.0, 0.0), Some(item), false);
        state.settle_command();
        assert!(
            state
                .cmd_prompt()
                .text
                .contains("Select grip to edit array"),
            "no centre is asked for: {}",
            state.cmd_prompt().text
        );
        reply(
            &mut state,
            &[
                Reply::Keyword("Items".into()),
                Reply::Number(4.0),
                Reply::Enter,
            ],
        );
        assert_eq!(ids(&state).len(), 4);
        let ArrayShape::Polar { center, items, .. } = state.session().document().arrays()[0].shape
        else {
            panic!()
        };
        assert_eq!((center, items), (p(10.0, 0.0), 4));
    }

    #[test]
    fn a_path_array_follows_its_curve_again_and_says_when_the_curve_is_gone() {
        let mut state = AppState::new();
        state.add_entity(Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(1.0, 1.0),
        });
        state.add_entity(Entity::Line {
            start: p(0.0, 10.0),
            end: p(20.0, 10.0),
        });
        let (square, line) = {
            let o = state.session().document().objects();
            (o[0].id, o[1].id)
        };
        state.click_select(Some(square), false);
        state.start_command("ARRAYPATH", crate::array_path::array_path);
        state.settle_command();
        state.answer_object(p(1.0, 10.0), Some(line), false);
        state.settle_command();
        reply(
            &mut state,
            &[
                Reply::Keyword("Items".into()),
                Reply::Number(5.0),
                Reply::Enter,
                Reply::Enter,
            ],
        );
        let squares = |state: &AppState| {
            state
                .session()
                .document()
                .objects()
                .iter()
                .filter(|o| matches!(o.entity, Entity::Rectangle { .. }))
                .count()
        };
        assert_eq!(squares(&state), 5);
        let item = state.session().document().arrays()[0].items[0];
        state.start_command("ARRAYEDIT", array_edit);
        state.settle_command();
        state.answer_object(p(5.0, 0.0), Some(item), false);
        state.settle_command();
        reply(
            &mut state,
            &[
                Reply::Keyword("Method".into()),
                Reply::Keyword("Divide".into()),
                Reply::Keyword("Items".into()),
                Reply::Number(3.0),
                Reply::Enter,
            ],
        );
        assert_eq!(squares(&state), 3);
        // Erase the path, and the array can no longer be laid out along it.
        state.perform(opendraft_commands::RemoveEntities::new(vec![line]));
        let item = state.session().document().arrays()[0].items[0];
        state.start_command("ARRAYEDIT", array_edit);
        state.settle_command();
        state.answer_object(p(0.0, 0.0), Some(item), false);
        state.settle_command();
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("path curve of the array is gone")));
    }
}
