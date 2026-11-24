//! Authoring dynamic blocks in the Block Editor: BPARAMETER, BACTION and
//! BACTIONSET edit the schema the editor is drafting
//! (`Document::block_editor_dynamic`) as undoable steps of the editor;
//! BSAVE then turns the draft into the block's schema. RESETBLOCK, in a
//! drawing, puts inserts back to the definition's values.
//!
//! The prompts follow AutoCAD's where it publishes them; the order of the
//! rest is this project's. Everything is in the block's own units and
//! coordinates, base point at the origin.

use std::collections::BTreeSet;

use opendraft_commands::{
    Answer, Cancelled, Editor, ModifyEntities, NumberOptions, Options, PointOptions,
    SetBlockEditorDynamic,
};
use opendraft_core::block::{
    ActionId, BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, DynamicParameter,
    DynamicParameterKind, DynamicValueSet, ParameterId,
};
use opendraft_core::{Bounds, Entity, EntityId};

use crate::text::plural;

const EPS: f64 = 1e-9;

/// The schema being drafted, or an empty one.
fn draft(ed: &Editor) -> Result<DynamicBlock, Cancelled> {
    Ok(ed
        .read(|doc| doc.block_editor_dynamic().cloned())?
        .unwrap_or_default())
}

fn commit(ed: &Editor, name: &'static str, schema: DynamicBlock) -> Result<(), Cancelled> {
    ed.transaction(name, move |tx| {
        tx.perform(SetBlockEditorDynamic::new(name, Some(schema)));
        Ok(())
    })
}

/// The first `{prefix}N` (N from 1) that `taken` does not hold, ignoring case.
fn free_name(prefix: &str, taken: &[&str]) -> String {
    (1..)
        .map(|n| format!("{prefix}{n}"))
        .find(|name| !taken.iter().any(|t| t.eq_ignore_ascii_case(name)))
        .unwrap_or_default()
}

/// A line where Enter takes `default`.
fn ask_line(ed: &Editor, message: &str, default: &str) -> Result<String, Cancelled> {
    let shown = if default.is_empty() {
        format!("{message} <none>")
    } else {
        format!("{message} <{default}>")
    };
    match ed.ask_string(Options::<String>::new(shown))? {
        Answer::Value(text) => Ok(text.trim().to_owned()),
        Answer::Enter | Answer::Keyword(_) => Ok(default.to_owned()),
    }
}

/// A name nothing else in `taken` has, asked again until it is one.
fn ask_name(
    ed: &Editor,
    message: &str,
    default: &str,
    taken: &[&str],
) -> Result<String, Cancelled> {
    loop {
        let name = ask_line(ed, message, default)?;
        if name.is_empty() || name.contains(char::is_whitespace) {
            ed.say("A name cannot be empty or contain spaces.");
        } else if taken.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
            ed.say(format!("The name \"{name}\" is already used."));
        } else {
            return Ok(name);
        }
    }
}

fn ask_yes_no(ed: &Editor, message: &str, default_yes: bool) -> Result<bool, Cancelled> {
    let default = if default_yes { "Yes" } else { "No" };
    let options = Options::<String>::new(format!("{message} [Yes/No] <{default}>"))
        .keyword("Yes", "Y")
        .keyword("No", "N")
        .default(default.to_owned());
    Ok(ed.get_keyword(options)? == "Yes")
}

/// The numbers of a typed list, sorted and without repeats; why not, if any
/// is not a finite number.
fn parse_list(text: &str) -> Result<Vec<f64>, String> {
    let mut values = Vec::new();
    for part in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match part.parse::<f64>() {
            Ok(v) if v.is_finite() => values.push(v),
            _ => return Err(format!("\"{part}\" is not a number.")),
        }
    }
    values.sort_by(f64::total_cmp);
    values.dedup();
    if values.is_empty() {
        Err("Enter at least one value.".to_owned())
    } else {
        Ok(values)
    }
}

fn ask_value_set(ed: &Editor, default_length: f64) -> Result<DynamicValueSet, Cancelled> {
    let options =
        Options::<String>::new("Enter distance value set type [None/List/Increment] <None>")
            .keyword("None", "N")
            .keyword("List", "L")
            .keyword("Increment", "I")
            .default("None".to_owned());
    match ed.get_keyword(options)?.as_str() {
        "List" => loop {
            let text = ed.get_string(Options::<String>::new(
                "Enter list of distance values (separated by commas)",
            ))?;
            match parse_list(&text) {
                Ok(mut values) => {
                    // The parameter's own length is always one of the choices.
                    if !values.iter().any(|v| (v - default_length).abs() <= EPS) {
                        values.push(default_length);
                        values.sort_by(f64::total_cmp);
                    }
                    return Ok(DynamicValueSet::List(values));
                }
                Err(why) => ed.say(why),
            }
        },
        "Increment" => loop {
            let min = ed.get_number(NumberOptions::new("Enter minimum distance"))?;
            let max = ed.get_number(NumberOptions::new("Enter maximum distance"))?;
            let step = ed.get_number(NumberOptions::new("Enter distance increment"))?;
            if min.is_finite() && max.is_finite() && step.is_finite() && min <= max && step > 0.0 {
                return Ok(DynamicValueSet::Range {
                    min,
                    max,
                    increment: Some(step),
                });
            }
            ed.say(
                "The minimum cannot be above the maximum, and the increment must be above zero.",
            );
        },
        _ => Ok(DynamicValueSet::Any),
    }
}

