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
use opendraft_core::block::{DynamicParameterKind, DynamicValue, ParameterId};
use opendraft_core::{Entity, EntityKind, Insert, Units};
use opendraft_ui::i18n::tr;
use opendraft_ui::Theme;

use crate::attr_rows::{cell_combo, VARIES};
use crate::properties::{self, Unit};
use crate::properties_ui::{row_frame, text_cell};
use crate::state::AppState;

/// One row: a parameter's label and its value in drawing units.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub parameter: ParameterId,
    /// Which of a parameter's values: 0, or 1 for a polar parameter's angle.
    pub part: u8,
    pub label: String,
    /// Whether the value is an angle, shown in degrees.
    pub angle: bool,
    /// Whether it is a yes or no (a flip).
    pub flag: bool,
    /// `None` when the references disagree. For a visibility parameter it is
    /// the position of the state shown in `states`.
    pub value: Option<f64>,
    /// The names of a visibility parameter's states, the choices of its
    /// drop-down; empty for every other kind.
    pub states: Vec<String>,
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
        .flat_map(|p| {
            // A polar, XY or point parameter has two values: the distance and
            // the angle, the two distances, or the two coordinates.
            let parts: &[u8] = if p.has_point() { &[0, 1] } else { &[0] };
            parts.iter().filter_map(move |&part| {
                let shown = |i: &Insert| shown_value(i, p.id, part);
                let first_value = shown(first)?;
                let mut same = true;
                for insert in rest {
                    let v = shown(insert)?;
                    same &= (v - first_value).abs() <= 1e-9 * first_value.abs().max(1.0);
                }
                let first_label = if p.label.is_empty() {
                    &p.name
                } else {
                    &p.label
                };
                let label = match (&p.kind, part) {
                    (DynamicParameterKind::Polar { angle_label: l, .. }, 1)
                    | (DynamicParameterKind::Xy { y_label: l, .. }, 1) => l.clone(),
                    (DynamicParameterKind::Point { .. }, 0) => format!("{first_label} X"),
                    (DynamicParameterKind::Point { .. }, _) => format!("{first_label} Y"),
                    _ => first_label.clone(),
                };
                Some(Row {
                    parameter: p.id,
                    part,
                    angle: p.is_angle() || (p.is_polar() && part == 1),
                    flag: p.is_flag(),
                    label,
                    value: same.then_some(first_value),
                    states: p
                        .visibility_states()
                        .map(|s| s.iter().map(|s| s.name.clone()).collect())
                        .unwrap_or_default(),
                })
            })
        })
        .collect()
}

/// A value as shown: in drawing units, or in degrees for an angle.
fn shown_value(insert: &Insert, parameter: ParameterId, part: u8) -> Option<f64> {
    if let Some(p) = insert.block.dynamic.as_ref()?.visibility() {
        if p.id == parameter {
            let (_, shown) = insert.visibility_state()?;
            let at = p.visibility_states()?.iter().position(|s| s.name == shown);
            return at.map(|i| i as f64);
        }
    }
    if let Some((a, b)) = insert.dynamic_pair(parameter) {
        let polar = insert
            .block
            .dynamic
            .as_ref()?
            .parameters
            .iter()
            .any(|p| p.id == parameter && p.is_polar());
        return Some(match (part, polar) {
            (1, true) => b.to_degrees().rem_euclid(360.0),
            (1, false) => b * insert.dynamic_scale(parameter)?,
            _ => a * insert.dynamic_scale(parameter)?,
        });
    }
    Some(insert.dynamic_value(parameter)? * insert.dynamic_scale(parameter)?)
}

/// `insert` with `parameter` set to `display`, a length in drawing units (an
/// angle in degrees; for a visibility parameter the position of the state
/// picked), brought into the parameter's value set; `None` when
/// that changes nothing
/// or cannot be done (an unknown parameter, a flattened insert, a value that
/// is not finite).
pub fn with_display_value(
    insert: &Insert,
    parameter: ParameterId,
    part: u8,
    display: f64,
) -> Option<Insert> {
    if let Some(p) = insert.block.dynamic.as_ref()?.visibility() {
        if p.id == parameter {
            // The position of the state picked.
            let at = (display.is_finite() && display >= 0.0).then_some(display as usize)?;
            let name = &p.visibility_states()?.get(at)?.name;
            return Some(insert.with_visibility_state(name)?).filter(|out| out != insert);
        }
    }
    let scale = insert.dynamic_scale(parameter)?;
    let p = insert
        .block
        .dynamic
        .as_ref()?
        .parameters
        .iter()
        .find(|p| p.id == parameter)?;
    if p.has_point() {
        // Change one of the two numbers and keep the other.
        let (a, b) = insert.dynamic_pair(parameter)?;
        let (a, b) = match (part, p.is_polar()) {
            (1, true) => (a, display.to_radians()),
            (1, false) => (a, display / scale),
            _ => (display / scale, b),
        };
        let end = p.fit_end(p.point_of(a, b)?)?;
        let out = insert.with_dynamic_value(parameter, DynamicValue::Point(end))?;
        return (out != *insert).then_some(out);
    }
    let value = p.normalize(display / scale)?;
    let out = insert.with_dynamic_value(parameter, p.make_value(value))?;
    (out != *insert).then_some(out)
}

fn unit_of(row: &Row) -> Unit {
    if row.flag {
        Unit::Flag
    } else if row.angle {
        Unit::Angle
    } else {
        Unit::Length
    }
}

/// The rows drawn, with the value cells editable. Returns the parameter and
/// the length typed, in drawing units (an angle, in degrees).
pub fn show(
    ui: &mut Ui,
    theme: &Theme,
    units: &Units,
    rows: &[Row],
) -> Option<(ParameterId, u8, f64)> {
    let mut edit = None;
    for row in rows {
        let (label_cell, cell) = row_frame(ui, theme, &row.label);
        ui.interact(
            label_cell,
            ui.id().with(("dynamic_label", row.parameter.0, row.part)),
            egui::Sense::hover(),
        )
        .on_hover_text(&row.label);
        if !row.states.is_empty() {
            // A visibility parameter: pick one of its states.
            let current = row.value.map(|v| v as usize);
            let varies = tr(VARIES);
            let text = current
                .and_then(|i| row.states.get(i))
                .map_or(&*varies, String::as_str);
            let id = egui::Id::new(("property_dynamic_states", row.parameter.0));
            if let Some(picked) = cell_combo(ui, theme, id, cell, text, &row.states, current) {
                edit = Some((row.parameter, 0, picked as f64));
            }
            continue;
        }
        let shown = row
            .value
            .map(|v| properties::format(units, unit_of(row), v))
            .unwrap_or_default();
        let id = egui::Id::new(("property_dynamic", row.parameter.0, row.part));
        if let Some(typed) = text_cell(ui, theme, cell, id, &shown) {
            if let Some(v) = properties::parse(units, unit_of(row), &typed) {
                edit = Some((row.parameter, row.part, v));
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
        part: u8,
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
                let made = with_display_value(insert, parameter, part, display)?;
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
