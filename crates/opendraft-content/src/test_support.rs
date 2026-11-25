//! Small drawings for tests.

use std::sync::Arc;

use opendraft_core::block::{BlockEntity, BlockObjectId, Insert};
use opendraft_core::{Attributes, BlockDef, BlockId, Document, Entity, Layer, Point};

use crate::envelope::header::RawHeader;
use crate::envelope::{Header, Licence};

pub fn circle(radius: f64) -> Entity {
    Entity::Circle {
        center: Point::new(0.0, 0.0),
        radius,
    }
}

/// Adds block `name` to `doc`, on layer `layer` (made if needed), holding a
/// circle of this radius and an insert of each of `nested`.
pub fn add_block(
    doc: &mut Document,
    name: &str,
    layer: &str,
    radius: f64,
    nested: &[&Arc<BlockDef>],
) -> Arc<BlockDef> {
    let layer_id = match doc.layers().iter().find(|l| l.name == layer) {
        Some(l) => l.id,
        None => {
            let id = doc.allocate_layer_id();
            doc.push_layer(Layer::new(id, layer));
            id
        }
    };
    let mut entities = vec![BlockEntity::new(
        BlockObjectId(0),
        circle(radius),
        Attributes::on_layer(layer_id),
    )];
    for (i, inner) in nested.iter().enumerate() {
        entities.push(BlockEntity::new(
            BlockObjectId(i as u64 + 1),
            Entity::Insert(Box::new(Insert::new(
                (*inner).clone(),
                Point::new(1.0, 1.0),
            ))),
            Attributes::on_layer(layer_id),
        ));
    }
    let def = Arc::new(BlockDef {
        unit: Default::default(),
        id: doc.allocate_block_id(),
        name: name.to_owned(),
        base: Point::new(0.0, 0.0),
        description: String::new(),
        entities,
        dynamic: None,
    });
    doc.push_block(def.clone());
    def
}

/// A drawing with one block.
pub fn drawing_with(name: &str, radius: f64) -> (Document, BlockId) {
    let mut doc = Document::default();
    let id = add_block(&mut doc, name, "Symbols", radius, &[]).id;
    (doc, id)
}

/// A drawing with `outer`, which nests `inner`.
pub fn drawing_with_nested(outer: &str, inner: &str) -> (Document, BlockId, BlockId) {
    let mut doc = Document::default();
    let inner = add_block(&mut doc, inner, "Symbols", 1.0, &[]);
    let outer = add_block(&mut doc, outer, "Symbols", 2.0, &[&inner]);
    (doc, outer.id, inner.id)
}

/// The radius of a block's first object, to tell blocks apart.
pub fn radius(def: &BlockDef) -> f64 {
    match def.entities.first().map(|e| &e.entity) {
        Some(Entity::Circle { radius, .. }) => *radius,
        _ => f64::NAN,
    }
}

/// A valid header for the blocks kind.
pub fn header(name: &str) -> Header {
    let raw = RawHeader {
        name: name.into(),
        version: "1.0.0".into(),
        title: "Test".into(),
        summary: "A test package".into(),
        description: String::new(),
        kind: crate::blocks_payload::KIND.into(),
        kind_version: crate::blocks_payload::KIND_VERSION,
        license: if name.starts_with("local/") {
            Licence::Personal
        } else {
            Licence::Cc0
        }
        .spdx()
        .into(),
        authors: Vec::new(),
        keywords: Vec::new(),
        repository: None,
        copyright: None,
        thumbnail: None,
    };
    Header::check(&raw).expect("a valid test header")
}
