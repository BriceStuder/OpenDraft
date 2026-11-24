use std::sync::Arc;

use opendraft_commands::SetBlocks;
use opendraft_core::block::{
    ActionId, BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, DynamicParameter,
    DynamicParameterKind, DynamicValue, DynamicValueSet,
};
use opendraft_core::{
    Attributes, BlockDef, BlockEntity, BlockId, Bounds, Entity, EntityId, LayerId, Point,
};

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn parameter(id: u64, name: &str, show: bool, set: DynamicValueSet) -> DynamicParameter {
    DynamicParameter {
        id: ParameterId(id),
        name: name.into(),
        label: String::new(),
        description: String::new(),
        show,
        grips: 1,
        kind: DynamicParameterKind::Linear {
            start: p(0.0, 0.0),
            end: p(4.0, 0.0),
            value_set: set,
        },
    }
}

/// A bar 4 long with a post at its end, and the block of it with a shown
/// parameter `Distance1` (set `set`), a hidden one, and a Stretch.
fn block(id: u64, set: DynamicValueSet) -> Arc<BlockDef> {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(parameter(1, "Distance1", true, set));
    schema
        .parameters
        .push(parameter(2, "Hidden", false, DynamicValueSet::Any));
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
    schema.next_parameter_id = 3;
    schema.next_action_id = 2;
    let line = |id: u64, a: Point, b: Point| {
        BlockEntity::new(
            BlockObjectId(id),
            Entity::Line { start: a, end: b },
            Attributes::on_layer(LayerId::DEFAULT),
        )
    };
    Arc::new(BlockDef {
        id: BlockId(id),
        name: format!("Bar{id}"),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: vec![
            line(0, p(0.0, 0.0), p(4.0, 0.0)),
            line(1, p(4.0, 0.0), p(4.0, 2.0)),
        ],
        dynamic: Some(Box::new(schema)),
    })
}

fn insert(block: &Arc<BlockDef>, at: f64, scale: f64) -> Insert {
    let mut i = Insert::new(block.clone(), p(at, 0.0));
    i.scale_x = scale;
    i.scale_y = scale;
    i
}

/// A drawing with the block and the inserts, all selected; their ids.
fn drawing(block: &Arc<BlockDef>, inserts: Vec<Insert>) -> (AppState, Vec<EntityId>) {
    let mut state = AppState::new();
    state.perform(SetBlocks::new("Block", vec![block.clone()]));
    for i in inserts {
        state.add_entity(Entity::Insert(Box::new(i)));
    }
    state.select_all();
    let ids = state.selection().iter().copied().collect();
    (state, ids)
}

fn insert_of(state: &AppState, id: EntityId) -> Insert {
    match state.session().document().get(id) {
        Some(Entity::Insert(i)) => (**i).clone(),
        _ => panic!("an insert"),
    }
}

#[test]
fn a_dynamic_reference_lists_its_shown_parameters_in_drawing_units() {
    let b = block(1, DynamicValueSet::Any);
    let r = rows(&[&insert(&b, 0.0, 2.0)]);
    assert_eq!(r.len(), 1, "the hidden parameter is not listed");
    assert_eq!((r[0].label.as_str(), r[0].value), ("Distance1", Some(8.0)));
    let mut plain = (*b).clone();
    plain.dynamic = None;
    assert!(rows(&[&Insert::new(Arc::new(plain), p(0.0, 0.0))]).is_empty());
}

#[test]
fn several_references_say_varies_when_their_values_differ_and_need_one_block() {
    let b = block(1, DynamicValueSet::Any);
    let a = insert(&b, 0.0, 1.0);
    let c = a
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(6.0))
        .unwrap();
    assert_eq!(rows(&[&a, &a.clone()])[0].value, Some(4.0));
    assert_eq!(rows(&[&a, &c])[0].value, None, "Varies");
    // The same length at another scale is the same value in the drawing.
    let big = insert(&b, 0.0, 2.0)
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(2.0))
        .unwrap();
    assert_eq!(rows(&[&a, &big])[0].value, Some(4.0));
    let other = block(2, DynamicValueSet::Any);
    assert!(rows(&[&a, &insert(&other, 0.0, 1.0)]).is_empty());
}

