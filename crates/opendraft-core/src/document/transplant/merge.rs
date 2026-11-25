//! The matching of one drawing's tables to another's while absorbing.

use std::collections::HashMap;
use std::sync::Arc;

use super::super::{after, Document};
use super::library::BlockClash;
use super::TransferWarning;
use crate::block::{BlockDef, BlockEntity, BlockId};
use crate::dimension::DimStyle;
use crate::layer::{Attr, Attributes, Layer, LayerId};
use crate::linetype::LinetypeId;
use crate::mleader::MLeaderContent;
use crate::text_style::TextStyleId;
use crate::Entity;

/// The matching of one drawing's tables to another's while absorbing.
pub(super) struct Merge<'a> {
    pub(super) dst: &'a mut Document,
    pub(super) src: &'a Document,
    pub(super) layers: HashMap<LayerId, LayerId>,
    pub(super) text_styles: HashMap<TextStyleId, TextStyleId>,
    /// The source's linetype ids to this drawing's: built as the objects are
    /// copied, and total (an id never maps to nothing), so a transfer cannot
    /// fail halfway. An id is only ever meaningful inside its own drawing.
    pub(super) linetypes: HashMap<LinetypeId, LinetypeId>,
    pub(super) warnings: Vec<TransferWarning>,
    pub(super) blocks: HashMap<BlockId, Arc<BlockDef>>,
    /// `None` matches blocks by name, as a paste does. `Some` is the caller's
    /// choice for each source block whose name the target has (see
    /// `transplant/library.rs`); a block with no choice is renamed.
    pub(super) choices: Option<&'a HashMap<BlockId, BlockClash>>,
}

