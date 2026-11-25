//! The Trash (specification 10.4): a deleted block, folder or replaced block
//! goes to `trash/` as a small package, never into a drawing, and stays for 30
//! days or until the Trash is emptied. Restoring puts it back.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use opendraft_core::{BlockDef, BlockId, Document};

use super::capture::{add_from, Captured, NameTaken, Plan};
use super::error::LibraryError;
use super::state::{TrashRecord, TRASH_DAYS};
use super::{store, Edit, Library};
use crate::blocks_payload::folder;
use crate::blocks_payload::{BlockUuid, BlocksPackage, Entry};

const DAY: u64 = 24 * 60 * 60;

/// The names of the blocks that contain `block`, other than those in `skip`.
fn parents_of(package: &BlocksPackage, block: BlockId, skip: &HashSet<BlockId>) -> Vec<String> {
    package
        .drawing
        .blocks()
        .iter()
        .filter(|b| !skip.contains(&b.id) && b.nested_blocks().any(|n| n == block))
        .map(|b| b.name.clone())
        .collect()
}

/// The blocks that go with `root`: itself, and each block below it that is a
/// component and that nothing else, outside the set, contains.
fn removable(package: &BlocksPackage, root: BlockId) -> HashSet<BlockId> {
    let mut below = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if let Some(def) = package.drawing.block(id) {
            for n in def.nested_blocks() {
                if seen.insert(n) {
                    below.push(n);
                    stack.push(n);
                }
            }
        }
    }
    let mut removed = HashSet::from([root]);
    loop {
        let more: Vec<BlockId> = below
            .iter()
            .copied()
            .filter(|b| !removed.contains(b))
            .filter(|b| package.entry_for_block(*b).is_some_and(|e| e.component))
            .filter(|b| parents_of(package, *b, &removed).is_empty())
            .collect();
        if more.is_empty() {
            return removed;
        }
        removed.extend(more);
    }
}

/// Moves a block, and the components only it uses, to the Trash.
pub(super) fn trash_one(
    edit: &mut Edit,
    id: BlockUuid,
    now: u64,
) -> Result<TrashRecord, LibraryError> {
    let entry = edit
        .package
        .entry(id)
        .cloned()
        .ok_or(LibraryError::NotFound)?;
    let parents = parents_of(&edit.package, entry.block, &HashSet::new());
    if !parents.is_empty() {
        return Err(LibraryError::InUse(parents));
    }
    let name = edit
        .package
        .drawing
        .block(entry.block)
        .map(|b| b.name.clone())
        .ok_or(LibraryError::NotFound)?;
    let removed = removable(&edit.package, entry.block);

    // The small package: the block with everything it contains, with the ids
    // it had, so a restore gives them back.
    let mut doc = Document::default();
    let brought = doc
        .bring_block(&edit.package.drawing, entry.block, &HashMap::new())
        .map_err(|e| LibraryError::Refused(e.to_string()))?;
    let entries = brought
        .blocks
        .iter()
        .filter_map(|(from, to)| {
            let e = edit.package.entry_for_block(*from)?;
            let is_root = *from == entry.block;
            Some(Entry {
                uuid: e.uuid,
                block: to.id,
                folder: if is_root {
                    e.folder.clone()
                } else {
                    String::new()
                },
                tags: e.tags.clone(),
                component: !is_root,
            })
        })
        .collect();
    let small = BlocksPackage {
        drawing: doc,
        entries,
    };
    let trash_id = edit.state.next_trash;
    edit.state.next_trash += 1;
    let path = store::save_trash_file(&edit.root, trash_id, &small)?;
    edit.created.push(path);

    let kept: Vec<Arc<BlockDef>> = edit
        .package
        .drawing
        .blocks()
        .iter()
        .filter(|b| !removed.contains(&b.id))
        .cloned()
        .collect();
    edit.package.drawing.set_blocks(kept);
    let gone: Vec<BlockUuid> = edit
        .package
        .entries
        .iter()
        .filter(|e| removed.contains(&e.block))
        .map(|e| e.uuid)
        .collect();
    edit.package.entries.retain(|e| !removed.contains(&e.block));
    for uuid in gone {
        edit.state.forget(uuid);
    }
    let record = TrashRecord {
        id: trash_id,
        uuid: id,
        name,
        folder: entry.folder,
        deleted: now,
    };
    edit.state.trash.push(record.clone());
    edit.package_changed = true;
    edit.state_changed = true;
    Ok(record)
}

