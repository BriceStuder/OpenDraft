//! Showing the Content Manager window and starting what it asks for.

use std::sync::Arc;

use eframe::egui;

use super::OpenDraftApp;
use crate::content_manager::{Env, Request};
use crate::library_add_cmd::library_add;
use crate::library_insert_cmd::library_insert;

impl OpenDraftApp {
    /// The Content Manager, while it is open. It is modeless: the drawing
    /// stays usable beside it, and a block it inserts is placed by LIBINSERT
    /// on the command line.
    pub(in crate::app) fn content_manager_window(&mut self, ctx: &egui::Context) {
        if !self.state.command_dialogs.content.open {
            return;
        }
        let mut window = std::mem::take(&mut self.state.command_dialogs.content);
        let defaults = Arc::clone(self.state.defaults());
        let requests = {
            let session = self.state.session();
            let env = Env {
                library: &defaults.library,
                document: session.document(),
                revision: session.revision(),
            };
            window.show(ctx, &self.theme, &env)
        };
        self.state.command_dialogs.content = window;
        for request in requests {
            match request {
                Request::Insert(id) => {
                    defaults.library.queue_pick(id);
                    self.state.start_command("LIBINSERT", library_insert);
                }
                Request::AddObjects => self.state.start_command("LIBADD", library_add),
            }
        }
    }
}
