//! JOIN: turn lines, arcs and open polylines that meet end to end into one
//! object, and elliptical arcs of one ellipse into one arc or the whole
//! ellipse. Lines that are all on one line join even across gaps or where
//! they overlap, and so do arcs of one circle. The Fuzz option lets ends that
//! miss by a distance join, and Close makes a lone arc a circle or closes a
//! polyline. The geometry is `opendraft_core::join`.

use opendraft_commands::{Answer, Cancelled, Editor, NumberOptions, Options};
use opendraft_core::{closed, join, Entity, EntityId};
use opendraft_ui::key;

use opendraft_commands::Wording;

/// Ends closer than this count as meeting.
const GAP: f64 = 1e-6;

/// JOIN: select the objects (or use those already selected), and whatever
/// meets end to end becomes one object. The first of each group keeps its
/// layer and place in the drawing order; the rest are removed. One undo step.
/// At the selection prompt, Fuzz sets how far apart ends may be and still
/// join. A single arc, elliptical arc or open polyline offers Close.
pub fn join_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let (chosen, fuzz) = choose(ed)?;
    let chosen = match chosen.as_slice() {
        [one] => match close_or_more(ed, *one)? {
            Chosen::Closed => return Ok(()),
            Chosen::These(more) => more,
        },
        _ => chosen,
    };
    let ids = chosen.clone();
    let found = ed.read(move |doc| -> Vec<(EntityId, Entity)> {
        ids.iter()
            .filter_map(|id| doc.object(*id).map(|o| (*id, o.entity.clone())))
            .collect()
    })?;
    let entities: Vec<Entity> = found.iter().map(|(_, e)| e.clone()).collect();
    // Nothing changes when any is refused, so the selection stays as it was.
    // Splines join only with splines.
    let others = |e: &Entity| {
        matches!(
            e,
            Entity::Line { .. }
                | Entity::Arc { .. }
                | Entity::Polyline { .. }
                | Entity::Ellipse(_)
                | Entity::Rectangle { .. }
        )
    };
    if entities.iter().any(|e| matches!(e, Entity::Spline(_))) && entities.iter().any(others) {
        ed.say(key!("JOIN does not support splines yet."));
        return Ok(());
    }
    let merged = join(&entities, GAP.max(fuzz));
    if merged.is_empty() {
        ed.say(key!("Nothing to join: the objects do not meet end to end."));
        return Ok(());
    }
    let gone: usize = merged.iter().map(|m| m.used.len()).sum();
    let groups = merged.len();
    ed.transaction("Join", move |tx| {
        for m in merged {
            let first = found[m.used[0]].0;
            let entity = m.entity;
            tx.modify(first, move |_| entity);
            let rest: Vec<EntityId> = m.used[1..].iter().map(|i| found[*i].0).collect();
            if !rest.is_empty() {
                tx.remove(rest);
            }
        }
        Ok(())
    })?;
    let said = Wording::new(key!("Joined %{objects} into %{into}."), &[]);
    let said = said.part("objects", crate::text::objects(gone));
    ed.say(said.part("into", crate::text::objects(groups)));
    Ok(())
}

/// The objects to join, and the Fuzz distance (0 for none). Selected objects
/// are used as they are; otherwise the Fuzz option is offered with the prompt.
fn choose(ed: &mut Editor) -> Result<(Vec<EntityId>, f64), Cancelled> {
    let mut fuzz = 0.0;
    if !ed.pickset().is_empty() {
        return Ok((ed.pickset().to_vec(), fuzz));
    }
    loop {
        let options = Options::<()>::new(key!("Select objects to join")).keyword("Fuzz", "F");
        match ed.ask_selection_or_keyword(options, "")? {
            Answer::Value(ids) => return Ok((ids, fuzz)),
            Answer::Keyword(_) => {
                let options = NumberOptions::new(key!("Specify fuzz distance")).default(fuzz);
                fuzz = ed.get_distance(options)?.max(0.0);
            }
            Answer::Enter => return Err(Cancelled),
        }
    }
}

/// What is left to join once a single object is chosen.
enum Chosen {
    /// It was made whole (Close), and the command is done.
    Closed,
    /// These are all the objects, the first one included.
    These(Vec<EntityId>),
}