impl Library {
    /// Moves a block to the Trash. Refused while another block contains it.
    pub fn delete_block(&mut self, id: BlockUuid, now: u64) -> Result<(), LibraryError> {
        self.edit(|e| trash_one(e, id, now).map(|_| ()))
    }

    /// Moves every block in a folder, and in the folders inside it, to the
    /// Trash and removes the folders.
    pub fn delete_folder(&mut self, path: &str, now: u64) -> Result<(), LibraryError> {
        if path.is_empty() || !self.folders().iter().any(|f| folder::same(f, path)) {
            return Err(LibraryError::NotFound);
        }
        self.edit(|e| {
            let mut pending: Vec<BlockUuid> = e
                .package
                .entries
                .iter()
                .filter(|x| !x.component && folder::is_within(&x.folder, path))
                .map(|x| x.uuid)
                .collect();
            // A block that holds another goes first, so the one inside can
            // follow; a block something outside the folder holds stays put
            // and stops the delete.
            while !pending.is_empty() {
                let next = pending.iter().position(|id| {
                    e.package.entry(*id).is_some_and(|x| {
                        parents_of(&e.package, x.block, &HashSet::new()).is_empty()
                    })
                });
                let Some(at) = next else {
                    let id = pending[0];
                    let parents = e
                        .package
                        .entry(id)
                        .map(|x| parents_of(&e.package, x.block, &HashSet::new()))
                        .unwrap_or_default();
                    return Err(LibraryError::InUse(parents));
                };
                let id = pending.remove(at);
                if e.package.entry(id).is_some() {
                    trash_one(e, id, now)?;
                }
            }
            e.state.folders.retain(|f| !folder::is_within(f, path));
            e.state_changed = true;
            Ok(())
        })
    }

    /// Puts a Trash item back where it was, with its id.
    pub fn restore(
        &mut self,
        trash: u64,
        if_taken: NameTaken,
        now: u64,
    ) -> Result<Captured, LibraryError> {
        self.edit(|e| {
            let record = e
                .state
                .trash
                .iter()
                .find(|t| t.id == trash)
                .cloned()
                .ok_or(LibraryError::NotFound)?;
            let small = store::read_trash_file(&e.root, trash)?;
            let root = small
                .entry(record.uuid)
                .cloned()
                .ok_or_else(|| LibraryError::Corrupt(format!("Trash item {trash} has no block")))?;
            let uuids: HashMap<BlockId, BlockUuid> =
                small.entries.iter().map(|x| (x.block, x.uuid)).collect();
            let plan = Plan {
                name: None,
                description: None,
                tags: &root.tags,
                folder: &record.folder,
                if_taken,
                uuids: Some(&uuids),
            };
            let captured = add_from(e, &small.drawing, root.block, &plan, now)?;
            e.state.add_folder(&record.folder);
            e.state.trash.retain(|t| t.id != trash);
            e.removed.push(store::trash_file(&e.root, trash));
            e.state_changed = true;
            Ok(captured)
        })
    }

    /// Removes items deleted more than 30 days before `now`.
    pub fn purge_trash(&mut self, now: u64) -> Result<usize, LibraryError> {
        self.remove_from_trash(|t| now.saturating_sub(t.deleted) > TRASH_DAYS * DAY)
    }

    pub fn empty_trash(&mut self) -> Result<usize, LibraryError> {
        self.remove_from_trash(|_| true)
    }

    fn remove_from_trash(
        &mut self,
        gone: impl Fn(&TrashRecord) -> bool,
    ) -> Result<usize, LibraryError> {
        self.edit(|e| {
            let ids: Vec<u64> = e
                .state
                .trash
                .iter()
                .filter(|t| gone(t))
                .map(|t| t.id)
                .collect();
            if ids.is_empty() {
                return Ok(0);
            }
            e.state.trash.retain(|t| !ids.contains(&t.id));
            e.removed
                .extend(ids.iter().map(|id| store::trash_file(&e.root, *id)));
            e.state_changed = true;
            Ok(ids.len())
        })
    }
}
