//! Reading the library: what the window lists.

use std::collections::HashSet;

use opendraft_core::{BlockId, Entity, InsertUnits};

use super::{Library, TrashRecord};
use crate::blocks_payload::folder;
use crate::blocks_payload::{BlockUuid, Entry};

/// One block as the window shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockInfo {
    pub id: BlockUuid,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    /// `""` for the top level of My blocks.
    pub folder: String,
    pub unit: InsertUnits,
    pub dynamic: bool,
    pub has_attributes: bool,
    pub favourite: bool,
    /// Names of the blocks it contains, at any depth, each once.
    pub components: Vec<String>,
}

impl Library {
    /// Every block the user can see (components appear only under their
    /// parent), in folder then name order.
    pub fn blocks(&self) -> Vec<BlockInfo> {
        let mut blocks: Vec<BlockInfo> = self
            .package
            .entries
            .iter()
            .filter(|e| !e.component)
            .filter_map(|e| self.info(e))
            .collect();
        blocks.sort_by_cached_key(|b| (b.folder.to_lowercase(), b.name.to_lowercase()));
        blocks
    }

    pub fn block(&self, id: BlockUuid) -> Option<BlockInfo> {
        self.package.entry(id).and_then(|e| self.info(e))
    }

    /// The blocks directly in `folder` (`""` for the top level).
    pub fn blocks_in(&self, folder: &str) -> Vec<BlockInfo> {
        let mut blocks = self.blocks();
        blocks.retain(|b| folder::same(&b.folder, folder));
        blocks
    }

    /// Every folder: the ones the user made and the ones holding a block,
    /// with their ancestors, in name order, each as first written.
    pub fn folders(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut folders = Vec::new();
        let held = self
            .package
            .entries
            .iter()
            .filter(|e| !e.component)
            .map(|e| e.folder.as_str());
        for path in self.state.folders.iter().map(String::as_str).chain(held) {
            for f in folder::with_ancestors(path) {
                if seen.insert(f.to_lowercase()) {
                    folders.push(f);
                }
            }
        }
        folders.sort_by_cached_key(|f| f.to_lowercase());
        folders
    }

    /// The folders directly inside `folder`, as full paths.
    pub fn subfolders(&self, folder: &str) -> Vec<String> {
        let mut folders = self.folders();
        folders.retain(|f| folder::same(folder::parent(f), folder));
        folders
    }

    pub fn favourites(&self) -> Vec<BlockInfo> {
        self.state
            .favourites
            .iter()
            .filter_map(|id| self.block(*id))
            .collect()
    }

    /// The blocks last inserted, most recent first.
    pub fn recents(&self) -> Vec<BlockInfo> {
        self.state
            .recents
            .iter()
            .filter_map(|id| self.block(*id))
            .collect()
    }

    /// What is in the Trash, newest first.
    pub fn trash_items(&self) -> Vec<TrashRecord> {
        let mut items = self.state.trash.clone();
        items.sort_by(|a, b| b.deleted.cmp(&a.deleted).then(b.id.cmp(&a.id)));
        items
    }

    pub(super) fn info(&self, entry: &Entry) -> Option<BlockInfo> {
        let def = self.package.drawing.block(entry.block)?;
        let mut components = Vec::new();
        let mut seen = HashSet::new();
        self.collect_components(entry.block, &mut seen, &mut components);
        Some(BlockInfo {
            id: entry.uuid,
            name: def.name.clone(),
            description: def.description.clone(),
            tags: entry.tags.clone(),
            folder: entry.folder.clone(),
            unit: def.unit,
            dynamic: def.is_dynamic(),
            has_attributes: def
                .entities
                .iter()
                .any(|e| matches!(e.entity, Entity::AttributeDefinition(_))),
            favourite: self.state.favourites.contains(&entry.uuid),
            components,
        })
    }

    fn collect_components(
        &self,
        block: BlockId,
        seen: &mut HashSet<BlockId>,
        out: &mut Vec<String>,
    ) {
        let Some(def) = self.package.drawing.block(block) else {
            return;
        };
        for inner in def.nested_blocks() {
            if seen.insert(inner) {
                if let Some(d) = self.package.drawing.block(inner) {
                    out.push(d.name.clone());
                }
                self.collect_components(inner, seen, out);
            }
        }
    }
}
