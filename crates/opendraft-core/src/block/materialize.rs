//! A drawing with its dynamic blocks made static, for a format that cannot
//! keep their parameters and actions (DXF).
//!
//! Every reference of a dynamic block is pointed at an ordinary block that
//! holds the objects it draws now, the values applied. A reference with no
//! values shares the definition's own picture, kept under the definition's
//! name; each other state of a block gets a block of its own, named
//! `<name>_1`, `<name>_2` and so on. Blocks nested in blocks are made static
//! innermost first. Nothing in the original drawing changes.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::block::{evaluate, BlockDef, BlockEntity, BlockId, DynamicInstanceState};
use crate::{Document, Entity, Insert};

/// A drawing made static, and what that cost.
#[derive(Clone, Debug)]
pub struct Materialized {
    pub document: Document,
    /// References in the drawing to a dynamic block.
    pub references: usize,
    /// Dynamic blocks in the block table.
    pub definitions: usize,
}

struct Work<'a> {
    src: &'a Document,
    /// The drawing being built: it allocates the ids of new blocks.
    out: Document,
    needs: HashMap<BlockId, bool>,
    made: HashMap<(BlockId, String), Arc<BlockDef>>,
    extra: Vec<Arc<BlockDef>>,
    names: HashSet<String>,
    variants: HashMap<BlockId, usize>,
}

impl Work<'_> {
    /// Whether `block` is dynamic, or holds an insert of one, at any depth.
    fn needs(&mut self, block: &BlockDef) -> bool {
        if let Some(known) = self.needs.get(&block.id) {
            return *known;
        }
        let nested: Vec<Arc<BlockDef>> = block
            .entities
            .iter()
            .filter_map(|e| match &e.entity {
                Entity::Insert(i) => Some(i.block.clone()),
                _ => None,
            })
            .collect();
        let needs = block.is_dynamic() || nested.iter().any(|b| self.needs(b));
        self.needs.insert(block.id, needs);
        needs
    }

    /// A name for another state of `block`, not used by any block.
    fn variant_name(&mut self, block: &BlockDef) -> String {
        let count = self.variants.entry(block.id).or_insert(0);
        loop {
            *count += 1;
            let name = format!("{}_{}", block.name, count);
            if self.names.insert(name.to_lowercase()) {
                return name;
            }
        }
    }

    /// The static block `block` is with `state` applied.
    fn make(
        &mut self,
        block: &Arc<BlockDef>,
        state: Option<&DynamicInstanceState>,
    ) -> Arc<BlockDef> {
        let state = state.filter(|s| !s.is_default());
        if !self.needs(block) {
            return block.clone();
        }
        let key = (block.id, format!("{state:?}"));
        if let Some(done) = self.made.get(&key) {
            return done.clone();
        }
        let evaluated =
            evaluate(block, state).map_or_else(|_| block.entities.clone(), |e| e.into_owned());
        let entities = evaluated
            .into_iter()
            .map(|e| match e.entity {
                Entity::Insert(nested) => {
                    let inner = self.make(&nested.block, nested.dynamic.as_deref());
                    let insert = Insert {
                        block: inner,
                        dynamic: None,
                        ..*nested
                    };
                    BlockEntity::new(e.id, Entity::Insert(Box::new(insert)), e.attrs)
                }
                _ => e,
            })
            .collect();
        let (id, name) = match state {
            None => (block.id, block.name.clone()),
            Some(_) => (self.out.allocate_block_id(), self.variant_name(block)),
        };
        let def = Arc::new(BlockDef {
            id,
            name,
            entities,
            dynamic: None,
            ..(**block).clone()
        });
        if state.is_some() {
            self.extra.push(def.clone());
        }
        self.made.insert(key, def.clone());
        def
    }
}

impl Document {
    /// This drawing with every dynamic block made static (see the module
    /// notes), or `None` when it has none. The drawing itself is unchanged.
    pub fn materialized_static(&self) -> Option<Materialized> {
        let mut work = Work {
            src: self,
            out: self.clone(),
            needs: HashMap::new(),
            made: HashMap::new(),
            extra: Vec::new(),
            names: self
                .blocks()
                .iter()
                .map(|b| b.name.to_lowercase())
                .collect(),
            variants: HashMap::new(),
        };
        let blocks = work.src.blocks().to_vec();
        if !blocks.iter().any(|b| work.needs(b)) {
            return None;
        }
        let mut table: Vec<Arc<BlockDef>> = blocks.iter().map(|b| work.make(b, None)).collect();
        let mut references = 0;
        let mut replaced = Vec::new();
        for object in work.src.objects() {
            let Entity::Insert(i) = &object.entity else {
                continue;
            };
            if i.block.is_dynamic() {
                references += 1;
            }
            if work.needs(&i.block) {
                let block = work.make(&i.block, i.dynamic.as_deref());
                let insert = Insert {
                    block,
                    dynamic: None,
                    ..(**i).clone()
                };
                replaced.push((object.id, Entity::Insert(Box::new(insert))));
            }
        }
        table.append(&mut work.extra);
        let definitions = blocks.iter().filter(|b| b.is_dynamic()).count();
        let mut out = work.out;
        out.set_blocks(table);
        for (id, entity) in replaced {
            out.replace(id, entity);
        }
        Some(Materialized {
            document: out,
            references,
            definitions,
        })
    }
}
