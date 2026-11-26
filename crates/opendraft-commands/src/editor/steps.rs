//! What a command asks of the session beside editing: taking back its own
//! step, and hiding objects from the view.

use opendraft_core::EntityId;

use super::{Cancelled, Editor};

impl Editor {
    /// Undo the most recent step in the drawing's history. A command uses
    /// this only to take back a step it made itself (Line's Undo option).
    pub fn undo(&self) -> Result<(), Cancelled> {
        self.run(|session| {
            session.undo();
        })
    }

    /// Hide exactly these objects, or show everything with none (the
    /// isolation commands). Not an edit and not an undo step.
    pub fn set_hidden_objects(&self, ids: Vec<EntityId>) -> Result<(), Cancelled> {
        self.run(move |session| session.set_hidden_objects(ids))
    }
}
