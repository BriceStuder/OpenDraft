//! The Select Color dialog: every AutoCAD index colour, laid out like
//! AutoCAD's Index Color tab, with a field that takes a name, a number or
//! R,G,B.

use eframe::egui::{self, Color32, Id, Rect, Sense, Stroke, Ui};
use opendraft_core::aci::aci_rgb;
use opendraft_ui::{widgets, Theme};

use super::names::{self, from_index, index_of};
use super::{display, swatch, Pick};

const CELL: f32 = 17.0;
const GAP: f32 = 2.0;
const HUES: u8 = 24;
const WIDTH: f32 = HUES as f32 * (CELL + GAP) + 32.0;
/// The shades of each hue, top row first: the full colours from light to
/// dark, then the pale ones from dark to light.
const SHADE_ROWS: [u8; 10] = [0, 2, 4, 6, 8, 9, 7, 5, 3, 1];

/// What the dialog holds while it is open.
#[derive(Clone, Default)]
pub struct State {
    pub chosen: Option<Pick>,
    pub text: String,
}

impl State {
    pub fn new(current: Option<Pick>) -> Self {
        Self {
            chosen: current,
            text: current.map(label).unwrap_or_default(),
        }
    }
}

fn label(pick: Pick) -> String {
    match pick {
        Pick::ByLayer => "ByLayer".to_owned(),
        Pick::ByBlock => "ByBlock".to_owned(),
        Pick::Color(color) => names::name(color),
    }
}

/// Shows the dialog kept under `id`; returns the colour once the user
/// presses OK (or double-clicks a colour). Closes itself on OK and Cancel.
pub fn show(ui: &Ui, theme: &Theme, id: Id, by_layer: bool) -> Option<Pick> {
    let mut state = ui.data(|d| d.get_temp::<State>(id))?;
    let mut open = true;
    let mut ok = false;
    let mut cancel = false;
    widgets::dialog(ui.ctx(), theme, "Select Color", &mut open, WIDTH, |ui| {
        let mut hovered = None;
        let mut double = false;
        let mut cell = |ui: &mut Ui, i: u8, state: &mut State| {
            let response = index_cell(ui, theme, i, state.chosen);
            if response.hovered() {
                hovered = Some(i);
            }
            if response.clicked() || response.double_clicked() {
                let pick = Pick::Color(from_index(i));
                state.chosen = Some(pick);
                state.text = label(pick);
                double |= response.double_clicked();
            }
        };
        widgets::field_label(ui, theme, "Index colors");
        egui::Grid::new(id.with("grid"))
            .spacing(egui::vec2(GAP, GAP))
            .show(ui, |ui| {
                for shade in SHADE_ROWS {
                    for hue in 0..HUES {
                        cell(ui, 10 + hue * 10 + shade, &mut state);
                    }
                    ui.end_row();
                }
            });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for i in 1..=9 {
                cell(ui, i, &mut state);
            }
            ui.add_space(24.0);
            for i in 250..=255 {
                cell(ui, i, &mut state);
            }
        });
        ui.add_space(6.0);
        ui.label(describe(hovered.or_else(|| chosen_index(&state))));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            widgets::field_label(ui, theme, "Color:");
            let typed = ui.add(egui::TextEdit::singleline(&mut state.text).desired_width(140.0));
            if typed.changed() {
                state.chosen = names::parse(&state.text, by_layer);
            }
            if typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                ok = state.chosen.is_some();
            }
            let (rect, _) = ui.allocate_exact_size(egui::vec2(32.0, 20.0), Sense::hover());
            match state.chosen {
                Some(Pick::Color(color)) => swatch(ui, theme, rect, display(theme, color)),
                _ => swatch(ui, theme, rect, Color32::TRANSPARENT),
            }
            if by_layer {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for pick in [Pick::ByBlock, Pick::ByLayer] {
                        if widgets::secondary_button(ui, theme, &label(pick)).clicked() {
                            state.chosen = Some(pick);
                            state.text = label(pick);
                        }
                    }
                });
            }
        });
        if state.chosen.is_none() && !state.text.trim().is_empty() {
            ui.colored_label(
                ui.visuals().error_fg_color,
                "Type a color name, a number from 1 to 255, or R,G,B.",
            );
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = widgets::secondary_button(ui, theme, "Cancel").clicked();
                ui.add_enabled_ui(state.chosen.is_some(), |ui| {
                    ok |= widgets::primary_button(ui, theme, "OK").clicked();
                });
            });
        });
        ok |= double;
    });
    let picked = state.chosen.filter(|_| ok);
    ui.data_mut(|d| {
        if ok || cancel || !open {
            d.remove::<State>(id);
        } else {
            d.insert_temp(id, state);
        }
    });
    picked
}

fn chosen_index(state: &State) -> Option<u8> {
    match state.chosen {
        Some(Pick::Color(color)) => index_of(color),
        _ => None,
    }
}

/// "Index color: 34   Red, Green, Blue: 255,170,0".
fn describe(index: Option<u8>) -> String {
    match index {
        Some(7) => "Index color: 7   White or black, whichever shows on the background".to_owned(),
        Some(i) => {
            let (r, g, b) = aci_rgb(i);
            format!("Index color: {i}   Red, Green, Blue: {r},{g},{b}")
        }
        None => " ".to_owned(),
    }
}

/// One square of the grid; colour 7 is half white, half black.
fn index_cell(ui: &mut Ui, theme: &Theme, i: u8, chosen: Option<Pick>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(CELL, CELL), Sense::click());
    if i == 7 {
        half_and_half(ui, rect);
    } else {
        let (r, g, b) = aci_rgb(i);
        ui.painter()
            .rect_filled(rect, 1.0, Color32::from_rgb(r, g, b));
    }
    let selected = matches!(chosen, Some(Pick::Color(color)) if index_of(color) == Some(i));
    if selected || response.hovered() {
        ui.painter().rect_stroke(
            rect,
            1.0,
            Stroke::new(if selected { 2.0_f32 } else { 1.5 }, theme.colors.text),
            egui::StrokeKind::Inside,
        );
    }
    response
}

/// The swatch of colour 7: white above the diagonal, black below.
pub fn half_and_half(ui: &Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 1.0, Color32::WHITE);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![rect.right_top(), rect.right_bottom(), rect.left_bottom()],
        Color32::BLACK,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_holds_every_shade_once() {
        let mut seen: Vec<u8> = SHADE_ROWS
            .iter()
            .flat_map(|shade| (0..HUES).map(move |hue| 10 + hue * 10 + shade))
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (10..=249).collect::<Vec<u8>>());
    }

    #[test]
    fn the_description_gives_the_rgb() {
        assert_eq!(
            describe(Some(1)),
            "Index color: 1   Red, Green, Blue: 255,0,0"
        );
        assert!(describe(Some(7)).contains("White or black"));
    }

    #[test]
    fn a_new_dialog_shows_the_current_colour() {
        let state = State::new(Some(Pick::Color(from_index(34))));
        assert_eq!(state.text, "34");
        assert_eq!(State::new(Some(Pick::ByLayer)).text, "ByLayer");
        assert_eq!(State::new(None).text, "");
    }
}
