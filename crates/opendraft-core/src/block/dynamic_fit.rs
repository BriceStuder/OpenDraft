//! Keeping a schema and the values of its inserts in step when the
//! definition is edited: the Block Editor moves the schema with the
//! geometry, drops objects the user deleted from the actions, and an insert
//! rebound to a new definition keeps the values the new schema still allows.

use crate::block::{
    BlockObjectId, DynamicActionKind, DynamicBlock, DynamicInstanceState, DynamicParameterKind,
    DynamicValue, DynamicValueEntry,
};
use crate::{Bounds, Point};

/// A schema pruned of deleted objects, and what that cost.
#[derive(Clone, Debug, PartialEq)]
pub struct Pruned {
    pub schema: DynamicBlock,
    /// One line for each action that lost objects.
    pub notes: Vec<String>,
    /// Names of the actions removed because no object was left.
    pub removed: Vec<String>,
}

impl DynamicBlock {
    /// The schema moved by `(dx, dy)`: parameter points and stretch frames.
    pub fn translated(&self, dx: f64, dy: f64) -> DynamicBlock {
        let mut out = self.clone();
        for p in &mut out.parameters {
            match &mut p.kind {
                DynamicParameterKind::Linear { start, end, .. } => {
                    *start = start.translated(dx, dy);
                    *end = end.translated(dx, dy);
                }
                DynamicParameterKind::Rotation { base, .. } => *base = base.translated(dx, dy),
                DynamicParameterKind::Flip { base, end }
                | DynamicParameterKind::Polar { base, end, .. } => {
                    *base = base.translated(dx, dy);
                    *end = end.translated(dx, dy);
                }
            }
        }
        for a in &mut out.actions {
            if let DynamicActionKind::Stretch { frame, .. } = &mut a.kind {
                *frame = Bounds {
                    min: Point::new(frame.min.x + dx, frame.min.y + dy),
                    max: Point::new(frame.max.x + dx, frame.max.y + dy),
                };
            }
        }
        out
    }

    /// The schema without the objects `exists` refuses. An action left with
    /// no object is removed.
    pub fn pruned(&self, exists: impl Fn(BlockObjectId) -> bool) -> Pruned {
        let mut schema = self.clone();
        let (mut notes, mut removed) = (Vec::new(), Vec::new());
        schema.actions.retain_mut(|a| {
            let (DynamicActionKind::Move { objects, .. }
            | DynamicActionKind::Stretch { objects, .. }
            | DynamicActionKind::Rotate { objects }
            | DynamicActionKind::Flip { objects }
            | DynamicActionKind::Scale { objects }) = &mut a.kind;
            let before = objects.len();
            objects.retain(|o| exists(*o));
            let lost = before - objects.len();
            if objects.is_empty() {
                removed.push(a.name.clone());
                return false;
            }
            if lost > 0 {
                notes.push(format!(
                    "Action \"{}\" lost {lost} deleted object{}.",
                    a.name,
                    if lost == 1 { "" } else { "s" }
                ));
            }
            true
        });
        Pruned {
            schema,
            notes,
            removed,
        }
    }

    /// The values of `state` this schema still allows: an entry for a
    /// parameter that is gone or of another type is dropped, and a value
    /// outside the parameter's set is brought into it. `None` when nothing
    /// is left to override.
    pub fn fit_state(&self, state: &DynamicInstanceState) -> Option<DynamicInstanceState> {
        let values: Vec<DynamicValueEntry> = state
            .values
            .iter()
            .filter_map(|e| {
                let p = self.parameters.iter().find(|p| p.id == e.parameter)?;
                if let (true, DynamicValue::Point(end)) = (p.is_polar(), &e.value) {
                    let fitted = p.fit_end(*end)?;
                    let DynamicParameterKind::Polar { end: drawn, .. } = &p.kind else {
                        return None;
                    };
                    if (fitted.x - drawn.x).abs() <= 1e-12 && (fitted.y - drawn.y).abs() <= 1e-12 {
                        return None;
                    }
                    return Some(DynamicValueEntry {
                        parameter: e.parameter,
                        value: DynamicValue::Point(fitted),
                    });
                }
                let value = p.normalize(p.number_of(&e.value)?)?;
                if p.default_value()
                    .is_some_and(|d| (value - d).abs() <= 1e-12)
                {
                    return None;
                }
                Some(DynamicValueEntry {
                    parameter: e.parameter,
                    value: p.make_value(value),
                })
            })
            .collect();
        (!values.is_empty()).then_some(DynamicInstanceState { values })
    }
}
