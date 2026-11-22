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
use opendraft_core::Document;

use crate::state::AppState;
use crate::zoom_cmd::{start_view_command, ZoomTo};

/// Why the view of the space being worked in cannot turn, if it cannot.
fn refusal(doc: &Document) -> Option<&'static str> {
    if doc.active_layout().is_some() {
        return Some("A layout's sheet does not turn: work in the Model tab or inside a viewport.");
    }
    let (layout, viewport) = doc.active_viewport()?;
    let locked = doc.layout(layout)?.viewport(viewport)?.locked;
    locked.then_some("This viewport is locked; unlock it to turn its view.")
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

/// ROTATEVIEW: how far to turn the drawing on the screen.
fn rotate_view(ed: &Editor) -> Result<Option<ZoomTo>, Cancelled> {
    let Some(twist) = current(ed)? else {
        return Ok(None);
    };
    let options = NumberOptions::new("Specify view rotation angle")
        .default(twist)
        .turn()
        .keyword("Points", "P")
        .keyword("Reset", "R");
    let twist = match ed.ask_angle(options)? {
        Answer::Value(angle) => angle,
        Answer::Keyword(k) if k == "Points" => {
            let first = ed.get_point("Specify first point of a line to read level")?;
            let options = PointOptions::new("Specify second point").base(first);
            let second = ed.get_point(options)?;
            if (second.x - first.x).hypot(second.y - first.y) < 1e-12 {
                ed.say("The points are the same.");
                return Ok(None);
            }
            -(second.y - first.y).atan2(second.x - first.x)
        }
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
    let options = Options::<String>::new("Enter an option")
        .keyword("Current", "C")
        .keyword("Named", "N")
        .keyword("World", "W")
        .default("Current".to_owned());
    let angle = match ed.get_keyword(options)?.as_str() {
        "World" => 0.0,
        "Named" => {
            let name =
                match ed.ask_string(Options::<String>::new("Enter name of coordinate system"))? {
                    Answer::Value(name) => name,
                    _ => return Ok(None),
                };
            let named = ed.read(move |doc| doc.named_ccs_called(&name).map(|n| n.ccs.angle))?;
            let Some(angle) = named else {
                ed.say("No coordinate system is saved under that name.");
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

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_6;
    use std::sync::{Arc, Mutex};

    use opendraft_commands::SetCcs;
    use opendraft_core::{Ccs, CcsSpace, Point};

    use super::*;

    fn asked(state: &mut AppState, name: &'static str, inputs: &[&str]) -> Option<ZoomTo> {
        // The app's own slot is drained by the pump, so keep a copy here.
        let ask = if name == "PLAN" { plan } else { rotate_view };
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
