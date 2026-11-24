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
    Cancelled, Editor, ModifyEntities, NumberOptions, Options, PointOptions, SetBlockEditorDynamic,
};
use opendraft_core::block::{
    ActionId, BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, DynamicFamily,
    DynamicParameter, DynamicParameterKind, ParameterId,
};
use opendraft_core::{Bounds, Entity, EntityId};

use crate::dynamic_block_prompts::{
    ask_grips, ask_name, ask_naming, ask_value_set, free_name, Quantity, EPS,
};
use crate::text::plural;

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

/// BPARAMETER: add a linear, rotation or flip parameter to the block being
/// edited.
pub fn bparameter(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut schema = draft(ed)?;
    if schema.parameters.len() >= DynamicBlock::MAX_PARAMETERS {
        ed.say("The block has as many parameters as it can.");
        return Ok(());
    }
    let parameter = match ed
        .get_keyword(
            Options::<String>::new("Enter parameter type [Linear/Rotation/Flip] <Linear>")
                .keyword("Linear", "L")
                .keyword("Rotation", "R")
                .keyword("Flip", "F")
                .default("Linear".to_owned()),
        )?
        .as_str()
    {
        "Rotation" => rotation_parameter(ed, &schema)?,
        "Flip" => flip_parameter(ed, &schema)?,
        _ => linear_parameter(ed, &schema)?,
    };
    let name = parameter.name.clone();
    schema.parameters.push(parameter);
    schema.next_parameter_id += 1;
    commit(ed, "Add parameter", schema)?;
    ed.say(format!("Parameter \"{name}\" added."));
    Ok(())
}

fn linear_parameter(ed: &Editor, schema: &DynamicBlock) -> Result<DynamicParameter, Cancelled> {
    let start = ed.get_point(PointOptions::new("Specify start point"))?;
    let end = loop {
        let end = ed.get_point(PointOptions::new("Specify endpoint").base(start))?;
        if (end.x - start.x).hypot(end.y - start.y) > EPS {
            break end;
        }
        ed.say("The parameter has no length.");
    };
    let length = (end.x - start.x).hypot(end.y - start.y);
    let named = ask_naming(ed, schema, "Distance", "distance")?;
    let grips = ask_grips(ed)?;
    let value_set = ask_value_set(ed, Quantity::Distance, length)?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips,
        kind: DynamicParameterKind::Linear {
            start,
            end,
            value_set,
        },
    })
}

fn rotation_parameter(ed: &Editor, schema: &DynamicBlock) -> Result<DynamicParameter, Cancelled> {
    let base = ed.get_point(PointOptions::new("Specify base point"))?;
    let radius = loop {
        let radius = ed.get_number(NumberOptions::new("Specify radius of parameter"))?;
        if radius.is_finite() && radius > EPS {
            break radius;
        }
        ed.say("The radius must be above zero.");
    };
    let named = ask_naming(ed, schema, "Angle", "angle")?;
    let grips = ask_grips(ed)?;
    let value_set = ask_value_set(ed, Quantity::Angle, 0.0)?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips,
        kind: DynamicParameterKind::Rotation {
            base,
            radius,
            value_set,
        },
    })
}

fn flip_parameter(ed: &Editor, schema: &DynamicBlock) -> Result<DynamicParameter, Cancelled> {
    let base = ed.get_point(PointOptions::new("Specify base point of reflection line"))?;
    let end = loop {
        let end =
            ed.get_point(PointOptions::new("Specify endpoint of reflection line").base(base))?;
        if (end.x - base.x).hypot(end.y - base.y) > EPS {
            break end;
        }
        ed.say("The reflection line has no length.");
    };
    let named = ask_naming(ed, schema, "Flip", "flip")?;
    let grips = ask_grips(ed)?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips,
        kind: DynamicParameterKind::Flip { base, end },
    })
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

/// BACTION: add a Move or Stretch action driven by a linear parameter, a
/// Rotate driven by a rotation, or a Flip driven by a flip.
pub fn baction(ed: &mut Editor) -> Result<(), Cancelled> {
    let schema = draft(ed)?;
    if schema.parameters.is_empty() {
        ed.say("Add a parameter with BPARAMETER first.");
        return Ok(());
    }
    if schema.actions.len() >= DynamicBlock::MAX_ACTIONS {
        ed.say("The block has as many actions as it can.");
        return Ok(());
    }
    let chosen = ed.get_keyword(
        Options::<String>::new("Enter action type [Move/Stretch/Rotate/Flip] <Move>")
            .keyword("Move", "M")
            .keyword("Stretch", "S")
            .keyword("Rotate", "R")
            .keyword("Flip", "F")
            .default("Move".to_owned()),
    )?;
    let stretch = chosen == "Stretch";
    // Each action follows its own kind of parameter.
    let (family, need) = match chosen.as_str() {
        "Rotate" => (DynamicFamily::Rotation, "rotation"),
        "Flip" => (DynamicFamily::Flip, "flip"),
        _ => (DynamicFamily::Linear, "linear"),
    };
    let usable: Vec<&DynamicParameter> = schema
        .parameters
        .iter()
        .filter(|p| p.kind.family() == family)
        .collect();
    if usable.is_empty() {
        ed.say(format!(
            "A {} action needs a {need} parameter.",
            chosen.to_lowercase()
        ));
        return Ok(());
    }
    let names: Vec<String> = usable.iter().map(|p| p.name.clone()).collect();
    let parameter = usable[ask_choice(ed, "Select parameter", &names)?].id;
    let Some(objects) = select_objects(ed)? else {
        return Ok(());
    };
    match family {
        DynamicFamily::Rotation => {
            return add_action(ed, schema, "Rotate", parameter, |_| {
                DynamicActionKind::Rotate { objects }
            })
        }
        DynamicFamily::Flip => {
            return add_action(ed, schema, "Flip", parameter, |_| DynamicActionKind::Flip {
                objects,
            })
        }
        DynamicFamily::Linear => {}
    }
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
    add_action(
        ed,
        schema,
        if stretch { "Stretch" } else { "Move" },
        parameter,
        |_| kind,
    )
}

/// Name an action of `prefix`, add it to `schema` and commit it if the
/// schema still checks.
fn add_action(
    ed: &Editor,
    mut schema: DynamicBlock,
    prefix: &str,
    parameter: ParameterId,
    kind: impl FnOnce(&DynamicBlock) -> DynamicActionKind,
) -> Result<(), Cancelled> {
    let taken: Vec<&str> = schema.actions.iter().map(|a| a.name.as_str()).collect();
    let name = ask_name(ed, "Enter action name", &free_name(prefix, &taken), &taken)?;
    let kind = kind(&schema);
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
    let (DynamicActionKind::Move { objects, .. }
    | DynamicActionKind::Stretch { objects, .. }
    | DynamicActionKind::Rotate { objects }
    | DynamicActionKind::Flip { objects }) = &mut schema.actions[at].kind;
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

#[cfg(test)]
#[path = "dynamic_rotation_cmd_tests.rs"]
mod rotation_tests;

#[cfg(test)]
#[path = "dynamic_flip_cmd_tests.rs"]
mod flip_tests;
