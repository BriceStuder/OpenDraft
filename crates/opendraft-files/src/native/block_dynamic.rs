//! The native format's dynamic block data: a definition's schema (its
//! parameters and actions) and the values one insert overrides.
//!
//! Every structure refuses a field it does not know, so a file written by a
//! newer version is refused rather than read with data dropped. Lists are
//! bounded while they are read, and the schema's version is checked.

use std::collections::HashSet;

use opendraft_core::block::{
    ActionId, BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, DynamicInstanceState,
    DynamicParameter, DynamicParameterKind, DynamicValue, DynamicValueEntry, DynamicValueSet,
    ParameterId,
};
use opendraft_core::Bounds;
use serde::{Deserialize, Serialize};

use super::entity::{from_point, invalid, point, PointDto};
use crate::FileError;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DynamicBlockDto {
    version: u16,
    next_parameter_id: u64,
    next_action_id: u64,
    #[serde(deserialize_with = "super::limits::schema_items")]
    parameters: Vec<ParameterDto>,
    #[serde(deserialize_with = "super::limits::schema_items")]
    actions: Vec<ActionDto>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParameterDto {
    id: u64,
    name: String,
    label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    show: bool,
    grips: u8,
    kind: ParameterKindDto,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ParameterKindDto {
    Linear(LinearDto),
    Rotation(RotationDto),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LinearDto {
    start: PointDto,
    end: PointDto,
    values: ValueSetDto,
}

/// A rotation parameter; its value set is in radians.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RotationDto {
    base: PointDto,
    radius: f64,
    values: ValueSetDto,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ValueSetDto {
    Any,
    List(#[serde(deserialize_with = "super::limits::value_list")] Vec<f64>),
    Range(RangeDto),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RangeDto {
    min: f64,
    max: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    increment: Option<f64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionDto {
    id: u64,
    name: String,
    parameter: u64,
    kind: ActionKindDto,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ActionKindDto {
    Move(ActionBodyDto),
    Stretch(ActionBodyDto),
    Rotate(RotateDto),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RotateDto {
    #[serde(deserialize_with = "super::limits::block_objects")]
    objects: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionBodyDto {
    #[serde(deserialize_with = "super::limits::block_objects")]
    objects: Vec<u64>,
    /// The stretch frame; a move has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frame: Option<[PointDto; 2]>,
    multiplier: f64,
    angle_offset: f64,
}

/// What one insert overrides.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DynamicStateDto {
    #[serde(deserialize_with = "super::limits::schema_items")]
    values: Vec<ValueEntryDto>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueEntryDto {
    parameter: u64,
    value: ValueDto,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ValueDto {
    Distance(f64),
    Angle(f64),
    Point(PointDto),
    Integer(i64),
    Text(String),
    Bool(bool),
}

impl DynamicBlockDto {
    /// The schema as stored, refused if it could not be read back.
    pub(super) fn of(
        schema: &DynamicBlock,
        objects: &HashSet<BlockObjectId>,
    ) -> Result<Self, FileError> {
        schema.check(objects).or_else(invalid)?;
        let dto = Self {
            version: schema.version,
            next_parameter_id: schema.next_parameter_id,
            next_action_id: schema.next_action_id,
            parameters: schema.parameters.iter().map(ParameterDto::of).collect(),
            actions: schema.actions.iter().map(ActionDto::of).collect(),
        };
        Ok(dto)
    }

    /// The schema, checked against the block's object ids.
    pub(super) fn to_schema(
        &self,
        objects: &HashSet<BlockObjectId>,
    ) -> Result<DynamicBlock, FileError> {
        let schema = DynamicBlock {
            version: self.version,
            parameters: self
                .parameters
                .iter()
                .map(ParameterDto::to_parameter)
                .collect::<Result<_, _>>()?,
            actions: self
                .actions
                .iter()
                .map(ActionDto::to_action)
                .collect::<Result<_, _>>()?,
            next_parameter_id: self.next_parameter_id,
            next_action_id: self.next_action_id,
        };
        schema.check(objects).or_else(invalid)?;
        Ok(schema)
    }
}

impl ParameterDto {
    fn of(p: &DynamicParameter) -> Self {
        let kind = match &p.kind {
            DynamicParameterKind::Linear {
                start,
                end,
                value_set,
            } => ParameterKindDto::Linear(LinearDto {
                start: point(*start),
                end: point(*end),
                values: ValueSetDto::of(value_set),
            }),
            DynamicParameterKind::Rotation {
                base,
                radius,
                value_set,
            } => ParameterKindDto::Rotation(RotationDto {
                base: point(*base),
                radius: *radius,
                values: ValueSetDto::of(value_set),
            }),
        };
        Self {
            id: p.id.0,
            name: p.name.clone(),
            label: p.label.clone(),
            description: p.description.clone(),
            show: p.show,
            grips: p.grips,
            kind,
        }
    }

    fn to_parameter(&self) -> Result<DynamicParameter, FileError> {
        let kind = match &self.kind {
            ParameterKindDto::Linear(l) => DynamicParameterKind::Linear {
                start: from_point(l.start)?,
                end: from_point(l.end)?,
                value_set: l.values.to_set(),
            },
            ParameterKindDto::Rotation(r) => DynamicParameterKind::Rotation {
                base: from_point(r.base)?,
                radius: r.radius,
                value_set: r.values.to_set(),
            },
        };
        Ok(DynamicParameter {
            id: ParameterId(self.id),
            name: self.name.clone(),
            label: self.label.clone(),
            description: self.description.clone(),
            show: self.show,
            grips: self.grips,
            kind,
        })
    }
}

impl ValueSetDto {
    fn of(set: &DynamicValueSet) -> Self {
        match set {
            DynamicValueSet::Any => Self::Any,
            DynamicValueSet::List(xs) => Self::List(xs.clone()),
            DynamicValueSet::Range {
                min,
                max,
                increment,
            } => Self::Range(RangeDto {
                min: *min,
                max: *max,
                increment: *increment,
            }),
        }
    }

    fn to_set(&self) -> DynamicValueSet {
        match self {
            Self::Any => DynamicValueSet::Any,
            Self::List(xs) => DynamicValueSet::List(xs.clone()),
            Self::Range(r) => DynamicValueSet::Range {
                min: r.min,
                max: r.max,
                increment: r.increment,
            },
        }
    }
}

impl ActionDto {
    fn of(a: &DynamicAction) -> Self {
        let body =
            |objects: &[BlockObjectId], frame: Option<&Bounds>, m: f64, o: f64| ActionBodyDto {
                objects: objects.iter().map(|o| o.0).collect(),
                frame: frame.map(|f| [point(f.min), point(f.max)]),
                multiplier: m,
                angle_offset: o,
            };
        let kind = match &a.kind {
            DynamicActionKind::Move {
                objects,
                multiplier,
                angle_offset,
            } => ActionKindDto::Move(body(objects, None, *multiplier, *angle_offset)),
            DynamicActionKind::Stretch {
                objects,
                frame,
                multiplier,
                angle_offset,
            } => ActionKindDto::Stretch(body(objects, Some(frame), *multiplier, *angle_offset)),
            DynamicActionKind::Rotate { objects } => ActionKindDto::Rotate(RotateDto {
                objects: objects.iter().map(|o| o.0).collect(),
            }),
        };
        Self {
            id: a.id.0,
            name: a.name.clone(),
            parameter: a.parameter.0,
            kind,
        }
    }

    fn to_action(&self) -> Result<DynamicAction, FileError> {
        let objects = |b: &ActionBodyDto| b.objects.iter().map(|&o| BlockObjectId(o)).collect();
        let kind = match &self.kind {
            ActionKindDto::Move(b) => {
                if b.frame.is_some() {
                    return invalid("a move action has a frame");
                }
                DynamicActionKind::Move {
                    objects: objects(b),
                    multiplier: b.multiplier,
                    angle_offset: b.angle_offset,
                }
            }
            ActionKindDto::Stretch(b) => {
                let Some([a, c]) = b.frame else {
                    return invalid("a stretch action has no frame");
                };
                let (min, max) = (from_point(a)?, from_point(c)?);
                if min.x > max.x || min.y > max.y {
                    return invalid("a stretch frame has its corners the wrong way round");
                }
                DynamicActionKind::Stretch {
                    objects: objects(b),
                    frame: Bounds { min, max },
                    multiplier: b.multiplier,
                    angle_offset: b.angle_offset,
                }
            }
            ActionKindDto::Rotate(r) => DynamicActionKind::Rotate {
                objects: r.objects.iter().map(|&o| BlockObjectId(o)).collect(),
            },
        };
        Ok(DynamicAction {
            id: ActionId(self.id),
            name: self.name.clone(),
            parameter: ParameterId(self.parameter),
            kind,
        })
    }
}

impl DynamicStateDto {
    pub(super) fn of(state: &DynamicInstanceState) -> Self {
        Self {
            values: state
                .values
                .iter()
                .map(|e| ValueEntryDto {
                    parameter: e.parameter.0,
                    value: match &e.value {
                        DynamicValue::Distance(v) => ValueDto::Distance(*v),
                        DynamicValue::Angle(v) => ValueDto::Angle(*v),
                        DynamicValue::Point(p) => ValueDto::Point(point(*p)),
                        DynamicValue::Integer(v) => ValueDto::Integer(*v),
                        DynamicValue::Text(v) => ValueDto::Text(v.clone()),
                        DynamicValue::Bool(v) => ValueDto::Bool(*v),
                    },
                })
                .collect(),
        }
    }

    pub(super) fn to_state(&self) -> Result<DynamicInstanceState, FileError> {
        let values = self
            .values
            .iter()
            .map(|e| {
                Ok(DynamicValueEntry {
                    parameter: ParameterId(e.parameter),
                    value: match &e.value {
                        ValueDto::Distance(v) => DynamicValue::Distance(*v),
                        ValueDto::Angle(v) => DynamicValue::Angle(*v),
                        ValueDto::Point(p) => DynamicValue::Point(from_point(*p)?),
                        ValueDto::Integer(v) => DynamicValue::Integer(*v),
                        ValueDto::Text(v) => DynamicValue::Text(v.clone()),
                        ValueDto::Bool(v) => DynamicValue::Bool(*v),
                    },
                })
            })
            .collect::<Result<_, FileError>>()?;
        let state = DynamicInstanceState { values };
        if !state.is_well_formed() {
            return invalid("a block insert has values that are not finite or not in order");
        }
        Ok(state)
    }
}

#[cfg(test)]
mod tests;
