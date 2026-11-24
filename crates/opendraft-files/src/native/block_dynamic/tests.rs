//! Saving and reading dynamic blocks: the round trip, and every way a file
//! can be wrong that must be refused rather than read with data lost.

use std::sync::Arc;

use opendraft_core::block::{
    ActionId, BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, DynamicParameter,
    DynamicParameterKind, DynamicValue, DynamicValueSet, ParameterId,
};
use opendraft_core::{
    Attributes, BlockDef, BlockEntity, Bounds, Document, Entity, Insert, LayerId, Point,
};
use serde_json::{json, Value};

use crate::{FileError, FileFormat, NativeFormat};

pub(super) fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

pub(super) fn schema(set: DynamicValueSet) -> DynamicBlock {
    let mut s = DynamicBlock::new();
    s.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Distance1".into(),
        label: "Width".into(),
        description: "the width".into(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Linear {
            start: p(0.0, 0.0),
            end: p(4.0, 0.0),
            value_set: set,
        },
    });
    s.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Stretch1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Stretch {
            objects: vec![BlockObjectId(7), BlockObjectId(9)],
            frame: Bounds::from_points(p(3.0, -1.0), p(5.0, 3.0)),
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    });
    s.actions.push(DynamicAction {
        id: ActionId(2),
        name: "Move1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Move {
            objects: vec![BlockObjectId(9)],
            multiplier: 0.5,
            angle_offset: 0.25,
        },
    });
    s.next_parameter_id = 2;
    s.next_action_id = 3;
    s
}

pub(super) fn object(id: u64, entity: Entity) -> BlockEntity {
    BlockEntity::new(
        BlockObjectId(id),
        entity,
        Attributes::on_layer(LayerId::DEFAULT),
    )
}

pub(super) fn bar(doc: &mut Document, dynamic: Option<DynamicBlock>) -> Arc<BlockDef> {
    Arc::new(BlockDef {
        id: doc.allocate_block_id(),
        name: "Bar".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: vec![
            object(
                7,
                Entity::Line {
                    start: p(0.0, 0.0),
                    end: p(4.0, 0.0),
                },
            ),
            object(
                9,
                Entity::Line {
                    start: p(4.0, 0.0),
                    end: p(4.0, 2.0),
                },
            ),
        ],
        dynamic: dynamic.map(Box::new),
    })
}

/// A dynamic block whose objects have ids 7 and 9, placed twice, one stretched.
fn drawing(set: DynamicValueSet) -> Document {
    let mut doc = Document::default();
    let block = bar(&mut doc, Some(schema(set)));
    doc.push_block(block.clone());
    doc.add(Entity::Insert(Box::new(Insert::new(
        block.clone(),
        p(0.0, 0.0),
    ))));
    let stretched = Insert::new(block, p(10.0, 0.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(6.0))
        .unwrap();
    doc.add(Entity::Insert(Box::new(stretched)));
    doc
}

pub(super) fn write(doc: &Document) -> Value {
    let bytes = NativeFormat.write(doc).unwrap().bytes;
    serde_json::from_slice(&bytes).unwrap()
}

pub(super) fn read(file: &Value) -> Result<Document, FileError> {
    NativeFormat
        .read(file.to_string().as_bytes())
        .map(|r| r.document)
}

fn refused(edit: impl FnOnce(&mut Value)) -> String {
    let mut file = write(&drawing(DynamicValueSet::Any));
    edit(&mut file);
    match read(&file) {
        Err(FileError::Invalid(why)) => why,
        Err(other) => format!("{other:?}"),
        Ok(_) => panic!("a damaged file was loaded"),
    }
}

fn remove(v: &mut Value, key: &str) {
    v.as_object_mut().unwrap().remove(key);
}

#[test]
fn a_dynamic_block_and_its_values_survive_a_round_trip_exactly() {
    for set in [
        DynamicValueSet::Any,
        DynamicValueSet::List(vec![2.0, 4.0, 6.0]),
        DynamicValueSet::Range {
            min: 2.0,
            max: 10.0,
            increment: Some(2.0),
        },
    ] {
        let doc = drawing(set);
        let back = read(&write(&doc)).unwrap();
        assert_eq!(back.blocks(), doc.blocks());
        assert_eq!(back.objects(), doc.objects());
        assert_eq!(back.validate(), Ok(()));
        let states: Vec<bool> = back
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Insert(i) => Some(i.dynamic.is_some()),
                _ => None,
            })
            .collect();
        assert_eq!(states, [false, true]);
    }
}

#[test]
fn object_ids_are_kept_not_renumbered() {
    let back = read(&write(&drawing(DynamicValueSet::Any))).unwrap();
    let ids: Vec<u64> = back.blocks()[0].entities.iter().map(|e| e.id.0).collect();
    assert_eq!(ids, [7, 9]);
}

#[test]
fn a_static_block_writes_no_dynamic_data_and_a_file_without_ids_still_reads() {
    let mut doc = Document::default();
    let block = bar(&mut doc, None);
    doc.push_block(block.clone());
    doc.add(Entity::Insert(Box::new(Insert::new(block, p(0.0, 0.0)))));
    let mut file = write(&doc);
    assert!(file["drawing"]["blocks"][0].get("dynamic").is_none());
    for o in file["drawing"]["blocks"][0]["objects"]
        .as_array_mut()
        .unwrap()
    {
        remove(o, "id");
    }
    let back = read(&file).unwrap();
    let ids: Vec<u64> = back.blocks()[0].entities.iter().map(|e| e.id.0).collect();
    assert_eq!(ids, [0, 1], "numbered in order");
}

