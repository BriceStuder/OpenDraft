//! Text in the Layers panel's name and linetype cells, and what it knows
//! about the drawing once for each state of it.

use std::collections::HashSet;
use std::sync::Arc;

use eframe::egui::{self, FontId, Rect, Ui};
use opendraft_core::{Document, LayerId};
use opendraft_ui::Theme;

/// Space left and right of the text in a cell.
const CELL_PAD: f32 = 6.0;

pub(crate) fn cell_text(ui: &Ui, theme: &Theme, rect: Rect, text: &str) {
    fit_text(ui, theme, rect, text, theme.colors.text);
}

/// Paints `text` in `rect` on one row, ending in an ellipsis when it does not
/// fit so it never runs into the next column. Returns whether it was cut.
pub(crate) fn fit_text(
    ui: &Ui,
    theme: &Theme,
    rect: Rect,
    text: &str,
    color: egui::Color32,
) -> bool {
    let font = FontId::proportional(theme.font_size_body);
    let room = (rect.width() - CELL_PAD * 2.0).max(0.0);
    let full = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), color);
    let cut = full.size().x > room;
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font, color, room);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    let pos = egui::pos2(
        rect.left() + CELL_PAD,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(pos, galley, color);
    cut
}

/// The state of the drawing and the layers in use then.
type InUse = ((u64, u64), Arc<HashSet<LayerId>>);

/// The layers with objects on them, found once for each state of the drawing
/// (`token`, as `Session::draw_token` gives it): finding them walks every
/// object, which a big drawing cannot afford in each frame.
pub(crate) fn layers_in_use(
    ui: &egui::Ui,
    document: &Document,
    token: (u64, u64),
) -> Arc<HashSet<LayerId>> {
    let id = ui.make_persistent_id("layers_in_use");
    let kept: Option<InUse> = ui.data(|d| d.get_temp(id));
    if let Some((_, set)) = kept.filter(|(at, _)| *at == token) {
        return set;
    }
    let set = Arc::new(document.layers_in_use());
    ui.data_mut(|d| d.insert_temp(id, (token, set.clone())));
    set
}
