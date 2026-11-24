//! A lookup parameter: a table whose rows each give a value to every input
//! parameter, picking a row sets them, and the row an insert shows is the one
//! its inputs match now. Also the generic "choice" a drop-down shows, for a
//! visibility state or a lookup row.

use std::collections::HashSet;

use crate::block::{
    DynamicBlock, DynamicParameter, DynamicParameterKind, Insert, LookupRow, ParameterId,
};

/// What the drop-down shows when no row matches the inputs.
pub const CUSTOM: &str = "Custom";

/// Most rows a lookup table may have.
pub const MAX_LOOKUP_ROWS: usize = 1024;
/// Most inputs a lookup table may have.
pub const MAX_LOOKUP_INPUTS: usize = 16;

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

impl DynamicParameter {
    /// The inputs and rows of a lookup parameter.
    pub fn lookup(&self) -> Option<(&[ParameterId], &[LookupRow])> {
        match &self.kind {
            DynamicParameterKind::Lookup { inputs, rows, .. } => Some((inputs, rows)),
            _ => None,
        }
    }
}

impl DynamicBlock {
    /// Why the lookup parameters could not be in this block, or `Ok`: each
    /// has inputs (linear, rotation or flip parameters, no repeats) and rows
    /// with names (not empty, not repeated, not "Custom") and one value for
    /// each input that the input's value set holds, and no two rows give the
    /// inputs the same values (a reverse lookup could not tell them apart).
    pub(super) fn check_lookups(&self) -> Result<(), String> {
        for p in self.parameters.iter().filter(|p| p.lookup().is_some()) {
            let (inputs, rows) = p.lookup().unwrap_or_default();
            let bad = |why: &str| Err(format!("lookup parameter {} {why}", p.id.0));
            if inputs.is_empty() || inputs.len() > MAX_LOOKUP_INPUTS {
                return bad("has no inputs or too many");
            }
            if rows.is_empty() || rows.len() > MAX_LOOKUP_ROWS {
                return bad("has no rows or too many");
            }
            let mut seen = HashSet::new();
            let mut sources = Vec::new();
            for id in inputs {
                let source = self.parameters.iter().find(|q| q.id == *id);
                let Some(q) = source.filter(|q| {
                    matches!(
                        q.kind,
                        DynamicParameterKind::Linear { .. }
                            | DynamicParameterKind::Rotation { .. }
                            | DynamicParameterKind::Flip { .. }
                    )
                }) else {
                    return bad("has an input that is not a linear, rotation or flip parameter");
                };
                if !seen.insert(*id) {
                    return bad("names an input twice");
                }
                sources.push(q);
            }
            let mut labels = HashSet::new();
            for (i, row) in rows.iter().enumerate() {
                let label = row.label.trim();
                if label.is_empty()
                    || label.eq_ignore_ascii_case(CUSTOM)
                    || !labels.insert(label.to_ascii_lowercase())
                {
                    return bad("has a row with no name, a name that repeats or is \"Custom\"");
                }
                if row.values.len() != sources.len() {
                    return bad("has a row without a value for every input");
                }
                let held = row
                    .values
                    .iter()
                    .zip(&sources)
                    .all(|(v, q)| q.normalize(*v).is_some_and(|n| same(n, *v)));
                if !held {
                    return bad("has a row value its input's value set does not hold");
                }
                let twin = rows[..i]
                    .iter()
                    .any(|r| r.values.iter().zip(&row.values).all(|(a, b)| same(*a, *b)));
                if twin {
                    return bad("has two rows with the same values");
                }
            }
        }
        Ok(())
    }
}

impl Insert {
    /// The choices of a visibility or lookup parameter and the one this
    /// insert shows: the state names, or the row labels and "Custom".
    pub fn choice(&self, parameter: ParameterId) -> Option<(Vec<String>, usize)> {
        let schema = self.block.dynamic.as_ref()?;
        let p = schema.parameters.iter().find(|p| p.id == parameter)?;
        if let Some(states) = p.visibility_states() {
            let (_, shown) = self.visibility_state()?;
            let at = states.iter().position(|s| s.name == shown)?;
            return Some((states.iter().map(|s| s.name.clone()).collect(), at));
        }
        let (inputs, rows) = p.lookup()?;
        let now: Vec<f64> = inputs
            .iter()
            .map(|i| self.dynamic_value(*i))
            .collect::<Option<_>>()?;
        let at = rows
            .iter()
            .position(|r| r.values.iter().zip(&now).all(|(a, b)| same(*a, *b)))
            .unwrap_or(rows.len());
        let mut names: Vec<String> = rows.iter().map(|r| r.label.clone()).collect();
        names.push(CUSTOM.to_owned());
        Some((names, at))
    }

    /// This insert with choice number `index` of `parameter` picked, or
    /// `None` when there is no such choice, it is "Custom" (which sets
    /// nothing) or it changes nothing.
    pub fn with_choice(&self, parameter: ParameterId, index: usize) -> Option<Insert> {
        let schema = self.block.dynamic.as_ref()?;
        let p = schema.parameters.iter().find(|p| p.id == parameter)?;
        let out = if let Some(states) = p.visibility_states() {
            self.with_visibility_state(&states.get(index)?.name)?
        } else {
            let (inputs, rows) = p.lookup()?;
            let row = rows.get(index)?;
            let mut out = self.clone();
            for (input, value) in inputs.iter().zip(&row.values) {
                let q = schema.parameters.iter().find(|q| q.id == *input)?;
                out = out.with_dynamic_value(*input, q.make_value(*value))?;
            }
            out
        };
        (out != *self).then_some(out)
    }
}
