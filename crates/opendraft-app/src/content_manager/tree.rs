//! The tree on the left: My blocks with its folders, and the other places.

use eframe::egui;
use opendraft_content::blocks_payload::folder;
use opendraft_content::blocks_payload::BlockUuid;
use opendraft_ui::Theme;

use opendraft_ui::icons;

use super::actions::Action;
use super::chrome;
use super::snapshot::Snapshot;
use super::{Editing, Place};

pub fn show(
    ui: &mut egui::Ui,
    theme: &Theme,
    place: &Place,
    snap: &Snapshot,
    actions: &mut Vec<Action>,
) {
    ui.spacing_mut().item_spacing.y = 2.0;
    reveal(ui, place);
    folder_node(ui, theme, place, snap, "", actions);
    ui.add_space(2.0);
    ui.separator();
    let rows = [
        (Place::Drawing, icons::CONTENT_DRAWING, "This drawing", None),
        (
            Place::Favourites,
            icons::CONTENT_FAVOURITE,
            "Favourites",
            Some(snap.favourites),
        ),
        (
            Place::Recent,
            icons::CONTENT_RECENT,
            "Recent",
            Some(snap.recents),
        ),
        (
            Place::Trash,
            icons::CONTENT_TRASH,
            "Trash",
            Some(snap.trash_total),
        ),
    ];
    for (target, icon, label, count) in rows {
        if chrome::nav_row(ui, theme, icon, label, count, *place == target).clicked() {
            actions.push(Action::Go(target));
        }
    }
}

/// The folders directly inside `path`.
fn children<'a>(folders: &'a [String], path: &str) -> Vec<&'a String> {
    folders
        .iter()
        .filter(|f| folder::same(folder::parent(f), path))
        .collect()
}

/// Opens the folders above the chosen one, once each time it changes, so the
/// row for it can be seen without taking away the user's own opening and closing.
fn reveal(ui: &egui::Ui, place: &Place) {
    let Place::Mine(path) = place else {
        return;
    };
    let seen = egui::Id::new("cm_revealed");
    let last: Option<String> = ui.data(|d| d.get_temp(seen));
    if last.as_deref() == Some(path.as_str()) {
        return;
    }
    ui.data_mut(|d| d.insert_temp(seen, path.clone()));
    let mut above = String::new();
    for part in path.split('/') {
        if !above.is_empty() || !path.is_empty() {
            let id = ui.make_persistent_id(("cm_folder", above.to_lowercase()));
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                true,
            );
            state.set_open(true);
            state.store(ui.ctx());
        }
        above = folder::join(&above, part);
    }
}

fn folder_node(
    ui: &mut egui::Ui,
    theme: &Theme,
    place: &Place,
    snap: &Snapshot,
    path: &str,
    actions: &mut Vec<Action>,
) {
    let label = if path.is_empty() {
        "My blocks"
    } else {
        folder::name(path)
    };
    let kids = children(&snap.folders, path);
    let id = ui.make_persistent_id(("cm_folder", path.to_lowercase()));
    let here = matches!(place, Place::Mine(p) if folder::same(p, path));
    let mut row = |ui: &mut egui::Ui| {
        let response = chrome::nav_row(ui, theme, icons::CONTENT_FOLDER, label, None, here);
        if response.clicked() {
            actions.push(Action::Go(Place::Mine(path.to_owned())));
        }
        drop_target(ui, theme, &response, path, actions);
        folder_menu(&response, path, actions);
    };
    if kids.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            row(ui);
        });
        return;
    }
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, path.is_empty())
        .show_header(ui, |ui| row(ui))
        .body(|ui| {
            for kid in kids {
                folder_node(ui, theme, place, snap, kid, actions);
            }
        });
}

/// A block dragged from the contents and dropped on a folder moves there.
fn drop_target(
    ui: &egui::Ui,
    theme: &Theme,
    response: &egui::Response,
    path: &str,
    actions: &mut Vec<Action>,
) {
    if response.dnd_hover_payload::<BlockUuid>().is_some() {
        ui.painter().rect_stroke(
            response.rect,
            3.0,
            egui::Stroke::new(1.5_f32, theme.colors.accent),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(dropped) = response.dnd_release_payload::<BlockUuid>() {
        actions.push(Action::Move(*dropped, path.to_owned()));
    }
}

fn folder_menu(response: &egui::Response, path: &str, actions: &mut Vec<Action>) {
    response.context_menu(|ui| {
        if ui.button("New folder").clicked() {
            actions.push(Action::Go(Place::Mine(path.to_owned())));
            actions.push(Action::Edit(Some(Editing::NewFolder {
                parent: path.to_owned(),
                text: String::new(),
            })));
            ui.close_menu();
        }
        if !path.is_empty() {
            if ui.button("Rename").clicked() {
                actions.push(Action::Edit(Some(Editing::RenameFolder {
                    path: path.to_owned(),
                    text: folder::name(path).to_owned(),
                })));
                ui.close_menu();
            }
            if ui.button("Delete (blocks go to the Trash)").clicked() {
                actions.push(Action::DeleteFolder(path.to_owned()));
                ui.close_menu();
            }
        }
    });
}
