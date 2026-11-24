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
        let (DynamicParameterKind::Linear { start, end, .. }
        | DynamicParameterKind::Polar {
            base: start, end, ..
        }) = &self.kind
        else {
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
            DynamicParameterKind::Linear { .. } | DynamicParameterKind::Polar { .. } => {
                self.axis().is_none()
            }
            DynamicParameterKind::Rotation { base, radius, .. } => {
                !(base.x.is_finite() && base.y.is_finite() && radius.is_finite() && *radius > EPS)
            }
            DynamicParameterKind::Flip { base, end } => {
                !(end.x - base.x).hypot(end.y - base.y).is_finite()
                    || (end.x - base.x).hypot(end.y - base.y) <= EPS
            }
            DynamicParameterKind::Xy { base, end, .. } => {
                !(base.x.is_finite()
                    && base.y.is_finite()
                    && end.x.is_finite()
                    && end.y.is_finite())
            }
            DynamicParameterKind::Point { position }
            | DynamicParameterKind::Visibility { position, .. }
            | DynamicParameterKind::Lookup { position, .. } => {
                !(position.x.is_finite() && position.y.is_finite())
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
            DynamicParameterKind::Linear { .. } | DynamicParameterKind::Polar { .. } => {
                self.axis().map(|a| a.length)
            }
            DynamicParameterKind::Rotation { .. } | DynamicParameterKind::Flip { .. } => {
                (!self.is_degenerate()).then_some(0.0)
            }
            // These have a point, not a number.
            DynamicParameterKind::Xy { .. }
            | DynamicParameterKind::Point { .. }
            | DynamicParameterKind::Visibility { .. }
            | DynamicParameterKind::Lookup { .. } => None,
        }
    }

    /// Whether its value is a yes or no (1 or 0): a flip.
    pub fn is_flag(&self) -> bool {
        matches!(self.kind, DynamicParameterKind::Flip { .. })
    }

    /// The values it may take.
    pub fn value_set(&self) -> &DynamicValueSet {
        /// A flip is not flipped (0) or flipped (1).
        static ANY: DynamicValueSet = DynamicValueSet::Any;
        static FLAG: DynamicValueSet = DynamicValueSet::Range {
            min: 0.0,
            max: 1.0,
            increment: Some(1.0),
        };
        match &self.kind {
            DynamicParameterKind::Linear { value_set, .. }
            | DynamicParameterKind::Rotation { value_set, .. } => value_set,
            DynamicParameterKind::Polar { distance_set, .. } => distance_set,
            DynamicParameterKind::Xy { x_set, .. } => x_set,
            DynamicParameterKind::Point { .. }
            | DynamicParameterKind::Visibility { .. }
            | DynamicParameterKind::Lookup { .. } => &ANY,
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

    pub fn is_xy(&self) -> bool {
        matches!(self.kind, DynamicParameterKind::Xy { .. })
    }

    pub fn is_polar(&self) -> bool {
        matches!(self.kind, DynamicParameterKind::Polar { .. })
    }

    /// Whether its value is a point: the end of a polar or XY parameter, or
    /// a point parameter's location.
    pub fn has_point(&self) -> bool {
        matches!(
            self.kind,
            DynamicParameterKind::Polar { .. }
                | DynamicParameterKind::Xy { .. }
                | DynamicParameterKind::Point { .. }
        )
    }

    /// Where the point of a polar, XY or point parameter is as drawn.
    pub fn drawn_end(&self) -> Option<Point> {
        match &self.kind {
            DynamicParameterKind::Polar { end, .. } | DynamicParameterKind::Xy { end, .. } => {
                Some(*end)
            }
            DynamicParameterKind::Point { position } => Some(*position),
            _ => None,
        }
    }

    /// The two numbers a point is shown as: a polar parameter's distance and
    /// angle (radians), an XY parameter's distances, a point's coordinates.
    pub fn parts_of(&self, end: Point) -> Option<(f64, f64)> {
        match &self.kind {
            DynamicParameterKind::Polar { base, .. } => {
                let (dx, dy) = (end.x - base.x, end.y - base.y);
                Some((dx.hypot(dy), dy.atan2(dx)))
            }
            DynamicParameterKind::Xy { base, .. } => Some((end.x - base.x, end.y - base.y)),
            DynamicParameterKind::Point { .. } => Some((end.x, end.y)),
            _ => None,
        }
    }

    /// The point whose numbers are `a` and `b` (the inverse of
    /// [`parts_of`](Self::parts_of)).
    pub fn point_of(&self, a: f64, b: f64) -> Option<Point> {
        match &self.kind {
            DynamicParameterKind::Polar { base, .. } => {
                Some(Point::new(base.x + a * b.cos(), base.y + a * b.sin()))
            }
            DynamicParameterKind::Xy { base, .. } => Some(Point::new(base.x + a, base.y + b)),
            DynamicParameterKind::Point { .. } => Some(Point::new(a, b)),
            _ => None,
        }
    }

    /// Whether the value sets it holds are all usable.
    pub fn sets_valid(&self) -> bool {
        match &self.kind {
            DynamicParameterKind::Polar {
                distance_set,
                angle_set,
                ..
            } => distance_set.is_valid() && angle_set.is_valid(),
            DynamicParameterKind::Xy { x_set, y_set, .. } => x_set.is_valid() && y_set.is_valid(),
            _ => self.value_set().is_valid(),
        }
    }

    /// Where the point of a polar, XY or point parameter is in an insert
    /// with `state`.
    pub fn end_point(
        &self,
        state: Option<&DynamicInstanceState>,
    ) -> Result<Point, DynamicBlockError> {
        let drawn = self
            .drawn_end()
            .ok_or(DynamicBlockError::BadValue(self.id))?;
        match state.and_then(|s| s.get(self.id)) {
            None => Ok(drawn),
            Some(DynamicValue::Point(p)) if p.x.is_finite() && p.y.is_finite() => Ok(*p),
            Some(_) => Err(DynamicBlockError::BadValue(self.id)),
        }
    }

    /// `end` brought into the sets of a polar or XY parameter (a polar
    /// parameter's distance and angle, the nearest turn in a list; an XY
    /// parameter's two distances). `None` for a point that is not finite, or
    /// a polar one that comes to no distance.
    pub fn fit_end(&self, end: Point) -> Option<Point> {
        if !(end.x.is_finite() && end.y.is_finite()) {
            return None;
        }
        let (base, distance_set, angle_set) = match &self.kind {
            DynamicParameterKind::Polar {
                base,
                distance_set,
                angle_set,
                ..
            } => (base, distance_set, angle_set),
            // Each distance of an XY parameter is brought into its own set.
            DynamicParameterKind::Xy {
                base, x_set, y_set, ..
            } => {
                let x = x_set.normalize(end.x - base.x)?;
                let y = y_set.normalize(end.y - base.y)?;
                return Some(Point::new(base.x + x, base.y + y));
            }
            DynamicParameterKind::Point { .. } => return Some(end),
            _ => return None,
        };
        let (dx, dy) = (end.x - base.x, end.y - base.y);
        let distance = distance_set.normalize(dx.hypot(dy))?;
        let angle = normalize_angle(angle_set, dy.atan2(dx), false)?;
        (distance > EPS).then(|| {
            Point::new(
                base.x + distance * angle.cos(),
                base.y + distance * angle.sin(),
            )
        })
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
        normalize_angle(self.value_set(), value, true)
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

/// `value`, an angle, brought into `set`: with no set it is wrapped into 0 to
/// a full turn when `wrap` (a polar angle keeps its own turn), and in a list
/// the nearest is the one a shorter turn away.
fn normalize_angle(set: &DynamicValueSet, value: f64, wrap: bool) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    match set {
        DynamicValueSet::Any => Some(if wrap { value.rem_euclid(TAU) } else { value }),
        DynamicValueSet::List(xs) if set.is_valid() => {
            let apart = |x: f64| (x - value).rem_euclid(TAU).min((value - x).rem_euclid(TAU));
            xs.iter()
                .copied()
                .min_by(|a, b| apart(*a).total_cmp(&apart(*b)))
        }
        set => set.normalize(value),
    }
}
