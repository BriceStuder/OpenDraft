//! The Load or Reload Linetypes window: where the definitions on offer come
//! from (the stock library, or a `.lin` file the user picks), which of them
//! are ticked, and what loading does to the draft table. A definition the
//! table has with another pattern is offered as a replacement, as AutoCAD's
//! Reload does; one it already has exactly is shown as loaded.

use eframe::egui::{self, Sense};
use opendraft_core::linetype::{lin, stock, Loaded};
use opendraft_core::StrokeElement;
use opendraft_ui::widgets;

use super::{paint_row, paint_sample, Choice, Draft, LIST_HEIGHT, LOAD_WIDTH, ROW_HEIGHT};
use crate::app::OpenDraftApp;
use crate::state::AppState;
use crate::text::plural;

/// The kind of file the Load window's File button asks for.
pub(in crate::app) const LIN_FILTER: crate::picker::Filter = ("Linetype files", &["lin"]);

/// One definition on offer.
struct Offer {
    name: String,
    description: String,
    elements: Vec<StrokeElement>,
}

/// What the Load window is offering.
pub(in crate::app) struct LoadState {
    /// "Stock library", or the file's name.
    source: String,
    offers: Vec<Offer>,
    ticked: Vec<bool>,
    /// Entries the file held that could not be read, one line each.
    unreadable: Vec<String>,
    /// How many offers had text or shape elements that were left out.
    dashes_only: usize,
}

impl LoadState {
    /// The stock library.
    pub(in crate::app) fn stock() -> Self {
        let offers: Vec<Offer> = stock::LIBRARY
            .iter()
            .map(|s| Offer {
                name: s.name.to_owned(),
                description: s.description.to_owned(),
                elements: s
                    .pattern
                    .iter()
                    .filter_map(|v| StrokeElement::from_dxf(*v))
                    .collect(),
            })
            .collect();
        Self {
            source: "Stock library".to_owned(),
            ticked: vec![false; offers.len()],
            offers,
            unreadable: Vec::new(),
            dashes_only: 0,
        }
    }

    /// The definitions in the bytes of a `.lin` file called `name`.
    pub(in crate::app) fn from_file(name: &str, bytes: &[u8]) -> Self {
        let file = lin::parse_bytes(bytes);
        let unreadable = file
            .notes
            .iter()
            .filter(|n| n.skipped)
            .map(|n| match &n.name {
                Some(entry) => format!("{entry} (line {}): {}", n.line, n.message),
                None => format!("line {}: {}", n.line, n.message),
            })
            .collect();
        let dashes_only = file.entries.iter().filter(|e| e.complex).count();
        let offers: Vec<Offer> = file
            .entries
            .into_iter()
            .map(|e| Offer {
                name: e.name,
                description: e.description,
                elements: e.elements,
            })
            .collect();
        Self {
            source: name.to_owned(),
            ticked: vec![false; offers.len()],
            offers,
            unreadable,
            dashes_only,
        }
    }

    /// Load what is ticked into `draft`. Returns the status line to show.
    pub(in crate::app) fn apply(self, draft: &mut Draft) -> String {
        let (mut added, mut replaced) = (0, 0);
        for (offer, _) in self.offers.into_iter().zip(self.ticked).filter(|(_, t)| *t) {
            match draft
                .table
                .load_definition(&offer.name, &offer.description, offer.elements)
            {
                Ok(Loaded::Added(_)) => added += 1,
                Ok(Loaded::Replaced(_)) => replaced += 1,
                Ok(Loaded::Unchanged(_)) | Err(_) => {}
            }
        }
        let mut line = format!("Loaded {}", plural(added, "linetype"));
        if replaced > 0 {
            line.push_str(&format!(", replaced {replaced}"));
        }
        line
    }
}

/// Ask for a `.lin` file and offer what it holds; says why on the status bar
/// when it cannot be read.
pub(super) fn choose_lin_file(state: &mut AppState, draft: &mut Draft) {
    let Some(path) = state
        .picker
        .open_file("Select Linetype File", None, LIN_FILTER)
    else {
        return;
    };
    let name = path.file_name().map_or_else(
        || "linetype file".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    match std::fs::read(&path) {
        Ok(bytes) => draft.load = Some(LoadState::from_file(&name, &bytes)),
        Err(error) => state.message = format!("Could not read {name}: {error}"),
    }
}

impl OpenDraftApp {
    /// The window itself; returns what the user chose, if anything.
    pub(super) fn load_window(&mut self, ctx: &egui::Context, draft: &mut Draft) -> Option<Choice> {
        let (mut open, mut choice) = (true, None);
        let theme = &self.theme;
        let Draft { table, load, .. } = draft;
        let Some(state) = load.as_mut() else {
            return Some(Choice::LoadCancel);
        };
        widgets::dialog(
            ctx,
            theme,
            "Load or Reload Linetypes",
            &mut open,
            LOAD_WIDTH,
            |ui| {
                ui.horizontal(|ui| {
                    widgets::field_label(ui, theme, "Source:");
                    ui.label(&state.source);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::secondary_button(ui, theme, "File...").clicked() {
                            choice = Some(Choice::LoadFile);
                        }
                        if widgets::secondary_button(ui, theme, "Stock library").clicked() {
                            choice = Some(Choice::LoadStock);
                        }
                    });
                });
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .max_height(LIST_HEIGHT + 40.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if state.offers.is_empty() {
                            ui.label("There are no linetypes to load in this file.");
                        }
                        for (offer, ticked) in state.offers.iter().zip(&mut state.ticked) {
                            let exists = table.named(&offer.name).is_some();
                            let changes = table.would_change(&offer.name, &offer.elements);
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), ROW_HEIGHT),
                                Sense::click(),
                            );
                            if response.clicked() && changes {
                                *ticked = !*ticked;
                            }
                            if response.hovered() {
                                ui.painter().rect_filled(rect, 2.0, theme.colors.hover);
                            }
                            let tick = if *ticked { "\u{2611}" } else { "\u{2610}" };
                            let mark = match (changes, exists) {
                                (false, _) => "loaded".to_owned(),
                                (true, true) => format!("{tick} replaces"),
                                (true, false) => tick.to_owned(),
                            };
                            paint_row(
                                ui,
                                theme,
                                rect,
                                &offer.name,
                                &offer.description,
                                Some(&mark),
                            );
                            paint_sample(ui, theme, rect, &offer.elements);
                        }
                    });
                notes(ui, theme, state);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, theme, "OK").clicked() {
                        choice = Some(Choice::LoadOk);
                    }
                    if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                        choice = Some(Choice::LoadCancel);
                    }
                });
            },
        );
        if !open {
            choice = Some(Choice::LoadCancel);
        }
        choice
    }
}

/// What the file held that is not on offer as it was written.
fn notes(ui: &mut egui::Ui, theme: &opendraft_ui::Theme, state: &LoadState) {
    if !state.unreadable.is_empty() {
        ui.add_space(4.0);
        let count = plural(state.unreadable.len(), "definition");
        ui.label(
            egui::RichText::new(format!("{count} in this file could not be read."))
                .size(theme.font_size_small)
                .color(theme.colors.text_weak),
        )
        .on_hover_text(state.unreadable.join("\n"));
    }
    if state.dashes_only > 0 {
        ui.add_space(4.0);
        let count = plural(state.dashes_only, "linetype");
        ui.label(
            egui::RichText::new(format!(
                "{count} with text or shapes will be drawn as plain dashes."
            ))
            .size(theme.font_size_small)
            .color(theme.colors.text_weak),
        );
    }
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
