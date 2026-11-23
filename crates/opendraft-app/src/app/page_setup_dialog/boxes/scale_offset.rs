//! The Scale and Offset rows of the Plot box. Fitted or centred, their
//! fields are off and show what Fit and Center come to (`Placed`).

use eframe::egui;
use opendraft_core::{plot_settings::flags, PlotScale, StandardScale};
use opendraft_ui::widgets;

use super::{flag_checkbox, has_flag, set_flag, Placed};
use crate::layouts::PageDraft;

/// Fit to paper, or plotted millimetres = drawing units.
pub(super) fn scale_row(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
    controls: &crate::plot_controls::PlotControlState,
    placed: Option<Placed>,
) {
    ui.horizontal(|ui| {
        let mut fit = matches!(
            draft.plot.scale,
            PlotScale::Standard {
                kind: StandardScale::Fit,
                ..
            }
        );
        let changed = ui
            .add_enabled_ui(controls.can_fit, |ui| {
                ui.checkbox(&mut fit, "Fit").changed()
            })
            .inner;
        if changed {
            if fit {
                if let PlotScale::Custom {
                    paper_units,
                    drawing_units,
                } = draft.plot.scale
                {
                    draft.kept_scale = (paper_units, drawing_units);
                }
                draft.plot.scale = PlotScale::Standard {
                    kind: StandardScale::Fit,
                    value: 1.0,
                };
            } else {
                draft.plot.scale = PlotScale::Custom {
                    paper_units: draft.kept_scale.0,
                    drawing_units: draft.kept_scale.1,
                };
            }
            set_flag(draft, flags::USE_STANDARD_SCALE, fit);
        }
        ui.add_enabled_ui(controls.can_edit_scale, |ui| {
            // Fitted, the fields show the scale it comes to, 1 mm = n units.
            let mut fitted = placed.map(|p| (1.0, 1.0 / p.scale));
            let (paper, drawing) = match (&mut draft.plot.scale, &mut fitted) {
                (
                    PlotScale::Custom {
                        paper_units,
                        drawing_units,
                    },
                    _,
                ) => (paper_units, drawing_units),
                (_, Some((paper, drawing))) if fit => (paper, drawing),
                _ => (&mut draft.kept_scale.0, &mut draft.kept_scale.1),
            };
            widgets::number_field_width(ui, theme, paper, "mm", 0.1, 62.0);
            ui.label("=");
            widgets::number_field_width(ui, theme, drawing, "units", 0.1, 74.0);
        });
    });
}

/// Centre the plot, or its offset from the printable corner.
pub(super) fn offset_row(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    draft: &mut PageDraft,
    controls: &crate::plot_controls::PlotControlState,
    placed: Option<Placed>,
) {
    ui.horizontal(|ui| {
        ui.add_enabled_ui(controls.can_center, |ui| {
            flag_checkbox(ui, draft, flags::PLOT_CENTERED, "Center");
        })
        .response
        .on_disabled_hover_text("A layout plots from the corner of its printable area.");
        ui.add_enabled_ui(controls.can_edit_offset, |ui| {
            // Centred, the fields show the offset the centring comes to.
            let mut centred = placed.map(|p| p.origin);
            let origin = match &mut centred {
                Some(origin) if controls.can_center && has_flag(draft, flags::PLOT_CENTERED) => {
                    origin
                }
                _ => &mut draft.plot.origin_mm,
            };
            widgets::field_label(ui, theme, "X");
            widgets::number_field_width(ui, theme, &mut origin.x, "", 1.0, 54.0);
            widgets::field_label(ui, theme, "Y");
            widgets::number_field_width(ui, theme, &mut origin.y, "mm", 1.0, 64.0);
        });
    });
}
