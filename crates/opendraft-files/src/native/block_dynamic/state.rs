//! What one insert of a dynamic block overrides, as the native format writes it.

use opendraft_core::block::{DynamicInstanceState, DynamicValue, DynamicValueEntry, ParameterId};
use serde::{Deserialize, Serialize};

use super::super::entity::{from_point, invalid, point, PointDto};
use crate::FileError;

/// What one insert overrides.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::native) struct DynamicStateDto {
    #[serde(deserialize_with = "crate::native::limits::schema_items")]
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

impl DynamicStateDto {
    pub(in crate::native) fn of(state: &DynamicInstanceState) -> Self {
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

    pub(in crate::native) fn to_state(&self) -> Result<DynamicInstanceState, FileError> {
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
