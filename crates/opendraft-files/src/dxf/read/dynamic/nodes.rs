//! The objects of an evaluation graph, one by one: what each DXF name holds
//! and which groups carry it. Group numbers are those AutoCAD wrote in the
//! sample files; an object that does not read as expected is `Other`, not a
//! guess.

use std::collections::HashSet;

use super::super::Record;
use super::{points, upper, Action, Column, NodeKind, NodeState, Parameter, Table, MAX_LIST};

/// The decoded object, or `None` for one that is plumbing and nothing to a
/// reader of the block (the grips' location components).
pub(super) fn kind(object: &Record, in_block: Option<&HashSet<String>>) -> Option<NodeKind> {
    let name = object.name.as_str();
    let decoded = match name {
        "BLOCKGRIPLOCATIONCOMPONENT" => return None,
        "BLOCKLINEARPARAMETER" => parameter(object).map(NodeKind::Linear),
        "BLOCKPOLARPARAMETER" => parameter(object).map(|mut p| {
            // `307` names the angle; `145`, `146` and `147` are its minimum,
            // maximum and increment; `175` and `176` count listed values.
            p.angle_name = object.text(307).unwrap_or("").to_owned();
            p.angle_range = match (object.number(145), object.number(146), object.number(147)) {
                (Some(lo), Some(hi), Some(step)) if step > 0.0 && lo <= hi => Some((lo, hi, step)),
                _ => None,
            };
            p.restricted = object.number(143).is_some_and(|i| i > 0.0)
                || object.int(175).is_some_and(|n| n > 0)
                || object.int(176).is_some_and(|n| n > 0);
            NodeKind::Polar(p)
        }),
        "BLOCKXYPARAMETER" => parameter(object).map(|mut p| {
            // `306` names the X distance and `305` the Y distance; the
            // increments are `144` (X) and `148` (Y), and `175` and `176`
            // count listed values.
            p.name = object.text(306).unwrap_or("").to_owned();
            p.angle_name = object.text(305).unwrap_or("").to_owned();
            p.restricted = object.number(144).is_some_and(|i| i > 0.0)
                || object.number(148).is_some_and(|i| i > 0.0)
                || object.int(175).is_some_and(|n| n > 0)
                || object.int(176).is_some_and(|n| n > 0);
            NodeKind::Xy(p)
        }),
        "BLOCKPOINTPARAMETER" => parameter(object).map(|mut p| {
            // The location is `1010`; `1011` is only where its label goes.
            p.end = p.start;
            NodeKind::Point(p)
        }),
        "BLOCKROTATIONPARAMETER" => parameter(object).map(|mut p| {
            // `143` is an angle increment and `175` how many listed angles.
            p.restricted = object.number(143).is_some_and(|i| i > 0.0)
                || object.int(175).is_some_and(|n| n > 0);
            NodeKind::Rotation(p)
        }),
        "BLOCKLOOKUPPARAMETER" => Some(NodeKind::Lookup {
            name: object.text(303).unwrap_or("").to_owned(),
        }),
        "BLOCKSTRETCHACTION" => Some(NodeKind::Stretch(action(object, in_block, true))),
        "BLOCKPOLARSTRETCHACTION" => Some(NodeKind::PolarStretch(action(object, in_block, false))),
        "BLOCKMOVEACTION" => Some(NodeKind::Move(action(object, in_block, true))),
        "BLOCKSCALEACTION" => Some(NodeKind::Scale(action(object, in_block, false))),
        "BLOCKFLIPPARAMETER" => parameter(object).map(NodeKind::Flip),
        "BLOCKFLIPACTION" => Some(NodeKind::FlipAction(action(object, in_block, false))),
        "BLOCKROTATEACTION" => Some(NodeKind::Rotate(action(object, in_block, false))),
        "BLOCKLOOKUPACTION" => table(object).map(NodeKind::LookupTable),
        n if n.starts_with("BLOCK") && n.ends_with("GRIP") => points(object, 1010, 1020)
            .first()
            .map(|&p| NodeKind::Grip(p)),
        _ => None,
    };
    Some(decoded.unwrap_or_else(|| NodeKind::Other(name.to_owned())))
}