#[test]
fn a_typed_length_is_divided_by_the_scale_and_brought_into_the_value_set() {
    let b = block(1, DynamicValueSet::List(vec![2.0, 4.0, 8.0]));
    let two = insert(&b, 0.0, 2.0);
    let made = with_display_value(&two, ParameterId(1), 0, 14.0).unwrap();
    assert_eq!(
        made.dynamic_value(ParameterId(1)),
        Some(8.0),
        "7 snaps to 8"
    );
    assert_eq!(
        with_display_value(&two, ParameterId(1), 0, 8.0),
        None,
        "no change"
    );
    assert_eq!(with_display_value(&two, ParameterId(9), 0, 8.0), None);
    assert_eq!(with_display_value(&two, ParameterId(1), 0, f64::NAN), None);
}

#[test]
fn editing_in_the_panel_changes_the_selected_references_in_one_undo_step() {
    let b = block(1, DynamicValueSet::Any);
    let (mut state, ids) = drawing(&b, vec![insert(&b, 0.0, 1.0), insert(&b, 20.0, 2.0)]);
    state.set_dynamic_value(ParameterId(1), 0, 10.0, None);
    // The scale-2 reference holds 5, the other 10: both show 10.
    let values: Vec<_> = ids
        .iter()
        .map(|&id| insert_of(&state, id).dynamic_value(ParameterId(1)))
        .collect();
    assert_eq!(values, [Some(10.0), Some(5.0)]);
    assert_eq!(state.session().document().validate(), Ok(()));
    let before = state.session().revision();
    state.set_dynamic_value(ParameterId(1), 0, 10.0, None);
    state.set_dynamic_value(ParameterId(1), 0, 3.0, Some(EntityKind::Line));
    assert_eq!(state.session().revision(), before, "no change, no step");
    assert_eq!(state.undo().as_deref(), Some("Edit dynamic property"));
    assert_eq!(insert_of(&state, ids[0]).dynamic, None);
}

#[test]
fn a_flattened_reference_is_left_alone() {
    let b = block(1, DynamicValueSet::Any);
    let mut flat = insert(&b, 0.0, 1.0);
    flat.scale_y = 1e-30;
    flat.scale_x = 1e-30;
    assert_eq!(flat.dynamic_scale(ParameterId(1)), None);
    assert_eq!(with_display_value(&flat, ParameterId(1), 0, 5.0), None);
}

/// A line turned about its start by a rotation parameter.
fn turning_line() -> Insert {
    let mut schema = DynamicBlock::new();
    schema.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Angle1".into(),
        label: "Turn".into(),
        description: String::new(),
        show: true,
        grips: 1,
        kind: DynamicParameterKind::Rotation {
            base: p(0.0, 0.0),
            radius: 1.0,
            value_set: DynamicValueSet::Any,
        },
    });
    schema.actions.push(DynamicAction {
        id: ActionId(1),
        name: "Rotate1".into(),
        parameter: ParameterId(1),
        kind: DynamicActionKind::Rotate {
            objects: vec![BlockObjectId(0)],
        },
    });
    schema.next_parameter_id = 2;
    schema.next_action_id = 2;
    let block = BlockDef {
        id: BlockId(3),
        name: "Turn".into(),
        base: p(0.0, 0.0),
        description: String::new(),
        unit: Default::default(),
        entities: vec![BlockEntity::new(
            BlockObjectId(0),
            Entity::Line {
                start: p(0.0, 0.0),
                end: p(2.0, 0.0),
            },
            Attributes::on_layer(LayerId::DEFAULT),
        )],
        dynamic: Some(Box::new(schema)),
    };
    Insert::new(Arc::new(block), p(5.0, 5.0))
}

