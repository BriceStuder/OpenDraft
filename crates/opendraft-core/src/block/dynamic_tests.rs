//! The dynamic slots of blocks and inserts: what equality sees, what
//! `validate` refuses, how the Block Editor carries ids and the schema, and
//! what pasting does with values the target cannot take. Nothing here
//! evaluates a schema: that is later work.

use std::collections::HashSet;
use std::sync::Arc;

use super::tests::{p, square_block};
use super::*;
use crate::document::Placement;
use crate::{Bounds, Document, DocumentError, EntityId, TransferWarning};

/// A schema valid for `square_block`: one linear parameter and a stretch of
/// both its objects.
pub(crate) fn sample_schema() -> DynamicBlock {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Distance1".into(),
        label: "Distance".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Linear {
            start: p(10.0, 10.0),
            end: p(11.0, 10.0),
            value_set: DynamicValueSet::Any,
        },
    });
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Stretch1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Stretch {
            objects: vec![BlockObjectId(0), BlockObjectId(1)],
            frame: Bounds::from_points(p(10.5, 9.0), p(12.0, 12.0)),
            multiplier: 1.0,
            angle_offset: 0.0,
        },
    });
    schema.next_parameter_id = 2;
    schema.next_action_id = 2;
    schema
}

fn dynamic_square() -> Arc<BlockDef> {
    let mut def = (*square_block()).clone();
    def.dynamic = Some(Box::new(sample_schema()));
    Arc::new(def)
}

fn stretched() -> DynamicInstanceState {
    let mut state = DynamicInstanceState::default();
    state.set(ParameterId(1), DynamicValue::Distance(3.0));
    state
}

fn ids(def: &BlockDef) -> Vec<u64> {
    def.entities.iter().map(|e| e.id.0).collect()
}

#[test]
fn objects_made_into_a_block_are_numbered_in_order() {
    let doc = Document::from_entities([
        Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        },
        Entity::Point { at: p(2.0, 2.0) },
        Entity::Circle {
            center: p(5.0, 5.0),
            radius: 1.0,
        },
    ]);
    let def = BlockDef::from_objects(BlockId(1), "Three", Point::default(), doc.objects());
    assert_eq!(ids(&def), [0, 1, 2]);
    assert!(!def.is_dynamic() && def.dynamic.is_none());
}

#[test]
fn a_definition_is_equal_only_to_one_with_the_same_ids_schema_and_objects() {
    let plain = square_block();
    assert_eq!(*plain, *square_block());
    assert_ne!(*plain, *dynamic_square());
    assert!(dynamic_square().is_dynamic());

    let mut renumbered = (*plain).clone();
    renumbered.entities[1].id = BlockObjectId(7);
    assert_ne!(*plain, renumbered);

    let mut other_schema = (*dynamic_square()).clone();
    other_schema.dynamic.as_mut().unwrap().parameters[0].label = "Length".into();
    assert_ne!(*dynamic_square(), other_schema);
}

#[test]
fn an_insert_is_equal_only_to_one_with_the_same_values() {
    let block = dynamic_square();
    let plain = Insert::new(block.clone(), p(0.0, 0.0));
    let mut set = plain.clone();
    set.dynamic = Some(Box::new(stretched()));
    assert_ne!(plain, set);
    assert_eq!(set.reset_dynamic(), plain);
    // An edit that is not a value change keeps the values.
    assert_eq!(set.translated(1.0, 2.0).dynamic, set.dynamic);
    assert_eq!(set.rotated(p(0.0, 0.0), 0.5).dynamic, set.dynamic);
    assert_eq!(set.mirrored(p(0.0, 0.0), p(0.0, 1.0)).dynamic, set.dynamic);
    assert_eq!(set.scaled(p(0.0, 0.0), 2.0).dynamic, set.dynamic);
    assert_eq!(set.rebound(block).dynamic, set.dynamic);
    let mut other = set.clone();
    other.dynamic = Some(Box::new({
        let mut s = DynamicInstanceState::default();
        s.set(ParameterId(1), DynamicValue::Distance(4.0));
        s
    }));
    assert_ne!(set, other);
}

