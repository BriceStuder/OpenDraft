//! The controls the Home tab draws in its ribbon slots, as in AutoCAD: the
//! layer list with its light bulb, sun and lock switches, and the colour,
//! lineweight and linetype lists. They show the selected objects' values and
//! change them. With nothing selected the layer list shows and sets the
//! current layer, and the other lists show and set the colour, linetype and
//! lineweight new objects are drawn with (ByLayer unless set).

use std::borrow::Cow;

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, Ui};
use opendraft_core::{Document, Layer, Object};
use opendraft_ui::{icons, menu, Theme};

use crate::attr_rows::{self, shared};
use crate::colour_ui;
use crate::layers::{AttrEdit, LayerEdit, LayerFlag};
use crate::state::AppState;
use crate::view_freeze::{FreezeTarget, VpSwitch};

pub const LAYER_LIST: &str = "layer.list";
pub const COLOR_LIST: &str = "properties.color";
pub const LINEWEIGHT_LIST: &str = "properties.lineweight";
pub const LINETYPE_LIST: &str = "properties.linetype";

/// The light bulb, sun and open padlock, in AutoCAD's yellow.
pub(crate) const SWITCH_ON: Color32 = Color32::from_rgb(240, 196, 64);
const ROW_H: f32 = 22.0;
/// Width of one switch icon in the layer list.
const SWITCH_W: f32 = 20.0;

/// What a ribbon control asks for.
pub enum SlotEdit {
    Layer(LayerEdit),
    /// A change to the selected objects.
    Attr(AttrEdit),
}

impl AppState {
    pub fn apply_slot_edit(&mut self, edit: SlotEdit) {
        match edit {
            SlotEdit::Layer(edit) => self.apply_layer_edit(edit),
            SlotEdit::Attr(edit) if self.docs[self.active].selection.is_empty() => {
                self.set_new_object_style(&edit)
            }
            SlotEdit::Attr(edit) => self.set_attrs(&edit, None),
        }
    }
}

