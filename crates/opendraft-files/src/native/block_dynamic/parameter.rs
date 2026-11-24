//! The native format's parameters: each kind of parameter as stored, and
//! the value sets they hold.

use opendraft_core::block::{
    BlockObjectId, DynamicParameter, DynamicParameterKind, DynamicValueSet, LookupRow, ParameterId,
    VisibilityState,
};
use serde::{Deserialize, Serialize};

use super::super::entity::{from_point, point, PointDto};
use crate::FileError;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ParameterDto {
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
    Flip(FlipDto),
    Polar(PolarDto),
    Xy(XyDto),
    Point(LocationDto),
    Visibility(VisibilityDto),
    Lookup(LookupDto),
}

/// A lookup parameter: where it is, its inputs and its rows.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LookupDto {
    position: PointDto,
    inputs: Vec<u64>,
    rows: Vec<LookupRowDto>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LookupRowDto {
    label: String,
    values: Vec<f64>,
}

/// An XY parameter: the base, the end and the sets of each distance.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct XyDto {
    base: PointDto,
    end: PointDto,
    xs: ValueSetDto,
    ys: ValueSetDto,
    y_label: String,
}

/// A point parameter: where it is.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocationDto {
    position: PointDto,
}

/// A visibility parameter: where it is and its states.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VisibilityDto {
    position: PointDto,
    states: Vec<StateDto>,
}

/// One visibility state: its name and the objects it hides.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateDto {
    name: String,
    #[serde(deserialize_with = "super::super::limits::block_objects")]
    hidden: Vec<u64>,
}

/// A polar parameter: the base, the end and the sets of each.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolarDto {
    base: PointDto,
    end: PointDto,
    distances: ValueSetDto,
    angles: ValueSetDto,
    angle_label: String,
}

/// A flip parameter: its mirror line.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FlipDto {
    base: PointDto,
    end: PointDto,
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
    List(#[serde(deserialize_with = "super::super::limits::value_list")] Vec<f64>),
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

impl ParameterDto {
    pub(super) fn of(p: &DynamicParameter) -> Self {
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
            DynamicParameterKind::Flip { base, end } => ParameterKindDto::Flip(FlipDto {
                base: point(*base),
                end: point(*end),
            }),
            DynamicParameterKind::Xy {
                base,
                end,
                x_set,
                y_set,
                y_label,
            } => ParameterKindDto::Xy(XyDto {
                base: point(*base),
                end: point(*end),
                xs: ValueSetDto::of(x_set),
                ys: ValueSetDto::of(y_set),
                y_label: y_label.clone(),
            }),
            DynamicParameterKind::Point { position } => ParameterKindDto::Point(LocationDto {
                position: point(*position),
            }),
            DynamicParameterKind::Lookup {
                position,
                inputs,
                rows,
            } => ParameterKindDto::Lookup(LookupDto {
                position: point(*position),
                inputs: inputs.iter().map(|i| i.0).collect(),
                rows: rows
                    .iter()
                    .map(|r| LookupRowDto {
                        label: r.label.clone(),
                        values: r.values.clone(),
                    })
                    .collect(),
            }),
            DynamicParameterKind::Visibility { position, states } => {
                ParameterKindDto::Visibility(VisibilityDto {
                    position: point(*position),
                    states: states
                        .iter()
                        .map(|s| StateDto {
                            name: s.name.clone(),
                            hidden: s.hidden.iter().map(|o| o.0).collect(),
                        })
                        .collect(),
                })
            }
            DynamicParameterKind::Polar {
                base,
                end,
                distance_set,
                angle_set,
                angle_label,
            } => ParameterKindDto::Polar(PolarDto {
                base: point(*base),
                end: point(*end),
                distances: ValueSetDto::of(distance_set),
                angles: ValueSetDto::of(angle_set),
                angle_label: angle_label.clone(),
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

    pub(super) fn to_parameter(&self) -> Result<DynamicParameter, FileError> {
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
            ParameterKindDto::Flip(f) => DynamicParameterKind::Flip {
                base: from_point(f.base)?,
                end: from_point(f.end)?,
            },
            ParameterKindDto::Xy(x) => DynamicParameterKind::Xy {
                base: from_point(x.base)?,
                end: from_point(x.end)?,
                x_set: x.xs.to_set(),
                y_set: x.ys.to_set(),
                y_label: x.y_label.clone(),
            },
            ParameterKindDto::Point(l) => DynamicParameterKind::Point {
                position: from_point(l.position)?,
            },
            ParameterKindDto::Lookup(l) => DynamicParameterKind::Lookup {
                position: from_point(l.position)?,
                inputs: l.inputs.iter().map(|i| ParameterId(*i)).collect(),
                rows: l
                    .rows
                    .iter()
                    .map(|r| LookupRow {
                        label: r.label.clone(),
                        values: r.values.clone(),
                    })
                    .collect(),
            },
            ParameterKindDto::Visibility(v) => DynamicParameterKind::Visibility {
                position: from_point(v.position)?,
                states: v
                    .states
                    .iter()
                    .map(|s| VisibilityState {
                        name: s.name.clone(),
                        hidden: s.hidden.iter().map(|o| BlockObjectId(*o)).collect(),
                    })
                    .collect(),
            },
            ParameterKindDto::Polar(p) => DynamicParameterKind::Polar {
                base: from_point(p.base)?,
                end: from_point(p.end)?,
                distance_set: p.distances.to_set(),
                angle_set: p.angles.to_set(),
                angle_label: p.angle_label.clone(),
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
