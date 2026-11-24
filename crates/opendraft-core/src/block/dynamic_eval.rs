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

use crate::block::{
    BlockDef, BlockEntity, BlockObjectId, DynamicActionKind, DynamicBlock, DynamicGrip,
    DynamicInstanceState, DynamicParameter, DynamicParameterKind, DynamicValue, DynamicValueSet,
    Insert, ParameterId,
};
use crate::{Entity, Point};

/// Lengths at or below this are zero (the repository's scale-free epsilon).
pub const EPS: f64 = 1e-9;

/// Why a schema and state could not be evaluated.
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicBlockError {
    MissingParameter(ParameterId),
    MissingObject(BlockObjectId),
    /// A linear parameter whose two points coincide, or are not finite.
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

impl DynamicValueSet {
    /// Whether the set can hold a value: a list is non-empty, finite and
    /// strictly increasing; a range is finite with `min <= max` and a
    /// positive increment, if it has one.
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Any => true,
            Self::List(xs) => {
                !xs.is_empty()
                    && xs.iter().all(|x| x.is_finite())
                    && xs.windows(2).all(|w| w[0] < w[1])
            }
            Self::Range {
                min,
                max,
                increment,
            } => {
                min.is_finite()
                    && max.is_finite()
                    && min <= max
                    && increment.is_none_or(|i| i.is_finite() && i > 0.0)
            }
        }
    }

    /// The value of the set nearest `value`, or `None` when `value` is not
    /// finite or the set is invalid. A list takes the nearest entry (the
    /// lower on a tie); a range clamps, then rounds to its increment counted
    /// from `min` (never past `max`).
    pub fn normalize(&self, value: f64) -> Option<f64> {
        if !value.is_finite() || !self.is_valid() {
            return None;
        }
        Some(match self {
            Self::Any => value,
            Self::List(xs) => {
                let mut best = xs[0];
                for &x in &xs[1..] {
                    if (x - value).abs() < (best - value).abs() {
                        best = x;
                    }
                }
                best
            }
            Self::Range {
                min,
                max,
                increment,
            } => {
                let v = value.clamp(*min, *max);
                match increment {
                    Some(i) => {
                        let mut steps = ((v - min) / i).round();
                        // Rounding up past the end lands on no step: take
                        // the last one inside the range.
                        if min + steps * i > *max {
                            steps -= 1.0;
                        }
                        (min + steps * i).clamp(*min, *max)
                    }
                    None => v,
                }
            }
        })
    }
}

/// A linear parameter's line, in block coordinates.
#[derive(Clone, Copy, Debug)]
struct Axis {
    start: Point,
    dir: (f64, f64),
    length: f64,
}

impl DynamicParameter {
    fn axis(&self) -> Option<Axis> {
        let DynamicParameterKind::Linear { start, end, .. } = &self.kind;
        let (vx, vy) = (end.x - start.x, end.y - start.y);
        let length = vx.hypot(vy);
        (length.is_finite() && length > EPS).then(|| Axis {
            start: *start,
            dir: (vx / length, vy / length),
            length,
        })
    }

    /// Whether its points make a line with a length.
    pub fn is_degenerate(&self) -> bool {
        self.axis().is_none()
    }

    /// The value of the untouched block: the distance between its points.
    pub fn default_value(&self) -> Option<f64> {
        self.axis().map(|a| a.length)
    }

    /// The values it may take.
    pub fn value_set(&self) -> &DynamicValueSet {
        let DynamicParameterKind::Linear { value_set, .. } = &self.kind;
        value_set
    }

    /// The value an insert with `state` has, and the axis it measures along.
    fn current(
        &self,
        state: Option<&DynamicInstanceState>,
    ) -> Result<(f64, Axis), DynamicBlockError> {
        let axis = self.axis().ok_or(DynamicBlockError::Degenerate(self.id))?;
        match state.and_then(|s| s.get(self.id)) {
            None => Ok((axis.length, axis)),
            Some(DynamicValue::Distance(v)) if v.is_finite() => Ok((*v, axis)),
            Some(_) => Err(DynamicBlockError::BadValue(self.id)),
        }
    }
}

