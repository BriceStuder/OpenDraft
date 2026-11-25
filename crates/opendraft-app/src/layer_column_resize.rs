//! Resizing the Layers panel's name, linetype and lineweight columns: drag
//! the line at a column's right edge in the header, or double-click it to fit
//! the column to its widest entry. The widths are kept in egui's persisted
//! memory, which eframe saves between runs.

use eframe::egui::{self, CursorIcon, Rect, Sense, Stroke, Ui};
use opendraft_core::Document;
use opendraft_ui::Theme;
use rust_i18n::t;

use crate::layer_cells::text_room;
use crate::layer_columns::{Columns, Layout, Widths, MIN_W};

/// A column that can be resized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Column {
    Name,
    Linetype,
    Weight,
}

fn key() -> egui::Id {
    egui::Id::new("layers_panel_column_widths")
}

/// The widths last given, or the defaults.
pub fn load(ctx: &egui::Context) -> Widths {
    ctx.data_mut(|d| d.get_persisted(key())).unwrap_or_default()
}

pub fn store(ctx: &egui::Context, widths: Widths) {
    ctx.data_mut(|d| d.insert_persisted(key(), widths));
}

/// `widths` with `column` set to `width` (never under the minimum).
pub fn resized(mut widths: Widths, column: Column, width: f32) -> Widths {
    let width = width.max(MIN_W);
    match column {
        Column::Name => widths.name = Some(width),
        Column::Linetype => widths.linetype = width,
        Column::Weight => widths.weight = width,
    }
    widths
}

/// The width the column's widest entry (or its title) needs.
pub fn content_width(ui: &Ui, theme: &Theme, document: &Document, column: Column) -> f32 {
    let layers = document.layers();
    let texts: Vec<String> = match column {
        Column::Name => layers.iter().map(|l| l.name.clone()).collect(),
        Column::Linetype => layers
            .iter()
            .filter_map(|l| document.linetypes().get(l.linetype))
            .map(|r| r.name.clone())
            .collect(),
        Column::Weight => layers
            .iter()
            .map(|l| crate::layers_ui::weight_label(l.lineweight))
            .collect(),
    };
    let title = match column {
        Column::Name => t!("Name"),
        Column::Linetype => t!("Linetype"),
        Column::Weight => t!("Weight"),
    };
    let widest = texts
        .iter()
        .map(|text| text_room(ui, theme, text))
        .fold(text_room(ui, theme, &title), f32::max);
    // A pixel to spare, so rounding never ends the text in an ellipsis.
    (widest + 1.0).ceil()
}

/// The handles at the resizable columns' right edges in the header at
/// `header`. Returns whether `widths` changed.
pub fn handles(
    ui: &Ui,
    theme: &Theme,
    header: Rect,
    layout: Layout,
    widths: &mut Widths,
    fit: impl Fn(Column) -> f32,
) -> bool {
    let cols = Columns::new(header.min, layout);
    let edges = [
        (Column::Name, cols.name.right(), layout.name),
        (Column::Linetype, cols.linetype.right(), layout.linetype),
        (Column::Weight, cols.weight.right(), layout.weight),
    ];
    let mut changed = false;
    for (column, x, width) in edges {
        let grip = Rect::from_x_y_ranges(x - 3.0..=x + 3.0, header.y_range());
        let response = ui.interact(grip, key().with(column), Sense::click_and_drag());
        let active = response.hovered() || response.dragged();
        let color = if active {
            theme.colors.accent
        } else {
            theme.colors.divider
        };
        let ys = egui::Rangef::new(header.top() + 4.0, header.bottom() - 4.0);
        ui.painter().vline(x - 0.5, ys, Stroke::new(1.0_f32, color));
        if active {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeColumn);
        }
        let new = if response.double_clicked() {
            Some(fit(column))
        } else if response.dragged() && response.drag_delta().x != 0.0 {
            Some(width + response.drag_delta().x)
        } else {
            None
        };
        if let Some(new) = new {
            let next = resized(*widths, column, new);
            changed |= next != *widths;
            *widths = next;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resizing_sets_one_column_and_keeps_the_minimum() {
        let start = Widths::default();
        let named = resized(start, Column::Name, 150.0);
        assert_eq!(named.name, Some(150.0));
        assert_eq!(named.linetype, start.linetype);
        let thin = resized(start, Column::Weight, 3.0);
        assert_eq!(thin.weight, MIN_W);
        assert_eq!(resized(start, Column::Linetype, 120.0).linetype, 120.0);
    }

    #[test]
    fn widths_are_kept_in_persisted_memory() {
        let ctx = egui::Context::default();
        assert_eq!(load(&ctx), Widths::default());
        let widths = resized(Widths::default(), Column::Name, 200.0);
        store(&ctx, widths);
        assert_eq!(load(&ctx), widths);
    }
}