/// With one object that could be closed (an arc, an elliptical arc or an open
/// polyline), ask for more to join to it, or Close.
fn close_or_more(ed: &Editor, one: EntityId) -> Result<Chosen, Cancelled> {
    let whole = ed.read(move |doc| doc.object(one).and_then(|o| closed(&o.entity)))?;
    let Some(whole) = whole else {
        return Ok(Chosen::These(vec![one]));
    };
    let options = Options::<()>::new(key!("Select objects to join to the source"))
        .keyword("cLose", "L")
        .allow_empty();
    match ed.ask_selection_or_keyword(options, "")? {
        Answer::Value(more) if !more.is_empty() => {
            let mut all = vec![one];
            all.extend(more.into_iter().filter(|id| *id != one));
            Ok(Chosen::These(all))
        }
        Answer::Keyword(_) => {
            ed.transaction("Join", move |tx| {
                tx.modify(one, move |_| whole);
                Ok(())
            })?;
            ed.say(key!("Closed the object."));
            Ok(Chosen::Closed)
        }
        _ => Ok(Chosen::These(vec![one])),
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::state::AppState;
    use opendraft_core::Point;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn joining(items: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in items {
            state.add_entity(e.clone());
        }
        state.select_all();
        state.start_command("Join", join_objects);
        state.settle_command();
        state
    }

    #[test]
    fn selected_lines_that_meet_become_one_polyline_in_one_undo_step() {
        let mut state = joining(&[
            line(p(0.0, 0.0), p(5.0, 0.0)),
            line(p(5.0, 0.0), p(5.0, 5.0)),
            line(p(20.0, 20.0), p(21.0, 20.0)),
        ]);
        let now = entities(&state);
        assert_eq!(now.len(), 2);
        assert!(matches!(&now[0], Entity::Polyline { points, .. } if points.len() == 3));
        assert_eq!(now[1], line(p(20.0, 20.0), p(21.0, 20.0)));
        state.undo();
        assert_eq!(entities(&state).len(), 3);
    }

    #[test]
    fn elliptical_arcs_join_among_themselves_and_lines_among_theirs() {
        let half = |start: f64| {
            let e = opendraft_core::Ellipse::new(p(0.0, 0.0), p(4.0, 0.0), 0.5, start, start + PI);
            Entity::Ellipse(e.unwrap())
        };
        let items = [
            line(p(0.0, 0.0), p(1.0, 0.0)),
            half(0.0),
            line(p(1.0, 0.0), p(1.0, 1.0)),
            half(PI),
        ];
        let state = joining(&items);
        let all = entities(&state);
        assert_eq!(all.len(), 2, "{all:?}");
        assert!(matches!(all[0], Entity::Polyline { .. }));
        assert!(
            matches!(all[1], Entity::Ellipse(e) if e.is_full()),
            "two halves close it"
        );
    }

    #[test]
    fn a_selection_with_a_spline_is_refused_whole() {
        let items = [
            line(p(-5.0, 0.0), p(-6.0, 0.0)),
            line(p(-6.0, 0.0), p(-6.0, 1.0)),
            Entity::Spline(Box::new(
                opendraft_core::Spline::from_control_points(1, vec![p(-5.0, 0.0), p(5.0, 0.0)])
                    .unwrap(),
            )),
        ];
        let state = joining(&items);
        assert_eq!(entities(&state), items, "the lines are not joined either");
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("JOIN does not support splines yet.")));
    }

    #[test]
    fn nothing_meeting_changes_nothing() {
        let state = joining(&[
            line(p(0.0, 0.0), p(1.0, 0.0)),
            line(p(5.0, 0.0), p(5.0, 1.0)),
        ]);
        assert_eq!(entities(&state).len(), 2);
    }

    #[test]
    fn lines_on_one_line_join_across_a_gap_in_one_undo_step() {
        let mut state = joining(&[
            line(p(0.0, 0.0), p(4.0, 0.0)),
            line(p(6.0, 0.0), p(9.0, 0.0)),
        ]);
        assert_eq!(entities(&state), vec![line(p(0.0, 0.0), p(9.0, 0.0))]);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("Joined 2 objects into 1 object.")));
        state.undo();
        assert_eq!(entities(&state).len(), 2);
    }

    /// Start JOIN with nothing selected, so its selection prompt is asked.
    fn asking(items: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in items {
            state.add_entity(e.clone());
        }
        state.start_command("Join", join_objects);
        state.settle_command();
        state
    }

    fn pick(state: &mut AppState, indexes: &[usize]) {
        let doc = state.session().document();
        let ids: Vec<EntityId> = indexes.iter().map(|i| doc.objects()[*i].id).collect();
        state.select_ids(ids, false);
        state.answer_enter();
        state.settle_command();
    }

    #[test]
    fn fuzz_lets_ends_that_miss_join_and_without_it_they_do_not() {
        let items = [
            line(p(0.0, 0.0), p(10.0, 0.0)),
            line(p(10.0, 0.5), p(10.0, 10.0)),
        ];
        let mut state = asking(&items);
        pick(&mut state, &[0, 1]);
        assert_eq!(entities(&state), items, "no fuzz, no join");
        let mut state = asking(&items);
        state.cmd_submit("f");
        state.settle_command();
        assert!(state.cmd_prompt().text.starts_with("Specify fuzz distance"));
        state.cmd_submit("1");
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select objects to join"));
        pick(&mut state, &[0, 1]);
        let now = entities(&state);
        assert_eq!(now.len(), 1);
        assert!(matches!(&now[0], Entity::Polyline { points, .. } if points.len() == 4));
    }

    fn quarter(start: f64) -> Entity {
        Entity::Arc {
            center: p(0.0, 0.0),
            radius: 5.0,
            start,
            end: start + PI / 2.0,
        }
    }

    #[test]
    fn arcs_of_one_circle_join_across_a_gap() {
        let state = joining(&[quarter(0.0), quarter(PI)]);
        let now = entities(&state);
        assert_eq!(now.len(), 1);
        assert!(matches!(now[0], Entity::Arc { .. }));
    }

    #[test]
    fn close_makes_a_lone_arc_a_circle_in_one_undo_step() {
        let mut state = asking(&[quarter(0.0)]);
        pick(&mut state, &[0]);
        assert!(state
            .cmd_prompt()
            .options
            .iter()
            .any(|o| o.label == "cLose"));
        state.cmd_submit("l");
        state.settle_command();
        assert!(!state.command_running());
        assert!(matches!(entities(&state)[0], Entity::Circle { .. }));
        state.undo();
        assert!(matches!(entities(&state)[0], Entity::Arc { .. }));
    }

    #[test]
    fn a_lone_arc_with_nothing_more_to_join_changes_nothing() {
        let mut state = asking(&[quarter(0.0)]);
        pick(&mut state, &[0]);
        state.answer_enter();
        state.settle_command();
        assert!(!state.command_running());
        assert!(matches!(entities(&state)[0], Entity::Arc { .. }));
    }

    #[test]
    fn a_lone_arc_takes_more_objects_chosen_at_the_close_prompt() {
        let mut state = asking(&[quarter(0.0), quarter(PI)]);
        pick(&mut state, &[0]);
        pick(&mut state, &[1]);
        let now = entities(&state);
        assert_eq!(now.len(), 1);
        assert!(matches!(now[0], Entity::Arc { .. }));
    }

    fn span(from: f64) -> Entity {
        Entity::Spline(Box::new(
            opendraft_core::Spline::from_control_points(
                3,
                vec![
                    p(from, 0.0),
                    p(from + 2.0, 3.0),
                    p(from + 4.0, 3.0),
                    p(from + 6.0, 0.0),
                ],
            )
            .unwrap(),
        ))
    }

    #[test]
    fn splines_that_meet_end_to_end_become_one_spline_in_one_undo_step() {
        let mut state = joining(&[span(0.0), span(6.0), span(30.0)]);
        let now = entities(&state);
        assert_eq!(now.len(), 2);
        let Entity::Spline(joined) = &now[0] else {
            panic!("{now:?}")
        };
        assert!(joined.start_point().x.abs() < 1e-9 && (joined.end_point().x - 12.0).abs() < 1e-9);
        assert_eq!(now[1], span(30.0));
        state.undo();
        assert_eq!(entities(&state).len(), 3);
    }

    #[test]
    fn a_spline_beside_a_line_is_still_refused() {
        let state = joining(&[span(0.0), line(p(6.0, 0.0), p(9.0, 0.0))]);
        assert_eq!(entities(&state).len(), 2);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("JOIN does not support splines yet.")));
    }
}
