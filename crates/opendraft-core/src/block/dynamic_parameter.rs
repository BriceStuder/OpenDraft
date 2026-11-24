//! What a parameter is made of, whatever its kind: its line or base point, the
//! value an untouched insert has, how a value is brought into its set, and the
//! value an insert's state gives it.

use std::f64::consts::TAU;

use crate::block::{
    DynamicInstanceState, DynamicParameter, DynamicParameterKind, DynamicValue, DynamicValueSet,
};
use crate::Point;

use super::dynamic_eval::{DynamicBlockError, EPS};

/// A linear parameter's line, in block coordinates.
#[derive(Clone, Copy, Debug)]
pub(super) struct Axis {
    pub start: Point,
    pub dir: (f64, f64),
    pub length: f64,
}

impl DynamicParameter {
    pub(super) fn axis(&self) -> Option<Axis> {
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
            DynamicParameterKind::Flip { base, end } => {
                !(end.x - base.x).hypot(end.y - base.y).is_finite()
                    || (end.x - base.x).hypot(end.y - base.y) <= EPS
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
            DynamicParameterKind::Rotation { .. } | DynamicParameterKind::Flip { .. } => {
                (!self.is_degenerate()).then_some(0.0)
            }
        }
    }

    /// Whether its value is a yes or no (1 or 0): a flip.
    pub fn is_flag(&self) -> bool {
        matches!(self.kind, DynamicParameterKind::Flip { .. })
    }

    /// The values it may take.
    pub fn value_set(&self) -> &DynamicValueSet {
        /// A flip is not flipped (0) or flipped (1).
        static FLAG: DynamicValueSet = DynamicValueSet::Range {
            min: 0.0,
            max: 1.0,
            increment: Some(1.0),
        };
        match &self.kind {
            DynamicParameterKind::Linear { value_set, .. }
            | DynamicParameterKind::Rotation { value_set, .. } => value_set,
            DynamicParameterKind::Flip { .. } => &FLAG,
        }
    }

    /// `value` as this parameter's kind of value.
    pub fn make_value(&self, value: f64) -> DynamicValue {
        if self.is_angle() {
            DynamicValue::Angle(value)
        } else if self.is_flag() {
            DynamicValue::Bool(value != 0.0)
        } else {
            DynamicValue::Distance(value)
        }
    }

    /// The number in `value` when it is this parameter's kind of value.
    pub fn number_of(&self, value: &DynamicValue) -> Option<f64> {
        match (&self.kind, value) {
            (DynamicParameterKind::Linear { .. }, DynamicValue::Distance(v))
            | (DynamicParameterKind::Rotation { .. }, DynamicValue::Angle(v)) => Some(*v),
            (DynamicParameterKind::Flip { .. }, DynamicValue::Bool(b)) => {
                Some(if *b { 1.0 } else { 0.0 })
            }
            _ => None,
        }
    }

    /// The value of the set nearest `value` (see [`DynamicValueSet::normalize`]),
    /// where an angle also counts turns: with no set it is brought into 0 to
    /// a full turn, and in a list the nearest is the one a shorter turn away.
    pub fn normalize(&self, value: f64) -> Option<f64> {
        if self.is_flag() {
            return value
                .is_finite()
                .then_some(if value >= 0.5 { 1.0 } else { 0.0 });
        }
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
    pub(super) fn value(
        &self,
        state: Option<&DynamicInstanceState>,
    ) -> Result<f64, DynamicBlockError> {
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
    pub(super) fn current(
        &self,
        state: Option<&DynamicInstanceState>,
    ) -> Result<(f64, Axis), DynamicBlockError> {
        let axis = self.axis().ok_or(DynamicBlockError::Degenerate(self.id))?;
        Ok((self.value(state)?, axis))
    }
}