impl DynamicBlock {
    /// Why `state` could not be the values of an insert of this block, or
    /// `Ok`: every entry names a parameter, is a finite distance, and is a
    /// value its parameter's set holds.
    pub fn check_state(&self, state: &DynamicInstanceState) -> Result<(), String> {
        for entry in &state.values {
            let Some(p) = self.parameters.iter().find(|p| p.id == entry.parameter) else {
                return Err(format!("parameter {} does not exist", entry.parameter.0));
            };
            let held = match entry.value {
                DynamicValue::Distance(v) => p
                    .value_set()
                    .normalize(v)
                    .is_some_and(|n| (n - v).abs() <= 1e-9 * v.abs().max(1.0)),
                _ => false,
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
        let (current, axis) = parameter.current(Some(state))?;
        let (objects, frame, multiplier, offset) = match &action.kind {
            DynamicActionKind::Move {
                objects,
                multiplier,
                angle_offset,
            } => (objects, None, *multiplier, *angle_offset),
            DynamicActionKind::Stretch {
                objects,
                frame,
                multiplier,
                angle_offset,
            } => (objects, Some(frame), *multiplier, *angle_offset),
        };
        let amount = (current - axis.length) * multiplier;
        if !amount.is_finite() {
            return Err(DynamicBlockError::BadValue(parameter.id));
        }
        if amount == 0.0 {
            continue;
        }
        let (sin, cos) = offset.sin_cos();
        let (ux, uy) = (
            axis.dir.0 * cos - axis.dir.1 * sin,
            axis.dir.0 * sin + axis.dir.1 * cos,
        );
        let (dx, dy) = (ux * amount, uy * amount);
        for id in objects {
            let at = *position
                .get(id)
                .ok_or(DynamicBlockError::MissingObject(*id))?;
            let object = &mut out[at];
            if matches!(object.entity, Entity::AttributeDefinition(_)) {
                return Err(DynamicBlockError::UnsupportedObject(*id));
            }
            match frame {
                None => object.entity = object.entity.translated(dx, dy),
                Some(frame) => {
                    let inside = |p: Point| frame.contains(&crate::Bounds::from_points(p, p));
                    if let Some(moved) = object.entity.stretched(inside, dx, dy) {
                        object.entity = moved;
                    }
                }
            }
        }
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

    /// The value of `parameter` on this insert, in the block's units.
    pub fn dynamic_value(&self, parameter: ParameterId) -> Option<f64> {
        let p = self.parameter(parameter)?;
        p.current(self.dynamic.as_deref()).ok().map(|(v, _)| v)
    }

    /// How long one unit of `parameter`'s value is in the drawing: the
    /// length its line has after the insert's transform. Multiply a value
    /// by it to show the value in drawing units, divide to read one typed
    /// there. `None` when the parameter is unknown, or the insert flattens
    /// the line to nothing.
    pub fn dynamic_scale(&self, parameter: ParameterId) -> Option<f64> {
        let axis = self.parameter(parameter)?.axis()?;
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
        let DynamicValue::Distance(v) = value else {
            return None;
        };
        if !v.is_finite() {
            return None;
        }
        let mut state = self.dynamic.as_deref().cloned().unwrap_or_default();
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
    /// its start.
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
                let (value, axis) = p.current(self.dynamic.as_deref()).ok()?;
                let at = map.apply(Point::new(
                    axis.start.x + axis.dir.0 * value,
                    axis.start.y + axis.dir.1 * value,
                ));
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
        let (current, axis) = p.current(self.dynamic.as_deref()).ok()?;
        let at = self.transform().inverse()?.apply(to_drawing);
        let raw = (at.x - axis.start.x) * axis.dir.0 + (at.y - axis.start.y) * axis.dir.1;
        let value = p.value_set().normalize(raw)?;
        if (value - current).abs() <= 1e-12 {
            return None;
        }
        self.with_dynamic_value(grip.parameter, DynamicValue::Distance(value))
    }
}
