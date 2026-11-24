//! The native format's dynamic block data: a definition's schema (its
//! parameters and actions) and the values one insert overrides.
//!
//! Every structure refuses a field it does not know, so a file written by a
//! newer version is refused rather than read with data dropped. Lists are
//! bounded while they are read, and the schema's version is checked.

use std::collections::HashSet;

use opendraft_core::block::{
    ActionId, BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, ParameterId,
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
    Rotate(ObjectsDto),
    Flip(ObjectsDto),
    Scale(ObjectsDto),
}

/// An action that only names its objects.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObjectsDto {
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
            DynamicActionKind::Rotate { objects } => ActionKindDto::Rotate(ObjectsDto {
                objects: objects.iter().map(|o| o.0).collect(),
            }),
            DynamicActionKind::Flip { objects } => ActionKindDto::Flip(ObjectsDto {
                objects: objects.iter().map(|o| o.0).collect(),
            }),
            DynamicActionKind::Scale { objects } => ActionKindDto::Scale(ObjectsDto {
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
            ActionKindDto::Flip(r) => DynamicActionKind::Flip {
                objects: r.objects.iter().map(|&o| BlockObjectId(o)).collect(),
            },
            ActionKindDto::Scale(r) => DynamicActionKind::Scale {
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

mod parameter;
mod state;
use parameter::ParameterDto;
pub(super) use state::DynamicStateDto;

#[cfg(test)]
mod kinds_tests;
#[cfg(test)]
mod tests;
