//! Changing what the modify commands remember.

use super::{Cancelled, Editor};

impl Editor {
    /// Change what FILLET, CHAMFER, OFFSET and LENGTHEN remember. It is kept
    /// in the drawing but is not an undo step.
    pub fn change_modify_settings(
        &self,
        change: impl FnOnce(&mut opendraft_core::ModifySettings) + Send + 'static,
    ) -> Result<(), Cancelled> {
        self.run(move |session| session.change_modify_settings(change))
    }
}
