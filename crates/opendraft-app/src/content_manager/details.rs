//! The details of the chosen block, below the contents: its facts, a
//! description and tags that can be edited, and what can be done with it.

use eframe::egui;
use opendraft_content::blocks_payload::BlockUuid;
use opendraft_content::library::BlockInfo;
use opendraft_ui::Theme;

use super::actions::{Action, Taken};
use super::snapshot::Snapshot;
use super::{Editing, Selected, Window};

/// The text being edited for the chosen block.
#[derive(Default)]
pub struct Buffers {
    id: Option<BlockUuid>,
    description: String,
    tags: String,
}

impl Buffers {
    /// Starts again from `block` when another one is chosen or its text was
    /// changed from elsewhere while the field was not being typed in.
    fn follow(&mut self, block: &BlockInfo, typing: bool) {
        if self.id != Some(block.id) {
            self.id = Some(block.id);
            self.description.clone_from(&block.description);
            self.tags = block.tags.join(", ");
        } else if !typing {
            if self.description != block.description {
                self.description.clone_from(&block.description);
            }
            let tags = block.tags.join(", ");
            if self.tags != tags {
                self.tags = tags;
            }
        }
    }
}

/// `"a, b ,c"` as tags.
fn parse_tags(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

impl Window {
    pub(super) fn details_pane(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        snap: &Snapshot,
        actions: &mut Vec<Action>,
    ) {
        match self.selected.clone() {
            Selected::Block(_) => match &snap.selected {
                Some(block) => self.block_details(ui, theme, block, actions),
                None => {
                    ui.weak("That block is no longer in the library.");
                }
            },
            Selected::Drawing(name) => {
                let known = snap.drawing.iter().find(|b| b.name == name);
                ui.strong(&name);
                if let Some(b) = known {
                    ui.label(format!(
                        "A block of this drawing, used {} time{}.",
                        b.references,
                        if b.references == 1 { "" } else { "s" }
                    ));
                    if b.in_library {
                        ui.weak("The library has a block of this name already.");
                    }
                }
                if ui.button("Add to library").clicked() {
                    actions.push(Action::AddFromDrawing(name, Taken::Cancel));
                }
            }
            Selected::Trashed(id) => {
                if let Some(item) = snap.trash.iter().find(|t| t.id == id) {
                    ui.strong(&item.name);
                    let from = if item.folder.is_empty() {
                        "My blocks"
                    } else {
                        &item.folder
                    };
                    ui.label(format!("Deleted from {from}. Kept for 30 days."));
                    if ui.button("Restore").clicked() {
                        actions.push(Action::Restore(id));
                    }
                }
            }
            Selected::Nothing => {
                ui.weak("Choose a block to see its details. Double-click a block to insert it.");
            }
        }
    }

    fn block_details(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        block: &BlockInfo,
        actions: &mut Vec<Action>,
    ) {
        let typing = ui.memory(|m| m.focused().is_some());
        self.details.follow(block, typing);
        ui.horizontal(|ui| {
            ui.strong(&block.name);
            ui.weak(format!(
                "{}{}{}",
                unit_text(block),
                if block.dynamic { ", dynamic" } else { "" },
                if block.has_attributes {
                    ", has attributes"
                } else {
                    ""
                }
            ));
        });
        if !block.components.is_empty() {
            ui.weak(format!("Contains: {}", block.components.join(", ")));
        }
        egui::Grid::new("cm_details_grid")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Description");
                let edit = ui.add(
                    egui::TextEdit::multiline(&mut self.details.description)
                        .desired_rows(2)
                        .desired_width(ui.available_width()),
                );
                if edit.lost_focus() && self.details.description != block.description {
                    actions.push(Action::SetDescription(
                        block.id,
                        self.details.description.clone(),
                    ));
                }
                ui.end_row();
                ui.label("Tags");
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.details.tags)
                        .hint_text("comma separated")
                        .desired_width(ui.available_width()),
                );
                if edit.lost_focus() && parse_tags(&self.details.tags) != block.tags {
                    actions.push(Action::SetTags(block.id, parse_tags(&self.details.tags)));
                }
                ui.end_row();
            });
        ui.horizontal(|ui| {
            if opendraft_ui::widgets::primary_button(ui, theme, "Insert").clicked() {
                actions.push(Action::Insert(block.id));
            }
            let star = if block.favourite {
                "\u{2605} Favourite"
            } else {
                "\u{2606} Favourite"
            };
            if ui.button(star).clicked() {
                actions.push(Action::Favourite(block.id, !block.favourite));
            }
            if ui.button("Rename").clicked() {
                actions.push(Action::Edit(Some(Editing::RenameBlock {
                    id: block.id,
                    text: block.name.clone(),
                })));
            }
            if ui.button("Delete").clicked() {
                actions.push(Action::Delete(block.id));
            }
        });
    }
}

fn unit_text(block: &BlockInfo) -> String {
    if block.unit == opendraft_core::InsertUnits::UNITLESS {
        "unitless".to_owned()
    } else {
        block.unit.label().to_owned()
    }
}
