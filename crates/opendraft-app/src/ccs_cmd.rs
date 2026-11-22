//! CCS (alias UCS): the custom coordinate system, AutoCAD's user
//! coordinate system under our own name. Two points set the origin and the
//! X axis; Object lines it up with an object, View with the view, Rotate
//! turns it, and Previous, World and Named go back to one. The Model tab and
//! each layout viewport have their own; a layout's sheet has none. Each
//! change is one undo step (`opendraft_commands::SetCcs`).

use std::f64::consts::FRAC_PI_2;

use opendraft_commands::{
    Answer, Cancelled, Command, Editor, NumberOptions, Options, PointOptions, SetCcs, SetNamedCcs,
};
use opendraft_core::ccs::name_refusal;
use opendraft_core::ccs_align::aligned_to;
use opendraft_core::units::{format_angle, format_point};
use opendraft_core::{Bounds, Ccs, CcsSpace, Document, NamedCcs, Point};

use crate::state::AppState;

/// What the command says on a layout's sheet.
const NO_SHEET: &str =
    "A layout's sheet has no custom coordinate system: work in the Model tab or inside a viewport.";

/// The name the current system goes by: World, a saved name it matches, or
/// Custom.
fn current_name(doc: &Document) -> String {
    let ccs = doc.ccs();
    if ccs.is_world() {
        return "World".to_owned();
    }
    let same = |n: &&NamedCcs| {
        let (a, b) = (n.ccs, ccs);
        (a.origin.x - b.origin.x).abs() < 1e-9
            && (a.origin.y - b.origin.y).abs() < 1e-9
            && (a.angle - b.angle).abs() < 1e-12
    };
    doc.named_ccs()
        .iter()
        .find(same)
        .map_or_else(|| "Custom".to_owned(), |n| n.name.clone())
}

/// Put `ccs` in force in `space` as one undo step.
fn set(ed: &Editor, space: CcsSpace, ccs: Ccs) -> Result<(), Cancelled> {
    ed.transaction("CCS", move |tx| {
        tx.perform(SetCcs::new(space, ccs));
        Ok(())
    })
}

/// The command. `view` is the part of the drawing on screen when it started
/// (the View option's corner).
fn ccs(ed: &mut Editor, view: Option<Bounds>) -> Result<(), Cancelled> {
    let (space, current, name) =
        ed.read(|doc| (doc.active_ccs_space(), doc.ccs(), current_name(doc)))?;
    let Some(space) = space else {
        ed.say(NO_SHEET);
        return Ok(());
    };
    ed.say(format!("Current coordinate system: {name}"));
    let options = PointOptions::new("Specify origin of CCS (Enter for World)")
        .keyword("Object", "OB")
        .keyword("View", "V")
        .keyword("Rotate", "R")
        .keyword("Previous", "P")
        .keyword("World", "W")
        .keyword("Named", "NA");
    match ed.ask_point(options)? {
        Answer::Value(origin) => by_points(ed, space, current, origin),
        Answer::Enter => set(ed, space, Ccs::WORLD),
        Answer::Keyword(k) => match k.as_str() {
            "Object" => by_object(ed, space, current),
            "View" => match view {
                Some(view) => set(ed, space, Ccs::new(view.min, 0.0)),
                None => {
                    ed.say("The view is not known yet.");
                    Ok(())
                }
            },
            "Rotate" => {
                let options = NumberOptions::new("Specify rotation angle about Z axis")
                    .default(FRAC_PI_2)
                    .turn()
                    .base(current.origin);
                let turn = ed.get_angle(options)?;
                set(ed, space, current.turned(turn))
            }
            "Previous" => previous(ed, space),
            "World" => set(ed, space, Ccs::WORLD),
            _ => named(ed, space, current),
        },
    }
}

/// Origin, then a point on the X axis (Enter keeps the current direction).
fn by_points(ed: &Editor, space: CcsSpace, current: Ccs, origin: Point) -> Result<(), Cancelled> {
    let options = PointOptions::new("Specify point on X-axis (Enter to accept)").base(origin);
    let ccs = match ed.ask_point(options)? {
        Answer::Value(on_x) => match Ccs::through(origin, on_x) {
            Some(ccs) => ccs,
            None => {
                ed.say("The point is the origin; the direction is kept.");
                current.moved_to(origin)
            }
        },
        _ => current.moved_to(origin),
    };
    set(ed, space, ccs)
}