#[test]
fn bounds_computed_for_one_drawing_do_not_make_it_unequal() {
    let mut a = Document::default();
    a.push_block(dynamic_square());
    a.add(Entity::Insert(Box::new(Insert::new(
        dynamic_square(),
        p(1.0, 1.0),
    ))));
    let b = a.clone();
    assert!(a.bounds().is_some());
    assert_eq!(a, b);
}

#[test]
fn the_schema_check_names_each_structural_fault() {
    let objects: HashSet<BlockObjectId> = [BlockObjectId(0), BlockObjectId(1)].into();
    assert_eq!(sample_schema().check(&objects), Ok(()));
    assert_eq!(DynamicBlock::new().check(&HashSet::new()), Ok(()));

    let fault = |edit: &dyn Fn(&mut DynamicBlock)| {
        let mut schema = sample_schema();
        edit(&mut schema);
        schema.check(&objects).unwrap_err()
    };
    assert!(fault(&|s| s.version = 2).contains("version"));
    assert!(fault(&|s| s.parameters[0].id = ParameterId(0)).contains("parameter id 0"));
    assert!(fault(&|s| s.next_parameter_id = 1).contains("not issued"));
    assert!(fault(&|s| {
        let again = s.parameters[0].clone();
        s.parameters.push(again);
    })
    .contains("repeated"));
    assert!(fault(&|s| s.actions[0].id = ActionId(9)).contains("not issued"));
    assert!(fault(&|s| s.actions[0].parameter = ParameterId(5)).contains("missing parameter"));
    let select = |ids: Vec<u64>| {
        move |s: &mut DynamicBlock| {
            if let DynamicActionKind::Stretch { objects, .. } = &mut s.actions[0].kind {
                *objects = ids.iter().map(|&i| BlockObjectId(i)).collect();
            }
        }
    };
    assert!(fault(&select(vec![])).contains("selects nothing"));
    assert!(fault(&select(vec![1, 0])).contains("unsorted"));
    assert!(fault(&select(vec![0, 0])).contains("repeats"));
    assert!(fault(&select(vec![0, 5])).contains("object 5"));
}

#[test]
fn state_is_well_formed_when_finite_and_sorted() {
    assert!(stretched().is_well_formed() && DynamicInstanceState::default().is_well_formed());
    let mut nan = DynamicInstanceState::default();
    nan.set(ParameterId(1), DynamicValue::Distance(f64::NAN));
    assert!(!nan.is_well_formed());
    let mut point = DynamicInstanceState::default();
    point.set(ParameterId(1), DynamicValue::Point(p(0.0, f64::INFINITY)));
    assert!(!point.is_well_formed());
    let unsorted = DynamicInstanceState {
        values: vec![
            DynamicValueEntry {
                parameter: ParameterId(2),
                value: DynamicValue::Bool(true),
            },
            DynamicValueEntry {
                parameter: ParameterId(1),
                value: DynamicValue::Bool(true),
            },
        ],
    };
    assert!(!unsorted.is_well_formed());
}

fn drawing_with(block: Arc<BlockDef>, state: Option<DynamicInstanceState>) -> Document {
    let mut doc = Document::default();
    doc.push_block(block.clone());
    let mut insert = Insert::new(block, p(0.0, 0.0));
    insert.dynamic = state.map(Box::new);
    doc.add(Entity::Insert(Box::new(insert)));
    doc
}

#[test]
fn validate_accepts_a_dynamic_block_and_its_values() {
    assert_eq!(drawing_with(dynamic_square(), None).validate(), Ok(()));
    assert_eq!(
        drawing_with(dynamic_square(), Some(stretched())).validate(),
        Ok(())
    );
    assert_eq!(drawing_with(square_block(), None).validate(), Ok(()));
}

