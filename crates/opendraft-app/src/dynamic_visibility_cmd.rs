//! Authoring a visibility parameter in the Block Editor: BPARAMETER adds it
//! (`visibility_parameter`), BVSTATE manages its named states, and BVHIDE and
//! BVSHOW choose which objects a state hides.
//!
//! AutoCAD edits one state at a time, with the editor showing only what that
//! state shows. This editor shows every object always, so each command names
//! the state it changes. A new object is shown in every state.

use std::collections::HashSet;

use opendraft_commands::{Cancelled, Editor, Options, PointOptions};
use opendraft_core::block::{
    BlockObjectId, DynamicBlock, DynamicParameter, DynamicParameterKind, ParameterId,
    VisibilityState,
};
use opendraft_ui::key;

use crate::dynamic_block_cmd::{ask_choice, commit, draft, select_objects};
use crate::dynamic_block_prompts::{ask_line, ask_naming};
use crate::text::plural;

/// A new visibility parameter: where it is and one state, `VisibilityState0`,
/// that hides nothing.
pub(crate) fn visibility_parameter(
    ed: &Editor,
    schema: &DynamicBlock,
) -> Result<DynamicParameter, Cancelled> {
    let position = ed.get_point(PointOptions::new("Specify parameter location"))?;
    let named = ask_naming(ed, schema, "Visibility", "visibility")?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips: 0,
        kind: DynamicParameterKind::Visibility {
            position,
            states: vec![VisibilityState {
                name: "VisibilityState0".to_owned(),
                hidden: Vec::new(),
            }],
        },
    })
}

/// The states of the schema's visibility parameter, or `None` after saying
/// there is none.
fn states_of<'a>(
    ed: &Editor,
    schema: &'a mut DynamicBlock,
) -> Option<&'a mut Vec<VisibilityState>> {
    let found = schema
        .parameters
        .iter_mut()
        .find_map(|p| match &mut p.kind {
            DynamicParameterKind::Visibility { states, .. } => Some(states),
            _ => None,
        });
    if found.is_none() {
        ed.say("Add a visibility parameter with BPARAMETER first.");
    }
    found
}

fn names_of(states: &[VisibilityState]) -> Vec<String> {
    states.iter().map(|s| s.name.clone()).collect()
}

/// A state name nothing else has: not empty, no longer than a line, and not
/// one of `taken` ignoring case.
fn ask_state_name(ed: &Editor, message: &str, taken: &[String]) -> Result<String, Cancelled> {
    loop {
        let name = ask_line(ed, message, "")?;
        if name.is_empty() {
            ed.say("A state needs a name.");
        } else if taken.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
            ed.say(format!("The name \"{name}\" is already used."));
        } else {
            return Ok(name);
        }
    }
}

/// The ids of the block's objects.
fn objects_of(ed: &Editor) -> Result<Vec<BlockObjectId>, Cancelled> {
    ed.read(|doc| doc.model_objects().map(|o| BlockObjectId(o.id.0)).collect())
}

/// `schema` saved when it checks against the block's objects.
pub(crate) fn save(
    ed: &Editor,
    name: &'static str,
    schema: DynamicBlock,
) -> Result<bool, Cancelled> {
    let candidate = schema.clone();
    let ok = ed.read(move |doc| {
        let ids: HashSet<BlockObjectId> =
            doc.model_objects().map(|o| BlockObjectId(o.id.0)).collect();
        candidate.check(&ids)
    })?;
    if let Err(why) = ok {
        ed.say(format!("That cannot be used: {why}."));
        return Ok(false);
    }
    commit(ed, name, schema)?;
    Ok(true)
}

