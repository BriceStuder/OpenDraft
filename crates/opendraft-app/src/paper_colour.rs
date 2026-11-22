//! The colour of light paper, the user's choice of a few presets (Settings >
//! Display > Layout paper): white, as the sheet prints, or a grey or warm
//! paper that glares less in the dark interface.
//!
//! Presets, not any colour, so every paper offered is one whose marks
//! (`canvas_style.rs`) are tested against it: each is at least `FLOOR`. A
//! preset is saved by its name, so its exact colour may be tuned later.
//! Plots are on white whatever is chosen; this is only how the screen shows
//! the sheet. The Layout paper group of Settings > Display, a row of
//! swatches, is here too.

use eframe::egui::{self, Color32};
use opendraft_ui::{widgets, Theme};
use serde::{Deserialize, Serialize};

use crate::settings::{LayoutPaper, Settings};

/// The least relative luminance of light paper (about #D9D9D9 as a grey):
/// every preset is at least this, and the marks are chosen to be seen on it.
#[cfg(test)]
pub const FLOOR: f64 = 0.70;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperColour {
    #[default]
    White,
    OffWhite,
    /// As light as yellow lines are above it, cyan lines are below it.
    LightGrey,
    Grey,
    Cream,
    Sepia,
}

impl PaperColour {
    /// In the order the swatches are shown.
    pub const ALL: [Self; 6] = [
        Self::White,
        Self::OffWhite,
        Self::LightGrey,
        Self::Grey,
        Self::Cream,
        Self::Sepia,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::White => "White",
            Self::OffWhite => "Off-white",
            Self::LightGrey => "Light grey",
            Self::Grey => "Grey",
            Self::Cream => "Cream",
            Self::Sepia => "Sepia",
        }
    }

    pub const fn color32(self) -> Color32 {
        match self {
            Self::White => Color32::WHITE,
            Self::OffWhite => Color32::from_gray(0xF5),
            Self::LightGrey => Color32::from_gray(0xEE),
            Self::Grey => Color32::from_gray(0xE0),
            Self::Cream => Color32::from_rgb(0xF0, 0xEA, 0xD6),
            Self::Sepia => Color32::from_rgb(0xE8, 0xDC, 0xC0),
        }
    }
}

/// Settings > Display's Layout paper group: light or dark, and light
/// paper's colour.
pub fn settings_group(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    widgets::group_box(ui, theme, "Layout paper", |ui| {
        ui.horizontal(|ui| {
            let paper = &mut draft.layout_paper;
            ui.radio_value(paper, LayoutPaper::Light, "Light, as printed");
            ui.radio_value(paper, LayoutPaper::Dark, "Dark, as model space");
        });
        let light = draft.layout_paper == LayoutPaper::Light;
        ui.add_enabled_ui(light, |ui| paper_colour(ui, theme, draft));
    });
}

/// The colour of light paper: a row of swatches, one per preset, the chosen
/// one ringed with the accent and named after the row.
fn paper_colour(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    let colour = &mut draft.layout_paper_colour;
    ui.horizontal(|ui| {
        ui.label("Colour");
        for paper in PaperColour::ALL {
            let chosen = *colour == paper;
            if swatch(ui, theme, paper.color32(), chosen)
                .on_hover_text(paper.label())
                .clicked()
            {
                *colour = paper;
            }
        }
        ui.label(egui::RichText::new(colour.label()).color(theme.colors.text_weak));
    });
}

/// A square of `fill` to click, ringed with the accent when `chosen`.
fn swatch(ui: &mut egui::Ui, theme: &Theme, fill: Color32, chosen: bool) -> egui::Response {
    let side = theme.unit * 3.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let c = &theme.colors;
        let enabled = ui.is_enabled();
        let fade = |colour: Color32| {
            if enabled {
                colour
            } else {
                colour.gamma_multiply(0.4)
            }
        };
        let painter = ui.painter();
        painter.rect_filled(rect.shrink(3.0), 1.0, fade(fill));
        let (width, ring) = if chosen {
            (2.0_f32, c.accent)
        } else if response.hovered() {
            (1.0, c.border_strong)
        } else {
            (1.0, c.border)
        };
        painter.rect_stroke(
            rect.shrink(1.0),
            2.0,
            egui::Stroke::new(width, fade(ring)),
            egui::StrokeKind::Middle,
        );
    }
    response
}

/// WCAG relative luminance of a colour.
#[cfg(test)]
pub fn luminance(c: Color32) -> f64 {
    let channel = |v: u8| {
        let c = f64::from(v) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_is_the_default_and_every_paper_is_light() {
        assert_eq!(PaperColour::default(), PaperColour::White);
        assert_eq!(PaperColour::LightGrey.color32(), Color32::from_gray(0xEE));
        for paper in PaperColour::ALL {
            let l = luminance(paper.color32());
            assert!(l >= FLOOR, "{paper:?}: {l:.3}");
        }
    }

    #[test]
    fn saved_by_name() {
        let text = |p: PaperColour| serde_json::to_string(&p).unwrap();
        assert_eq!(text(PaperColour::White), r#""white""#);
        assert_eq!(text(PaperColour::OffWhite), r#""off_white""#);
        assert_eq!(text(PaperColour::LightGrey), r#""light_grey""#);
        assert_eq!(text(PaperColour::Sepia), r#""sepia""#);
    }
}
