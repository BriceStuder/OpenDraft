//! A DXF cannot keep a dynamic block's parameters and actions, so it gets
//! each reference's current shape as an ordinary block, with a note.

use std::sync::Arc;

use opendraft_core::block::{
    ActionId, BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, DynamicParameter,
    DynamicParameterKind, DynamicValue, DynamicValueSet, ParameterId,
};
use opendraft_core::{
    Attributes, BlockDef, BlockEntity, BlockId, Bounds, Document, Entity, Insert, LayerId, Point,
};

use crate::{DxfFormat, FileFormat};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A bar 4 long with a post at its end, stretched by one parameter.
fn bar(id: u64, name: &str, inner: Option<&Arc<BlockDef>>) -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Distance1".into(),
        label: "Distance".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Linear {
            start: p(0.0, 0.0),
            end: p(4.0, 0.0),
            value_set: DynamicValueSet::Any,
        },
    });
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Stretch1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Stretch {
            objects: vec![BlockObjectId(0), BlockObjectId(1)],
            frame: Bounds::from_points(p(3.0, -1.0), p(5.0, 3.0)),
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    });
    schema.next_parameter_id = 2;
    schema.next_action_id = 2;
    let line = |id: u64, a: Point, b: Point| {
        BlockEntity::new(
            BlockObjectId(id),
            Entity::Line { start: a, end: b },
            Attributes::on_layer(LayerId::DEFAULT),
        )
    };
    let mut entities = vec![
        line(0, p(0.0, 0.0), p(4.0, 0.0)),
        line(1, p(4.0, 0.0), p(4.0, 2.0)),
    ];
    if let Some(inner) = inner {
        entities.push(BlockEntity::new(
            BlockObjectId(2),
            Entity::Insert(Box::new(Insert::new((*inner).clone(), p(0.0, 5.0)))),
            Attributes::on_layer(LayerId::DEFAULT),
        ));
    }
    Arc::new(BlockDef {
        id: BlockId(id),
        name: name.into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities,
        dynamic: Some(Box::new(schema)),
    })
}

