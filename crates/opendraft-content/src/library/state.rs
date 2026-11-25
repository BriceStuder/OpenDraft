//! `library.json`: only what the user decided (specification 10.3). Losing it
//! costs the folders that hold no block, the favourites, the recents and the
//! Trash's index, and nothing else.

use serde::{Deserialize, Serialize};

use crate::blocks_payload::BlockUuid;

/// The most recently inserted blocks that are remembered.
pub const MAX_RECENTS: usize = 20;
/// Days a deleted block stays in the Trash.
pub const TRASH_DAYS: u64 = 30;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub version: u32,
    /// Folders the user made, so one can exist with nothing in it. A folder
    /// holding a block exists without being listed.
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub favourites: Vec<BlockUuid>,
    /// Most recent first.
    #[serde(default)]
    pub recents: Vec<BlockUuid>,
    #[serde(default)]
    pub trash: Vec<TrashRecord>,
    #[serde(default)]
    pub next_trash: u64,
}

/// A deleted block in `trash/<id>.odpack`. The date and the folder it was in
/// are kept here, not in the file (specification 5.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashRecord {
    pub id: u64,
    pub uuid: BlockUuid,
    pub name: String,
    pub folder: String,
    /// Seconds since 1970.
    pub deleted: u64,
}

impl State {
    pub fn add_folder(&mut self, path: &str) {
        for folder in crate::blocks_payload::folder::with_ancestors(path) {
            if !self
                .folders
                .iter()
                .any(|f| crate::blocks_payload::folder::same(f, &folder))
            {
                self.folders.push(folder);
            }
        }
    }

    pub fn forget(&mut self, id: BlockUuid) {
        self.favourites.retain(|f| *f != id);
        self.recents.retain(|f| *f != id);
    }
}
