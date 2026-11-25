//! Turning the view. ROTATEVIEW (alias `RV`) turns the drawing on the screen
//! by an angle, by two points that are to read level, or back to none (Reset).
//! PLAN turns it so the X axis of the coordinate system in force (Current),
//! a saved one (Named) or the world's reads level, then shows the extents, as
//! AutoCAD's PLAN does. Both act on the Model tab's view or on the view of the
//! layout viewport being worked through; a layout's sheet never turns.
//! Ctrl+middle-drag does the same by hand (`opendraft_canvas`'s `twist.rs`).
//! Like ZOOM, they only ask; the view changes on the UI thread.

use opendraft_commands::{
    Answer, Cancelled, Command, Editor, NumberOptions, Options, PointOptions,
};
use opendraft_core::ccs_align::aligned_to;
use opendraft_core::{Document, Point};
use opendraft_ui::key;

use crate::state::AppState;
use crate::zoom_cmd::{start_view_command, ZoomTo};

/// Why the view of the space being worked in cannot turn, if it cannot.
fn refusal(doc: &Document) -> Option<&'static str> {
    if doc.active_layout().is_some() {
        return Some(key!(
            "A layout's sheet does not turn: work in the Model tab or inside a viewport."
        ));
    }
    let (layout, viewport) = doc.active_viewport()?;
    let locked = doc.layout(layout)?.viewport(viewport)?.locked;
    locked.then_some(key!("This viewport is locked; unlock it to turn its view."))
}

/// The view's twist, or why it cannot turn.
fn current(ed: &Editor) -> Result<Option<f64>, Cancelled> {
    let (twist, refused) = ed.read(|doc| (doc.view_twist(), refusal(doc)))?;
    if let Some(why) = refused {
        ed.say(why);
        return Ok(None);
    }
    Ok(Some(twist))
}

/// The twist that lays the line from `first` to `second` level, pointing right
/// (none when they are the same point).
fn level_twist(first: Point, second: Point) -> Option<f64> {
    if (second.x - first.x).hypot(second.y - first.y) < 1e-12 {
        return None;
    }
    Some(-(second.y - first.y).atan2(second.x - first.x))
}

/// A picked object to read level, by the rules of CCS Object: a line or a
/// polyline segment reads along itself from the end nearer the pick, a circle
/// or arc along its radius to the pick (or the nearer end).
fn object_twist(ed: &Editor) -> Result<Option<f64>, Cancelled> {
    loop {
        let picked = match ed.ask_object(Options::new(key!("Select object to read level")))? {
            Answer::Value(picked) => picked,
            _ => return Ok(None),
        };
        let found =
            ed.read(move |doc| doc.object(picked.id).map(|o| (o.entity.clone(), doc.ccs())))?;
        match found.and_then(|(entity, ccs)| aligned_to(&entity, picked.at, ccs)) {
            Some(ccs) => return Ok(Some(-ccs.angle)),
            None => ed.say(key!("That object has no direction to read level.")),
        }
    }
}

/// Two points of a line to read level, or an object (the Object keyword at
/// the first point): the twist that does it.
fn points_twist(ed: &Editor) -> Result<Option<f64>, Cancelled> {
    let options = PointOptions::new(key!("Specify first point of a line to read level"))
        .keyword("Object", "O");
    let first = match ed.ask_point(options)? {
        Answer::Value(first) => first,
        Answer::Keyword(_) => return object_twist(ed),
        Answer::Enter => return Ok(None),
    };
    let options = PointOptions::new(key!("Specify second point")).base(first);
    let second = ed.get_point(options)?;
    let twist = level_twist(first, second);
    if twist.is_none() {
        ed.say(key!("The points are the same."));
    }
    Ok(twist)
}

/// The navigation gizmo's level button: ROTATEVIEW's Points option on its own.
fn level_view(ed: &Editor) -> Result<Option<ZoomTo>, Cancelled> {
    if current(ed)?.is_none() {
        return Ok(None);
    }
    Ok(points_twist(ed)?.map(|twist| ZoomTo::Twist { twist, plan: false }))
}