#[test]
fn an_angle_row_reads_and_takes_degrees() {
    let insert = turning_line();
    let shown = rows(&[&insert]);
    assert_eq!(shown.len(), 1);
    assert!(shown[0].angle);
    assert_eq!(shown[0].value, Some(0.0));
    let turned = with_display_value(&insert, ParameterId(1), 0, 90.0).unwrap();
    let row = rows(&[&turned]);
    assert!((row[0].value.unwrap() - 90.0).abs() < 1e-9);
    assert!(
        (turned.bounds().max.y - 7.0).abs() < 1e-9,
        "{:?}",
        turned.bounds()
    );
    // Typing 450 is the same turn as 90.
    let wrapped = with_display_value(&insert, ParameterId(1), 0, 450.0).unwrap();
    assert!((rows(&[&wrapped])[0].value.unwrap() - 90.0).abs() < 1e-9);
    // Typing what it already is changes nothing.
    assert!(with_display_value(&insert, ParameterId(1), 0, 0.0).is_none());
}

#[test]
fn a_flip_row_is_a_yes_or_no_that_takes_one_or_zero() {
    let turning = turning_line();
    let mut block = (*turning.block).clone();
    let schema = block.dynamic.as_mut().unwrap();
    schema.parameters[0].kind = DynamicParameterKind::Flip {
        base: p(0.0, 0.0),
        end: p(0.0, 1.0),
    };
    schema.actions[0].kind = DynamicActionKind::Flip {
        objects: vec![BlockObjectId(0)],
    };
    let insert = Insert::new(Arc::new(block), p(5.0, 5.0));
    let shown = rows(&[&insert]);
    assert!(shown[0].flag && !shown[0].angle);
    assert_eq!(shown[0].value, Some(0.0));
    let yes = with_display_value(&insert, ParameterId(1), 0, 1.0).unwrap();
    assert_eq!(rows(&[&yes])[0].value, Some(1.0));
    // The line (0,0)-(2,0) mirrors about x = 0 into (0,0)-(-2,0), moved by 5.
    assert!(
        (yes.bounds().min.x - 3.0).abs() < 1e-9,
        "{:?}",
        yes.bounds()
    );
    assert!(with_display_value(&insert, ParameterId(1), 0, 0.0).is_none());
}

/// Two lines; "Plain" hides the second and "Alt" the first.
fn alternatives() -> Arc<BlockDef> {
    use opendraft_core::block::VisibilityState;
    let mut schema = DynamicBlock::new();
    schema.parameters.push(DynamicParameter {
        id: ParameterId(1),
        name: "Visibility1".into(),
        label: "Shape".into(),
        description: String::new(),
        show: true,
        grips: 0,
        kind: DynamicParameterKind::Visibility {
            position: p(0.0, 0.0),
            states: vec![
                VisibilityState {
                    name: "Plain".into(),
                    hidden: vec![BlockObjectId(1)],
                },
                VisibilityState {
                    name: "Alt".into(),
                    hidden: vec![BlockObjectId(0)],
                },
            ],
        },
    });
    schema.next_parameter_id = 2;
    let mut def = (*block(9, DynamicValueSet::Any)).clone();
    def.dynamic = Some(Box::new(schema));
    Arc::new(def)
}

#[test]
fn a_visibility_parameter_is_a_row_of_choices_and_picking_one_changes_the_state() {
    let block = alternatives();
    let plain = Insert::new(block.clone(), p(0.0, 0.0));
    let one = rows(&[&plain]);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].label, "Shape");
    assert_eq!(one[0].states, ["Plain", "Alt"]);
    assert_eq!(one[0].value, Some(0.0));
    // Picking the second state; picking the one already shown changes nothing.
    let alt = with_display_value(&plain, ParameterId(1), 0, 1.0).unwrap();
    assert_eq!(alt.visibility_state().map(|s| s.1), Some("Alt"));
    assert!(with_display_value(&plain, ParameterId(1), 0, 0.0).is_none());
    assert!(with_display_value(&plain, ParameterId(1), 0, 5.0).is_none());
    // References in different states vary.
    let both = rows(&[&plain, &alt]);
    assert_eq!(both[0].value, None);
}