/// Line the system up with a picked object.
fn by_object(ed: &Editor, space: CcsSpace, current: Ccs) -> Result<(), Cancelled> {
    loop {
        let picked = match ed.ask_object(Options::new("Select object to align CCS"))? {
            Answer::Value(picked) => picked,
            _ => return Ok(()),
        };
        let entity = ed.read(move |doc| doc.object(picked.id).map(|o| o.entity.clone()))?;
        match entity.and_then(|e| aligned_to(&e, picked.at, current)) {
            Some(ccs) => return set(ed, space, ccs),
            None => ed.say("That object does not set a coordinate system."),
        }
    }
}

fn previous(ed: &Editor, space: CcsSpace) -> Result<(), Cancelled> {
    if ed.read(move |doc| doc.previous_ccs(space))?.is_none() {
        ed.say("There is no previous coordinate system.");
        return Ok(());
    }
    ed.transaction("CCS", move |tx| {
        tx.perform(SetCcs::previous(space));
        Ok(())
    })
}

/// Restore, Save, Delete and list the systems saved by name.
fn named(ed: &Editor, space: CcsSpace, current: Ccs) -> Result<(), Cancelled> {
    let options = Options::<String>::new("Enter an option")
        .keyword("Restore", "R")
        .keyword("Save", "S")
        .keyword("Delete", "D")
        .keyword("?", "?");
    let Some(choice) = ed.ask_keyword(options)? else {
        return Ok(());
    };
    if choice == "?" {
        return list(ed);
    }
    let message = match choice.as_str() {
        "Save" => "Enter name to save the current CCS as",
        "Delete" => "Enter name of coordinate system to delete",
        _ => "Enter name of coordinate system to restore",
    };
    let name = match ed.ask_string(Options::<String>::new(message))? {
        Answer::Value(name) => name.trim().to_owned(),
        _ => return Ok(()),
    };
    if name == "?" {
        return list(ed);
    }
    let all = ed.read(|doc| doc.named_ccs().to_vec())?;
    let found = all.iter().position(|n| n.name.eq_ignore_ascii_case(&name));
    match (choice.as_str(), found) {
        ("Save", _) => save(ed, all, found, name, current),
        ("Delete", Some(at)) => {
            let mut kept = all;
            kept.remove(at);
            ed.transaction("CCS", move |tx| {
                tx.perform(SetNamedCcs::new(kept));
                Ok(())
            })
        }
        (_, Some(at)) => set(ed, space, all[at].ccs),
        (_, None) => {
            ed.say(format!("No coordinate system is saved as \"{name}\"."));
            Ok(())
        }
    }
}

fn save(
    ed: &Editor,
    mut all: Vec<NamedCcs>,
    found: Option<usize>,
    name: String,
    current: Ccs,
) -> Result<(), Cancelled> {
    if let Some(why) = name_refusal(&name) {
        ed.say(why);
        return Ok(());
    }
    match found {
        Some(at) => {
            let options = Options::<String>::new(format!("\"{name}\" already exists. Replace it?"))
                .keyword("Yes", "Y")
                .keyword("No", "N")
                .default("No".to_owned());
            if ed.ask_keyword(options)?.as_deref() != Some("Yes") {
                return Ok(());
            }
            all[at].ccs = current;
        }
        None => all.push(NamedCcs { name, ccs: current }),
    }
    ed.transaction("CCS", move |tx| {
        tx.perform(SetNamedCcs::new(all));
        Ok(())
    })
}

/// List the saved systems, in world coordinates.
fn list(ed: &Editor) -> Result<(), Cancelled> {
    let lines = ed.read(|doc| {
        let units = doc.units();
        let lines: Vec<String> = doc
            .named_ccs()
            .iter()
            .map(|n| {
                format!(
                    "  {}: origin {}, X axis at {}",
                    n.name,
                    format_point(units, n.ccs.origin),
                    format_angle(units, n.ccs.angle)
                )
            })
            .collect();
        lines
    })?;
    if lines.is_empty() {
        ed.say("No coordinate systems are saved.");
    } else {
        ed.say("Saved coordinate systems (in world coordinates):");
        for line in lines {
            ed.say(line);
        }
    }
    Ok(())
}

