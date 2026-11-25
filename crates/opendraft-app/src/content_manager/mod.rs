//! The Content Manager window: the user's block library beside the drawing.
//! A tree on the left (My blocks and its folders, This drawing, Favourites,
//! Recent, Trash), the blocks of the chosen place as thumbnails or a list, and
//! the chosen block's details below. It is modeless: it stays open while the
//! user draws. The state of the library is `opendraft_content::library`; this
//! module only shows it and turns clicks into actions on it.
//!
//! Each frame the window takes a [`snapshot::Snapshot`] of what it shows, draws
//! from that, and collects [`actions::Action`]s, which are carried out after
//! drawing, so no drawing code changes the library halfway through a frame.

use eframe::egui;
use opendraft_content::blocks_payload::BlockUuid;
use opendraft_core::Document;
use opendraft_ui::Theme;

use crate::content_library::LibraryHandle;

mod actions;
mod contents;
mod details;
mod snapshot;
mod thumbnails;
mod tree;

use actions::Action;
use snapshot::Snapshot;

/// What the window is showing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// My blocks, in this folder (`""` for the top).
    Mine(String),
    Drawing,
    Favourites,
    Recent,
    Trash,
}

impl Default for Place {
    fn default() -> Self {
        Place::Mine(String::new())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Grid,
    List,
}

/// What is chosen in the contents pane.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Selected {
    #[default]
    Nothing,
    Block(BlockUuid),
    /// A block of the open drawing, by name.
    Drawing(String),
    Trashed(u64),
}

/// A name being typed.
#[derive(Clone, Debug)]
pub enum Editing {
    NewFolder { parent: String, text: String },
    RenameFolder { path: String, text: String },
    RenameBlock { id: BlockUuid, text: String },
}

/// What the window asks of the app, which owns the commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Start LIBINSERT on this block.
    Insert(BlockUuid),
    /// Start LIBADD (on the selected objects, if any).
    AddObjects,
}

/// What the window reads of the app.
pub struct Env<'a> {
    pub library: &'a LibraryHandle,
    pub document: &'a Document,
    /// Changes whenever the drawing does, so pictures of its blocks are redrawn.
    pub revision: u64,
}

#[derive(Default)]
pub struct Window {
    pub open: bool,
    place: Place,
    back: Vec<Place>,
    forward: Vec<Place>,
    search: String,
    view: View,
    selected: Selected,
    editing: Option<Editing>,
    status: String,
    /// A block of the drawing whose name the library already has, waiting
    /// for the user to say what to do.
    taken: Option<String>,
    thumbs: thumbnails::Thumbnails,
    details: details::Buffers,
}

impl Window {
    /// Opens the window on `place`.
    pub fn open_on(&mut self, place: Place) {
        self.open = true;
        self.go(place);
    }

    fn go(&mut self, place: Place) {
        if self.place != place {
            let from = std::mem::replace(&mut self.place, place);
            self.back.push(from);
            self.forward.clear();
        }
        self.selected = Selected::Nothing;
        self.editing = None;
    }

    /// Draws the window if it is open and says what the user asked the app to do.
    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme, env: &Env) -> Vec<Request> {
        if !self.open {
            return Vec::new();
        }
        let snapshot = Snapshot::take(self, env);
        self.thumbs.keep_current(snapshot.stamp, env.revision);
        let mut actions = Vec::new();
        let mut open = true;
        let c = &theme.colors;
        let frame = egui::Frame::new()
            .fill(c.panel_bg)
            .stroke(egui::Stroke::new(1.0_f32, c.border))
            .corner_radius(egui::CornerRadius::same(theme.radius + 3))
            .shadow(ctx.style().visuals.window_shadow);
        egui::Window::new("Content Manager")
            .id(egui::Id::new("content_manager_window"))
            .open(&mut open)
            .frame(frame)
            .default_size([840.0, 560.0])
            .min_size([560.0, 340.0])
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| self.ui(ui, theme, env, &snapshot, &mut actions));
        self.open = open;
        self.apply(actions, env)
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        env: &Env,
        snap: &Snapshot,
        actions: &mut Vec<Action>,
    ) {
        egui::TopBottomPanel::top("cm_toolbar").show_inside(ui, |ui| {
            self.toolbar(ui, theme, actions);
        });
        egui::TopBottomPanel::bottom("cm_status").show_inside(ui, |ui| {
            self.status_bar(ui, snap, actions);
        });
        egui::SidePanel::left("cm_tree")
            .default_width(190.0)
            .width_range(130.0..=360.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    tree::show(ui, theme, &self.place, snap, actions);
                });
            });
        egui::TopBottomPanel::bottom("cm_details")
            .default_height(130.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                self.details_pane(ui, theme, snap, actions);
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.contents_pane(ui, theme, env, snap, actions);
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, theme: &Theme, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            let back = !self.back.is_empty();
            let forward = !self.forward.is_empty();
            if ui
                .add_enabled(back, egui::Button::new("\u{25C0}"))
                .on_hover_text("Back")
                .clicked()
            {
                actions.push(Action::Back);
            }
            if ui
                .add_enabled(forward, egui::Button::new("\u{25B6}"))
                .on_hover_text("Forward")
                .clicked()
            {
                actions.push(Action::Forward);
            }
            let up = matches!(&self.place, Place::Mine(p) if !p.is_empty());
            if ui
                .add_enabled(up, egui::Button::new("\u{25B2}"))
                .on_hover_text("Up one level")
                .clicked()
            {
                actions.push(Action::Up);
            }
            ui.add_space(6.0);
            ui.allocate_ui(egui::vec2(240.0, 24.0), |ui| {
                opendraft_ui::widgets::text_field(ui, theme, &mut self.search, "Search blocks");
            });
            ui.add_space(6.0);
            ui.selectable_value(&mut self.view, View::Grid, "Thumbnails");
            ui.selectable_value(&mut self.view, View::List, "List");
            ui.add_space(6.0);
            if ui
                .button("Add to library")
                .on_hover_text("Add the selected objects, or a block of this drawing (LIBADD)")
                .clicked()
            {
                actions.push(Action::AddObjects);
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, snap: &Snapshot, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            if let Some(name) = self.taken.clone() {
                ui.label(format!("The library already has \"{name}\":"));
                if ui.button("Keep both").clicked() {
                    actions.push(Action::AddFromDrawing(
                        name.clone(),
                        actions::Taken::KeepBoth,
                    ));
                }
                if ui.button("Replace it").clicked() {
                    actions.push(Action::AddFromDrawing(
                        name.clone(),
                        actions::Taken::Replace,
                    ));
                }
                if ui.button("Cancel").clicked() {
                    actions.push(Action::DropOffer);
                }
            } else if self.status.is_empty() {
                ui.weak(snap.count_text(self));
            } else {
                ui.label(&self.status);
            }
        });
    }
}

#[cfg(test)]
mod tests;
