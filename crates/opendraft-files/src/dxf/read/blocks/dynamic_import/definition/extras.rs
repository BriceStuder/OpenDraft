//! The parameters of a definition that take no action: its visibility
//! parameter and its lookup parameter with the table.

use std::collections::{HashMap, HashSet};

use opendraft_core::block::{
    DynamicParameter, DynamicParameterKind, LookupRow, ParameterId, CUSTOM,
};

use super::super::super::super::dynamic::{DynamicDefinition, NodeKind};
use super::{upper, DraftVisibility, Kept};

/// The definition's visibility parameter, added to `parameters` with its
/// states still empty (they need the objects' ids). A second one is left
/// out.
pub(super) fn visibility(
    def: &DynamicDefinition,
    parameters: &mut Vec<(u32, DynamicParameter)>,
    kept: &mut HashMap<u32, Kept>,
    names: &mut HashSet<String>,
    left_out: &mut Vec<String>,
) -> Option<DraftVisibility> {
    let mut found = def.nodes.iter().filter_map(|n| match &n.kind {
        NodeKind::Visibility(v) => Some((n.id, v)),
        _ => None,
    });
    let (node, v) = found.next()?;
    if found.next().is_some() {
        left_out.push("second visibility parameter".to_owned());
    }
    let id = ParameterId(parameters.len() as u64 + 1);
    let mut name = match v.name.trim() {
        "" => format!("Visibility{}", id.0),
        n => n.to_owned(),
    };
    if !names.insert(name.to_lowercase()) {
        name = format!("{name}{}", id.0);
        names.insert(name.to_lowercase());
    }
    // A state with no name or a name already used gets one of its own.
    let mut used = HashSet::new();
    let states: Vec<(String, HashSet<String>)> = v
        .states
        .iter()
        .enumerate()
        .map(|(i, (state, shown))| {
            let mut state = state.trim().to_owned();
            if state.is_empty() || !used.insert(state.to_lowercase()) {
                state = format!("{state}State{}", i + 1);
                used.insert(state.to_lowercase());
            }
            (state, shown.iter().map(|h| upper(h)).collect())
        })
        .collect();
    kept.insert(
        node,
        Kept {
            id,
            zero_angle: None,
            unread: false,
            polar: false,
            states: states.iter().map(|(s, _)| s.to_lowercase()).collect(),
        },
    );
    parameters.push((
        node,
        DynamicParameter {
            id,
            name: name.clone(),
            label: name,
            description: String::new(),
            show: true,
            grips: 0,
            kind: DynamicParameterKind::Visibility {
                position: v.position,
                states: Vec::new(),
            },
        },
    ));
    Some(DraftVisibility {
        parameter: id,
        governed: v.governed.iter().map(|h| upper(h)).collect(),
        states,
    })
}

/// The definition's lookup parameter and its table, added to `parameters`.
/// AutoCAD names the property of a linear parameter's value in the table
/// `UpdatedDistance`, so a table with that one input column is taken to
/// read the block's one linear parameter; a table with other inputs cannot
/// be matched to a parameter and is left out. Each row's label is the text
/// of its output column and its value the number in the input column.
pub(super) fn lookup(
    def: &DynamicDefinition,
    parameters: &mut Vec<(u32, DynamicParameter)>,
    kept: &mut HashMap<u32, Kept>,
    names: &mut HashSet<String>,
    left_out: &mut Vec<String>,
    linked: &dyn Fn(u32, u32) -> bool,
) -> Option<ParameterId> {
    let (node, name, position) = def.nodes.iter().find_map(|n| match &n.kind {
        NodeKind::Lookup { name, position } => Some((n.id, name, *position)),
        _ => None,
    })?;
    let table = def.nodes.iter().find_map(|n| match &n.kind {
        NodeKind::LookupTable(t) if linked(node, n.id) => Some(t),
        _ => None,
    });
    let Some(table) = table else {
        left_out.push("lookup parameter".to_owned());
        return None;
    };
    let mut linears = parameters
        .iter()
        .filter(|(_, p)| matches!(p.kind, DynamicParameterKind::Linear { .. }));
    let input = match (linears.next(), linears.next()) {
        (Some((_, p)), None) => Some(p),
        _ => None,
    };
    let inputs: Vec<usize> = (0..table.columns.len())
        .filter(|c| !table.columns[*c].output)
        .collect();
    let outputs: Vec<usize> = (0..table.columns.len())
        .filter(|c| table.columns[*c].output)
        .collect();
    let (Some(input), [column], [label]) = (input, inputs.as_slice(), outputs.as_slice()) else {
        left_out.push("lookup action".to_owned());
        return None;
    };
    if table.columns[*column].property != "UpdatedDistance" {
        left_out.push("lookup action".to_owned());
        return None;
    }
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for row in &table.rows {
        let value = row.get(*column).and_then(|c| c.trim().parse::<f64>().ok());
        let text = row.get(*label).map(|t| t.trim().to_owned());
        let (Some(value), Some(text)) = (value, text) else {
            left_out.push("lookup action".to_owned());
            return None;
        };
        let held = input
            .normalize(value)
            .is_some_and(|n| (n - value).abs() <= 1e-9 * value.abs().max(1.0));
        let duplicate = rows
            .iter()
            .any(|r: &LookupRow| (r.values[0] - value).abs() <= 1e-9 * value.abs().max(1.0));
        if text.is_empty()
            || text.eq_ignore_ascii_case(CUSTOM)
            || !held
            || duplicate
            || !seen.insert(text.to_lowercase())
        {
            left_out.push("lookup action".to_owned());
            return None;
        }
        rows.push(LookupRow {
            label: text,
            values: vec![value],
        });
    }
    if rows.is_empty() {
        left_out.push("lookup action".to_owned());
        return None;
    }
    let id = ParameterId(parameters.len() as u64 + 1);
    let mut name = match name.trim() {
        "" => format!("Lookup{}", id.0),
        n => n.to_owned(),
    };
    if !names.insert(name.to_lowercase()) {
        name = format!("{name}{}", id.0);
        names.insert(name.to_lowercase());
    }
    kept.insert(
        node,
        Kept {
            id,
            zero_angle: None,
            unread: false,
            polar: false,
            states: Vec::new(),
        },
    );
    parameters.push((
        node,
        DynamicParameter {
            id,
            name: name.clone(),
            label: name,
            description: String::new(),
            show: true,
            grips: 0,
            kind: DynamicParameterKind::Lookup {
                position,
                inputs: vec![input.id],
                rows,
            },
        },
    ));
    Some(id)
}
