//! The Layers panel: a table of the drawing's layers with their switches,
//! colour, linetype and lineweight. Reports what the user asked for; the
//! application state does it.

use eframe::egui::{self, Align2, Rect, Sense, Stroke, Ui};
use opendraft_core::{Document, Layer, LayerId, Lineweight};
use opendraft_ui::{icons, menu, widgets, Theme};
use rust_i18n::t;

use crate::colour_ui::{self, Pick};
use crate::layer_cells::{cell_text, fit_text, layers_in_use};
use crate::layer_columns::{core_width, header, Columns, Layout, EXTRA_W, NAME_MIN, ROW_HEIGHT};
use crate::layer_menu::{self, MenuPick};
use crate::layer_selection::LayerSelection;
use crate::layers::{LayerEdit, LayerFlag};
use crate::linetype_ui;
use crate::view_freeze::{FreezeTarget, VpSwitch};

/// `target` is where the VP Freeze column freezes layers, if anywhere: the
/// viewport being worked on in a layout, or the Model tab.
pub fn show(
    ui: &mut Ui,
    theme: &Theme,
    document: &Document,
    token: (u64, u64),
    target: Option<FreezeTarget>,
) -> Option<LayerEdit> {
    let in_use = layers_in_use(ui, document, token);
    let base = ui.make_persistent_id("layers_panel");
    let mut selection = LayerSelection::load(ui.ctx(), base.with("selection"), document);
    let order: Vec<LayerId> = document.layers_by_name().iter().map(|l| l.id).collect();
    let mut edit = None;

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(6.0);
        if widgets::secondary_button(ui, theme, &t!("New"))
            .on_hover_text(t!("Create a layer"))
            .clicked()
        {
            edit = Some(LayerEdit::New);
        }
        ui.add_enabled_ui(!selection.ids().is_empty(), |ui| {
            if widgets::secondary_button(ui, theme, &t!("Delete"))
                .on_hover_text(t!("Delete the selected layers"))
                .clicked()
            {
                edit = Some(LayerEdit::Delete(selection.ids().to_vec()));
            }
        });
        ui.add_enabled_ui(selection.single().is_some(), |ui| {
            if widgets::secondary_button(ui, theme, &t!("Set Current"))
                .on_hover_text(t!("Draw new objects on the selected layer"))
                .clicked()
            {
                edit = selection.single().map(LayerEdit::SetCurrent);
            }
        });
    });
    ui.add_space(4.0);

    let menu_key = base.with("menu");
    let modifiers = ui.input(|i| i.modifiers);
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let core = core_width(target.is_some());
            let wide = ui.available_width() >= NAME_MIN + core + EXTRA_W;
            let width = ui.available_width().max(NAME_MIN + core);
            let shape = Layout {
                width,
                wide,
                vp: target.is_some(),
            };
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            header(ui, theme, shape);
            for layer in document.layers_by_name() {
                let picked = row(
                    ui,
                    theme,
                    document,
                    shape,
                    layer,
                    RowState {
                        base,
                        current: layer.id == document.current_layer(),
                        in_use: in_use.contains(&layer.id),
                        selected: selection.contains(layer.id),
                        targets: selection.targets(layer.id),
                        vp: target.map(|t| t.switch(layer)),
                    },
                );
                if let Some(action) = picked.edit {
                    edit = Some(action);
                }
                if picked.select {
                    selection.click(layer.id, &order, modifiers.command, modifiers.shift);
                }
                if let Some(at) = picked.menu {
                    selection.point_at(layer.id);
                    layer_menu::open(ui, menu_key, at);
                }
            }
            // The empty space under the rows: a click clears the selection,
            // a right-click opens the menu with nothing selected.
            let rest = ui.available_size().max(egui::vec2(width, ROW_HEIGHT));
            let (_, below) = ui.allocate_exact_size(rest, Sense::click());
            if below.clicked() {
                selection.clear();
            }
            if below.secondary_clicked() {
                selection.clear();
                let at = below.interact_pointer_pos().unwrap_or(below.rect.min);
                layer_menu::open(ui, menu_key, at);
            }
        });
    match layer_menu::show(ui, theme, menu_key, &selection) {
        Some(MenuPick::Edit(action)) => edit = Some(action),
        Some(MenuPick::Rename(id)) => start_rename(ui, base, id, document),
        Some(MenuPick::SelectAll) => selection.select_all(&order),
        Some(MenuPick::SelectAllButCurrent) => {
            selection.select_all_but(&order, document.current_layer())
        }
        Some(MenuPick::Invert) => selection.invert(&order),
        Some(MenuPick::ClearAll) => selection.clear(),
        None => {}
    }
    selection.store(ui.ctx(), base.with("selection"));
    edit
}