/// The CCS command and the World and Previous buttons.
pub fn commands() -> Vec<Command<AppState>> {
    vec![
        Command::new("view.ccs", "Coordinate System", |s: &mut AppState| {
            s.start_ccs()
        })
        .checked(|s| s.is_running("CCS")),
        Command::new("view.ccs_world", "World", AppState::ccs_world),
        Command::new("view.ccs_previous", "Previous", AppState::ccs_previous),
    ]
}

impl AppState {
    /// Start CCS, remembering what is on screen for its View option.
    pub fn start_ccs(&mut self) {
        let view = self.docs[self.active].view.visible_area();
        self.start_command("CCS", move |ed| ccs(ed, view));
    }

    /// The World button: the world system, at once, in the space being edited.
    pub fn ccs_world(&mut self) {
        match self.session().document().active_ccs_space() {
            Some(space) => {
                if !self.session().document().ccs().is_world() {
                    self.perform(SetCcs::new(space, Ccs::WORLD));
                }
            }
            None => self.message = NO_SHEET.to_owned(),
        }
    }

    /// The Previous button: back to the system before this one.
    pub fn ccs_previous(&mut self) {
        let document = self.session().document();
        match document.active_ccs_space() {
            Some(space) if document.previous_ccs(space).is_some() => {
                self.perform(SetCcs::previous(space));
            }
            Some(_) => self.message = "There is no previous coordinate system.".to_owned(),
            None => self.message = NO_SHEET.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(state: &mut AppState, inputs: &[&str]) {
        state.start_ccs();
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    fn ccs(state: &AppState) -> Ccs {
        state.session().document().ccs()
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    #[test]
    fn two_points_set_the_origin_and_x_axis_and_undo_takes_it_back() {
        let mut state = AppState::new();
        run(&mut state, &["10,0", "10,5"]);
        assert!(!state.command_running());
        let c = ccs(&state);
        assert!(near(c.origin, Point::new(10.0, 0.0)) && (c.angle - FRAC_PI_2).abs() < 1e-12);
        state.undo();
        assert!(ccs(&state).is_world());
    }

    #[test]
    fn enter_at_the_second_point_moves_the_origin_only() {
        let mut state = AppState::new();
        run(&mut state, &["r", "30"]);
        run(&mut state, &["5,5", ""]);
        let c = ccs(&state);
        assert!((c.angle - 30.0_f64.to_radians()).abs() < 1e-12);
        // 5,5 was typed in the turned system.
        let expected = Ccs::new(Point::new(0.0, 0.0), c.angle).to_world(Point::new(5.0, 5.0));
        assert!(near(c.origin, expected));
    }

    #[test]
    fn world_previous_and_names() {
        let mut state = AppState::new();
        run(&mut state, &["r", "45"]);
        run(&mut state, &["na", "s", "Wing A"]);
        run(&mut state, &["w"]);
        assert!(ccs(&state).is_world());
        run(&mut state, &["p"]);
        assert!((ccs(&state).angle - 45.0_f64.to_radians()).abs() < 1e-12);
        run(&mut state, &[""]);
        assert!(ccs(&state).is_world(), "Enter is World");
        run(&mut state, &["na", "r", "wing a"]);
        assert!((ccs(&state).angle - 45.0_f64.to_radians()).abs() < 1e-12);
        run(&mut state, &["na", "d", "Wing A"]);
        assert!(state.session().document().named_ccs().is_empty());
        run(&mut state, &["na", "r", "Wing A"]);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("No coordinate system is saved")));
    }

    #[test]
    fn an_object_sets_the_system() {
        let mut state = AppState::new();
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        for input in ["0,0", "0,10", ""] {
            state.cmd_submit(input);
            state.settle_command();
        }
        let id = state.session().document().objects()[0].id;
        state.start_ccs();
        state.settle_command();
        state.cmd_submit("ob");
        state.settle_command();
        state.answer_object(Point::new(0.0, 1.0), Some(id), false);
        state.settle_command();
        let c = ccs(&state);
        assert!(near(c.origin, Point::new(0.0, 0.0)) && (c.angle - FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn the_buttons_go_to_world_and_back() {
        let mut state = AppState::new();
        run(&mut state, &["r", "90"]);
        state.ccs_world();
        assert!(ccs(&state).is_world());
        state.ccs_previous();
        assert!((ccs(&state).angle - FRAC_PI_2).abs() < 1e-12);
    }
}
