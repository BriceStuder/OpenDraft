//! Importing blocks from a drawing file: the list of the file's blocks to tick,
//! the folder they go to, and what to do with a name the library has.

use eframe::egui;
use opendraft_content::blocks_payload::folder;
use opendraft_content::library::{importable, Candidate, NameTaken};
use opendraft_core::Document;

use super::actions::Action;
use super::{Env, Place, Window};
use crate::content_library::now;

/// What to do with a block whose name the library has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImportTaken {
    /// Leave it out.
    #[default]
    Skip,
    KeepBoth,
    Replace,
}

impl ImportTaken {
    fn policy(self) -> NameTaken {
        match self {
            ImportTaken::Skip => NameTaken::Cancel,
            ImportTaken::KeepBoth => NameTaken::KeepBoth,
            ImportTaken::Replace => NameTaken::Replace,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ImportTaken::Skip => "Leave it out",
            ImportTaken::KeepBoth => "Add under a new name",
            ImportTaken::Replace => "Replace it (the old one goes to the Trash)",
        }
    }
}

/// A file read, and what the user has ticked of it.
pub struct ImportDraft {
    file: String,
    document: Document,
    candidates: Vec<(Candidate, bool)>,
    /// What the reader could not carry over.
    warnings: Vec<String>,
    folder: String,
    taken: ImportTaken,
}

impl ImportDraft {
    #[cfg(test)]
    pub(super) fn untick_all(&mut self) {
        self.candidates.iter_mut().for_each(|(_, on)| *on = false);
    }
}

impl Window {
    /// Starts an import of the blocks of `document`, read from `file`. Says so
    /// in the status line instead when it has none to offer.
    pub fn start_import(&mut self, file: &str, document: Document, warnings: Vec<String>) {
        let candidates: Vec<(Candidate, bool)> = importable(&document)
            .into_iter()
            .map(|c| (c, true))
            .collect();
        if candidates.is_empty() {
            self.status = format!("{file} has no blocks to import.");
            return;
        }
        let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
        let folder = folder::check_segment(stem)
            .map_or_else(|_| "Imported".to_owned(), |()| stem.to_owned());
        self.import = Some(ImportDraft {
            file: file.to_owned(),
            document,
            candidates,
            warnings,
            folder,
            taken: ImportTaken::default(),
        });
        self.open = true;
    }

    /// The import panel, in place of the contents.
    pub(super) fn import_panel(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let Some(draft) = &mut self.import else {
            return;
        };
        ui.strong(format!("Import blocks from {}", draft.file));
        ui.horizontal(|ui| {
            if ui.small_button("Select all").clicked() {
                draft.candidates.iter_mut().for_each(|(_, on)| *on = true);
            }
            if ui.small_button("Select none").clicked() {
                draft.candidates.iter_mut().for_each(|(_, on)| *on = false);
            }
            let ticked = draft.candidates.iter().filter(|(_, on)| *on).count();
            ui.weak(format!("{ticked} of {} ticked", draft.candidates.len()));
        });
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for (candidate, on) in &mut draft.candidates {
                    let holds = match candidate.contains {
                        0 => String::new(),
                        n => format!(", holds {n} more"),
                    };
                    ui.checkbox(
                        on,
                        format!("{} ({} objects{holds})", candidate.name, candidate.objects),
                    );
                }
            });
        ui.separator();
        let problem = folder::check_path(
            draft.folder.trim().trim_matches('/'),
            folder::MAX_LIBRARY_DEPTH,
        )
        .err();
        ui.horizontal(|ui| {
            ui.label("Into folder");
            ui.add(egui::TextEdit::singleline(&mut draft.folder).desired_width(220.0));
            if let Some(problem) = &problem {
                ui.colored_label(egui::Color32::LIGHT_RED, problem.to_string());
            }
        });
        ui.horizontal(|ui| {
            ui.label("A name the library has");
            egui::ComboBox::from_id_salt("cm_import_taken")
                .selected_text(draft.taken.label())
                .show_ui(ui, |ui| {
                    for choice in [
                        ImportTaken::Skip,
                        ImportTaken::KeepBoth,
                        ImportTaken::Replace,
                    ] {
                        ui.selectable_value(&mut draft.taken, choice, choice.label());
                    }
                });
        });
        if !draft.warnings.is_empty() {
            ui.collapsing(
                format!("{} notes from reading the file", draft.warnings.len()),
                |ui| {
                    for w in &draft.warnings {
                        ui.weak(w);
                    }
                },
            );
        }
        let ticked = draft.candidates.iter().any(|(_, on)| *on);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(ticked && problem.is_none(), egui::Button::new("Import"))
                .clicked()
            {
                actions.push(Action::RunImport);
            }
            if ui.button("Cancel").clicked() {
                actions.push(Action::CancelImport);
            }
        });
    }

    /// Imports what is ticked. On a failure the draft stays, to try again.
    pub(super) fn run_import(&mut self, env: &Env) {
        let Some(draft) = self.import.take() else {
            return;
        };
        let ids: Vec<_> = draft
            .candidates
            .iter()
            .filter(|(_, on)| *on)
            .map(|(c, _)| c.id)
            .collect();
        if ids.is_empty() {
            self.status = "Tick the blocks to import.".to_owned();
            self.import = Some(draft);
            return;
        }
        let folder = draft.folder.trim().trim_matches('/').to_owned();
        let result = env
            .library
            .with(|lib| lib.import(&draft.document, &ids, &folder, draft.taken.policy(), now()));
        match result {
            Ok(Ok(report)) => {
                let mut said = format!(
                    "Imported {} into {}.",
                    crate::text::plural(report.added.len(), "block"),
                    if folder.is_empty() {
                        "My blocks"
                    } else {
                        &folder
                    }
                );
                if !report.skipped.is_empty() {
                    let skipped: Vec<String> = report
                        .skipped
                        .iter()
                        .map(|(name, why)| format!("{name} ({why})"))
                        .collect();
                    said.push_str(&format!(" Not imported: {}.", skipped.join(", ")));
                }
                self.status = said;
                self.go(Place::Mine(folder));
            }
            Ok(Err(error)) | Err(error) => {
                self.status = error.to_string();
                self.import = Some(draft);
            }
        }
    }
}