/// BPARAMETER: add a linear parameter to the block being edited.
pub fn bparameter(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut schema = draft(ed)?;
    if schema.parameters.len() >= DynamicBlock::MAX_PARAMETERS {
        ed.say("The block has as many parameters as it can.");
        return Ok(());
    }
    ed.get_keyword(
        Options::<String>::new("Enter parameter type [Linear] <Linear>")
            .keyword("Linear", "L")
            .default("Linear".to_owned()),
    )?;
    let start = ed.get_point(PointOptions::new("Specify start point"))?;
    let end = loop {
        let end = ed.get_point(PointOptions::new("Specify endpoint").base(start))?;
        if (end.x - start.x).hypot(end.y - start.y) > EPS {
            break end;
        }
        ed.say("The parameter has no length.");
    };
    let length = (end.x - start.x).hypot(end.y - start.y);
    let names: Vec<&str> = schema.parameters.iter().map(|p| p.name.as_str()).collect();
    let name = ask_name(
        ed,
        "Enter parameter name",
        &free_name("Distance", &names),
        &names,
    )?;
    let label = ask_line(ed, "Enter distance label", &name)?;
    let description = ask_line(ed, "Enter parameter description", "")?;
    let show = ask_yes_no(ed, "Show the parameter in the Properties palette?", true)?;
    let grips = loop {
        let n = ed
            .get_number(NumberOptions::new("Enter number of grips [0/1] <1>").default(1.0))?
            .round();
        if n == 0.0 || n == 1.0 {
            break n as u8;
        }
        ed.say("Two grips are not available yet; use 0 or 1.");
    };
    let value_set = ask_value_set(ed, length)?;
    let id = ParameterId(schema.next_parameter_id);
    schema.next_parameter_id += 1;
    schema.parameters.push(DynamicParameter {
        id,
        name: name.clone(),
        label,
        description,
        show,
        grips,
        kind: DynamicParameterKind::Linear {
            start,
            end,
            value_set,
        },
    });
    commit(ed, "Add parameter", schema)?;
    ed.say(format!("Parameter \"{name}\" added."));
    Ok(())
}

/// The objects to act on, as ids of the block's objects: the pickset or a
/// selection, without attribute definitions. `None` after saying why when
/// nothing is left.
fn select_objects(ed: &mut Editor) -> Result<Option<Vec<BlockObjectId>>, Cancelled> {
    let picked = ed.pickset_or_select("Select objects")?;
    let (usable, skipped) = ed.read(move |doc| {
        let mut usable = BTreeSet::new();
        let mut skipped = 0usize;
        for id in picked {
            match doc.get(id) {
                Some(Entity::AttributeDefinition(_)) => skipped += 1,
                Some(_) => {
                    usable.insert(BlockObjectId(id.0));
                }
                None => {}
            }
        }
        (usable, skipped)
    })?;
    if skipped > 0 {
        ed.say(format!(
            "{} left out: an action cannot move an attribute definition.",
            plural(skipped, "attribute definition")
        ));
    }
    if usable.is_empty() {
        ed.say("No objects were selected.");
        return Ok(None);
    }
    Ok(Some(usable.into_iter().collect()))
}

/// Which of `names` the user means.
fn ask_choice(ed: &Editor, message: &str, names: &[String]) -> Result<usize, Cancelled> {
    if names.len() == 1 {
        return Ok(0);
    }
    let mut options =
        Options::<String>::new(format!("{message} <{}>", names[0])).default(names[0].clone());
    for name in names {
        options = options.keyword(name.clone(), name.clone());
    }
    let chosen = ed.get_keyword(options)?;
    Ok(names.iter().position(|n| *n == chosen).unwrap_or(0))
}

