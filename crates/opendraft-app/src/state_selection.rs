//! The selection of the active drawing, as the state hands it out.

use std::collections::BTreeSet;

use opendraft_core::EntityId;

use crate::state::AppState;

impl AppState {
    pub fn selection(&self) -> &BTreeSet<EntityId> {
        &self.docs[self.active].selection
    }

    /// Select every selectable object (not hidden, frozen or locked).
    pub fn select_all(&mut self) {
        let doc = &mut self.docs[self.active];
        doc.selection = doc
            .session
            .document()
            .selectable_objects()
            .map(|o| o.id)
            .collect();
    }

    /// Add `ids` to the selection, or remove them when `remove` is set.
    pub fn select_ids(&mut self, ids: impl IntoIterator<Item = EntityId>, remove: bool) {
        let selection = &mut self.docs[self.active].selection;
        for i in ids {
            if remove {
                selection.remove(&i);
            } else {
                selection.insert(i);
            }
        }
    }
}
