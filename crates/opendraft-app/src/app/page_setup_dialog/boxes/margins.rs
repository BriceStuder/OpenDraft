//! The plot's margins, as the page is read: one value for all four sides
//! with the padlock closed, Left, Bottom, Right and Top with it open.
//!
//! A file (OpenDraft PDF or SVG) has no edge it cannot reach, so any margin
//! from 0 is allowed; a printer's margins are its own and are only shown.

use eframe::egui;
use opendraft_core::PlotRotation;
use opendraft_ui::{icons, widgets};

use crate::layouts::PageDraft;

/// The margins row of the Plot box: the caption, then the controls.
pub(super) fn margins_row(ui: &mut egui::Ui, theme: &opendraft_ui::Theme, draft: &mut PageDraft) {
    widgets::field_label(ui, theme, "Margins");
    let (.., read) = draft.page_as_read();
    let mut margins = read;
    let same = margins.iter().all(|m| (m - margins[0]).abs() < 1e-9);
    let id = ui.make_persistent_id("plot_margins_linked");
    let wanted = ui.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || true));
    // Different sides cannot be shown as one value, whatever was asked.
    let linked = wanted && same;
    ui.horizontal_wrapped(|ui| {
        let (icon, tip) = if linked {
            (
                icons::LAYER_LOCK,
                "One margin for all four sides; click to set each side",
            )
        } else {
            (
                icons::LAYER_UNLOCK,
                "Each side its own margin; click for one margin",
            )
        };
        if widgets::icon_toggle(ui, theme, icon, linked, tip).clicked() {
            ui.data_mut(|d| d.insert_temp(id, !linked));
            if !linked {
                margins = [margins[0]; 4];
            }
        }
        if linked {
            let mut all = margins[0];
            if widgets::number_field_width(ui, theme, &mut all, "mm", 0.1, 74.0).changed() {
                margins = [all; 4];
            }
        } else {
            for (side, value) in ["L", "B", "R", "T"].into_iter().zip(margins.iter_mut()) {
                ui.label(side);
                widgets::number_field_width(ui, theme, value, "", 0.1, 40.0);
            }
        }
    });
    if margins != read {
        set_as_read(draft, margins.map(|m| m.max(0.0)));
    }
}

/// Store margins given as the page is read (left, bottom, right, top) on the
/// paper as it feeds: the inverse of `page_as_read`.
pub(super) fn set_as_read(draft: &mut PageDraft, [left, bottom, right, top]: [f64; 4]) {
    draft.plot.margins_mm = match draft.plot.rotation {
        PlotRotation::CounterClockwise90 | PlotRotation::Clockwise90 => [top, left, bottom, right],
        PlotRotation::Degrees0 | PlotRotation::Degrees180 => [left, bottom, right, top],
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendraft_core::{LayoutId, PlotSettings};

    #[test]
    fn margins_given_as_read_come_back_as_read_whatever_the_turn() {
        for rotation in [
            PlotRotation::Degrees0,
            PlotRotation::CounterClockwise90,
            PlotRotation::Degrees180,
            PlotRotation::Clockwise90,
        ] {
            let mut plot = PlotSettings::for_layout(210.0, 297.0);
            plot.rotation = rotation;
            let mut draft = PageDraft {
                space: opendraft_core::plot::PlotSpace::Layout(LayoutId(1)),
                name: "A".into(),
                width: 210.0,
                height: 297.0,
                plot,
                kept_scale: (1.0, 1.0),
            };
            set_as_read(&mut draft, [1.0, 2.0, 3.0, 4.0]);
            assert_eq!(draft.page_as_read().2, [1.0, 2.0, 3.0, 4.0], "{rotation:?}");
            // Portrait to landscape and back: each margin stays on its edge
            // of the page as it is read.
            super::super::turn_page(&mut draft);
            assert_eq!(draft.page_as_read().2, [1.0, 2.0, 3.0, 4.0], "{rotation:?}");
            super::super::turn_page(&mut draft);
            assert_eq!(draft.page_as_read().2, [1.0, 2.0, 3.0, 4.0], "{rotation:?}");
        }
    }
}
