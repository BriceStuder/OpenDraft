//! Capture: a block of a drawing becomes a block of the library
//! (specification 10.4). The block and the blocks it contains are copied with
//! their layers, linetypes and text styles; the library's own blocks are never
//! changed by a capture except when the user chose to replace one.

use std::collections::{HashMap, HashSet};

use opendraft_core::block::block_name_problem;
use opendraft_core::{BlockClash, BlockDef, BlockId, Document};

use super::error::LibraryError;
use super::organise::redefine;
use super::trash::trash_one;
use super::{Edit, Library};
use crate::blocks_payload::folder;
use crate::blocks_payload::{BlockUuid, Entry, MAX_DESCRIPTION, MAX_TAG, MAX_TAGS};
use crate::envelope::text::{check_line, check_paragraphs};

/// What to do when the library already has a block of the name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NameTaken {
    /// Stop and say so.
    #[default]
    Cancel,
    /// Add the new one as `Name_2`.
    KeepBoth,
    /// Move the old one to the Trash and add the new one.
    Replace,
}

#[derive(Clone, Debug, Default)]
pub struct CaptureRequest {
    /// The library name; the block's own when `None`.
    pub name: Option<String>,
    /// The block's own when `None`.
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// Where it goes (`""` for the top level).
    pub folder: String,
    pub if_name_taken: NameTaken,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Captured {
    pub id: BlockUuid,
    pub name: String,
    /// An old block of the name went to the Trash.
    pub replaced: bool,
    /// What did not carry over exactly, worded for the user.
    pub warnings: Vec<String>,
}

/// What `add_from` is asked to do.
pub(super) struct Plan<'a> {
    pub name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub tags: &'a [String],
    pub folder: &'a str,
    pub if_taken: NameTaken,
    /// Ids to give the blocks (restoring from the Trash), by source block.
    pub uuids: Option<&'a HashMap<BlockId, BlockUuid>>,
}

impl Library {
    /// Copies block `block` of `source` (and what it contains) into the
    /// library. `now` is seconds since 1970, for the Trash when a block is
    /// replaced.
    pub fn capture(
        &mut self,
        source: &Document,
        block: BlockId,
        request: &CaptureRequest,
        now: u64,
    ) -> Result<Captured, LibraryError> {
        let plan = Plan {
            name: request.name.as_deref(),
            description: request.description.as_deref(),
            tags: &request.tags,
            folder: &request.folder,
            if_taken: request.if_name_taken,
            uuids: None,
        };
        self.edit(|edit| add_from(edit, source, block, &plan, now))
    }
}

pub(super) fn check_tags(tags: &[String]) -> Result<(), LibraryError> {
    let bad = |t: &str| LibraryError::InvalidName(format!("{t:?} is not a valid tag."));
    if tags.len() > MAX_TAGS {
        return Err(LibraryError::InvalidName(format!(
            "A block can have at most {MAX_TAGS} tags."
        )));
    }
    for tag in tags {
        if check_line(tag, MAX_TAG).is_err() || tag.trim() != tag {
            return Err(bad(tag));
        }
    }
    Ok(())
}

pub(super) fn check_name(name: &str) -> Result<(), LibraryError> {
    if let Some(problem) = block_name_problem(name) {
        return Err(LibraryError::InvalidName(problem.to_owned()));
    }
    check_line(name, 255).map_err(|e| LibraryError::InvalidName(format!("The block name {e}.")))
}

pub(super) fn check_description(text: &str) -> Result<(), LibraryError> {
    if text.is_empty() {
        return Ok(());
    }
    check_paragraphs(text, MAX_DESCRIPTION)
        .map_err(|e| LibraryError::InvalidName(format!("The description {e}.")))
}

