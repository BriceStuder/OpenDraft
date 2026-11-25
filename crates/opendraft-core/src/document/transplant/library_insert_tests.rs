use std::collections::HashMap;
use std::sync::Arc;

use super::*;
use crate::block::{BlockEntity, BlockObjectId, Insert};
use crate::layer::Attributes;
use crate::{BlockDef, BlockId, Entity, Point};

fn make(doc: &mut Document, name: &str, r: f64, nested: &[&Arc<BlockDef>]) -> Arc<BlockDef> {
    let mut entities = vec![BlockEntity::new(
        BlockObjectId(0),
        Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: r,
        },
        Attributes::on_layer(doc.current_layer),
    )];
    for (i, n) in nested.iter().enumerate() {
        entities.push(BlockEntity::new(
            BlockObjectId(i as u64 + 1),
            Entity::Insert(Box::new(Insert::new((*n).clone(), Point::new(1.0, 1.0)))),
            Attributes::on_layer(doc.current_layer),
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

fn place(doc: &mut Document, block: &Arc<BlockDef>, at: f64) -> EntityId {
    let insert = Insert::new(block.clone(), Point::new(at, 0.0));
    doc.add_with(
        Entity::Insert(Box::new(insert)),
        Attributes::on_layer(doc.current_layer),
    )
}

fn radius_of(def: &BlockDef) -> f64 {
    match def.entities[0].entity {
        Entity::Circle { radius, .. } => radius,
        _ => f64::NAN,
    }
}

/// The radius of the block the object at `id` inserts.
fn shown(doc: &Document, id: EntityId) -> f64 {
    match doc.object(id).map(|o| &o.entity) {
        Some(Entity::Insert(i)) => radius_of(&i.block),
        _ => f64::NAN,
    }
}

fn replace_all(_src: &Document, ids: &[BlockId]) -> HashMap<BlockId, BlockClash> {
    ids.iter().map(|id| (*id, BlockClash::Replace)).collect()
}

#[test]
fn a_replaced_block_keeps_its_id_and_every_reference_follows() {
    let mut dst = Document::default();
    let old = make(&mut dst, "valve", 1.0, &[]);
    let a = place(&mut dst, &old, 0.0);
    let b = place(&mut dst, &old, 10.0);
    let mut src = Document::default();
    let new = make(&mut src, "VALVE", 5.0, &[]);

    let brought = dst
        .bring_block(&src, new.id, &replace_all(&src, &[new.id]))
        .expect("brings");
    assert_eq!(brought.block.id, old.id);
    assert_eq!(brought.block.name, "valve");
    assert_eq!(brought.rebound, 2);
    assert_eq!(dst.blocks().len(), 1);
    assert_eq!((shown(&dst, a), shown(&dst, b)), (5.0, 5.0));
    assert_eq!(dst.validate(), Ok(()));

    dst.unbring(&brought);
    assert_eq!((shown(&dst, a), shown(&dst, b)), (1.0, 1.0));
    assert_eq!(radius_of(dst.block(old.id).expect("kept")), 1.0);
    dst.rebring(&brought);
    assert_eq!((shown(&dst, a), shown(&dst, b)), (5.0, 5.0));
    assert_eq!(dst.validate(), Ok(()));
}

#[test]
fn blocks_that_hold_a_replaced_block_follow_it_too() {
    let mut dst = Document::default();
    let nut = make(&mut dst, "nut", 1.0, &[]);
    let bolt = make(&mut dst, "bolt", 2.0, &[&nut]);
    let on_page = place(&mut dst, &bolt, 0.0);
    let mut src = Document::default();
    let new_nut = make(&mut src, "nut", 9.0, &[]);

    let brought = dst
        .bring_block(&src, new_nut.id, &replace_all(&src, &[new_nut.id]))
        .expect("brings");
    let Some(Entity::Insert(i)) = dst.object(on_page).map(|o| &o.entity) else {
        unreachable!("an insert")
    };
    let inner = i.block.nested_blocks().next().and_then(|id| dst.block(id));
    assert_eq!(inner.map(|b| radius_of(b)), Some(9.0));
    assert_eq!(dst.validate(), Ok(()));
    dst.unbring(&brought);
    assert_eq!(dst.validate(), Ok(()));
    let inner = dst
        .block_named("bolt")
        .and_then(|b| b.nested_blocks().next())
        .and_then(|id| dst.block(id));
    assert_eq!(inner.map(|b| radius_of(b)), Some(1.0));
}

#[test]
fn a_replacement_that_would_hold_itself_is_refused_and_changes_nothing() {
    let mut dst = Document::default();
    let valve = make(&mut dst, "valve", 1.0, &[]);
    let wrap = make(&mut dst, "wrap", 2.0, &[&valve]);
    let mut src = Document::default();
    let src_wrap = make(&mut src, "wrap", 3.0, &[]);
    let src_valve = make(&mut src, "valve", 4.0, &[&src_wrap]);
    let before: Vec<_> = dst
        .blocks()
        .iter()
        .map(|b| (b.id, b.name.clone()))
        .collect();

    // The new valve would hold the drawing's wrap, which holds the valve.
    let choices = HashMap::from([
        (src_valve.id, BlockClash::Replace),
        (src_wrap.id, BlockClash::UseExisting),
    ]);
    let error = dst
        .bring_block(&src, src_valve.id, &choices)
        .expect_err("a loop");
    assert!(matches!(error, BringError::Refused(_)), "{error:?}");
    let after: Vec<_> = dst
        .blocks()
        .iter()
        .map(|b| (b.id, b.name.clone()))
        .collect();
    assert_eq!(before, after);
    assert_eq!(radius_of(dst.block(valve.id).expect("kept")), 1.0);
    assert_eq!(radius_of(dst.block(wrap.id).expect("kept")), 2.0);
    assert_eq!(dst.validate(), Ok(()));
}

fn at(x: f64) -> LibraryPlacement {
    LibraryPlacement {
        insertion: Point::new(x, 2.0),
        scale_x: 2.0,
        scale_y: 2.0,
        rotation: 0.5,
        values: None,
    }
}

#[test]
fn inserting_adds_the_block_and_a_reference_and_undoes_and_redoes_exactly() {
    let mut src = Document::default();
    let valve = make(&mut src, "valve", 3.0, &[]);
    let mut dst = Document::default();
    let objects_before = dst.len();

    let placed = dst
        .insert_from(&src, valve.id, &HashMap::new(), &at(7.0), None)
        .expect("inserts");
    assert_eq!(dst.len(), objects_before + 1);
    assert_eq!(dst.blocks().len(), 1);
    let Some(Entity::Insert(i)) = dst.object(placed.object.id).map(|o| &o.entity) else {
        unreachable!("an insert")
    };
    assert_eq!(
        (i.insertion, i.scale_x, i.rotation),
        (Point::new(7.0, 2.0), 2.0, 0.5)
    );
    assert_eq!(dst.validate(), Ok(()));

    dst.unplace(&placed);
    assert_eq!(dst.len(), objects_before);
    assert!(dst.blocks().is_empty());
    dst.replace_placed(&placed);
    assert_eq!(dst.len(), objects_before + 1);
    assert_eq!(dst.blocks().len(), 1);
    assert_eq!(dst.validate(), Ok(()));
}

#[test]
fn inserting_over_a_taken_name_with_replace_changes_the_drawings_references() {
    let mut dst = Document::default();
    let old = make(&mut dst, "valve", 1.0, &[]);
    let existing = place(&mut dst, &old, 0.0);
    let mut src = Document::default();
    let new = make(&mut src, "valve", 5.0, &[]);

    let placed = dst
        .insert_from(&src, new.id, &replace_all(&src, &[new.id]), &at(3.0), None)
        .expect("inserts");
    assert_eq!(placed.brought.rebound, 1);
    assert_eq!(shown(&dst, existing), 5.0);
    assert_eq!(shown(&dst, placed.object.id), 5.0);
    assert_eq!(dst.blocks().len(), 1);

    dst.unplace(&placed);
    assert_eq!(shown(&dst, existing), 1.0);
    assert_eq!(dst.len(), 1);
    assert_eq!(dst.validate(), Ok(()));
}

#[test]
fn inserting_over_a_taken_name_by_default_adds_a_renamed_block() {
    let mut dst = Document::default();
    let old = make(&mut dst, "valve", 1.0, &[]);
    let existing = place(&mut dst, &old, 0.0);
    let mut src = Document::default();
    let new = make(&mut src, "valve", 5.0, &[]);
    let placed = dst
        .insert_from(&src, new.id, &HashMap::new(), &at(3.0), None)
        .expect("inserts");
    assert_eq!(placed.brought.block.name, "valve_2");
    assert_eq!(shown(&dst, existing), 1.0);
    assert_eq!(shown(&dst, placed.object.id), 5.0);
}