/// ROTATEVIEW: how far to turn the drawing on the screen.
fn rotate_view(ed: &Editor) -> Result<Option<ZoomTo>, Cancelled> {
    let Some(twist) = current(ed)? else {
        return Ok(None);
    };
    let options = NumberOptions::new(key!("Specify view rotation angle"))
        .default(twist)
        .turn()
        .keyword("Points", "P")
        .keyword("Object", "O")
        .keyword("Reset", "R");
    let twist = match ed.ask_angle(options)? {
        Answer::Value(angle) => angle,
        Answer::Keyword(k) if k == "Object" => match object_twist(ed)? {
            Some(twist) => twist,
            None => return Ok(None),
        },
        Answer::Keyword(k) if k == "Points" => match points_twist(ed)? {
            Some(twist) => twist,
            None => return Ok(None),
        },
        Answer::Keyword(_) => 0.0,
        Answer::Enter => return Ok(None),
    };
    Ok(Some(ZoomTo::Twist { twist, plan: false }))
}

/// PLAN: the view level with a coordinate system, then the extents.
fn plan(ed: &Editor) -> Result<Option<ZoomTo>, Cancelled> {
    if current(ed)?.is_none() {
        return Ok(None);
    }
    let options = Options::<String>::new(key!("Enter an option"))
        .keyword("Current", "C")
        .keyword("Named", "N")
        .keyword("World", "W")
        .default("Current".to_owned());
    let angle = match ed.get_keyword(options)?.as_str() {
        "World" => 0.0,
        "Named" => {
            let name = match ed.ask_string(Options::<String>::new(key!(
                "Enter name of coordinate system"
            )))? {
                Answer::Value(name) => name,
                _ => return Ok(None),
            };
            let named = ed.read(move |doc| doc.named_ccs_called(&name).map(|n| n.ccs.angle))?;
            let Some(angle) = named else {
                ed.say(key!("No coordinate system is saved under that name."));
                return Ok(None);
            };
            angle
        }
        _ => ed.read(|doc| doc.ccs().angle)?,
    };
    // Turning the drawing back by the system's angle lays its X axis level.
    Ok(Some(ZoomTo::Twist {
        twist: -angle,
        plan: true,
    }))
}

/// ROTATEVIEW and PLAN.
pub fn commands() -> Vec<Command<AppState>> {
    vec![
        Command::new("view.rotate", "Rotate View", |s: &mut AppState| {
            start_view_command(s, "ROTATEVIEW", rotate_view)
        })
        .checked(|s| s.is_running("ROTATEVIEW")),
        Command::new("view.plan", "Plan", |s: &mut AppState| {
            start_view_command(s, "PLAN", plan)
        })
        .checked(|s| s.is_running("PLAN")),
    ]
}