/// The state a modified instance records for a node of this kind.
pub(super) fn state(kind: &NodeKind, record: &Record) -> Option<NodeState> {
    match kind {
        NodeKind::Linear(_) | NodeKind::Polar(_) | NodeKind::Xy(_) | NodeKind::Rotation(_) => {
            let at = points(record, 10, 20);
            Some(NodeState::Parameter {
                start: *at.first()?,
                end: *at.get(1)?,
            })
        }
        NodeKind::Lookup { .. } => Some(NodeState::Lookup(record.text(1)?.to_owned())),
        _ => None,
    }
}

fn parameter(object: &Record) -> Option<Parameter> {
    Some(Parameter {
        name: object.text(305).unwrap_or("").to_owned(),
        start: *points(object, 1010, 1020).first()?,
        end: *points(object, 1011, 1021).first()?,
        values: value_list(object),
        restricted: false,
        angle_name: String::new(),
        angle_range: None,
    })
}

/// `175` is how many allowed values follow, as `144` groups.
fn value_list(object: &Record) -> Vec<f64> {
    let Some(at) = object.groups.iter().position(|g| g.code == 175) else {
        return Vec::new();
    };
    let count = object.groups[at].value.trim().parse::<usize>().unwrap_or(0);
    object.groups[at + 1..]
        .iter()
        .filter(|g| g.code == 144)
        .filter_map(|g| g.value.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
        .take(count.min(MAX_LIST))
        .collect()
}

/// An action's selection is the `330` handles after its `71` count, and for
/// stretches also its `331` or `332` handles; only objects of the block count
/// (the polar stretch also names its parameter's objects).
fn action(object: &Record, in_block: Option<&HashSet<String>>, factors: bool) -> Action {
    let mut selection: Vec<String> = Vec::new();
    let mut pending = 0usize;
    for g in &object.groups {
        match g.code {
            71 => pending = g.value.trim().parse().unwrap_or(0),
            330 if pending > 0 => {
                pending -= 1;
                selection.push(g.value.clone());
            }
            331 | 332 => selection.push(g.value.clone()),
            _ => {}
        }
    }
    let mut seen = HashSet::new();
    selection.retain(|h| in_block.is_some_and(|b| b.contains(&upper(h))) && seen.insert(upper(h)));
    selection.truncate(MAX_LIST);
    Action {
        selection,
        frame: points(object, 1011, 1021),
        multiplier: factors.then(|| object.number(140)).flatten(),
        angle_offset: factors.then(|| object.number(141)).flatten(),
    }
}

/// `92` rows by `93` columns of `302` cells in row order; each column is a
/// `281` (0 input, 1 output) followed by its `304` property name.
fn table(object: &Record) -> Option<Table> {
    let size = |code| object.int(code).and_then(|v| usize::try_from(v).ok());
    let (rows, columns) = (size(92)?, size(93)?);
    if rows.checked_mul(columns)? > MAX_LIST {
        return None;
    }
    let cells: Vec<&str> = object
        .groups
        .iter()
        .filter(|g| g.code == 302)
        .map(|g| g.value.as_str())
        .collect();
    let mut named = Vec::new();
    let mut output = false;
    for g in &object.groups {
        match g.code {
            281 => output = g.value.trim() == "1",
            304 => named.push(Column {
                property: g.value.clone(),
                output,
            }),
            _ => {}
        }
    }
    if columns == 0 || cells.len() != rows * columns || named.len() != columns {
        return None;
    }
    Some(Table {
        columns: named,
        rows: cells
            .chunks(columns)
            .map(|row| row.iter().map(|c| (*c).to_owned()).collect())
            .collect(),
    })
}
