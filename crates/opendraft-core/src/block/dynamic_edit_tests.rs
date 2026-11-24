//! Editing a dynamic definition in the Block Editor: the schema moves with
//! the geometry, deleted objects leave the actions, and inserts keep the
//! values the new schema still allows.

use std::sync::Arc;

use super::dynamic_tests::sample_schema;
use super::tests::{p, square_block};
use super::*;
use crate::{Document, Entity, EntityId};

/// The square (base point at 10, 10) with `sample_schema`, and a drawing
/// holding it and one insert with `state`.
fn drawing(state: Option<f64>) -> (Document, Arc<BlockDef>) {
    let mut def = (*square_block()).clone();
    def.dynamic = Some(Box::new(sample_schema()));
    let def = Arc::new(def);
    let mut doc = Document::default();
    doc.push_block(def.clone());
    let mut insert = Insert::new(def.clone(), p(0.0, 0.0));
    if let Some(v) = state {
        insert = insert
            .with_dynamic_value(ParameterId(1), DynamicValue::Distance(v))
            .unwrap();
    }
    doc.add(Entity::Insert(Box::new(insert)));
    (doc, def)
}

#[test]
fn the_editor_shows_the_schema_where_it_shows_the_geometry() {
    let (doc, def) = drawing(None);
    let editor = doc.block_editor_document(def.id).unwrap();
    let draft = editor.block_editor_dynamic().unwrap();
    let DynamicParameterKind::Linear { start, end, .. } = &draft.parameters[0].kind else {
        panic!("a linear parameter");
    };
    assert_eq!((*start, *end), (p(0.0, 0.0), p(1.0, 0.0)));
    let DynamicActionKind::Stretch { frame, .. } = &draft.actions[0].kind else {
        panic!("not a stretch");
    };
    assert_eq!((frame.min, frame.max), (p(0.5, -1.0), p(2.0, 2.0)));
}

#[test]
fn saving_with_no_edits_keeps_the_picture_of_every_insert() {
    let (doc, def) = drawing(Some(3.0));
    let editor = doc.block_editor_document(def.id).unwrap();
    let save = doc.block_edit_save(&editor, def.id).unwrap();
    assert_eq!(save.states_changed, 0);
    assert!(save.notes.is_empty() && save.removed_actions.is_empty());
    let saved = &save.blocks[0];
    assert_eq!(
        saved.entities.iter().map(|e| e.id).collect::<Vec<_>>(),
        def.entities.iter().map(|e| e.id).collect::<Vec<_>>()
    );
    let now = save
        .objects
        .first()
        .map_or_else(|| doc.objects()[0].entity.bounds(), |(_, e)| e.bounds());
    assert_eq!(now, doc.objects()[0].entity.bounds());
}

#[test]
fn an_object_deleted_in_the_editor_leaves_its_actions() {
    let (doc, def) = drawing(None);
    let mut editor = doc.block_editor_document(def.id).unwrap();
    editor.remove(EntityId(1));
    let save = doc.block_edit_save(&editor, def.id).unwrap();
    let kept = save.blocks[0].dynamic.as_ref().unwrap();
    let DynamicActionKind::Stretch { objects, .. } = &kept.actions[0].kind else {
        panic!("not a stretch");
    };
    assert_eq!(objects, &[BlockObjectId(0)]);
    assert_eq!(save.notes.len(), 1);
    assert!(save.notes[0].contains("Stretch1"), "{:?}", save.notes);
    assert!(save.removed_actions.is_empty());
}

#[test]
fn an_action_with_no_object_left_is_removed_and_named() {
    let (doc, def) = drawing(None);
    let mut editor = doc.block_editor_document(def.id).unwrap();
    editor.remove(EntityId(0));
    editor.remove(EntityId(1));
    let save = doc.block_edit_save(&editor, def.id).unwrap();
    assert!(save.blocks[0].dynamic.as_ref().unwrap().actions.is_empty());
    assert_eq!(save.removed_actions, ["Stretch1"]);
}

#[test]
fn an_insert_keeps_its_value_brought_into_the_new_set_and_loses_it_when_the_parameter_goes() {
    let (doc, def) = drawing(Some(3.4));
    let mut editor = doc.block_editor_document(def.id).unwrap();
    let mut draft = editor.block_editor_dynamic().unwrap().clone();
    let DynamicParameterKind::Linear { value_set, .. } = &mut draft.parameters[0].kind else {
        panic!("a linear parameter");
    };
    *value_set = DynamicValueSet::List(vec![2.0, 4.0]);
    editor.set_block_editor_dynamic(Some(draft));
    let save = doc.block_edit_save(&editor, def.id).unwrap();
    assert_eq!(save.states_changed, 1);
    let Entity::Insert(i) = &save.objects[0].1 else {
        panic!("not an insert");
    };
    assert_eq!(i.dynamic_value(ParameterId(1)), Some(4.0));

    editor.set_block_editor_dynamic(Some(DynamicBlock::new()));
    let save = doc.block_edit_save(&editor, def.id).unwrap();
    let Entity::Insert(i) = &save.objects[0].1 else {
        panic!("not an insert");
    };
    assert_eq!(i.dynamic, None, "no parameter, no value");
    assert_eq!(save.states_changed, 1);
}

#[test]
fn a_value_that_the_new_set_would_make_the_default_is_not_kept() {
    let (doc, def) = drawing(Some(3.0));
    let mut editor = doc.block_editor_document(def.id).unwrap();
    let mut draft = editor.block_editor_dynamic().unwrap().clone();
    let DynamicParameterKind::Linear { value_set, .. } = &mut draft.parameters[0].kind else {
        panic!("a linear parameter");
    };
    *value_set = DynamicValueSet::Range {
        min: 0.0,
        max: 1.0,
        increment: None,
    };
    editor.set_block_editor_dynamic(Some(draft));
    let save = doc.block_edit_save(&editor, def.id).unwrap();
    let Entity::Insert(i) = &save.objects[0].1 else {
        panic!("not an insert");
    };
    // 3.0 clamps to 1.0, which is the parameter's default length.
    assert_eq!(i.dynamic, None);
}

#[test]
fn a_draft_with_an_action_for_a_missing_parameter_is_refused() {
    let (doc, def) = drawing(None);
    let mut editor = doc.block_editor_document(def.id).unwrap();
    let mut draft = editor.block_editor_dynamic().unwrap().clone();
    draft.actions[0].parameter = ParameterId(9);
    editor.set_block_editor_dynamic(Some(draft));
    assert!(doc.block_edit_save(&editor, def.id).is_err());
}

#[test]
fn a_static_blocks_inserts_have_no_values_after_a_rebind() {
    let (doc, _) = drawing(Some(3.0));
    let Entity::Insert(i) = &doc.objects()[0].entity else {
        panic!("not an insert");
    };
    let rebound = i.rebound(square_block());
    assert_eq!(rebound.dynamic, None);
}