impl AppState {
    /// The gizmo's level button: ask for two points and turn the view so the
    /// line between them reads level, pointing right.
    pub fn start_level_view(&mut self) {
        start_view_command(self, "ROTATEVIEW", level_view);
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_6;
    use std::sync::{Arc, Mutex};

    use opendraft_commands::SetCcs;
    use opendraft_core::{Ccs, CcsSpace, Point};

    use super::*;

    fn asked(state: &mut AppState, name: &'static str, inputs: &[&str]) -> Option<ZoomTo> {
        // The app's own slot is drained by the pump, so keep a copy here.
        let ask = match name {
            "PLAN" => plan,
            "LEVEL" => level_view,
            _ => rotate_view,
        };
        let seen = Arc::new(Mutex::new(None));
        let mine = Arc::clone(&seen);
        state.start_command(name, move |ed| {
            *mine.lock().unwrap() = ask(ed)?;
            Ok(())
        });
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
        let answer = seen.lock().unwrap().take();
        answer
    }

    fn twist_of(to: Option<ZoomTo>) -> (f64, bool) {
        match to {
            Some(ZoomTo::Twist { twist, plan }) => (twist, plan),
            other => panic!("expected a twist, got {other:?}"),
        }
    }

    #[test]
    fn rotateview_takes_an_angle_two_points_or_reset() {
        let mut state = AppState::new();
        let (t, plan) = twist_of(asked(&mut state, "ROTATEVIEW", &["30"]));
        assert!((t - 30.0_f64.to_radians()).abs() < 1e-12 && !plan);
        let (t, _) = twist_of(asked(&mut state, "ROTATEVIEW", &["p", "0,0", "0,10"]));
        assert!(
            (t + std::f64::consts::FRAC_PI_2).abs() < 1e-12,
            "a vertical line reads level"
        );
        let (t, _) = twist_of(asked(&mut state, "ROTATEVIEW", &["r"]));
        assert_eq!(t, 0.0);
    }

    #[test]
    fn plan_lays_the_systems_x_axis_level_and_shows_the_extents() {
        let mut state = AppState::new();
        state.perform(SetCcs::new(
            CcsSpace::Model,
            Ccs::new(Point::new(0.0, 0.0), FRAC_PI_6),
        ));
        let (t, plan) = twist_of(asked(&mut state, "PLAN", &[""]));
        assert!((t + FRAC_PI_6).abs() < 1e-12 && plan);
        let (t, _) = twist_of(asked(&mut state, "PLAN", &["w"]));
        assert_eq!(t, 0.0);
    }

    #[test]
    fn level_reads_a_line_level_pointing_right() {
        let mut state = AppState::new();
        let (t, plan) = twist_of(asked(&mut state, "LEVEL", &["0,0", "0,10"]));
        assert!((t + std::f64::consts::FRAC_PI_2).abs() < 1e-12 && !plan);
        // The other way round, the view is turned half a turn further.
        let (back, _) = twist_of(asked(&mut state, "LEVEL", &["0,10", "0,0"]));
        assert!((back - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(level_twist(Point::new(1.0, 1.0), Point::new(1.0, 1.0)).is_none());
    }

    /// A line from 0,0 to 0,10 in the drawing.
    fn with_a_vertical_line() -> (AppState, opendraft_core::EntityId) {
        let mut state = AppState::new();
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        for input in ["0,0", "0,10", ""] {
            state.cmd_submit(input);
            state.settle_command();
        }
        let id = state.session().document().objects()[0].id;
        (state, id)
    }

    #[test]
    fn rotateview_and_level_can_take_an_object() {
        for name in ["ROTATEVIEW", "LEVEL"] {
            let (mut state, id) = with_a_vertical_line();
            let ask = if name == "LEVEL" {
                level_view
            } else {
                rotate_view
            };
            let seen = Arc::new(Mutex::new(None));
            let mine = Arc::clone(&seen);
            state.start_command(name, move |ed| {
                *mine.lock().unwrap() = ask(ed)?;
                Ok(())
            });
            state.settle_command();
            state.cmd_submit("o");
            state.settle_command();
            // Picked near its start, the line reads from there to its far end.
            state.answer_object(Point::new(0.0, 1.0), Some(id), false);
            state.settle_command();
            let (t, plan) = twist_of(seen.lock().unwrap().take());
            assert!(
                (t + std::f64::consts::FRAC_PI_2).abs() < 1e-12 && !plan,
                "{name}"
            );
        }
    }

    #[test]
    fn level_does_nothing_on_the_same_point_or_a_layouts_sheet() {
        let mut state = AppState::new();
        assert!(asked(&mut state, "LEVEL", &["3,3", "3,3"]).is_none());
        state.new_layout();
        assert!(asked(&mut state, "LEVEL", &[]).is_none());
    }

    #[test]
    fn a_layouts_sheet_does_not_turn() {
        let mut state = AppState::new();
        state.new_layout();
        assert!(asked(&mut state, "ROTATEVIEW", &[]).is_none());
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("does not turn")));
    }
}
