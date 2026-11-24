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
use std::f64::consts::TAU;
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

/// A linear parameter's line, in block coordinates.
#[derive(Clone, Copy, Debug)]
struct Axis {
    start: Point,
    dir: (f64, f64),
    length: f64,
}

impl DynamicParameter {
    fn axis(&self) -> Option<Axis> {
        let DynamicParameterKind::Linear { start, end, .. } = &self.kind else {
            return None;
        };
        let (vx, vy) = (end.x - start.x, end.y - start.y);
        let length = vx.hypot(vy);
        (length.is_finite() && length > EPS).then(|| Axis {
            start: *start,
            dir: (vx / length, vy / length),
            length,
        })
    }

    /// Whether its points make a line with a length (a linear parameter), or
    /// it has a base and a radius (a rotation).
    pub fn is_degenerate(&self) -> bool {
        match &self.kind {
            DynamicParameterKind::Linear { .. } => self.axis().is_none(),
            DynamicParameterKind::Rotation { base, radius, .. } => {
                !(base.x.is_finite() && base.y.is_finite() && radius.is_finite() && *radius > EPS)
            }
        }
    }

    /// Whether its value is an angle (in radians) rather than a distance.
    pub fn is_angle(&self) -> bool {
        matches!(self.kind, DynamicParameterKind::Rotation { .. })
    }

    /// The value of the untouched block: the distance between a linear
    /// parameter's points, or no turn for a rotation.
    pub fn default_value(&self) -> Option<f64> {
        match &self.kind {
            DynamicParameterKind::Linear { .. } => self.axis().map(|a| a.length),
            DynamicParameterKind::Rotation { .. } => (!self.is_degenerate()).then_some(0.0),
        }
    }

    /// The values it may take.
    pub fn value_set(&self) -> &DynamicValueSet {
        let (DynamicParameterKind::Linear { value_set, .. }
        | DynamicParameterKind::Rotation { value_set, .. }) = &self.kind;
        value_set
    }

    /// `value` as this parameter's kind of value.
    pub fn make_value(&self, value: f64) -> DynamicValue {
        if self.is_angle() {
            DynamicValue::Angle(value)
        } else {
            DynamicValue::Distance(value)
        }
    }

    /// The number in `value` when it is this parameter's kind of value.
    pub fn number_of(&self, value: &DynamicValue) -> Option<f64> {
        match (self.is_angle(), value) {
            (false, DynamicValue::Distance(v)) | (true, DynamicValue::Angle(v)) => Some(*v),
            _ => None,
        }
    }

    /// The value of the set nearest `value` (see [`DynamicValueSet::normalize`]),
    /// where an angle also counts turns: with no set it is brought into 0 to
    /// a full turn, and in a list the nearest is the one a shorter turn away.
    pub fn normalize(&self, value: f64) -> Option<f64> {
        if !self.is_angle() {
            return self.value_set().normalize(value);
        }
        if !value.is_finite() {
            return None;
        }
        match self.value_set() {
            DynamicValueSet::Any => Some(value.rem_euclid(TAU)),
            DynamicValueSet::List(xs) if self.value_set().is_valid() => {
                let apart = |x: f64| (x - value).rem_euclid(TAU).min((value - x).rem_euclid(TAU));
                xs.iter()
                    .copied()
                    .min_by(|a, b| apart(*a).total_cmp(&apart(*b)))
            }
            set => set.normalize(value),
        }
    }

    /// The value an insert with `state` has.
    fn value(&self, state: Option<&DynamicInstanceState>) -> Result<f64, DynamicBlockError> {
        let default = self
            .default_value()
            .ok_or(DynamicBlockError::Degenerate(self.id))?;
        match state.and_then(|s| s.get(self.id)) {
            None => Ok(default),
            Some(v) => self
                .number_of(v)
                .filter(|v| v.is_finite())
                .ok_or(DynamicBlockError::BadValue(self.id)),
        }
    }

    /// The value an insert with `state` has, and the axis a linear parameter
    /// measures along.
    fn current(
        &self,
        state: Option<&DynamicInstanceState>,
    ) -> Result<(f64, Axis), DynamicBlockError> {
        let axis = self.axis().ok_or(DynamicBlockError::Degenerate(self.id))?;
        Ok((self.value(state)?, axis))
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
            let held = p.number_of(&entry.value).is_some_and(|v| {
                p.normalize(v)
                    .is_some_and(|n| (n - v).abs() <= 1e-9 * v.abs().max(1.0))
            });
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
        if let DynamicActionKind::Rotate { objects } = &action.kind {
            rotate(&mut out, &position, objects, parameter, state)?;
            continue;
        }
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
            DynamicActionKind::Rotate { .. } => continue,
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

/// Turn `objects` about a rotation parameter's base point by its value.
fn rotate(
    out: &mut [BlockEntity],
    position: &HashMap<BlockObjectId, usize>,
    objects: &[BlockObjectId],
    parameter: &DynamicParameter,
    state: &DynamicInstanceState,
) -> Result<(), DynamicBlockError> {
    let DynamicParameterKind::Rotation { base, .. } = &parameter.kind else {
        return Err(DynamicBlockError::BadValue(parameter.id));
    };
    let angle = parameter.value(Some(state))?;
    if angle == 0.0 {
        return Ok(());
    }
    for id in objects {
        let at = *position
            .get(id)
            .ok_or(DynamicBlockError::MissingObject(*id))?;
        let object = &mut out[at];
        if matches!(object.entity, Entity::AttributeDefinition(_)) {
            return Err(DynamicBlockError::UnsupportedObject(*id));
        }
        object.entity = object.entity.rotated(*base, angle);
    }
    Ok(())
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
        let axis = p.axis()?;
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
        let v = p.number_of(&value)?;
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
        let current = p.value(self.dynamic.as_deref()).ok()?;
        let at = self.transform().inverse()?.apply(to_drawing);
        let raw = match &p.kind {
            DynamicParameterKind::Linear { .. } => {
                let axis = p.axis()?;
                (at.x - axis.start.x) * axis.dir.0 + (at.y - axis.start.y) * axis.dir.1
            }
            DynamicParameterKind::Rotation { base, .. } => (at.y - base.y).atan2(at.x - base.x),
        };
        let value = p.normalize(raw)?;
        if (value - current).abs() <= 1e-12 {
            return None;
        }
        self.with_dynamic_value(grip.parameter, p.make_value(value))
    }
}