/// BACTION: add a Move or Stretch action driven by a parameter.
pub fn baction(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut schema = draft(ed)?;
    if schema.parameters.is_empty() {
        ed.say("Add a parameter with BPARAMETER first.");
        return Ok(());
    }
    if schema.actions.len() >= DynamicBlock::MAX_ACTIONS {
        ed.say("The block has as many actions as it can.");
        return Ok(());
    }
    let stretch = ed.get_keyword(
        Options::<String>::new("Enter action type [Move/Stretch] <Move>")
            .keyword("Move", "M")
            .keyword("Stretch", "S")
            .default("Move".to_owned()),
    )? == "Stretch";
    let names: Vec<String> = schema.parameters.iter().map(|p| p.name.clone()).collect();
    let parameter = schema.parameters[ask_choice(ed, "Select parameter", &names)?].id;
    let Some(objects) = select_objects(ed)? else {
        return Ok(());
    };
    let frame = if stretch {
        let first = ed.get_point(PointOptions::new("Specify first corner of stretch frame"))?;
        let second = loop {
            let second = ed.get_corner(first, PointOptions::new("Specify opposite corner"))?;
            if (second.x - first.x).abs() > EPS && (second.y - first.y).abs() > EPS {
                break second;
            }
            ed.say("The stretch frame has no width or no height.");
        };
        Some(Bounds::from_points(first, second))
    } else {
        None
    };
    let multiplier = loop {
        let m = ed.get_number(NumberOptions::new("Specify distance multiplier").default(1.0))?;
        if m.is_finite() && m > 0.0 {
            break m;
        }
        ed.say("The multiplier must be above zero.");
    };
    let angle_offset = ed.get_angle(NumberOptions::new("Specify angle offset").default(0.0))?;
    let taken: Vec<&str> = schema.actions.iter().map(|a| a.name.as_str()).collect();
    let prefix = if stretch { "Stretch" } else { "Move" };
    let name = ask_name(ed, "Enter action name", &free_name(prefix, &taken), &taken)?;
    let kind = match frame {
        Some(frame) => DynamicActionKind::Stretch {
            objects,
            frame,
            multiplier,
            angle_offset,
        },
        None => DynamicActionKind::Move {
            objects,
            multiplier,
            angle_offset,
        },
    };
    let id = ActionId(schema.next_action_id);
    schema.next_action_id += 1;
    schema.actions.push(DynamicAction {
        id,
        name: name.clone(),
        parameter,
        kind,
    });
    let candidate = schema.clone();
    let ok = ed.read(move |doc| {
        let ids = doc.model_objects().map(|o| BlockObjectId(o.id.0)).collect();
        candidate.check(&ids)
    })?;
    if let Err(why) = ok {
        ed.say(format!("The action cannot be added: {why}."));
        return Ok(());
    }
    commit(ed, "Add action", schema)?;
    ed.say(format!("Action \"{name}\" added."));
    Ok(())
}

/// BACTIONSET: replace the objects an action acts on.
pub fn bactionset(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut schema = draft(ed)?;
    if schema.actions.is_empty() {
        ed.say("Add an action with BACTION first.");
        return Ok(());
    }
    let names: Vec<String> = schema.actions.iter().map(|a| a.name.clone()).collect();
    let at = ask_choice(ed, "Select action", &names)?;
    let Some(chosen) = select_objects(ed)? else {
        return Ok(());
    };
    let count = chosen.len();
    let (DynamicActionKind::Move { objects, .. } | DynamicActionKind::Stretch { objects, .. }) =
        &mut schema.actions[at].kind;
    *objects = chosen;
    let candidate = schema.clone();
    let ok = ed.read(move |doc| {
        let ids = doc.model_objects().map(|o| BlockObjectId(o.id.0)).collect();
        candidate.check(&ids)
    })?;
    if let Err(why) = ok {
        ed.say(format!("The selection cannot be used: {why}."));
        return Ok(());
    }
    let name = names[at].clone();
    commit(ed, "Change action objects", schema)?;
    ed.say(format!(
        "Action \"{name}\" now acts on {}.",
        plural(count, "object")
    ));
    Ok(())
}

/// RESETBLOCK: put dynamic block references back to the definition's values.
pub fn resetblock(ed: &mut Editor) -> Result<(), Cancelled> {
    let picked = ed.pickset_or_select("Select dynamic block references")?;
    let (dynamic, changed): (usize, Vec<(EntityId, Entity)>) = ed.read(move |doc| {
        let mut dynamic = 0usize;
        let mut changed = Vec::new();
        for id in picked {
            if let Some(Entity::Insert(i)) = doc.get(id) {
                if i.block.is_dynamic() {
                    dynamic += 1;
                    if i.dynamic.is_some() {
                        changed.push((id, Entity::Insert(Box::new(i.reset_dynamic()))));
                    }
                }
            }
        }
        (dynamic, changed)
    })?;
    if dynamic == 0 {
        ed.say("No dynamic block references selected.");
        return Ok(());
    }
    let count = changed.len();
    if count > 0 {
        ed.transaction("Reset dynamic blocks", move |tx| {
            tx.perform(ModifyEntities::new("Reset dynamic blocks", changed));
            Ok(())
        })?;
    }
    ed.say(format!(
        "Reset {}.",
        plural(count, "dynamic block reference")
    ));
    Ok(())
}

#[cfg(test)]
#[path = "dynamic_block_cmd_tests.rs"]
mod tests;