/// Put the name cell of layer `id` into editing, as a double-click does.
fn start_rename(ui: &Ui, base: egui::Id, id: LayerId, document: &Document) {
    let Some(layer) = document.layer(id) else {
        return;
    };
    let editing_id = base.with(("layer", id)).with("renaming");
    ui.data_mut(|d| {
        d.insert_temp(editing_id, true);
        d.insert_temp(editing_id.with("text"), layer.name.clone());
    });
}

/// What a row reported.
#[derive(Default)]
struct RowResult {
    edit: Option<LayerEdit>,
    /// The row itself was clicked (select it).
    select: bool,
    /// The row was right-clicked here (open the menu).
    menu: Option<egui::Pos2>,
}

/// What a row shows besides the layer's own settings.
struct RowState {
    /// The panel's id, under which each row keeps its state.
    base: egui::Id,
    current: bool,
    in_use: bool,
    selected: bool,
    /// The layers a change on this row applies to (see `LayerSelection::targets`).
    targets: Vec<LayerId>,
    /// The layer's VP Freeze switch (`None`: no column).
    vp: Option<VpSwitch>,
}

fn row(
    ui: &mut Ui,
    theme: &Theme,
    document: &Document,
    layout: Layout,
    layer: &Layer,
    state: RowState,
) -> RowResult {
    let RowState {
        base,
        current,
        in_use,
        selected,
        targets,
        vp,
    } = state;
    let c = &theme.colors;
    let mut result = RowResult::default();
    let (rect, background) =
        ui.allocate_exact_size(egui::vec2(layout.width, ROW_HEIGHT), Sense::click());
    let cols = Columns::new(rect.min, layout);
    let id = base.with(("layer", layer.id));

    if selected {
        ui.painter()
            .rect_filled(rect, 0.0, c.accent.gamma_multiply(0.3));
    } else if background.hovered() {
        ui.painter().rect_filled(rect, 0.0, c.hover);
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0_f32, c.divider),
    );
    if background.clicked() {
        result.select = true;
    }
    // A right-click anywhere on the visible part of the row, whichever cell
    // is under it (not on a row scrolled out under the buttons).
    let right = ui.input(|i| i.pointer.secondary_clicked()) && ui.rect_contains_pointer(rect);
    result.menu = right
        .then(|| ui.input(|i| i.pointer.interact_pos()))
        .flatten();

    // Layers with objects on them read stronger than empty ones.
    let dim = !layer.is_visible();
    let name_color = if dim || !in_use { c.text_weak } else { c.text };

    // Status: a tick on the current layer. Click to make a layer current.
    let status = ui.interact(cols.status, id.with("status"), Sense::click());
    if current {
        opendraft_ui::icon_set::paint(
            ui.painter(),
            cols.status.center(),
            Align2::CENTER_CENTER,
            icons::LAYER_PANEL_CURRENT,
            13.0,
            c.accent_text,
        );
    }
    if status.on_hover_text(t!("Current layer")).clicked() {
        result.edit = Some(LayerEdit::SetCurrent(layer.id));
    }

    // Name: double-click to rename.
    let editing_id = id.with("renaming");
    let editing = ui.data(|d| d.get_temp::<bool>(editing_id)).unwrap_or(false);
    if editing {
        let mut text = ui
            .data(|d| d.get_temp::<String>(editing_id.with("text")))
            .unwrap_or_else(|| layer.name.clone());
        let field = ui.put(
            cols.name.shrink2(egui::vec2(2.0, 1.0)),
            egui::TextEdit::singleline(&mut text).id(editing_id.with("edit")),
        );
        if !field.has_focus() && !field.lost_focus() {
            field.request_focus();
        }
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if field.lost_focus() {
            if !escape {
                result.edit = Some(LayerEdit::Rename(layer.id, text.clone()));
            }
            ui.data_mut(|d| d.insert_temp(editing_id, false));
        } else {
            ui.data_mut(|d| d.insert_temp(editing_id.with("text"), text));
        }
    } else {
        let cut = fit_text(ui, theme, cols.name, &layer.name, name_color);
        let mut name = ui.interact(cols.name, id.with("name"), Sense::click());
        if cut {
            name = name.on_hover_text(&layer.name);
        }
        if name.clicked() {
            result.select = true;
        }
        if name.double_clicked() && layer.id != LayerId::DEFAULT {
            ui.data_mut(|d| {
                d.insert_temp(editing_id, true);
                d.insert_temp(editing_id.with("text"), layer.name.clone());
            });
        }
    }

    // Switches: the ribbon list's bulb, sun and padlock, yellow while the
    // layer can be seen and edited, grey when not.
    let [on_glyph, thaw_glyph, lock_glyph] = crate::ribbon_slots::switch_glyphs(layer, c.text_weak);
    let switch = |ui: &mut Ui, cell: Rect, name: &str, glyph: (char, egui::Color32), tip: &str| {
        let response = ui.interact(cell, id.with(name), Sense::click());
        if response.hovered() {
            ui.painter().rect_stroke(
                cell.shrink(1.0),
                2.0,
                egui::Stroke::new(1.0_f32, c.border),
                egui::StrokeKind::Inside,
            );
        }
        // The VP Freeze switch is the sun or snowflake in a square.
        if name == "vp_freeze" {
            crate::ribbon_slots::paint_vp_switch(ui.painter(), cell.center(), 14.0, glyph);
        } else {
            let (icon, color) = glyph;
            opendraft_ui::icon_set::paint(
                ui.painter(),
                cell.center(),
                Align2::CENTER_CENTER,
                icon,
                14.0,
                color,
            );
        }
        response.on_hover_text(tip).clicked()
    };
    let on_tip = if layer.on {
        t!("On: click to turn off")
    } else {
        t!("Off: click to turn on")
    };
    if switch(ui, cols.on, "on", on_glyph, &on_tip) {
        result.edit = Some(LayerEdit::SetFlag(
            targets.clone(),
            LayerFlag::On,
            !layer.on,
        ));
    }
    let freeze_tip = if layer.frozen {
        t!("Frozen: click to thaw")
    } else {
        t!("Thawed: click to freeze")
    };
    if switch(ui, cols.freeze, "freeze", thaw_glyph, &freeze_tip) {
        result.edit = Some(LayerEdit::SetFlag(
            targets.clone(),
            LayerFlag::Frozen,
            !layer.frozen,
        ));
    }
    if let Some(vp) = vp {
        let glyph = crate::ribbon_slots::vp_glyph(vp.frozen, c.text_weak);
        if switch(ui, cols.vp_freeze, "vp_freeze", glyph, &vp.tip()) {
            result.edit = Some(LayerEdit::SetInViewport(targets.clone(), !vp.frozen));
        }
    }
    let lock_tip = if layer.locked {
        t!("Locked: click to unlock")
    } else {
        t!("Unlocked: click to lock")
    };
    if switch(ui, cols.lock, "lock", lock_glyph, &lock_tip) {
        result.edit = Some(LayerEdit::SetFlag(
            targets.clone(),
            LayerFlag::Locked,
            !layer.locked,
        ));
    }

    // Colour.
    let mut color_ui = ui.new_child(egui::UiBuilder::new().max_rect(cols.color));
    if let Some(Pick::Color(color)) =
        colour_ui::swatch_button(&mut color_ui, theme, id.with("color"), layer.color, false)
    {
        result.edit = Some(LayerEdit::SetColor(targets.clone(), color));
    }

    // Linetype and lineweight: flat text cells that open a list.
    if !layout.wide {
        return result;
    }
    let cell = ui.interact(cols.linetype, id.with("linetype"), Sense::click());
    let linetype_name = document
        .linetypes()
        .get(layer.linetype)
        .map_or("?", |r| r.name.as_str());
    cell_text(ui, theme, cols.linetype, linetype_name);
    let menu_id = id.with("linetype_menu");
    if let Some(linetype) = linetype_ui::pick(ui, theme, menu_id, &cell, document, layer.linetype) {
        result.edit = Some(LayerEdit::SetLinetype(targets.clone(), linetype));
    }

    let weights: Vec<String> = Lineweight::STANDARD
        .iter()
        .map(|w| weight_label(*w))
        .collect();
    let current_weight = Lineweight::STANDARD
        .iter()
        .position(|w| *w == layer.lineweight);
    let cell = ui.interact(cols.weight, id.with("weight"), Sense::click());
    cell_text(ui, theme, cols.weight, &weight_label(layer.lineweight));
    if let Some(i) = menu::list_menu(
        ui,
        theme,
        id.with("weight_menu"),
        &cell,
        &weights,
        current_weight,
    ) {
        result.edit = Some(LayerEdit::SetLineweight(targets, Lineweight::STANDARD[i]));
    }
    result
}

/// "0.25 mm".
pub fn weight_label(weight: Lineweight) -> String {
    format!("{:.2} mm", weight.mm())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_show_in_millimetres() {
        assert_eq!(weight_label(Lineweight(25)), "0.25 mm");
        assert_eq!(weight_label(Lineweight(211)), "2.11 mm");
    }
}