/// Draw the control of every slot the ribbon laid out; returns the edit
/// made, if any.
pub fn show(
    ui: &mut Ui,
    theme: &Theme,
    state: &AppState,
    slots: &[(&'static str, Rect)],
) -> Option<SlotEdit> {
    let doc = &state.docs[state.active];
    let document = doc.session.document();
    let selected: Vec<&Object> = crate::selected_objects::of(ui.ctx(), doc);
    let mut edit = None;
    for &(slot, rect) in slots {
        let id = egui::Id::new(("ribbon_slot", slot));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        // The Home page has no drawing behind it, so the lists are inert.
        if state.home {
            child.disable();
        }
        let made = match slot {
            LAYER_LIST => layer_list(
                &mut child,
                theme,
                id,
                document,
                &selected,
                state.freeze_target(),
            ),
            COLOR_LIST | LINEWEIGHT_LIST | LINETYPE_LIST => {
                property_list(&mut child, theme, id, slot, document, &selected).map(SlotEdit::Attr)
            }
            _ => None,
        };
        edit = made.or(edit);
    }
    edit
}

/// One of the three property lists, with its icon at the left.
fn property_list(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    slot: &str,
    document: &Document,
    selected: &[&Object],
) -> Option<AttrEdit> {
    let rect = ui.max_rect();
    let icon = match slot {
        COLOR_LIST => icons::PROPERTIES_COLOR,
        LINEWEIGHT_LIST => icons::PROPERTIES_LINEWEIGHT,
        _ => icons::PROPERTIES_LINETYPE,
    };
    opendraft_ui::icon_set::paint(
        ui.painter(),
        egui::pos2(rect.left() + 9.0, rect.center().y),
        Align2::CENTER_CENTER,
        icon,
        14.0,
        theme.colors.text,
    );
    let area = Rect::from_min_max(egui::pos2(rect.left() + 22.0, rect.top()), rect.max);
    if selected.is_empty() {
        // Nothing to change: the lists show and set what new objects get.
        let (color, linetype, lineweight) = document.new_object_style();
        return match slot {
            COLOR_LIST => {
                let layer = document.layer(document.current_layer()).map(|l| l.color);
                attr_rows::color_value_field(ui, theme, id, area, Some(color.clone()), layer)
            }
            LINEWEIGHT_LIST => {
                attr_rows::lineweight_value_field(ui, theme, id, area, Some(lineweight.clone()))
            }
            _ => attr_rows::linetype_value_field(
                ui,
                theme,
                id,
                area,
                document,
                Some(linetype.clone()),
            ),
        };
    }
    match slot {
        COLOR_LIST => attr_rows::color_field(ui, theme, id, area, document, selected),
        LINEWEIGHT_LIST => attr_rows::lineweight_field(ui, theme, id, area, selected),
        _ => attr_rows::linetype_field(ui, theme, id, area, document, selected),
    }
}

/// The layer list: the selected objects' layer (blank when they differ) or
/// the current layer. Its menu lists every layer with switches that toggle
/// without closing it; picking a layer moves the selection to it, or makes
/// it current when nothing is selected. With a viewport to work on (in a
/// layout), each layer also has the Layers panel's VP Freeze switch.
fn layer_list(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    document: &Document,
    selected: &[&Object],
    target: Option<FreezeTarget>,
) -> Option<SlotEdit> {
    let vp_frozen = |layer: &Layer| target.map(|t| t.switch(layer));
    let rect = ui.max_rect();
    let response = ui.interact(rect, id.with("field"), Sense::click());
    let shown = if selected.is_empty() {
        Some(document.current_layer())
    } else {
        shared(selected.iter().map(|o| o.attrs.layer))
    };
    menu::paint_combo(ui, theme, rect, "", response.hovered());
    if let Some(layer) = shown.and_then(|id| document.layer(id)) {
        layer_row_contents(ui, theme, rect, layer, theme.colors.text, vp_frozen(layer));
    }

    let mut toggled = None;
    let picked = menu::popup(ui, theme, id.with("menu"), &response, rect.width(), |ui| {
        let rows = |ui: &mut Ui| {
            let mut picked = None;
            for layer in document.layers_by_name() {
                let current = shown == Some(layer.id);
                let (pick, toggle) = menu_row(ui, theme, layer, current, vp_frozen(layer));
                if pick {
                    picked = Some(layer.id);
                }
                if toggle.is_some() {
                    toggled = toggle;
                }
            }
            picked
        };
        egui::ScrollArea::vertical()
            .max_height(ROW_H * 16.0)
            .show(ui, rows)
            .inner
    });
    if let Some(layer) = picked {
        return Some(if selected.is_empty() {
            SlotEdit::Layer(LayerEdit::SetCurrent(layer))
        } else {
            SlotEdit::Attr(AttrEdit::Layer(layer))
        });
    }
    toggled.map(SlotEdit::Layer)
}

/// One layer in the menu. Returns whether the layer was picked and the
/// switch toggled, if any. `vp_frozen` is the layer's VP Freeze switch
/// (`None`: none).
fn menu_row(
    ui: &mut Ui,
    theme: &Theme,
    layer: &Layer,
    current: bool,
    vp_frozen: Option<VpSwitch>,
) -> (bool, Option<LayerEdit>) {
    let c = &theme.colors;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::hover());
    let toggles = toggles(vp_frozen.is_some());
    let switches = toggles.len() as f32 * SWITCH_W + 4.0;
    let name = Rect::from_min_max(egui::pos2(rect.left() + switches, rect.top()), rect.max);
    let picked = ui.interact(name, ui.id().with(("layer_pick", layer.id)), Sense::click());
    if current {
        ui.painter().rect_filled(rect, 2.0, c.accent);
    } else if picked.hovered() {
        ui.painter().rect_filled(rect, 2.0, c.hover);
    }
    let text = if current { c.text_on_accent } else { c.text };
    let mut toggled = None;
    for (i, toggle) in toggles.into_iter().enumerate() {
        let at = rect.left() + 2.0 + i as f32 * SWITCH_W;
        let switch = Rect::from_min_size(egui::pos2(at, rect.top()), egui::vec2(SWITCH_W, ROW_H));
        let response = ui.interact(
            switch,
            ui.id().with(("layer_flag", layer.id, i)),
            Sense::click(),
        );
        if response.hovered() {
            ui.painter().rect_stroke(
                switch.shrink(1.0),
                2.0,
                Stroke::new(1.0_f32, c.border),
                egui::StrokeKind::Inside,
            );
        }
        let (tip, edit) = match toggle {
            Toggle::Viewport => {
                let frozen = vp_frozen.is_some_and(|s| s.frozen);
                (
                    vp_frozen.map_or(Cow::Borrowed(""), VpSwitch::tip),
                    LayerEdit::SetInViewport(vec![layer.id], !frozen),
                )
            }
            Toggle::Flag(flag) => (switch_tip(layer, flag), LayerEdit::Toggle(layer.id, flag)),
        };
        if response.on_hover_text(tip.as_ref()).clicked() {
            toggled = Some(edit);
        }
    }
    layer_row_contents(ui, theme, rect, layer, text, vp_frozen);
    (picked.clicked(), toggled)
}

/// A switch of the layer list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Toggle {
    Flag(LayerFlag),
    /// Freeze in the viewport being worked on.
    Viewport,
}