fn stretched(block: &Arc<BlockDef>, at: f64, to: f64) -> Insert {
    Insert::new(block.clone(), p(at, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(to))
        .unwrap()
}

/// The drawing's references in order: untouched at 0, stretched to 6 at 20,
/// stretched to 6 again at 40, stretched to 9 at 60.
fn drawing() -> (Document, Arc<BlockDef>) {
    let mut doc = Document::default();
    let block = bar(doc.allocate_block_id().0, "Bar", None);
    doc.push_block(block.clone());
    doc.add(Entity::Insert(Box::new(Insert::new(
        block.clone(),
        p(0.0, 0.0),
    ))));
    doc.add(Entity::Insert(Box::new(stretched(&block, 20.0, 6.0))));
    doc.add(Entity::Insert(Box::new(stretched(&block, 40.0, 6.0))));
    doc.add(Entity::Insert(Box::new(stretched(&block, 60.0, 9.0))));
    (doc, block)
}

/// The note about dynamic blocks, from among the notes a DXF always carries.
fn dynamic_notes(notes: &[String]) -> Vec<String> {
    notes
        .iter()
        .filter(|n| n.contains("dynamic block"))
        .cloned()
        .collect()
}

fn export(doc: &Document) -> (Document, Vec<String>) {
    let encoded = DxfFormat.write(doc).unwrap();
    let back = DxfFormat.read(&encoded.bytes).unwrap().document;
    (back, dynamic_notes(&encoded.notes))
}

fn inserts(doc: &Document) -> Vec<Insert> {
    doc.objects()
        .iter()
        .filter_map(|o| match &o.entity {
            Entity::Insert(i) => Some((**i).clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn each_reference_is_written_as_the_shape_it_has_now() {
    let (doc, _) = drawing();
    let (back, notes) = export(&doc);
    let want: Vec<_> = inserts(&doc).iter().map(|i| i.bounds()).collect();
    let got: Vec<_> = inserts(&back).iter().map(|i| i.bounds()).collect();
    assert_eq!(got.len(), 4);
    for (g, w) in got.iter().zip(&want) {
        assert!(
            (g.max.x - w.max.x).abs() < 1e-6 && (g.max.y - w.max.y).abs() < 1e-6,
            "{g:?} vs {w:?}"
        );
    }
    assert!(back.blocks().iter().all(|b| !b.is_dynamic()));
    assert_eq!(notes.len(), 1);
    assert!(
        notes[0].starts_with("4 dynamic block references were written as ordinary blocks of"),
        "{notes:?}"
    );
}

#[test]
fn one_block_is_made_for_each_state_not_for_each_reference() {
    let (doc, _) = drawing();
    let m = doc.materialized_static().unwrap();
    let names: Vec<&str> = m
        .document
        .blocks()
        .iter()
        .map(|b| b.name.as_str())
        .collect();
    assert_eq!(names, ["Bar", "Bar_1", "Bar_2"], "default, 6 and 9");
    assert_eq!((m.references, m.definitions), (4, 1));
    let used: Vec<_> = inserts(&m.document)
        .iter()
        .map(|i| i.block.name.clone())
        .collect();
    assert_eq!(used, ["Bar", "Bar_1", "Bar_1", "Bar_2"]);
    assert_eq!(m.document.validate(), Ok(()));
}

#[test]
fn a_new_name_avoids_the_names_a_drawing_already_has() {
    let (mut doc, block) = drawing();
    let mut other = (*bar(doc.allocate_block_id().0, "bar_1", None)).clone();
    other.dynamic = None;
    doc.push_block(Arc::new(other));
    let m = doc.materialized_static().unwrap();
    let names: Vec<String> = m
        .document
        .blocks()
        .iter()
        .map(|b| b.name.to_lowercase())
        .collect();
    let unique: std::collections::HashSet<_> = names.iter().collect();
    assert_eq!(unique.len(), names.len(), "{names:?}");
    assert!(names.contains(&"bar_2".to_owned()), "{names:?}");
    let _ = block;
}

#[test]
fn the_open_drawing_is_not_changed_by_an_export() {
    let (doc, _) = drawing();
    let before = (doc.blocks().to_vec(), doc.objects().to_vec());
    let _ = DxfFormat.write(&doc).unwrap();
    assert_eq!((doc.blocks().to_vec(), doc.objects().to_vec()), before);
    assert!(inserts(&doc)[1].dynamic.is_some());
}

#[test]
fn a_dynamic_block_nested_in_another_is_made_static_innermost_first() {
    let mut doc = Document::default();
    let inner = bar(doc.allocate_block_id().0, "Inner", None);
    let mut outer_def = (*bar(doc.allocate_block_id().0, "Outer", Some(&inner))).clone();
    // The outer holds the inner stretched to 7.
    if let Some(e) = outer_def.entities.last_mut() {
        e.entity = Entity::Insert(Box::new(stretched(&inner, 0.0, 7.0)));
    }
    let outer = Arc::new(outer_def);
    doc.push_block(inner);
    doc.push_block(outer.clone());
    doc.add(Entity::Insert(Box::new(Insert::new(outer, p(0.0, 0.0)))));
    let m = doc.materialized_static().unwrap();
    assert!(m.document.blocks().iter().all(|b| !b.is_dynamic()));
    assert_eq!(m.document.validate(), Ok(()));
    let want = doc.objects()[0].entity.bounds();
    let got = m.document.objects()[0].entity.bounds();
    assert!((got.max.x - want.max.x).abs() < 1e-9 && (got.max.y - want.max.y).abs() < 1e-9);
    let (back, notes) = export(&doc);
    assert_eq!(notes.len(), 1);
    assert_eq!(inserts(&back).len(), 1);
}

#[test]
fn a_drawing_with_no_dynamic_block_is_written_as_before() {
    let mut doc = Document::default();
    let mut def = (*bar(doc.allocate_block_id().0, "Plain", None)).clone();
    def.dynamic = None;
    let block = Arc::new(def);
    doc.push_block(block.clone());
    doc.add(Entity::Insert(Box::new(Insert::new(block, p(0.0, 0.0)))));
    assert!(doc.materialized_static().is_none());
    let encoded = DxfFormat.write(&doc).unwrap();
    assert!(
        dynamic_notes(&encoded.notes).is_empty(),
        "{:?}",
        encoded.notes
    );
}

#[test]
fn a_dynamic_block_nobody_inserted_is_still_written_and_noted() {
    let mut doc = Document::default();
    let block = bar(doc.allocate_block_id().0, "Lonely", None);
    doc.push_block(block);
    let (back, notes) = export(&doc);
    assert!(back.block_named("Lonely").is_some_and(|b| !b.is_dynamic()));
    assert!(
        notes[0].starts_with("1 dynamic block was written"),
        "{notes:?}"
    );
}
