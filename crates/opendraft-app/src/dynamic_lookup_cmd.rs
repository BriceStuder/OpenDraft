//! Authoring a lookup parameter in the Block Editor: BPARAMETER adds it
//! (`lookup_parameter`, with its first row), and BLOOKUPTABLE adds, deletes
//! and lists the rows of its table.
//!
//! A row names a choice and gives a value to each input parameter. AutoCAD
//! edits the table in a dialog; this is the command-line version of it.
//! Distances are in the block's units, angles in degrees, a flip as 0 or 1.

use opendraft_commands::{Cancelled, Editor, NumberOptions, Options, PointOptions};
use opendraft_core::block::{
    DynamicBlock, DynamicParameter, DynamicParameterKind, LookupRow, ParameterId, CUSTOM,
};
use opendraft_ui::key;

use crate::dynamic_block_cmd::{ask_choice, draft};
use crate::dynamic_block_prompts::{ask_line, ask_naming};
use crate::dynamic_visibility_cmd::save;
use crate::text::plural;

/// The parameters a lookup can take values from.
pub(crate) fn input_candidates(schema: &DynamicBlock) -> Vec<&DynamicParameter> {
    schema
        .parameters
        .iter()
        .filter(|p| {
            matches!(
                p.kind,
                DynamicParameterKind::Linear { .. }
                    | DynamicParameterKind::Rotation { .. }
                    | DynamicParameterKind::Flip { .. }
            )
        })
        .collect()
}

/// The inputs, chosen one at a time until Enter.
fn ask_inputs(ed: &Editor, schema: &DynamicBlock) -> Result<Vec<ParameterId>, Cancelled> {
    let candidates = input_candidates(schema);
    let mut chosen: Vec<ParameterId> = Vec::new();
    loop {
        let left: Vec<&&DynamicParameter> = candidates
            .iter()
            .filter(|p| !chosen.contains(&p.id))
            .collect();
        if left.is_empty() {
            return Ok(chosen);
        }
        let mut options = Options::<String>::new(if chosen.is_empty() {
            "Select input parameter".to_owned()
        } else {
            "Select another input parameter [Done] <Done>".to_owned()
        });
        for p in &left {
            options = options.keyword(p.name.clone(), p.name.clone());
        }
        if !chosen.is_empty() {
            options = options.keyword("Done", "D").default("Done".to_owned());
        }
        let answer = ed.get_keyword(options)?;
        match left.iter().find(|p| p.name == answer) {
            Some(p) => chosen.push(p.id),
            None => return Ok(chosen),
        }
    }
}

/// A row: its label, then a value for each input.
fn ask_row(
    ed: &Editor,
    schema: &DynamicBlock,
    inputs: &[ParameterId],
    taken: &[String],
) -> Result<LookupRow, Cancelled> {
    let label = loop {
        let label = ask_line(ed, "Enter row name", "")?;
        if label.is_empty() {
            ed.say("A row needs a name.");
        } else if label.eq_ignore_ascii_case(CUSTOM)
            || taken.iter().any(|t| t.eq_ignore_ascii_case(&label))
        {
            ed.say(format!("The name \"{label}\" is already used."));
        } else {
            break label;
        }
    };
    let mut values = Vec::new();
    for id in inputs {
        let Some(q) = schema.parameters.iter().find(|q| q.id == *id) else {
            continue;
        };
        let (unit, angle) = (
            if q.is_angle() {
                " (degrees)"
            } else if q.is_flag() {
                " (0 or 1)"
            } else {
                ""
            },
            q.is_angle(),
        );
        let value = loop {
            let typed = ed.get_number(NumberOptions::new(format!(
                "Enter value for {}{unit}",
                q.name
            )))?;
            let stored = if angle { typed.to_radians() } else { typed };
            let held = q
                .normalize(stored)
                .is_some_and(|n| (n - stored).abs() <= 1e-9 * stored.abs().max(1.0));
            if held {
                break stored;
            }
            ed.say(format!("{} does not allow that value.", q.name));
        };
        values.push(value);
    }
    Ok(LookupRow { label, values })
}

/// A new lookup parameter: where it is, its inputs and its first row.
pub(crate) fn lookup_parameter(
    ed: &Editor,
    schema: &DynamicBlock,
) -> Result<DynamicParameter, Cancelled> {
    let position = ed.get_point(PointOptions::new("Specify parameter location"))?;
    let named = ask_naming(ed, schema, "Lookup", "lookup")?;
    let inputs = ask_inputs(ed, schema)?;
    let row = ask_row(ed, schema, &inputs, &[])?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips: 0,
        kind: DynamicParameterKind::Lookup {
            position,
            inputs,
            rows: vec![row],
        },
    })
}

/// BLOOKUPTABLE: list, add or delete the rows of a lookup parameter's table.
pub fn blookuptable(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut schema = draft(ed)?;
    let lookups: Vec<usize> = (0..schema.parameters.len())
        .filter(|i| schema.parameters[*i].lookup().is_some())
        .collect();
    if lookups.is_empty() {
        ed.say("Add a lookup parameter with BPARAMETER first.");
        return Ok(());
    }
    let names: Vec<String> = lookups
        .iter()
        .map(|i| schema.parameters[*i].name.clone())
        .collect();
    let at = lookups[ask_choice(ed, "Select lookup parameter", &names)?];
    let chosen = ed.get_keyword(
        Options::<String>::new(key!("Enter an option"))
            .keyword("Add", "A")
            .keyword("Delete", "D")
            .keyword("List", "L")
            .default("List".to_owned()),
    )?;
    let snapshot = schema.clone();
    let DynamicParameterKind::Lookup { inputs, rows, .. } = &mut schema.parameters[at].kind else {
        return Ok(());
    };
    match chosen.as_str() {
        "Add" => {
            let taken: Vec<String> = rows.iter().map(|r| r.label.clone()).collect();
            let row = ask_row(ed, &snapshot, inputs, &taken)?;
            let label = row.label.clone();
            rows.push(row);
            if save(ed, "Add lookup row", schema)? {
                ed.say(format!("Row \"{label}\" added."));
            }
        }
        "Delete" => {
            if rows.len() == 1 {
                ed.say("A lookup table needs at least one row.");
                return Ok(());
            }
            let labels: Vec<String> = rows.iter().map(|r| r.label.clone()).collect();
            let gone = ask_choice(ed, "Select row to delete", &labels)?;
            let label = rows.remove(gone).label;
            if save(ed, "Delete lookup row", schema)? {
                ed.say(format!("Row \"{label}\" deleted."));
            }
        }
        _ => {
            let heads: Vec<&str> = inputs
                .iter()
                .filter_map(|i| {
                    snapshot
                        .parameters
                        .iter()
                        .find(|p| p.id == *i)
                        .map(|p| p.name.as_str())
                })
                .collect();
            let lines: Vec<String> = rows
                .iter()
                .map(|r| {
                    let cells: Vec<String> = heads
                        .iter()
                        .zip(&r.values)
                        .map(|(h, v)| format!("{h}={v}"))
                        .collect();
                    format!("{}: {}", r.label, cells.join(", "))
                })
                .collect();
            ed.say(format!(
                "{}: {}",
                plural(rows.len(), "row"),
                lines.join("; ")
            ));
        }
    }
    Ok(())
}