pub(super) fn add_from(
    edit: &mut Edit,
    source: &Document,
    root: BlockId,
    plan: &Plan,
    now: u64,
) -> Result<Captured, LibraryError> {
    let def = source.block(root).ok_or(LibraryError::NotFound)?.clone();
    let mut name = plan.name.map_or(def.name.as_str(), str::trim).to_owned();
    check_name(&name)?;
    folder::check_path(plan.folder, folder::MAX_LIBRARY_DEPTH)?;
    check_tags(plan.tags)?;
    if let Some(text) = plan.description {
        check_description(text)?;
    }

    let mut replaced = false;
    if let Some(old) = edit.package.drawing.block_named(&name).cloned() {
        match plan.if_taken {
            NameTaken::Cancel => return Err(LibraryError::NameTaken(name)),
            NameTaken::KeepBoth => name = edit.package.drawing.unused_block_name_like(&name),
            NameTaken::Replace => {
                let entry = edit
                    .package
                    .entry_for_block(old.id)
                    .ok_or(LibraryError::NotFound)?
                    .uuid;
                trash_one(edit, entry, now)?;
                replaced = true;
            }
        }
    }

    // A block the library already has, of the same name and shape, is used
    // as it is; any other block of a name it has is added under a new one.
    let before: HashSet<BlockId> = edit.package.drawing.blocks().iter().map(|b| b.id).collect();
    let clashes = edit
        .package
        .drawing
        .block_clashes(source, root)
        .ok_or(LibraryError::NotFound)?;
    let mut choices = HashMap::new();
    for clash in clashes.iter().filter(|c| c.source != root) {
        let same = match (
            source.block(clash.source),
            edit.package.drawing.block(clash.existing),
        ) {
            (Some(a), Some(b)) => same_shape(a, b),
            _ => false,
        };
        if same {
            choices.insert(clash.source, BlockClash::UseExisting);
        }
    }
    let brought = edit
        .package
        .drawing
        .bring_block(source, root, &choices)
        .map_err(|e| LibraryError::Refused(e.to_string()))?;

    let wanted_name = (brought.block.name != name).then(|| name.clone());
    if wanted_name.is_some() || plan.description.is_some() {
        if let Some(other) = edit.package.drawing.block_named(&name) {
            if other.id != brought.block.id {
                return Err(LibraryError::NameTaken(name));
            }
        }
        redefine(&mut edit.package, brought.block.id, |d| {
            if let Some(n) = &wanted_name {
                d.name.clone_from(n);
            }
            if let Some(text) = plan.description {
                d.description = text.to_owned();
            }
        })?;
    }

    let mut id = None;
    for (from, made) in &brought.blocks {
        if before.contains(&made.id) {
            continue;
        }
        let is_root = *from == root;
        let uuid = plan
            .uuids
            .and_then(|m| m.get(from))
            .filter(|u| edit.package.entry(**u).is_none())
            .copied()
            .unwrap_or_default();
        if is_root {
            id = Some(uuid);
        }
        edit.package.entries.push(Entry {
            uuid,
            block: made.id,
            folder: if is_root {
                plan.folder.to_owned()
            } else {
                String::new()
            },
            tags: if is_root {
                plan.tags.to_vec()
            } else {
                Vec::new()
            },
            component: !is_root,
        });
    }
    edit.package_changed = true;
    Ok(Captured {
        id: id.ok_or(LibraryError::NotFound)?,
        name,
        replaced,
        warnings: brought.warnings.iter().map(ToString::to_string).collect(),
    })
}

/// Whether two blocks of one name hold the same thing: the same base point,
/// unit and objects (compared by shape, not by layer, which is each drawing's
/// own). A "no" is safe: the block is added under a new name.
fn same_shape(a: &BlockDef, b: &BlockDef) -> bool {
    let shape = |d: &BlockDef| {
        format!(
            "{:?}",
            d.entities.iter().map(|e| &e.entity).collect::<Vec<_>>()
        )
    };
    a.base == b.base
        && a.unit == b.unit
        && a.entities.len() == b.entities.len()
        && shape(a) == shape(b)
}
