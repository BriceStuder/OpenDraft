//! The smaller options of TRIM and EXTEND's object prompt: Edge, Project,
//! Crossing and eRase. The prompt loop is in `edges.rs`.

use opendraft_canvas::Region;
use opendraft_commands::{Answer, Cancelled, Editor, Options, PointOptions};
use opendraft_ui::key;

use crate::defaults::EditorDefaults;
use crate::edges_sweep::ask_window;
use opendraft_commands::Wording;

/// The Edge option: whether edges act as if extended past their ends, the
/// current choice by default. The choice is remembered (`EDGEMODE`).
pub(crate) fn switch_edge(ed: &Editor) -> Result<(), Cancelled> {
    let extend = ed.defaults().trim.get().edge_extend;
    let options = Options::<String>::new(key!("Enter an implied edge extension mode"))
        .keyword("Extend", "E")
        .keyword("No extend", "N")
        .default(if extend { "Extend" } else { "No extend" }.to_owned());
    let now = match ed.ask_keyword(options)?.as_deref() {
        Some("Extend") => true,
        Some("No extend") => false,
        _ => extend,
    };
    ed.defaults().trim.change(|m| m.edge_extend = now);
    Ok(())
}

/// The Project option: how objects are projected onto the plane they are
/// trimmed or extended in, remembered as `PROJMODE` (0 None, 1 Ucs, 2 View).
/// Everything here is flat (the coordinate system and the view only turn
/// within the one plane), so the three choices always give the same result;
/// the option is kept so scripts and the variable agree with AutoCAD, and it
/// says so.
pub(crate) fn switch_project(ed: &Editor) -> Result<(), Cancelled> {
    let names = ["None", "Ucs", "View"];
    let current = ed.defaults().trim.get().project.min(2);
    let options = Options::<String>::new(key!("Enter a projection option"))
        .keyword("None", "N")
        .keyword("Ucs", "U")
        .keyword("View", "V")
        .default(names[usize::from(current)].to_owned());
    let chosen = ed.ask_keyword(options)?;
    if let Some(position) = names.iter().position(|n| Some(*n) == chosen.as_deref()) {
        let project = u8::try_from(position).unwrap_or(current);
        ed.defaults().trim.change(|m| m.project = project);
        ed.say(key!(
            "All objects here are flat, so projection makes no difference."
        ))
    }
    Ok(())
}

/// The Crossing option, typed: two corners of a window that touches what it
/// crosses (a click in empty space starts the same window). `None` when the
/// first corner was left unanswered with Enter.
pub(crate) fn ask_crossing(ed: &Editor) -> Result<Option<Region>, Cancelled> {
    let first = PointOptions::new(key!("Specify first crossing corner"));
    let Answer::Value(first) = ed.ask_point(first)? else {
        return Ok(None);
    };
    Ok(Some(Region::Crossing(ask_window(ed, first)?)))
}

/// The eRase option: select objects and erase them without leaving the
/// command. Returns whether anything was erased.
pub(crate) fn erase_some(ed: &Editor) -> Result<bool, Cancelled> {
    let options = Options::<()>::new(key!("Select objects to erase")).allow_empty();
    let ids = ed.ask_selection(options)?;
    if ids.is_empty() {
        return Ok(false);
    }
    let count = ids.len();
    ed.transaction("Erase", move |tx| {
        tx.remove(ids);
        Ok(())
    })?;
    ed.say(
        Wording::new(key!("Erased %{objects}"), &[]).part("objects", crate::text::objects(count)),
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use opendraft_core::{Entity, Point};

    use crate::edges::trim_objects;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn trimming(entities: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in entities {
            state.add_entity(e.clone());
        }
        state.start_command("Trim", trim_objects);
        state.settle_command();
        state
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn offers(state: &mut AppState, label: &str) -> bool {
        state.cmd_prompt().options.iter().any(|o| o.label == label)
    }

    #[test]
    fn the_new_options_are_offered_at_the_object_prompt() {
        let mut state = trimming(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        for label in ["Fence", "Crossing", "mOde", "Project", "Edge", "eRase"] {
            assert!(offers(&mut state, label), "{label}");
        }
    }

    #[test]
    fn a_typed_crossing_trims_what_the_window_touches() {
        let mut state = trimming(&[
            line(p(0.0, 0.0), p(10.0, 0.0)),
            line(p(4.0, -5.0), p(4.0, 5.0)),
        ]);
        state.cmd_submit("c");
        state.settle_command();
        state.answer_point(p(6.0, 1.0));
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Specify opposite corner"));
        state.answer_point(p(8.0, -1.0));
        state.settle_command();
        assert_eq!(entities(&state)[0], line(p(0.0, 0.0), p(4.0, 0.0)));
        assert!(state.command_running(), "still at the object prompt");
    }

    #[test]
    fn erase_removes_the_selected_objects_and_stays_in_the_command() {
        let mut state = trimming(&[
            line(p(0.0, 0.0), p(10.0, 0.0)),
            line(p(0.0, 5.0), p(10.0, 5.0)),
        ]);
        state.cmd_submit("r");
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select objects to erase"));
        state.select_all();
        state.cmd_submit("");
        state.settle_command();
        assert!(entities(&state).is_empty());
        assert!(state.command_running());
        // One undo step.
        state.undo();
        assert_eq!(entities(&state).len(), 2);
    }

    #[test]
    fn project_is_remembered_as_projmode_and_enter_keeps_it() {
        let mut state = trimming(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        assert_eq!(state.defaults().trim.get().project, 1);
        state.cmd_submit("p");
        state.settle_command();
        state.cmd_submit("v");
        state.settle_command();
        assert_eq!(state.defaults().trim.get().project, 2);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("projection makes no difference")));
        state.cmd_submit("p");
        state.settle_command();
        state.cmd_submit("");
        state.settle_command();
        assert_eq!(state.defaults().trim.get().project, 2);
        let table = crate::sysvars::table();
        assert_eq!(
            table.get("PROJMODE", &state),
            Ok(opendraft_commands::Value::Int(2))
        );
        assert!(table.set("PROJMODE", "3", &mut state).is_err());
    }
}
