//! The Layers panel's right-click menu, with AutoCAD's Layer Properties
//! Manager wording. It acts on the panel's selection.

use eframe::egui::{self, Align2, FontId, Sense, Ui};
use opendraft_core::LayerId;
use opendraft_ui::{menu, Theme};
use rust_i18n::t;

use crate::layer_selection::LayerSelection;
use crate::layers::LayerEdit;

const WIDTH: f32 = 190.0;
const ROW_HEIGHT: f32 = 22.0;

/// What was picked in the menu.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuPick {
    Edit(LayerEdit),
    /// Start renaming this layer in its row.
    Rename(LayerId),
    SelectAll,
    SelectAllButCurrent,
    Invert,
    ClearAll,
}

/// Open the menu under the pointer at `at`. The panel calls this on a
/// right-click, after pointing the selection at the row clicked.
pub fn open(ui: &Ui, key: egui::Id, at: egui::Pos2) {
    ui.data_mut(|d| {
        d.insert_temp(key, true);
        d.insert_temp(key.with("at"), at);
    });
}

/// Draw the menu if it is open. It closes on a pick, Esc or a press
/// elsewhere.
pub fn show(ui: &Ui, theme: &Theme, key: egui::Id, selection: &LayerSelection) -> Option<MenuPick> {
    let at = ui.data(|d| d.get_temp::<egui::Pos2>(key.with("at")))?;
    let anchor = egui::Rect::from_min_size(at, egui::Vec2::ZERO);
    menu::popup_at(ui, theme, key, false, anchor, WIDTH, |ui| {
        items(ui, theme, selection)
    })
}

/// The menu's rows, greyed when they cannot apply to the selection.
fn items(ui: &mut Ui, theme: &Theme, selection: &LayerSelection) -> Option<MenuPick> {
    let single = selection.single();
    let any = !selection.ids().is_empty();
    let mut picked = None;
    let mut row = |ui: &mut Ui, label: &str, enabled: bool, pick: MenuPick| {
        if item(ui, theme, label, enabled) {
            picked = Some(pick);
        }
    };
    row(ui, &t!("New Layer"), true, MenuPick::Edit(LayerEdit::New));
    row(
        ui,
        &t!("Set Current"),
        single.is_some(),
        MenuPick::Edit(LayerEdit::SetCurrent(single.unwrap_or(LayerId::DEFAULT))),
    );
    row(
        ui,
        &t!("Rename Layer"),
        single.is_some_and(|id| id != LayerId::DEFAULT),
        MenuPick::Rename(single.unwrap_or(LayerId::DEFAULT)),
    );
    let delete = if selection.ids().len() > 1 {
        t!("Delete Layers")
    } else {
        t!("Delete Layer")
    };
    row(
        ui,
        &delete,
        any,
        MenuPick::Edit(LayerEdit::Delete(selection.ids().to_vec())),
    );
    separator(ui, theme);
    row(ui, &t!("Select All"), true, MenuPick::SelectAll);
    row(
        ui,
        &t!("Select All but Current"),
        true,
        MenuPick::SelectAllButCurrent,
    );
    row(ui, &t!("Invert Selection"), true, MenuPick::Invert);
    row(ui, &t!("Clear All"), any, MenuPick::ClearAll);
    picked
}

/// One row; a greyed one cannot be clicked.
fn item(ui: &mut Ui, theme: &Theme, label: &str, enabled: bool) -> bool {
    if enabled {
        return menu::menu_row(ui, theme, label, false);
    }
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(theme.font_size_body),
        theme.colors.text_weak,
    );
    false
}

fn separator(ui: &mut Ui, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 7.0), Sense::hover());
    ui.painter().hline(
        rect.x_range().shrink(4.0),
        rect.center().y,
        egui::Stroke::new(1.0_f32, theme.colors.divider),
    );
}