/// The switches in order: bulb, sun, the VP Freeze one when `vp` (where the
/// Layers panel has its column), padlock.
fn toggles(vp: bool) -> Vec<Toggle> {
    let mut toggles = vec![Toggle::Flag(LayerFlag::On), Toggle::Flag(LayerFlag::Frozen)];
    if vp {
        toggles.push(Toggle::Viewport);
    }
    toggles.push(Toggle::Flag(LayerFlag::Locked));
    toggles
}

/// The VP Freeze switch's glyph and colour: the sun and snowflake of the
/// Freeze switch, framed by corners by `paint_vp_switch`.
pub(crate) fn vp_glyph(frozen: bool, weak: Color32) -> (char, Color32) {
    if frozen {
        (icons::LAYER_LIST_FROZEN, weak)
    } else {
        (icons::LAYER_LIST_THAWED, SWITCH_ON)
    }
}

/// How much smaller the sun or snowflake is in its square: just enough not
/// to touch it.
const VP_GLYPH_SCALE: f32 = 0.9;

/// How far each corner mark runs along a side of the square.
const VP_CORNER: f32 = 0.25;

/// The VP Freeze switch: the sun or snowflake, a little smaller, framed by
/// the four corners of a square `size` wide, so it reads apart from the
/// Freeze switch beside it. The corners are one screen pixel wide, on whole
/// pixels so they stay sharp.
pub(crate) fn paint_vp_switch(
    painter: &Painter,
    center: Pos2,
    size: f32,
    (glyph, color): (char, Color32),
) {
    let ppp = painter.ctx().pixels_per_point();
    let snap = |v: f32| (v * ppp).round() / ppp;
    let side = snap(size);
    let min = egui::pos2(snap(center.x - side / 2.0), snap(center.y - side / 2.0));
    let square = Rect::from_min_size(min, egui::vec2(side, side));
    let stroke = Stroke::new(1.0 / ppp, color);
    for corner in corner_marks(square, 1.0 / ppp, snap(side * VP_CORNER)) {
        painter.add(egui::Shape::line(corner.to_vec(), stroke));
    }
    opendraft_ui::icon_set::paint(
        painter,
        square.center(),
        Align2::CENTER_CENTER,
        glyph,
        size * VP_GLYPH_SCALE,
        color,
    );
}

/// The four corner marks of `square`, each an L of three points with arms
/// `arm` long, running down the middle of the edge pixels (`pixel` wide) so
/// a one-pixel line fills them exactly.
fn corner_marks(square: Rect, pixel: f32, arm: f32) -> [[Pos2; 3]; 4] {
    let inner = square.shrink(pixel / 2.0);
    let (l, r, t, b) = (inner.left(), inner.right(), inner.top(), inner.bottom());
    let p = egui::pos2;
    [
        [p(l, t + arm), p(l, t), p(l + arm, t)],
        [p(r - arm, t), p(r, t), p(r, t + arm)],
        [p(r, b - arm), p(r, b), p(r - arm, b)],
        [p(l + arm, b), p(l, b), p(l, b - arm)],
    ]
}

fn switch_tip(layer: &Layer, flag: LayerFlag) -> Cow<'static, str> {
    match flag {
        LayerFlag::On if layer.on => rust_i18n::t!("On: click to turn off"),
        LayerFlag::On => rust_i18n::t!("Off: click to turn on"),
        LayerFlag::Frozen if layer.frozen => rust_i18n::t!("Frozen: click to thaw"),
        LayerFlag::Frozen => rust_i18n::t!("Thawed: click to freeze"),
        LayerFlag::Locked if layer.locked => rust_i18n::t!("Locked: click to unlock"),
        LayerFlag::Locked => rust_i18n::t!("Unlocked: click to lock"),
    }
}

/// The bulb, sun and padlock of a layer (in this list and the Layers
/// panel), and their colours: yellow when the
/// layer can be seen and edited (on, thawed, unlocked), `weak` grey when not,
/// so a locked layer stands out as the off and frozen ones do.
pub(crate) fn switch_glyphs(layer: &Layer, weak: Color32) -> [(char, Color32); 3] {
    [
        if layer.on {
            (icons::LAYER_LIST_ON, SWITCH_ON)
        } else {
            (icons::LAYER_LIST_OFF, weak)
        },
        if layer.frozen {
            (icons::LAYER_LIST_FROZEN, weak)
        } else {
            (icons::LAYER_LIST_THAWED, SWITCH_ON)
        },
        if layer.locked {
            (icons::LAYER_LIST_LOCKED, weak)
        } else {
            (icons::LAYER_LIST_UNLOCKED, SWITCH_ON)
        },
    ]
}

