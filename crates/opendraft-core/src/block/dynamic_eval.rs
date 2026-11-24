//! Evaluating a dynamic block: what an insert's objects are once its values
//! are applied, and the grips and edits that change those values.
//!
//! Evaluation always starts from the authored definition and applies the
//! actions in schema order, so it is deterministic and never accumulates
//! drift. It works in block coordinates; only grip input and output cross the
//! insert's transform. It never panics: a schema or state it cannot follow is
//! an error, and the insert then draws as its authored definition.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

use super::dynamic_apply::apply;
use crate::block::{
    BlockDef, BlockEntity, BlockObjectId, DynamicBlock, DynamicGrip, DynamicInstanceState,
    DynamicParameter, DynamicParameterKind, DynamicValue, Insert, ParameterId,
};
use crate::Point;

/// Lengths at or below this are zero (the repository's scale-free epsilon).
pub const EPS: f64 = 1e-9;

/// Why a schema and state could not be evaluated.
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicBlockError {
    MissingParameter(ParameterId),
    MissingObject(BlockObjectId),
    /// A linear parameter whose two points coincide, or a rotation with no
    /// radius, or either not finite.
    Degenerate(ParameterId),
    /// A value that is not finite, or of the wrong type for its parameter.
    BadValue(ParameterId),
    /// An object an action cannot move (an attribute definition).
    UnsupportedObject(BlockObjectId),
}

impl fmt::Display for DynamicBlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingParameter(p) => write!(f, "no parameter {}", p.0),
            Self::MissingObject(o) => write!(f, "no object {}", o.0),
            Self::Degenerate(p) => write!(f, "parameter {} has no length", p.0),
            Self::BadValue(p) => write!(f, "parameter {} has an unusable value", p.0),
            Self::UnsupportedObject(o) => write!(f, "object {} cannot be moved by an action", o.0),
        }
    }
}

impl std::error::Error for DynamicBlockError {}

impl DynamicBlock {
    /// Why `state` could not be the values of an insert of this block, or
    /// `Ok`: every entry names a parameter, is a finite distance, and is a
    /// value its parameter's set holds.
    pub fn check_state(&self, state: &DynamicInstanceState) -> Result<(), String> {
        for entry in &state.values {
            let Some(p) = self.parameters.iter().find(|p| p.id == entry.parameter) else {
                return Err(format!("parameter {} does not exist", entry.parameter.0));
            };
            let held = match &entry.value {
                DynamicValue::Point(end) if p.has_point() => p.fit_end(*end).is_some_and(|n| {
                    (n.x - end.x).abs() <= 1e-9 * end.x.abs().max(1.0)
                        && (n.y - end.y).abs() <= 1e-9 * end.y.abs().max(1.0)
                }),
                value => p.number_of(value).is_some_and(|v| {
                    p.normalize(v)
                        .is_some_and(|n| (n - v).abs() <= 1e-9 * v.abs().max(1.0))
                }),
            };
            if !held {
                return Err(format!(
                    "the value of parameter {} is not one its value set allows",
                    p.id.0
                ));
            }
        }
        Ok(())
    }
}

/// The objects of `block` with `state` applied. An untouched insert borrows
/// the definition's own.
pub fn evaluate<'a>(
    block: &'a BlockDef,
    state: Option<&DynamicInstanceState>,
) -> Result<Cow<'a, [BlockEntity]>, DynamicBlockError> {
    let (Some(schema), Some(state)) = (&block.dynamic, state) else {
        return Ok(Cow::Borrowed(&block.entities));
    };
    if state.is_default() {
        return Ok(Cow::Borrowed(&block.entities));
    }
    let position: HashMap<BlockObjectId, usize> = block
        .entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id, i))
        .collect();
    let mut out = block.entities.clone();
    for action in &schema.actions {
        let parameter = schema
            .parameters
            .iter()
            .find(|p| p.id == action.parameter)
            .ok_or(DynamicBlockError::MissingParameter(action.parameter))?;
        apply(&mut out, &position, action, parameter, state)?;
    }
    Ok(Cow::Owned(out))
}

