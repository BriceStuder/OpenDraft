//! The Layers panel's columns: their widths, where each sits in a row,
//! and the header that names them.

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
/// Columns every row has (and the VP Freeze one while a viewport is the target);
/// linetype and weight only appear when there is room.
pub fn core_width(vp: bool) -> f32 {
    STATUS_W + SWITCH_W * if vp { 4.0 } else { 3.0 } + COLOR_W
}
pub const EXTRA_W: f32 = LINETYPE_W + WEIGHT_W;

/// How wide a row is and whether the extra columns fit.
#[derive(Clone, Copy)]
pub struct Layout {
    pub width: f32,
    pub wide: bool,
    /// The VP Freeze column is shown.
    pub vp: bool,
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
    pub color: Rect,
    pub linetype: Rect,
    pub weight: Rect,
}

impl Columns {
    pub fn new(origin: egui::Pos2, layout: Layout) -> Self {
        let extra = if layout.wide { EXTRA_W } else { 0.0 };
        let name_w = layout.width - core_width(layout.vp) - extra;
        let mut x = origin.x;
        let mut next = |w: f32| {
            let rect = Rect::from_min_size(egui::pos2(x, origin.y), egui::vec2(w, ROW_HEIGHT));
            x += w;
            rect
        };
        Self {
            status: next(STATUS_W),
            name: next(name_w),
            on: next(SWITCH_W),
            freeze: next(SWITCH_W),
            vp_freeze: next(if layout.vp { SWITCH_W } else { 0.0 }),
            lock: next(SWITCH_W),
            color: next(COLOR_W),
            linetype: next(if layout.wide { LINETYPE_W } else { 0.0 }),
            weight: next(if layout.wide { WEIGHT_W } else { 0.0 }),
        }
    }
}

pub fn header(ui: &mut Ui, theme: &Theme, shape: Layout) {
    let (width, wide) = (shape.width, shape.wide);
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
    text(cols.color, &t!("Color"));
    if wide {
        text(cols.linetype, &t!("Linetype"));
        text(cols.weight, &t!("Weight"));
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0_f32, c.divider),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_tile_the_row_without_gaps() {
        let width = 480.0;
        let cols = Columns::new(
            egui::pos2(10.0, 0.0),
            Layout {
                width,
                wide: true,
                vp: true,
            },
        );
        assert_eq!(cols.status.left(), 10.0);
        assert_eq!(cols.status.right(), cols.name.left());
        assert_eq!(cols.on.right(), cols.freeze.left());
        assert_eq!(cols.freeze.right(), cols.vp_freeze.left());
        assert_eq!(cols.vp_freeze.right(), cols.lock.left());
        assert_eq!(cols.weight.right(), 10.0 + width);
    }

    #[test]
    fn narrow_rows_drop_the_extra_columns() {
        let cols = Columns::new(
            egui::pos2(0.0, 0.0),
            Layout {
                width: 260.0,
                wide: false,
                vp: false,
            },
        );
        assert_eq!(cols.vp_freeze.width(), 0.0, "hidden without a viewport");
        assert_eq!(cols.linetype.width(), 0.0);
        assert_eq!(cols.weight.width(), 0.0);
        assert_eq!(cols.color.right(), 260.0);
    }
}
