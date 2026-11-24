//! The Custom section of the Properties panel: one row for each parameter a
//! dynamic block shows, its value in drawing units, edited in place. For
//! several references it lists the parameters of the block they all insert,
//! and a value they do not share reads "Varies".
//!
//! A value is kept in the block's units; the panel shows it times the
//! insert's scale along the parameter's line (`Insert::dynamic_scale`) and
//! reads a typed one divided by it.

use eframe::egui::{self, Ui};
use opendraft_commands::ModifyEntities;
use opendraft_core::block::ParameterId;
use opendraft_core::{Entity, EntityKind, Insert, Units};
use opendraft_ui::Theme;

use crate::properties::{self, Unit};
use crate::properties_ui::{row_frame, text_cell};
use crate::state::AppState;

/// One row: a parameter's label and its value in drawing units.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub parameter: ParameterId,
    pub label: String,
    /// Whether the value is an angle, shown in degrees.
    pub angle: bool,
    /// `None` when the references disagree.
    pub value: Option<f64>,
}

/// The rows for `inserts`: the shown parameters of their block, in its
/// order. None unless every one inserts the same dynamic block.
pub fn rows(inserts: &[&Insert]) -> Vec<Row> {
    let Some((first, rest)) = inserts.split_first() else {
        return Vec::new();
    };
    let Some(schema) = &first.block.dynamic else {
        return Vec::new();
    };
    if rest.iter().any(|i| i.block.id != first.block.id) {
        return Vec::new();
    }
    schema
        .parameters
        .iter()
        .filter(|p| p.show)
        .filter_map(|p| {
            let shown = |i: &Insert| Some(i.dynamic_value(p.id)? * i.dynamic_scale(p.id)?);
            let first_value = shown(first)?;
            let mut same = true;
            for insert in rest {
                let v = shown(insert)?;
                same &= (v - first_value).abs() <= 1e-9 * first_value.abs().max(1.0);
            }
            Some(Row {
                parameter: p.id,
                angle: p.is_angle(),
                label: if p.label.is_empty() {
                    p.name.clone()
                } else {
                    p.label.clone()
                },
                value: same.then_some(first_value),
            })
        })
        .collect()
}

/// `insert` with `parameter` set to `display`, a length in drawing units (an
/// angle in degrees), brought into the parameter's value set; `None` when
/// that changes nothing
/// or cannot be done (an unknown parameter, a flattened insert, a value that
/// is not finite).
pub fn with_display_value(insert: &Insert, parameter: ParameterId, display: f64) -> Option<Insert> {
    let scale = insert.dynamic_scale(parameter)?;
    let p = insert
        .block
        .dynamic
        .as_ref()?
        .parameters
        .iter()
        .find(|p| p.id == parameter)?;
    let value = p.normalize(display / scale)?;
    let out = insert.with_dynamic_value(parameter, p.make_value(value))?;
    (out != *insert).then_some(out)
}

fn unit_of(row: &Row) -> Unit {
    if row.angle {
        Unit::Angle
    } else {
        Unit::Length
    }
}

/// The rows drawn, with the value cells editable. Returns the parameter and
/// the length typed, in drawing units (an angle, in degrees).
pub fn show(ui: &mut Ui, theme: &Theme, units: &Units, rows: &[Row]) -> Option<(ParameterId, f64)> {
    let mut edit = None;
    for row in rows {
        let (label_cell, cell) = row_frame(ui, theme, &row.label);
        ui.interact(
            label_cell,
            ui.id().with(("dynamic_label", row.parameter.0)),
            egui::Sense::hover(),
        )
        .on_hover_text(&row.label);
        let shown = row
            .value
            .map(|v| properties::format(units, unit_of(row), v))
            .unwrap_or_default();
        let id = egui::Id::new(("property_dynamic", row.parameter.0));
        if let Some(typed) = text_cell(ui, theme, cell, id, &shown) {
            if let Some(v) = properties::parse(units, unit_of(row), &typed) {
                edit = Some((row.parameter, v));
            }
        }
    }
    edit
}

impl AppState {
    /// Give `parameter` of every selected reference (of `kind`, when the
    /// panel is narrowed to one) the length `display`, in drawing units: one
    /// undo step. A value the parameter's set does not hold is brought into
    /// it, and the message says so.
    pub fn set_dynamic_value(
        &mut self,
        parameter: ParameterId,
        display: f64,
        kind: Option<EntityKind>,
    ) {
        self.interrupt_command();
        let doc = &mut self.docs[self.active];
        let document = doc.session.document();
        let mut label = String::new();
        let changes: Vec<_> = doc
            .selection
            .iter()
            .filter_map(|&id| {
                let object = document.object(id)?;
                if kind.is_some_and(|k| object.entity.kind() != k) {
                    return None;
                }
                let Entity::Insert(insert) = &object.entity else {
                    return None;
                };
                let made = with_display_value(insert, parameter, display)?;
                if label.is_empty() {
                    label = insert
                        .block
                        .dynamic
                        .as_ref()?
                        .parameters
                        .iter()
                        .find(|p| p.id == parameter)
                        .map(|p| p.name.clone())?;
                }
                Some((id, Entity::Insert(Box::new(made))))
            })
            .collect();
        if changes.is_empty() {
            return;
        }
        self.message = format!("Changed {label}");
        doc.session
            .perform(ModifyEntities::new("Edit dynamic property", changes));
    }
}

#[cfg(test)]
#[path = "dynamic_rows_tests.rs"]
mod tests;
