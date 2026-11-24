//! What one action does to the objects of a block, given its parameter's
//! value in an insert's state.

use std::collections::HashMap;

use crate::block::{
    BlockEntity, BlockObjectId, DynamicAction, DynamicActionKind, DynamicInstanceState,
    DynamicParameter, DynamicParameterKind,
};
use crate::{Bounds, Entity, Point};

use super::dynamic_eval::DynamicBlockError;

type Position = HashMap<BlockObjectId, usize>;

/// Replace each of `objects` by what `change` makes of it (`None` leaves it).
/// An attribute definition cannot be changed by an action.
fn each(
    out: &mut [BlockEntity],
    position: &Position,
    objects: &[BlockObjectId],
    change: impl Fn(&Entity) -> Option<Entity>,
) -> Result<(), DynamicBlockError> {
    for id in objects {
        let at = *position
            .get(id)
            .ok_or(DynamicBlockError::MissingObject(*id))?;
        let object = &mut out[at];
        if matches!(object.entity, Entity::AttributeDefinition(_)) {
            return Err(DynamicBlockError::UnsupportedObject(*id));
        }
        if let Some(changed) = change(&object.entity) {
            object.entity = changed;
        }
    }
    Ok(())
}

/// How far a polar parameter's end has gone from where it was drawn.
struct Polar {
    base: Point,
    was: Point,
    now: Point,
}

impl Polar {
    fn of(
        parameter: &DynamicParameter,
        state: &DynamicInstanceState,
    ) -> Result<Polar, DynamicBlockError> {
        let DynamicParameterKind::Polar { base, end, .. } = &parameter.kind else {
            return Err(DynamicBlockError::BadValue(parameter.id));
        };
        Ok(Polar {
            base: *base,
            was: *end,
            now: parameter.end_point(Some(state))?,
        })
    }

    fn angle(&self, at: Point) -> f64 {
        (at.y - self.base.y).atan2(at.x - self.base.x)
    }

    fn distance(&self, at: Point) -> f64 {
        (at.x - self.base.x).hypot(at.y - self.base.y)
    }
}

/// The vector `(dx, dy)` turned by `offset` and multiplied.
fn shifted(dx: f64, dy: f64, multiplier: f64, offset: f64) -> (f64, f64) {
    let (sin, cos) = offset.sin_cos();
    (
        (dx * cos - dy * sin) * multiplier,
        (dx * sin + dy * cos) * multiplier,
    )
}

/// Apply `action` to `out`.
pub(super) fn apply(
    out: &mut [BlockEntity],
    position: &Position,
    action: &DynamicAction,
    parameter: &DynamicParameter,
    state: &DynamicInstanceState,
) -> Result<(), DynamicBlockError> {
    let bad = || DynamicBlockError::BadValue(parameter.id);
    match &action.kind {
        DynamicActionKind::Rotate { objects } => {
            let (base, angle) = match &parameter.kind {
                DynamicParameterKind::Rotation { base, .. } => {
                    (*base, parameter.value(Some(state))?)
                }
                DynamicParameterKind::Polar { .. } => {
                    let p = Polar::of(parameter, state)?;
                    (p.base, p.angle(p.now) - p.angle(p.was))
                }
                _ => return Err(bad()),
            };
            if angle == 0.0 {
                return Ok(());
            }
            each(out, position, objects, |e| Some(e.rotated(base, angle)))
        }
        DynamicActionKind::Flip { objects } => {
            let DynamicParameterKind::Flip { base, end } = &parameter.kind else {
                return Err(bad());
            };
            if parameter.value(Some(state))? == 0.0 {
                return Ok(());
            }
            each(out, position, objects, |e| Some(e.mirrored(*base, *end)))
        }
        DynamicActionKind::Scale { objects } => {
            let (origin, factor) = match &parameter.kind {
                DynamicParameterKind::Polar { .. } => {
                    let p = Polar::of(parameter, state)?;
                    (p.base, p.distance(p.now) / p.distance(p.was))
                }
                _ => {
                    let (current, axis) = parameter.current(Some(state))?;
                    (axis.start, current / axis.length)
                }
            };
            if !(factor.is_finite() && factor > 0.0) {
                return Err(bad());
            }
            if factor == 1.0 {
                return Ok(());
            }
            each(out, position, objects, |e| Some(e.scaled(origin, factor)))
        }
        DynamicActionKind::Move {
            objects,
            multiplier,
            angle_offset,
        }
        | DynamicActionKind::Stretch {
            objects,
            multiplier,
            angle_offset,
            ..
        } => {
            let (dx, dy) = match &parameter.kind {
                DynamicParameterKind::Polar { .. }
                | DynamicParameterKind::Xy { .. }
                | DynamicParameterKind::Point { .. } => {
                    let was = parameter.drawn_end().ok_or_else(bad)?;
                    let now = parameter.end_point(Some(state))?;
                    shifted(now.x - was.x, now.y - was.y, *multiplier, *angle_offset)
                }
                _ => {
                    let (current, axis) = parameter.current(Some(state))?;
                    let amount = (current - axis.length) * multiplier;
                    let (sin, cos) = angle_offset.sin_cos();
                    let (ux, uy) = (
                        axis.dir.0 * cos - axis.dir.1 * sin,
                        axis.dir.0 * sin + axis.dir.1 * cos,
                    );
                    (ux * amount, uy * amount)
                }
            };
            if !(dx.is_finite() && dy.is_finite()) {
                return Err(bad());
            }
            if dx == 0.0 && dy == 0.0 {
                return Ok(());
            }
            match &action.kind {
                DynamicActionKind::Stretch { frame, .. } => each(out, position, objects, |e| {
                    let inside = |p: Point| frame.contains(&Bounds::from_points(p, p));
                    e.stretched(inside, dx, dy)
                }),
                _ => each(out, position, objects, |e| Some(e.translated(dx, dy))),
            }
        }
    }
}