#[test]
fn a_dynamic_block_without_ids_or_with_only_some_is_refused() {
    let why = refused(|f| {
        for o in f["drawing"]["blocks"][0]["objects"].as_array_mut().unwrap() {
            remove(o, "id");
        }
    });
    assert!(why.contains("without ids"), "{why}");
    let why = refused(|f| remove(&mut f["drawing"]["blocks"][0]["objects"][1], "id"));
    assert!(why.contains("without ids"), "{why}");
}

#[test]
fn two_objects_with_one_id_are_refused() {
    let why = refused(|f| f["drawing"]["blocks"][0]["objects"][1]["id"] = json!(7));
    assert!(why.contains("one id"), "{why}");
}

#[test]
fn an_unknown_schema_version_or_field_is_refused() {
    let why = refused(|f| f["drawing"]["blocks"][0]["dynamic"]["version"] = json!(2));
    assert!(why.contains("version"), "{why}");
    let why = refused(|f| f["drawing"]["blocks"][0]["dynamic"]["flip"] = json!(true));
    assert!(why.contains("flip"), "{why}");
    let why = refused(|f| {
        f["drawing"]["blocks"][0]["dynamic"]["parameters"][0]["kind"]["linear"]["spin"] = json!(1)
    });
    assert!(why.contains("spin"), "{why}");
    let why = refused(|f| f["drawing"]["objects"][1]["entity"]["dynamic"]["extra"] = json!(1));
    assert!(why.contains("extra"), "{why}");
}

#[test]
fn a_schema_that_cannot_exist_is_refused() {
    let why =
        refused(|f| f["drawing"]["blocks"][0]["dynamic"]["actions"][0]["parameter"] = json!(5));
    assert!(why.contains("missing parameter"), "{why}");
    let why = refused(|f| {
        f["drawing"]["blocks"][0]["dynamic"]["actions"][1]["kind"]["move"]["objects"] = json!([42])
    });
    assert!(why.contains("object 42"), "{why}");
    let why = refused(|f| {
        f["drawing"]["blocks"][0]["dynamic"]["actions"][0]["kind"]["stretch"]["frame"] =
            json!([[5.0, 3.0], [3.0, -1.0]])
    });
    assert!(why.contains("wrong way"), "{why}");
    let why = refused(|f| {
        remove(
            &mut f["drawing"]["blocks"][0]["dynamic"]["actions"][0]["kind"]["stretch"],
            "frame",
        )
    });
    assert!(why.contains("no frame"), "{why}");
    let why = refused(|f| {
        f["drawing"]["blocks"][0]["dynamic"]["parameters"][0]["kind"]["linear"]["end"] =
            json!([0.0, 0.0])
    });
    assert!(why.contains("Bar"), "{why}");
}

#[test]
fn values_the_block_cannot_take_are_refused() {
    let why = refused(|f| {
        f["drawing"]["blocks"][0]["dynamic"]["parameters"][0]["kind"]["linear"]["values"] =
            json!({"list": [2.0, 4.0]});
        f["drawing"]["objects"][1]["entity"]["dynamic"]["values"][0]["value"] =
            json!({"distance": 5.0});
    });
    assert!(why.contains("value set"), "{why}");
    let why = refused(|f| {
        f["drawing"]["objects"][1]["entity"]["dynamic"]["values"][0]["parameter"] = json!(3)
    });
    assert!(why.contains("does not exist"), "{why}");
    let why = refused(|f| {
        f["drawing"]["objects"][1]["entity"]["dynamic"]["values"][0]["value"] =
            json!({"text": "wide"})
    });
    assert!(why.contains("value set"), "{why}");
    let why = refused(|f| {
        let entry = f["drawing"]["objects"][1]["entity"]["dynamic"]["values"][0].clone();
        f["drawing"]["objects"][1]["entity"]["dynamic"]["values"] = json!([entry.clone(), entry]);
    });
    assert!(why.contains("not in order"), "{why}");
    let why = refused(|f| remove(&mut f["drawing"]["blocks"][0], "dynamic"));
    assert!(why.contains("no parameters"), "{why}");
}

#[test]
fn a_list_past_its_cap_is_refused_as_it_is_read() {
    let why = refused(|f| {
        f["drawing"]["blocks"][0]["dynamic"]["parameters"][0]["kind"]["linear"]["values"] =
            json!({"list": vec![1.0; 70_000]});
    });
    assert!(why.contains("more than"), "{why}");
}

#[test]
fn a_document_with_a_schema_that_cannot_exist_is_not_written() {
    let mut doc = Document::default();
    let mut s = schema(DynamicValueSet::Any);
    s.actions[0].parameter = ParameterId(9);
    let block = bar(&mut doc, Some(s));
    doc.push_block(block);
    assert!(NativeFormat.write(&doc).is_err());
}

#[test]
fn nested_dynamic_inserts_and_blocks_in_any_order_round_trip() {
    let mut doc = drawing(DynamicValueSet::Any);
    let inner = doc.blocks()[0].clone();
    let stretched = Insert::new(inner, p(1.0, 1.0))
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(8.0))
        .unwrap();
    let outer = Arc::new(BlockDef {
        id: doc.allocate_block_id(),
        name: "Holder".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: vec![object(0, Entity::Insert(Box::new(stretched)))],
        dynamic: None,
    });
    doc.push_block(outer);
    let mut file = write(&doc);
    file["drawing"]["blocks"].as_array_mut().unwrap().reverse();
    let back = read(&file).unwrap();
    assert_eq!(back.validate(), Ok(()));
    let held = back.block_named("Holder").unwrap();
    let Entity::Insert(i) = &held.entities[0].entity else {
        panic!("not an insert");
    };
    assert!(i.dynamic.is_some() && i.block.is_dynamic());
}