impl Merge<'_> {
    pub(super) fn layer(&mut self, id: LayerId) -> LayerId {
        if let Some(done) = self.layers.get(&id) {
            return *done;
        }
        let found = match self.src.layer(id) {
            None => self.dst.current_layer,
            Some(layer) => match self.dst.layers.iter().find(|l| layer_match(l, layer)) {
                Some(same) => same.id,
                None => {
                    let mut copy = layer.clone();
                    copy.id = self.dst.allocate_layer_id();
                    copy.linetype = self.linetype(copy.linetype);
                    let id = copy.id;
                    self.dst.push_layer(copy);
                    id
                }
            },
        };
        self.layers.insert(id, found);
        found
    }

    /// The record of this drawing that stands for the source's linetype `id`.
    /// The three special records are the same in every drawing. Otherwise the
    /// drawing's record of that name is used (and a different pattern is
    /// reported: the drawing's own wins), or the source's record is added.
    pub(super) fn linetype(&mut self, id: LinetypeId) -> LinetypeId {
        if let Some(done) = self.linetypes.get(&id) {
            return *done;
        }
        let found = if id.is_special() {
            id
        } else if let Some(record) = self.src.linetypes.get(id) {
            match self.dst.linetypes.named(&record.name) {
                Some(same) => {
                    if same.elements != record.elements {
                        self.warnings
                            .push(TransferWarning::LinetypeDiffers(record.name.clone()));
                    }
                    same.id
                }
                None => {
                    let mut copy = record.clone();
                    copy.id = LinetypeId(self.dst.linetypes.next_id());
                    let new = copy.id;
                    if self.dst.linetypes.push(copy).is_ok() {
                        new
                    } else {
                        self.warnings
                            .push(TransferWarning::LinetypeRefused(record.name.clone()));
                        LinetypeId::CONTINUOUS
                    }
                }
            }
        } else {
            self.warnings.push(TransferWarning::LinetypeMissing(id.0));
            LinetypeId::CONTINUOUS
        };
        self.linetypes.insert(id, found);
        found
    }

    pub(super) fn text_style(&mut self, id: TextStyleId) -> TextStyleId {
        if let Some(done) = self.text_styles.get(&id) {
            return *done;
        }
        let found = match self.src.text_style(id) {
            None => TextStyleId::STANDARD,
            Some(style) => match self.dst.text_style_named(&style.name) {
                Some(same) => same.id,
                None => {
                    let mut copy = style.clone();
                    copy.id = self.dst.allocate_text_style_id();
                    let id = copy.id;
                    self.dst.push_text_style(copy);
                    id
                }
            },
        };
        self.text_styles.insert(id, found);
        found
    }

    pub(super) fn attrs(&mut self, attrs: &Attributes) -> Attributes {
        let mut out = attrs.clone();
        out.layer = self.layer(attrs.layer);
        if let Attr::Set(id) = &attrs.linetype {
            out.linetype = Attr::Set(self.linetype(*id));
        }
        out
    }

    pub(super) fn dim_style(&mut self, style: &DimStyle) -> DimStyle {
        let mut out = style.clone();
        out.text_style = self.text_style(style.text_style);
        if self.dst.dim_style_named(&out.name).is_none() {
            self.dst.dim_styles.push(out.clone());
        }
        out
    }

    pub(super) fn mleader_style(&mut self, name: &str) {
        if self.dst.mleader_style_named(name).is_some() {
            return;
        }
        if let Some(style) = self.src.mleader_style_named(name) {
            let mut copy = style.clone();
            copy.content.text_style = self.text_style(copy.content.text_style);
            if let Attr::Set(id) = copy.leader_linetype {
                copy.leader_linetype = Attr::Set(self.linetype(id));
            }
            copy.id = crate::mleader::MLeaderStyleId(self.dst.next_mleader_style_id);
            self.dst.next_mleader_style_id = after(self.dst.next_mleader_style_id);
            self.dst.mleader_styles.push(copy);
        }
    }

    /// The block in the target that stands for `def`: the one of its name, or
    /// a new copy.
    pub(super) fn block(&mut self, def: &Arc<BlockDef>) -> Arc<BlockDef> {
        if let Some(done) = self.blocks.get(&def.id) {
            return done.clone();
        }
        let existing = self.dst.block_named(&def.name).cloned();
        let found = match (existing, self.choices) {
            (Some(same), None) => same,
            (None, _) => self.copy_block(def, def.name.clone()),
            (Some(same), Some(choices)) => match choices.get(&def.id) {
                Some(BlockClash::UseExisting) => same,
                _ => {
                    let name = self.dst.unused_block_name_like(&def.name);
                    self.copy_block(def, name)
                }
            },
        };
        self.blocks.insert(def.id, found.clone());
        found
    }

    /// A new block in the target, a copy of `def` called `name`.
    fn copy_block(&mut self, def: &Arc<BlockDef>, name: String) -> Arc<BlockDef> {
        let entities = def
            .entities
            .iter()
            .map(|e| BlockEntity::new(e.id, self.entity(&e.entity), self.attrs(&e.attrs)))
            .collect();
        let copy = Arc::new(BlockDef {
            unit: def.unit,
            id: self.dst.allocate_block_id(),
            name,
            base: def.base,
            description: def.description.clone(),
            entities,
            dynamic: def.dynamic.clone(),
        });
        self.dst.push_block(copy.clone());
        copy
    }

    pub(super) fn entity(&mut self, entity: &Entity) -> Entity {
        match entity {
            Entity::MText(m) => {
                let mut m = m.clone();
                m.style = self.text_style(m.style);
                let fields = self.detached(&m.fields_as_text());
                m.set_fields_from(fields);
                Entity::MText(m)
            }
            Entity::Text(t) => {
                let mut t = t.clone();
                t.placement.style = self.text_style(t.placement.style);
                Entity::Text(t)
            }
            Entity::Insert(i) => {
                let mut i = i.clone();
                let source = i.block.clone();
                i.block = self.block(&i.block);
                // Values mean something only to the schema they were made for.
                if i.dynamic.is_some() && i.block.dynamic != source.dynamic {
                    i.dynamic = None;
                    let name = source.name.clone();
                    self.warnings
                        .push(TransferWarning::DynamicStateDropped(name));
                }
                self.insert_attributes(&mut i);
                Entity::Insert(i)
            }
            Entity::AttributeDefinition(d) => {
                let mut d = d.clone();
                self.attribute_definition(&mut d);
                Entity::AttributeDefinition(d)
            }
            Entity::Dimension(d) => {
                let mut d = d.clone();
                d.style = self.dim_style(&d.style);
                // Its objects have other ids here, or none.
                d.assoc.clear();
                Entity::Dimension(d)
            }
            Entity::MLeader(l) => {
                let mut l = l.clone();
                self.mleader_style(&l.style_name);
                if let MLeaderContent::MText(t) = &mut l.content {
                    t.text.style = self.text_style(t.text.style);
                }
                Entity::MLeader(l)
            }
            other => other.clone(),
        }
    }
}

fn layer_match(a: &Layer, b: &Layer) -> bool {
    a.name.eq_ignore_ascii_case(&b.name)
}