/// BVSTATE: list, add, rename or delete the states of the visibility
/// parameter. The first state is the one a new reference shows.
pub fn bvstate(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut schema = draft(ed)?;
    let Some(states) = states_of(ed, &mut schema) else {
        return Ok(());
    };
    let chosen = ed.get_keyword(
        Options::<String>::new(key!("Enter an option"))
            .keyword("New", "N")
            .keyword("Rename", "R")
            .keyword("Delete", "D")
            .keyword("List", "L")
            .default("List".to_owned()),
    )?;
    match chosen.as_str() {
        "New" => {
            let name = ask_state_name(ed, "Enter new state name", &names_of(states))?;
            let hide_all = ed.get_keyword(
                Options::<String>::new(key!("Visibility of the objects in the new state"))
                    .keyword("Show all", "S")
                    .keyword("Hide all", "H")
                    .default("Show all".to_owned()),
            )? == "Hide all";
            let hidden = if hide_all {
                objects_of(ed)?
            } else {
                Vec::new()
            };
            let Some(states) = states_of(ed, &mut schema) else {
                return Ok(());
            };
            states.push(VisibilityState {
                name: name.clone(),
                hidden,
            });
            if save(ed, "Add visibility state", schema)? {
                ed.say(format!("State \"{name}\" added."));
            }
        }
        "Rename" => {
            let names = names_of(states);
            let at = ask_choice(ed, "Select state to rename", &names)?;
            let name = ask_state_name(ed, "Enter new name", &names)?;
            let Some(states) = states_of(ed, &mut schema) else {
                return Ok(());
            };
            states[at].name = name.clone();
            if save(ed, "Rename visibility state", schema)? {
                ed.say(format!("State renamed to \"{name}\"."));
            }
        }
        "Delete" => {
            if states.len() == 1 {
                ed.say("A visibility parameter needs at least one state.");
                return Ok(());
            }
            let names = names_of(states);
            let at = ask_choice(ed, "Select state to delete", &names)?;
            let Some(states) = states_of(ed, &mut schema) else {
                return Ok(());
            };
            let gone = states.remove(at).name;
            if save(ed, "Delete visibility state", schema)? {
                ed.say(format!("State \"{gone}\" deleted."));
            }
        }
        _ => {
            let lines: Vec<String> = states
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let first = if i == 0 { " (default)" } else { "" };
                    format!(
                        "{}{first}: hides {}",
                        s.name,
                        plural(s.hidden.len(), "object")
                    )
                })
                .collect();
            ed.say(lines.join("; "));
        }
    }
    Ok(())
}

/// BVHIDE and BVSHOW: choose objects and hide them (or show them) in one
/// state or in all of them.
fn set_visible(ed: &mut Editor, show: bool) -> Result<(), Cancelled> {
    let mut schema = draft(ed)?;
    let Some(states) = states_of(ed, &mut schema) else {
        return Ok(());
    };
    let mut names = names_of(states);
    if names.len() > 1 {
        names.push("All".to_owned());
    }
    let word = if show { "show" } else { "hide" };
    let at = ask_choice(
        ed,
        &format!("Select state in which to {word} the objects"),
        &names,
    )?;
    let Some(chosen) = select_objects(ed)? else {
        return Ok(());
    };
    let Some(states) = states_of(ed, &mut schema) else {
        return Ok(());
    };
    let every = at >= states.len();
    let count = chosen.len();
    for (i, state) in states.iter_mut().enumerate() {
        if every || i == at {
            let mut hidden: HashSet<BlockObjectId> = state.hidden.iter().copied().collect();
            for o in &chosen {
                if show {
                    hidden.remove(o);
                } else {
                    hidden.insert(*o);
                }
            }
            state.hidden = hidden.into_iter().collect();
            state.hidden.sort();
        }
    }
    let label = if every {
        "all states".to_owned()
    } else {
        names[at].clone()
    };
    let undo = if show {
        "Show in visibility state"
    } else {
        "Hide in visibility state"
    };
    if save(ed, undo, schema)? {
        ed.say(format!(
            "{} {} in {label}.",
            plural(count, "object"),
            if show { "shown" } else { "hidden" }
        ));
    }
    Ok(())
}

pub fn bvhide(ed: &mut Editor) -> Result<(), Cancelled> {
    set_visible(ed, false)
}

pub fn bvshow(ed: &mut Editor) -> Result<(), Cancelled> {
    set_visible(ed, true)
}