#[test]
fn validate_refuses_values_on_a_block_that_has_no_parameters() {
    let doc = drawing_with(square_block(), Some(stretched()));
    assert!(matches!(
        doc.validate(),
        Err(DocumentError::BadDynamicState(_, why)) if why.contains("no parameters")
    ));
    // The same inside a definition.
    let mut outer = Document::default();
    outer.push_block(square_block());
    let mut inner = Insert::new(square_block(), p(0.0, 0.0));
    inner.dynamic = Some(Box::new(stretched()));
    let holder = Arc::new(BlockDef {
        unit: Default::default(),
        id: BlockId(2),
        name: "Holder".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        entities: vec![BlockEntity::new(
            BlockObjectId(0),
            Entity::Insert(Box::new(inner)),
            Attributes::on_layer(LayerId::DEFAULT),
        )],
        dynamic: None,
    });
    outer.push_block(holder);
    assert!(matches!(outer.validate(), Err(DocumentError::BadBlock(name)) if name == "Holder"));
}

#[test]
fn validate_refuses_values_that_are_not_finite_or_a_schema_that_cannot_exist() {
    let mut nan = DynamicInstanceState::default();
    nan.set(ParameterId(1), DynamicValue::Distance(f64::NAN));
    assert!(matches!(
        drawing_with(dynamic_square(), Some(nan)).validate(),
        Err(DocumentError::BadInsert(_))
    ));
    let mut def = (*dynamic_square()).clone();
    def.dynamic.as_mut().unwrap().actions[0].parameter = ParameterId(9);
    let err = drawing_with(Arc::new(def), None).validate().unwrap_err();
    assert!(
        matches!(&err, DocumentError::BadDynamicBlock(name, why)
            if name == "Square" && why.contains("missing parameter")),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "block \"Square\" is not a valid dynamic block: action 1 names a missing parameter"
    );
    assert_eq!(
        DocumentError::BadDynamicState(EntityId(4), "x".into()).to_string(),
        "object 4 has an invalid dynamic state: x"
    );
}

#[test]
fn validate_refuses_two_objects_of_a_block_with_one_id() {
    let mut def = (*square_block()).clone();
    def.entities[1].id = def.entities[0].id;
    let mut doc = Document::default();
    doc.push_block(Arc::new(def));
    assert!(matches!(doc.validate(), Err(DocumentError::BadBlock(_))));
}

#[test]
fn the_editor_keeps_each_objects_id_and_saving_gives_them_back() {
    let mut def = (*square_block()).clone();
    def.base = p(0.0, 0.0);
    def.entities[0].id = BlockObjectId(5);
    def.entities[1].id = BlockObjectId(9);
    let def = Arc::new(def);
    let mut doc = Document::default();
    doc.push_block(def.clone());
    // The drawing has many objects, so its counter is far past the block's ids.
    for _ in 0..20 {
        doc.add(Entity::Point { at: p(0.0, 0.0) });
    }
    let editor = doc.block_editor_document(def.id).unwrap();
    let in_editor: Vec<u64> = editor.objects().iter().map(|o| o.id.0).collect();
    assert_eq!(in_editor, [5, 9]);
    assert_eq!(editor.validate(), Ok(()));
    let save = doc.block_edit_save(&editor, def.id).unwrap();
    assert_eq!(
        *save.blocks[0], *def,
        "no edit changes nothing, ids included"
    );
}

#[test]
fn a_new_object_in_the_editor_gets_an_id_no_object_of_the_block_has() {
    let doc = drawing_with(square_block(), None);
    let id = square_block().id;
    let mut editor = doc.block_editor_document(id).unwrap();
    let added = editor.add(Entity::Point { at: p(1.0, 1.0) });
    let save = doc.block_edit_save(&editor, id).unwrap();
    let new = &save.blocks[0];
    let unique: HashSet<u64> = ids(new).into_iter().collect();
    assert_eq!(unique.len(), 3);
    assert!(new.entities.iter().any(|e| e.id.0 == added.0));
}

