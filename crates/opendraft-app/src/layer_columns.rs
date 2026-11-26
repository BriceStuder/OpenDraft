//! The Layers panel's columns: their widths, where each sits in a row,
//! and the header that names them. The name, linetype and lineweight
//! columns can be resized (`layer_column_resize`); the switches and the
//! colour keep their width.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui};
use opendraft_ui::{icons, Theme};
use rust_i18n::t;

pub const ROW_HEIGHT: f32 = 22.0;
pub const NAME_MIN: f32 = 90.0;
const STATUS_W: f32 = 22.0;
const SWITCH_W: f32 = 26.0;
const COLOR_W: f32 = 44.0;
const LINETYPE_W: f32 = 92.0;
const WEIGHT_W: f32 = 78.0;
/// The narrowest a resized column can be.
pub const MIN_W: f32 = 40.0;
/// The fixed columns (and the VP Freeze one while a viewport is the target).
pub fn core_width(vp: bool) -> f32 {
    STATUS_W + SWITCH_W * if vp { 5.0 } else { 4.0 } + COLOR_W
}

/// The widths the user gave the resizable columns, kept between runs.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Widths {
    /// `None` until the user resizes it: the name then fills the room left.
    pub name: Option<f32>,
    pub linetype: f32,
    pub weight: f32,
}

impl Default for Widths {
    fn default() -> Self {
        Self {
            name: None,
            linetype: LINETYPE_W,
            weight: WEIGHT_W,
        }
    }
}

/// How wide a row and its resizable columns are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub width: f32,
    /// The VP Freeze column is shown.
    pub vp: bool,
    pub name: f32,
    pub linetype: f32,
    pub weight: f32,
}

impl Layout {
    /// The row for a panel `available` wide. Every column shows; a name not
    /// yet resized fills the room left (never under `NAME_MIN`). The row is
    /// never narrower than the panel, and a wider one scrolls sideways.
    pub fn fit(available: f32, vp: bool, widths: Widths) -> Self {
        let core = core_width(vp);
        let (linetype, weight) = (widths.linetype.max(MIN_W), widths.weight.max(MIN_W));
        let set_name = widths.name.map(|w| w.max(MIN_W));
        let extra = linetype + weight;
        let name = set_name.unwrap_or((available - core - extra).max(NAME_MIN));
        Self {
            width: available.max(name + core + extra),
            vp,
            name,
            linetype,
            weight,
        }
    }
}

/// Column rectangles for a row starting at `origin`. Hidden columns are empty.
pub struct Columns {
    pub status: Rect,
    pub name: Rect,
    pub on: Rect,
    pub freeze: Rect,
    /// Zero width unless a viewport is the target.
    pub vp_freeze: Rect,
    pub lock: Rect,
    pub plot: Rect,
    pub color: Rect,
    pub linetype: Rect,
    pub weight: Rect,
}

impl Columns {
    pub fn new(origin: egui::Pos2, layout: Layout) -> Self {
        let mut x = origin.x;
        let mut next = |w: f32| {
            let rect = Rect::from_min_size(egui::pos2(x, origin.y), egui::vec2(w, ROW_HEIGHT));
            x += w;
            rect
        };
        Self {
            status: next(STATUS_W),
            name: next(layout.name),
            on: next(SWITCH_W),
            freeze: next(SWITCH_W),
            vp_freeze: next(if layout.vp { SWITCH_W } else { 0.0 }),
            lock: next(SWITCH_W),
            plot: next(SWITCH_W),
            color: next(COLOR_W),
            linetype: next(layout.linetype),
            weight: next(layout.weight),
        }
    }
}