/// A layer's switches, colour and name across `rect`, as AutoCAD's list.
fn layer_row_contents(
    ui: &Ui,
    theme: &Theme,
    rect: Rect,
    layer: &Layer,
    text: Color32,
    vp_frozen: Option<VpSwitch>,
) {
    let c = &theme.colors;
    let painter = ui.painter();
    let [on, thaw, lock] = switch_glyphs(layer, c.text_weak);
    let toggles = toggles(vp_frozen.is_some());
    for (i, toggle) in toggles.iter().enumerate() {
        let at = egui::pos2(
            rect.left() + 2.0 + (i as f32 + 0.5) * SWITCH_W,
            rect.center().y,
        );
        let (glyph, color) = match toggle {
            Toggle::Flag(LayerFlag::On) => on,
            Toggle::Flag(LayerFlag::Frozen) => thaw,
            Toggle::Flag(LayerFlag::Locked) => lock,
            Toggle::Viewport => {
                let frozen = vp_frozen.is_some_and(|s| s.frozen);
                paint_vp_switch(painter, at, 13.0, vp_glyph(frozen, c.text_weak));
                continue;
            }
        };
        opendraft_ui::icon_set::paint(painter, at, Align2::CENTER_CENTER, glyph, 13.0, color);
    }
    let swatch = Rect::from_center_size(
        egui::pos2(
            rect.left() + toggles.len() as f32 * SWITCH_W + 12.0,
            rect.center().y,
        ),
        egui::vec2(11.0, 11.0),
    );
    colour_ui::paint_swatch(painter, theme, swatch, layer.color);
    // The name stops short of the field's chevron.
    let room = Rect::from_min_max(
        egui::pos2(swatch.right() + 6.0, rect.top()),
        egui::pos2(rect.right() - 22.0, rect.bottom()),
    );
    painter.with_clip_rect(room.intersect(ui.clip_rect())).text(
        room.left_center(),
        Align2::LEFT_CENTER,
        &layer.name,
        FontId::proportional(theme.font_size_body),
        text,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_padlock_is_yellow_when_open_and_grey_when_locked_like_the_bulb_and_sun() {
        let grey = Color32::GRAY;
        let mut layer = Layer::new(opendraft_core::LayerId::DEFAULT, "0");
        let colours = |l: &Layer| switch_glyphs(l, grey).map(|(_, c)| c);
        assert_eq!(colours(&layer), [SWITCH_ON; 3], "on, thawed, unlocked");
        layer.locked = true;
        assert_eq!(colours(&layer)[2], grey);
        layer.on = false;
        layer.frozen = true;
        assert_eq!(colours(&layer), [grey; 3]);
    }

    #[test]
    fn the_vp_freeze_switch_sits_between_the_sun_and_the_padlock_in_a_layout() {
        use LayerFlag::{Frozen, Locked, On};
        assert_eq!(
            toggles(false),
            [Toggle::Flag(On), Toggle::Flag(Frozen), Toggle::Flag(Locked)]
        );
        assert_eq!(
            toggles(true),
            [
                Toggle::Flag(On),
                Toggle::Flag(Frozen),
                Toggle::Viewport,
                Toggle::Flag(Locked)
            ]
        );
    }

    #[test]
    fn the_vp_freeze_switch_is_a_yellow_sun_or_a_grey_snowflake() {
        let grey = Color32::GRAY;
        assert_eq!(vp_glyph(false, grey), (icons::LAYER_LIST_THAWED, SWITCH_ON));
        assert_eq!(vp_glyph(true, grey), (icons::LAYER_LIST_FROZEN, grey));
    }

    #[test]
    fn the_vp_corners_run_down_the_middle_of_the_edge_pixels() {
        let square = Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(14.0, 14.0));
        let [top_left, top_right, bottom_right, bottom_left] = corner_marks(square, 1.0, 3.0);
        let p = egui::pos2;
        assert_eq!(top_left, [p(10.5, 23.5), p(10.5, 20.5), p(13.5, 20.5)]);
        assert_eq!(top_right, [p(20.5, 20.5), p(23.5, 20.5), p(23.5, 23.5)]);
        assert_eq!(bottom_right, [p(23.5, 30.5), p(23.5, 33.5), p(20.5, 33.5)]);
        assert_eq!(bottom_left, [p(13.5, 33.5), p(10.5, 33.5), p(10.5, 30.5)]);
    }
}