#[test]
fn the_editor_drafts_the_blocks_schema_and_saving_carries_it() {
    let mut def = (*dynamic_square()).clone();
    def.base = p(0.0, 0.0);
    let def = Arc::new(def);
    let mut doc = Document::default();
    doc.push_block(def.clone());
    let mut editor = doc.block_editor_document(def.id).unwrap();
    assert_eq!(editor.block_editor_dynamic(), Some(&sample_schema()));
    assert_eq!(doc.block_editor_dynamic(), None);
    assert_eq!(editor.validate(), Ok(()), "the draft is not drawing data");
    // Saving with the draft untouched leaves the block as it was.
    assert_eq!(
        *doc.block_edit_save(&editor, def.id).unwrap().blocks[0],
        *def
    );
    // Editing the draft and saving changes the block's schema; clearing it
    // makes the block ordinary.
    let mut drafted = sample_schema();
    drafted.parameters[0].label = "Length".into();
    editor.set_block_editor_dynamic(Some(drafted.clone()));
    let saved = doc.block_edit_save(&editor, def.id).unwrap();
    assert_eq!(saved.blocks[0].dynamic.as_deref(), Some(&drafted));
    editor.set_block_editor_dynamic(None);
    let saved = doc.block_edit_save(&editor, def.id).unwrap();
    assert!(!saved.blocks[0].is_dynamic());
    // A drawing and an editor of an ordinary block carry no draft.
    let plain = drawing_with(square_block(), None);
    let editor = plain.block_editor_document(square_block().id).unwrap();
    assert_eq!(editor.block_editor_dynamic(), None);
}

#[test]
fn a_draft_that_cannot_exist_is_refused_on_save() {
    let mut def = (*dynamic_square()).clone();
    def.base = p(0.0, 0.0);
    let def = Arc::new(def);
    let mut doc = Document::default();
    doc.push_block(def.clone());
    let mut editor = doc.block_editor_document(def.id).unwrap();
    let mut broken = sample_schema();
    broken.actions[0].parameter = ParameterId(9);
    editor.set_block_editor_dynamic(Some(broken));
    assert!(doc.block_edit_save(&editor, def.id).is_err());
}

fn paste(src: &Document, dst: &mut Document) -> crate::Absorbed {
    let ids: Vec<EntityId> = src.objects().iter().map(|o| o.id).collect();
    dst.absorb(
        &src.extract(&ids),
        &Placement::Loose { dx: 0.0, dy: 0.0 },
        None,
    )
}

#[test]
fn pasting_values_into_a_drawing_whose_block_has_another_schema_drops_them() {
    let src = drawing_with(dynamic_square(), Some(stretched()));
    // A drawing that has a block of that name, without parameters.
    let mut dst = Document::default();
    let mut ordinary = (*square_block()).clone();
    ordinary.id = dst.allocate_block_id();
    dst.push_block(Arc::new(ordinary));
    let absorbed = paste(&src, &mut dst);
    assert_eq!(
        absorbed.warnings,
        [TransferWarning::DynamicStateDropped("Square".into())]
    );
    let Some(Entity::Insert(placed)) = dst.objects().last().map(|o| &o.entity) else {
        panic!("the insert came across");
    };
    assert!(placed.dynamic.is_none());
    assert_eq!(dst.validate(), Ok(()));
    assert!(absorbed.warnings[0].to_string().contains("Square"));
}

#[test]
fn pasting_values_into_a_drawing_without_the_block_keeps_them_with_a_copy_of_its_schema() {
    let src = drawing_with(dynamic_square(), Some(stretched()));
    let mut dst = Document::default();
    let absorbed = paste(&src, &mut dst);
    assert!(absorbed.warnings.is_empty());
    let Some(Entity::Insert(placed)) = dst.objects().last().map(|o| &o.entity) else {
        panic!("the insert came across");
    };
    assert_eq!(placed.dynamic.as_deref(), Some(&stretched()));
    assert_eq!(placed.block.dynamic.as_deref(), Some(&sample_schema()));
    assert_eq!(ids(&placed.block), [0, 1], "object ids come with the block");
    assert_eq!(dst.validate(), Ok(()));
}

#[test]
fn a_selection_pasted_as_a_block_numbers_its_objects() {
    let mut src = Document::default();
    src.add(Entity::Point { at: p(0.0, 0.0) });
    src.add(Entity::Point { at: p(1.0, 1.0) });
    let mut dst = Document::default();
    let clip = src.extract(&[EntityId(0), EntityId(1)]);
    dst.absorb(
        &clip,
        &Placement::Block {
            name: "Clip".into(),
            base: p(0.0, 0.0),
            at: p(0.0, 0.0),
        },
        None,
    );
    assert_eq!(ids(&dst.blocks()[0]), [0, 1]);
    assert_eq!(dst.validate(), Ok(()));
}