impl Insert {
    /// The objects this insert draws, in block coordinates: the definition's
    /// with the insert's values applied. A schema or state that cannot be
    /// followed leaves the authored objects.
    pub fn evaluated(&self) -> Cow<'_, [BlockEntity]> {
        evaluate(&self.block, self.dynamic.as_deref())
            .unwrap_or(Cow::Borrowed(&self.block.entities))
    }

    /// The two numbers of a polar, XY or point parameter on this insert, in
    /// the block's units: a polar parameter's distance and angle (radians
    /// from +X), an XY parameter's distances, a point's coordinates.
    pub fn dynamic_pair(&self, parameter: ParameterId) -> Option<(f64, f64)> {
        let p = self.parameter(parameter)?;
        p.parts_of(p.end_point(self.dynamic.as_deref()).ok()?)
    }

    /// The value of `parameter` on this insert, in the block's units.
    pub fn dynamic_value(&self, parameter: ParameterId) -> Option<f64> {
        let p = self.parameter(parameter)?;
        p.value(self.dynamic.as_deref()).ok()
    }

    /// How long one unit of `parameter`'s value is in the drawing: the
    /// length its line has after the insert's transform. Multiply a value
    /// by it to show the value in drawing units, divide to read one typed
    /// there. `None` when the parameter is unknown, or the insert flattens
    /// the line to nothing. An angle is shown in degrees, whatever the
    /// insert's scale, so its factor is 180 over pi.
    pub fn dynamic_scale(&self, parameter: ParameterId) -> Option<f64> {
        let p = self.parameter(parameter)?;
        if p.is_angle() {
            return Some(180.0 / std::f64::consts::PI);
        }
        if p.is_flag() {
            return Some(1.0);
        }
        let Some(axis) = p.axis() else {
            // An XY distance or a point's coordinate: along the block's X.
            let map = self.transform();
            let (a, b) = (
                map.apply(Point::new(0.0, 0.0)),
                map.apply(Point::new(1.0, 0.0)),
            );
            let scale = (b.x - a.x).hypot(b.y - a.y);
            return (p.has_point() && scale.is_finite() && scale > EPS).then_some(scale);
        };
        let map = self.transform();
        let a = map.apply(axis.start);
        let b = map.apply(Point::new(
            axis.start.x + axis.dir.0,
            axis.start.y + axis.dir.1,
        ));
        let scale = (b.x - a.x).hypot(b.y - a.y);
        (scale.is_finite() && scale > EPS).then_some(scale)
    }

    fn parameter(&self, id: ParameterId) -> Option<&DynamicParameter> {
        self.block
            .dynamic
            .as_ref()?
            .parameters
            .iter()
            .find(|p| p.id == id)
    }

    /// This insert with `parameter` set to `value`, or `None` when the block
    /// has no such parameter, the value is of the wrong type or not finite.
    /// A value equal to the default is not stored, and an insert left with
    /// no values has no state.
    pub fn with_dynamic_value(
        &self,
        parameter: ParameterId,
        value: DynamicValue,
    ) -> Option<Insert> {
        let p = self.parameter(parameter)?;
        let mut state = self.dynamic.as_deref().cloned().unwrap_or_default();
        if let (DynamicValue::Point(end), Some(drawn)) = (&value, p.drawn_end()) {
            let drawn = &drawn;
            if !(end.x.is_finite() && end.y.is_finite()) {
                return None;
            }
            let same = (end.x - drawn.x).abs() <= 1e-12 && (end.y - drawn.y).abs() <= 1e-12;
            if same {
                state.values.retain(|e| e.parameter != parameter);
            } else {
                state.set(parameter, value);
            }
            return Some(Insert {
                dynamic: (!state.is_default()).then(|| Box::new(state)),
                ..self.clone()
            });
        }
        let v = p.number_of(&value)?;
        if !v.is_finite() {
            return None;
        }
        if (v - p.default_value()?).abs() <= 1e-12 {
            state.values.retain(|e| e.parameter != parameter);
        } else {
            state.set(parameter, value);
        }
        Some(Insert {
            dynamic: (!state.is_default()).then(|| Box::new(state)),
            ..self.clone()
        })
    }

    /// The grips the parameters show, in drawing coordinates, in schema
    /// order. A linear parameter's end grip sits `value` along its line from
    /// its start; a rotation's sits its radius from the base, turned by the
    /// value.
    pub fn dynamic_grips(&self) -> Vec<(DynamicGrip, Point)> {
        let Some(schema) = &self.block.dynamic else {
            return Vec::new();
        };
        let map = self.transform();
        schema
            .parameters
            .iter()
            .filter(|p| p.grips >= 1)
            .filter_map(|p| {
                if p.has_point() {
                    let end = p.end_point(self.dynamic.as_deref()).ok()?;
                    let grip = DynamicGrip {
                        parameter: p.id,
                        index: 0,
                    };
                    return Some((grip, map.apply(end)));
                }
                let value = p.value(self.dynamic.as_deref()).ok()?;
                let at = match &p.kind {
                    DynamicParameterKind::Linear { .. } => {
                        let axis = p.axis()?;
                        Point::new(
                            axis.start.x + axis.dir.0 * value,
                            axis.start.y + axis.dir.1 * value,
                        )
                    }
                    DynamicParameterKind::Rotation { base, radius, .. } => {
                        Point::new(base.x + radius * value.cos(), base.y + radius * value.sin())
                    }
                    // The middle of the mirror line.
                    DynamicParameterKind::Flip { base, end } => {
                        Point::new((base.x + end.x) / 2.0, (base.y + end.y) / 2.0)
                    }
                    DynamicParameterKind::Polar { .. }
                    | DynamicParameterKind::Xy { .. }
                    | DynamicParameterKind::Point { .. } => return None,
                };
                let at = map.apply(at);
                let grip = DynamicGrip {
                    parameter: p.id,
                    index: 0,
                };
                Some((grip, at))
            })
            .collect()
    }

    /// This insert with a parameter's grip dragged to `to_drawing`: the drag
    /// is mapped back into block coordinates, projected on the parameter's
    /// line and brought into its value set. `None` for a degenerate insert,
    /// an unknown or reserved grip, a point that is not finite, or a drag
    /// that changes nothing.
    pub fn with_dynamic_grip_moved(&self, grip: DynamicGrip, to_drawing: Point) -> Option<Insert> {
        if grip.index != 0 || !(to_drawing.x.is_finite() && to_drawing.y.is_finite()) {
            return None;
        }
        let p = self.parameter(grip.parameter)?;
        let at = self.transform().inverse()?.apply(to_drawing);
        if p.has_point() {
            let now = p.end_point(self.dynamic.as_deref()).ok()?;
            let end = p.fit_end(at)?;
            if (end.x - now.x).abs() <= 1e-12 && (end.y - now.y).abs() <= 1e-12 {
                return None;
            }
            return self.with_dynamic_value(grip.parameter, DynamicValue::Point(end));
        }
        let current = p.value(self.dynamic.as_deref()).ok()?;
        let raw = match &p.kind {
            DynamicParameterKind::Linear { .. } => {
                let axis = p.axis()?;
                (at.x - axis.start.x) * axis.dir.0 + (at.y - axis.start.y) * axis.dir.1
            }
            DynamicParameterKind::Rotation { base, .. } => (at.y - base.y).atan2(at.x - base.x),
            DynamicParameterKind::Polar { .. }
            | DynamicParameterKind::Xy { .. }
            | DynamicParameterKind::Point { .. } => return None,
            // Flipped when the point is left of the line from its base to its end.
            DynamicParameterKind::Flip { base, end } => {
                let side = (end.x - base.x) * (at.y - base.y) - (end.y - base.y) * (at.x - base.x);
                if side > 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
        };
        let value = p.normalize(raw)?;
        if (value - current).abs() <= 1e-12 {
            return None;
        }
        self.with_dynamic_value(grip.parameter, p.make_value(value))
    }
}
