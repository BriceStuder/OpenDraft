//! A block's name at the top of its details, with a pencil beside it to
//! rename the block there: the name turns into a field, Enter or OK keeps
//! the new name, Escape or Cancel leaves it.

use eframe::egui;
use opendraft_ui::{icons, Theme};

use super::chrome::icon_button;

/// What the name line asks for this frame.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum NameEdit {
    Nothing,
    /// The pencil was clicked.
    Start,
    Accept(String),
    Cancel,
}

/// The name as a heading, then the pencil and `facts` (unit, dynamic...);
/// or, while `typing` holds the name being typed, a field in its place.
pub(super) fn name_line(
    ui: &mut egui::Ui,
    theme: &Theme,
    name: &str,
    facts: &str,
    typing: Option<&mut String>,
) -> NameEdit {
    let mut asked = NameEdit::Nothing;
    ui.horizontal(|ui| {
        let Some(text) = typing else {
            ui.label(
                egui::RichText::new(name)
                    .size(theme.font_size_heading)
                    .color(theme.colors.text),
            );
            if icon_button(ui, theme, icons::CONTENT_RENAME, false, true, "Rename").clicked() {
                asked = NameEdit::Start;
            }
            ui.weak(facts);
            return;
        };
        let field = ui.add(
            egui::TextEdit::singleline(text)
                .font(egui::FontId::proportional(theme.font_size_heading))
                .desired_width(220.0),
        );
        field.request_focus();
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("OK").clicked() || enter {
            asked = NameEdit::Accept(text.trim().to_owned());
        }
        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            asked = NameEdit::Cancel;
        }
    });
    asked
}
