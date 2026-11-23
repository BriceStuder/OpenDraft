//! The Model and layout tabs under the drawing, as in AutoCAD. Reports what
//! was clicked; the application state switches spaces.

use eframe::egui::{self, Rect, Response, Sense, Ui};
use opendraft_canvas::Space;
use opendraft_core::{Document, LayoutId};
use opendraft_ui::Theme;
use rust_i18n::t;

/// Height of a tab, smaller than other controls to keep the bar thin.
pub const TAB_HEIGHT: f32 = 20.0;

/// Space either side of a tab's name.
const TAB_PAD_X: f32 = 8.0;

/// Space between two tabs.
const TAB_GAP: f32 = 2.0;

/// What the user asked for with the tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabAction {
    Model,
    Layout(LayoutId),
    New,
    PageSetup(LayoutId),
    Copy(LayoutId),
    Delete(LayoutId),
}

/// Fill of the bar the tabs sit on: the panels' tone.
pub fn bar_fill(theme: &Theme) -> egui::Color32 {
    theme.colors.panel_bg
}

/// The line along the top of the bar that parts it from the panels above.
pub fn paint_bar_line(painter: &egui::Painter, theme: &Theme, bar: Rect) {
    let stroke = egui::Stroke::new(1.0_f32, theme.colors.border);
    painter.hline(bar.x_range(), bar.top() + 0.5, stroke);
}

/// One tab: flat, its name weak until hovered; the active tab sits on a
/// fill lighter than the bar.
fn tab(ui: &mut Ui, theme: &Theme, text: &str, active: bool) -> Response {
    let colors = &theme.colors;
    let font = egui::FontId::proportional(theme.font_size_body);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, colors.text);
    let height = ui.available_height().min(TAB_HEIGHT);
    let size = egui::vec2(galley.size().x + 2.0 * TAB_PAD_X, height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        // Only the active tab is filled, two steps lighter than the bar and
        // one darker than the command line's input band above it; hover
        // only brightens a name.
        if active {
            ui.painter()
                .rect_filled(rect, theme.radius, colors.raised_high);
        }
        let color = if active || response.hovered() {
            colors.text
        } else {
            colors.text_weak
        };
        let at = rect.center() - galley.size() / 2.0;
        ui.painter().galley(at, galley, color);
    }
    response
}

pub fn show(ui: &mut Ui, theme: &Theme, document: &Document, space: Space) -> Option<TabAction> {
    let mut action = None;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = TAB_GAP;
        if tab(ui, theme, &t!("Model"), space == Space::Model).clicked() {
            action = Some(TabAction::Model);
        }
        for layout in document.layouts() {
            let response = tab(ui, theme, &layout.name, space.layout() == Some(layout.id));
            if response.clicked() {
                action = Some(TabAction::Layout(layout.id));
            }
            response.context_menu(|ui| {
                for (label, act) in [
                    (t!("New layout"), TabAction::New),
                    (t!("Page Setup..."), TabAction::PageSetup(layout.id)),
                    (t!("Copy"), TabAction::Copy(layout.id)),
                    (t!("Delete"), TabAction::Delete(layout.id)),
                ] {
                    if ui.button(label).clicked() {
                        action = Some(act);
                        ui.close_menu();
                    }
                }
            });
        }
        ui.add_space(4.0);
        let new = ui
            .add(
                opendraft_ui::icon_set::Icon::new(opendraft_ui::icons::LAYOUT_TABS_NEW, 12.0)
                    .toggle(false)
                    .color(theme.colors.text_weak),
            )
            .on_hover_text(t!("New layout"))
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if new.clicked() {
            action = Some(TabAction::New);
        }
    });
    action
}

/// Do what a tab asked for.
pub fn apply(state: &mut crate::state::AppState, action: TabAction) {
    match action {
        TabAction::Model => state.enter_model(),
        TabAction::Layout(id) => state.enter_layout(id),
        TabAction::New => state.new_layout(),
        TabAction::PageSetup(id) => {
            state.enter_layout(id);
            state.open_page_setup();
        }
        TabAction::Copy(id) => state.duplicate_layout(id),
        TabAction::Delete(id) => state.delete_layout(id),
    }
}
