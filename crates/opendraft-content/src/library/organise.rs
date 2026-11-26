//! Organising: rename, describe, tag, move, folders, favourites, recents
//! (specification 10.4). A block's id never changes.

use opendraft_core::{BlockDef, BlockId};

use super::capture::{check_description, check_name, check_tags};
use super::error::LibraryError;
use super::state::MAX_RECENTS;
use super::Library;
use crate::blocks_payload::folder;
use crate::blocks_payload::{BlockUuid, BlocksPackage};

/// Rebuilds block `block` with `change` applied, and every block that
/// contains it, so all of them point at the new definition.
pub(super) fn redefine(
    package: &mut BlocksPackage,
    block: BlockId,
    change: impl FnOnce(&mut BlockDef),
) -> Result<(), LibraryError> {
    let mut def = (**package.drawing.block(block).ok_or(LibraryError::NotFound)?).clone();
    change(&mut def);
    let redefinition = package
        .drawing
        .redefined_blocks(def)
        .map_err(|m| LibraryError::Refused(m.to_owned()))?;
    package.drawing.set_blocks(redefinition.blocks);
    Ok(())
}

impl Library {
    /// Gives a block a new name. Its id, folder and tags stay.
    pub fn rename_block(&mut self, id: BlockUuid, name: &str) -> Result<(), LibraryError> {
        let name = name.trim().to_owned();
        check_name(&name)?;
        self.edit(|e| {
            let block = e.package.entry(id).ok_or(LibraryError::NotFound)?.block;
            if let Some(other) = e.package.drawing.block_named(&name) {
                if other.id != block {
                    return Err(LibraryError::NameTaken(name));
                }
            }
            redefine(&mut e.package, block, |d| d.name.clone_from(&name))?;
            e.package_changed = true;
            Ok(())
        })
    }

    pub fn set_description(&mut self, id: BlockUuid, text: &str) -> Result<(), LibraryError> {
        check_description(text)?;
        self.edit(|e| {
            let block = e.package.entry(id).ok_or(LibraryError::NotFound)?.block;
            redefine(&mut e.package, block, |d| d.description = text.to_owned())?;
            e.package_changed = true;
            Ok(())
        })
    }

    pub fn set_tags(&mut self, id: BlockUuid, tags: Vec<String>) -> Result<(), LibraryError> {
        check_tags(&tags)?;
        self.edit(|e| {
            let entry = e
                .package
                .entries
                .iter_mut()
                .find(|x| x.uuid == id)
                .ok_or(LibraryError::NotFound)?;
            entry.tags = tags;
            e.package_changed = true;
            Ok(())
        })
    }

    /// Puts a block in `folder` (`""` for the top level), making the folder if
    /// it is new. A component moves with its parent and cannot be moved alone.
    pub fn move_block(&mut self, id: BlockUuid, to: &str) -> Result<(), LibraryError> {
        folder::check_path(to, folder::MAX_LIBRARY_DEPTH)?;
        self.edit(|e| {
            let entry = e
                .package
                .entries
                .iter_mut()
                .find(|x| x.uuid == id)
                .ok_or(LibraryError::NotFound)?;
            if entry.component {
                return Err(LibraryError::Refused(
                    "A part of another block cannot be moved on its own.".into(),
                ));
            }
            entry.folder = to.to_owned();
            e.state.add_folder(to);
            e.package_changed = true;
            e.state_changed = true;
            Ok(())
        })
    }

    /// Makes a folder (and the ones above it that are missing).
    pub fn create_folder(&mut self, path: &str) -> Result<(), LibraryError> {
        if path.is_empty() {
            return Err(LibraryError::Folder(folder::FolderError::BadName(
                String::new(),
            )));
        }
        folder::check_path(path, folder::MAX_LIBRARY_DEPTH)?;
        self.edit(|e| {
            e.state.add_folder(path);
            e.state_changed = true;
            Ok(())
        })
    }

    /// Renames or moves a folder, with everything in it: `to` is its new full
    /// path. Refused when `to` is inside the folder itself, or when a folder
    /// of that name is there already.
    pub fn rename_folder(&mut self, from: &str, to: &str) -> Result<(), LibraryError> {
        if from.is_empty() {
            return Err(LibraryError::Folder(folder::FolderError::BadName(
                String::new(),
            )));
        }
        folder::check_path(to, folder::MAX_LIBRARY_DEPTH)?;
        if to.is_empty() || (folder::is_within(to, from) && !folder::same(to, from)) {
            return Err(LibraryError::Refused(
                "A folder cannot go inside itself.".into(),
            ));
        }
        let before = self.folders();
        if !before.iter().any(|f| folder::same(f, from)) {
            return Err(LibraryError::NotFound);
        }
        if !folder::same(from, to) && before.iter().any(|f| folder::same(f, to)) {
            return Err(LibraryError::Refused(format!(
                "There is already a folder {to}."
            )));
        }
        self.edit(|e| {
            for entry in e.package.entries.iter_mut().filter(|x| !x.component) {
                if let Some(path) = folder::reroot(&entry.folder, from, to) {
                    entry.folder = path;
                }
            }
            let moved: Vec<String> = e
                .state
                .folders
                .iter()
                .filter_map(|f| folder::reroot(f, from, to))
                .collect();
            e.state.folders.retain(|f| !folder::is_within(f, from));
            e.state.folders.extend(moved);
            e.state.add_folder(to);
            for entry in e.package.entries.iter().filter(|x| !x.component) {
                folder::check_path(&entry.folder, folder::MAX_LIBRARY_DEPTH)?;
            }
            e.package_changed = true;
            e.state_changed = true;
            Ok(())
        })
    }

    pub fn set_favourite(&mut self, id: BlockUuid, favourite: bool) -> Result<(), LibraryError> {
        if self.package.entry(id).is_none() {
            return Err(LibraryError::NotFound);
        }
        self.edit(|e| {
            e.state.favourites.retain(|f| *f != id);
            if favourite {
                e.state.favourites.push(id);
            }
            e.state_changed = true;
            Ok(())
        })
    }

    /// Notes that a block was just inserted: it goes to the front of Recent.
    pub fn touch_recent(&mut self, id: BlockUuid) -> Result<(), LibraryError> {
        if self.package.entry(id).is_none() {
            return Err(LibraryError::NotFound);
        }
        self.edit(|e| {
            e.state.recents.retain(|r| *r != id);
            e.state.recents.insert(0, id);
            e.state.recents.truncate(MAX_RECENTS);
            e.state_changed = true;
            Ok(())
        })
    }
}
