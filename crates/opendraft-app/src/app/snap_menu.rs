//! The menu that Shift+right-click (or Ctrl+right-click) opens at a point
//! prompt: one-shot object snaps for the next point only.

use eframe::egui;
use opendraft_ui::i18n::tr;
use opendraft_ui::key;
use rust_i18n::t;

use super::OpenDraftApp;
use crate::snap_once::parse;

/// Menu rows: a label and the word a user would type, or `None` for a rule.
const ROWS: &[Option<(&str, &str)>] = &[
    Some((key!("Temporary Track Point"), "tt")),
    Some((key!("Tracking"), "tk")),
    Some((key!("From"), "fro")),
    Some((key!("Mid Between 2 Points"), "m2p")),
    None,
    Some((key!("Endpoint"), "end")),
    Some((key!("Midpoint"), "mid")),
    Some((key!("Intersection"), "int")),
    Some((key!("Apparent Intersection"), "app")),
    Some((key!("Extended Intersection"), "extint")),
    Some((key!("Geometric Center"), "gcen")),
    Some((key!("Center"), "cen")),
    Some((key!("Quadrant"), "qua")),
    Some((key!("Tangent"), "tan")),
    Some((key!("Perpendicular"), "per")),
    Some((key!("Extension"), "ext")),
    Some((key!("Parallel"), "par")),
    Some((key!("Node"), "nod")),
    Some((key!("Insertion"), "ins")),
    Some((key!("Nearest"), "nea")),
    None,
    Some((key!("None"), "non")),
];

impl OpenDraftApp {
    /// Show the override menu where it was asked for, and start the snap that
    /// is picked. It closes on a pick, a click elsewhere, or when no point is
    /// being asked for any more.
    pub(super) fn snap_menu(&mut self, ctx: &egui::Context) {
        let Some((x, y)) = self.state.snap_menu else {
            return;
        };
        if self.state.waiting().is_none() {
            self.state.snap_menu = None;
            return;
        }
        let mut word = None;
        let mut settings = false;
        let area = egui::Area::new(egui::Id::new("snap_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(x, y))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(190.0);
                    // Justified: each row takes the whole width, so a press
                    // anywhere along it picks it.
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                        for row in ROWS {
                            match row {
                                None => {
                                    ui.separator();
                                }
                                Some((label, typed)) => {
                                    if ui.selectable_label(false, tr(label)).clicked() {
                                        word = Some(*typed);
                                    }
                                }
                            }
                        }
                    });
                    ui.separator();
                    settings = ui
                        .with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                            ui.selectable_label(false, t!("Snap Settings...")).clicked()
                        })
                        .inner;
                });
            });
        let outside = ctx.input(|i| i.pointer.primary_clicked())
            && ctx
                .pointer_latest_pos()
                .is_some_and(|p| !area.response.rect.contains(p));
        if let Some(wish) = word.and_then(parse) {
            self.state.begin_one_shot(wish);
        }
        if settings {
            self.settings_tab = 1;
            self.state.show_settings = true;
        }
        if word.is_some() || settings || outside {
            self.state.snap_menu = None;
        }
    }
}
