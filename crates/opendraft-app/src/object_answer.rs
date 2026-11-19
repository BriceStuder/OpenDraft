//! Canvas answers to a command waiting for an object: a click on one, and,
//! at a prompt that takes a sweep (TRIM, EXTEND), a click in empty space or
//! a path dragged from there.

use opendraft_commands::{Reply, Wants};
use opendraft_core::{EntityId, Point};

use crate::state::AppState;

impl AppState {
    /// A click on the canvas while a command waits for an object: the object
    /// under it, if there is one; or, at a sweep prompt, a click on nothing.
    pub fn answer_object(&mut self, at: Point, picked: Option<EntityId>, shift: bool) {
        let Some(ask) = self.waiting() else { return };
        if ask.wants == Wants::Object {
            match picked {
                Some(id) => {
                    self.docs[self.active].view.set_last_point(at);
                    self.reply(Reply::Object { id, at, shift });
                }
                None if ask.sweep => self.reply(Reply::Missed { at, shift }),
                None => {}
            }
        }
    }

    /// A path dragged from empty space while a command waits for an object
    /// and takes a sweep.
    pub fn answer_path(&mut self, points: Vec<Point>, shift: bool) {
        let Some(ask) = self.waiting() else { return };
        if ask.wants == Wants::Object && ask.sweep && points.len() >= 2 {
            self.reply(Reply::Path { points, shift });
        }
    }
}