/// Paints the header row and returns where it is.
pub fn header(ui: &mut Ui, theme: &Theme, shape: Layout) -> Rect {
    let width = shape.width;
    let c = &theme.colors;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c.panel_raised);
    let cols = Columns::new(rect.min, shape);
    let text = |rect: Rect, label: &str| {
        ui.painter().text(
            rect.left_center() + egui::vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(theme.font_size_small),
            c.text_weak,
        );
    };
    let glyph = |rect: Rect, icon: char| {
        opendraft_ui::icon_set::paint(
            ui.painter(),
            rect.center(),
            Align2::CENTER_CENTER,
            icon,
            13.0,
            c.text_weak,
        );
    };
    text(cols.name, &t!("Name"));
    // The same bulb, sun and padlock as the ribbon's layer list.
    glyph(cols.on, icons::LAYER_LIST_ON);
    glyph(cols.freeze, icons::LAYER_LIST_THAWED);
    if shape.vp {
        ui.painter().text(
            cols.vp_freeze.center(),
            Align2::CENTER_CENTER,
            t!("VP"),
            FontId::proportional(theme.font_size_small),
            c.text_weak,
        );
    }
    glyph(cols.lock, icons::LAYER_LIST_UNLOCKED);
    glyph(cols.plot, icons::FILE_PLOT);
    text(cols.color, &t!("Color"));
    text(cols.linetype, &t!("Linetype"));
    text(cols.weight, &t!("Weight"));
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0_f32, c.divider),
    );
    rect
}

/// The Plot switch: a yellow printer for a plotted layer, a grey one struck
/// through for a layer left out of plots (Defpoints always is).
pub fn paint_plot_switch(painter: &egui::Painter, cell: Rect, plots: bool, weak: egui::Color32) {
    let color = if plots {
        crate::ribbon_slots::SWITCH_ON
    } else {
        weak
    };
    let at = cell.center();
    let icon = opendraft_ui::icon_set::paint(
        painter,
        at,
        Align2::CENTER_CENTER,
        icons::FILE_PLOT,
        14.0,
        color,
    );
    if !plots {
        let (a, b) = (icon.left_bottom(), icon.right_top());
        painter.line_segment([a, b], Stroke::new(1.5_f32, color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_tile_the_row_without_gaps() {
        let width = 480.0;
        let layout = Layout::fit(width, true, Widths::default());
        let cols = Columns::new(egui::pos2(10.0, 0.0), layout);
        assert_eq!(cols.status.left(), 10.0);
        assert_eq!(cols.status.right(), cols.name.left());
        assert_eq!(cols.on.right(), cols.freeze.left());
        assert_eq!(cols.freeze.right(), cols.vp_freeze.left());
        assert_eq!(cols.vp_freeze.right(), cols.lock.left());
        assert_eq!(cols.weight.right(), 10.0 + width);
    }

    #[test]
    fn a_narrow_panel_keeps_every_column_and_scrolls() {
        let widths = Widths::default();
        let layout = Layout::fit(260.0, false, widths);
        let cols = Columns::new(egui::pos2(0.0, 0.0), layout);
        assert_eq!(cols.vp_freeze.width(), 0.0, "hidden without a viewport");
        assert_eq!(cols.name.width(), NAME_MIN);
        assert_eq!(cols.linetype.width(), widths.linetype);
        assert_eq!(cols.weight.width(), widths.weight);
        assert!(layout.width > 260.0);
        assert_eq!(cols.weight.right(), layout.width);
    }

    #[test]
    fn a_resized_name_keeps_its_width_and_a_wider_row_scrolls() {
        let widths = Widths {
            name: Some(120.0),
            linetype: 60.0,
            weight: 50.0,
        };
        let roomy = Layout::fit(600.0, false, widths);
        assert_eq!((roomy.name, roomy.width), (120.0, 600.0));
        let cols = Columns::new(egui::pos2(0.0, 0.0), roomy);
        assert_eq!(cols.name.width(), 120.0);
        assert_eq!((cols.linetype.width(), cols.weight.width()), (60.0, 50.0));
        assert!(cols.weight.right() < 600.0, "the room left stays empty");

        let wide_name = Widths {
            name: Some(500.0),
            ..widths
        };
        let narrow = Layout::fit(300.0, false, wide_name);
        assert_eq!(narrow.width, 500.0 + core_width(false) + 110.0);
    }

    #[test]
    fn widths_never_fall_below_the_minimum() {
        let tiny = Widths {
            name: Some(1.0),
            linetype: 0.0,
            weight: -5.0,
        };
        let layout = Layout::fit(800.0, false, tiny);
        assert_eq!(
            (layout.name, layout.linetype, layout.weight),
            (MIN_W, MIN_W, MIN_W)
        );
    }
}
