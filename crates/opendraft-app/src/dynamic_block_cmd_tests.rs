use opendraft_core::block::{
    DynamicActionKind, DynamicParameterKind, DynamicValue, DynamicValueSet, ParameterId,
};
use opendraft_core::Point;

use super::*;
use crate::block_cmd::define_block;
use crate::state::AppState;

pub(super) fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

pub(super) fn submit_all(state: &mut AppState, answers: &[&str]) {
    for answer in answers {
        state.cmd_submit(answer);
        state.settle_command();
    }
}

pub(super) fn said(state: &AppState, text: &str) -> bool {
    state.cmdline.log.iter().any(|l| l.contains(text))
}

/// A drawing with block "Bar": a bar from (10,10) to (14,10) and a post at
/// its right end, base point (10,10), made into one reference, with the
/// Block Editor open on it.
pub(super) fn editing_bar() -> AppState {
    let mut state = AppState::new();
    state.add_entity(Entity::Line {
        start: p(10.0, 10.0),
        end: p(14.0, 10.0),
    });
    state.add_entity(Entity::Line {
        start: p(14.0, 10.0),
        end: p(14.0, 12.0),
    });
    state.select_all();
    state.start_command("BLOCK", define_block);
    state.settle_command();
    submit_all(&mut state, &["Bar", "10,10"]);
    state.select_all();
    state.bedit();
    assert!(state.in_block_editor());
    state
}

pub(super) fn drafted(state: &AppState) -> Option<DynamicBlock> {
    state.docs[state.active]
        .session
        .document()
        .block_editor_dynamic()
        .cloned()
}

pub(super) fn add_parameter(state: &mut AppState) {
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    submit_all(state, &["", "0,0", "4,0", "", "", "", "", "", ""]);
}

fn add_stretch(state: &mut AppState) {
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(state, &["Stretch", "3,-1", "5,3", "", "", ""]);
}

#[test]
fn a_parameter_takes_its_defaults_and_is_one_undo_step_of_the_editor() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    let schema = drafted(&state).unwrap();
    let param = &schema.parameters[0];
    assert_eq!(
        (param.name.as_str(), param.label.as_str(), param.grips),
        ("Distance1", "Distance1", 1)
    );
    assert!(param.show && param.description.is_empty());
    let DynamicParameterKind::Linear {
        start,
        end,
        value_set,
    } = &param.kind
    else {
        panic!("a linear parameter");
    };
    assert_eq!(
        (*start, *end, value_set),
        (p(0.0, 0.0), p(4.0, 0.0), &DynamicValueSet::Any)
    );
    assert!(said(&state, "Parameter \"Distance1\" added."));
    assert_eq!(state.undo().as_deref(), Some("Add parameter"));
    assert_eq!(drafted(&state), None);
}

#[test]
fn a_second_parameter_gets_the_next_free_name_and_a_taken_name_is_asked_again() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    submit_all(&mut state, &["", "0,1", "3,1", "distance1", "Width"]);
    assert!(said(&state, "already used"));
    submit_all(&mut state, &["", "", "", "", ""]);
    let names: Vec<String> = drafted(&state)
        .unwrap()
        .parameters
        .iter()
        .map(|p| p.name.clone())
        .collect();
    assert_eq!(names, ["Distance1", "Width"]);
}

#[test]
fn a_parameter_with_no_length_two_grips_or_a_bad_list_is_asked_again() {
    let mut state = editing_bar();
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    submit_all(&mut state, &["", "0,0", "0,0"]);
    assert!(said(&state, "no length"));
    submit_all(&mut state, &["4,0", "", "", "", "", "2"]);
    assert!(said(&state, "Two grips are not available yet"));
    submit_all(&mut state, &["0", "list", "2, x"]);
    assert!(said(&state, "\"x\" is not a number"));
    submit_all(&mut state, &["8, 2, 8"]);
    assert!(!state.command_running());
    let schema = drafted(&state).unwrap();
    assert_eq!(schema.parameters[0].grips, 0);
    // Sorted, without repeats, and holding the parameter's own length.
    assert_eq!(
        schema.parameters[0].value_set(),
        &DynamicValueSet::List(vec![2.0, 4.0, 8.0])
    );
}

#[test]
fn an_increment_value_set_is_checked() {
    let mut state = editing_bar();
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    submit_all(
        &mut state,
        &["", "0,0", "4,0", "", "", "", "", "", "increment"],
    );
    submit_all(&mut state, &["5", "1", "1"]);
    assert!(said(&state, "cannot be above the maximum"));
    submit_all(&mut state, &["1", "9", "2"]);
    assert_eq!(
        drafted(&state).unwrap().parameters[0].value_set(),
        &DynamicValueSet::Range {
            min: 1.0,
            max: 9.0,
            increment: Some(2.0)
        }
    );
}

#[test]
fn escape_leaves_the_draft_as_it_was() {
    let mut state = editing_bar();
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    submit_all(&mut state, &["", "0,0", "4,0"]);
    state.abort_command();
    assert_eq!(drafted(&state), None);
}

#[test]
fn an_action_needs_a_parameter_first() {
    let mut state = editing_bar();
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    assert!(said(&state, "Add a parameter with BPARAMETER first."));
    assert!(!state.command_running());
}

#[test]
fn a_stretch_action_takes_the_selection_and_the_frame() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    add_stretch(&mut state);
    let schema = drafted(&state).unwrap();
    let action = &schema.actions[0];
    assert_eq!(action.name, "Stretch1");
    assert_eq!(action.parameter, schema.parameters[0].id);
    let DynamicActionKind::Stretch {
        objects,
        frame,
        multiplier,
        angle_offset,
    } = &action.kind
    else {
        panic!("not a stretch");
    };
    assert_eq!(objects.len(), 2);
    assert_eq!((frame.min, frame.max), (p(3.0, -1.0), p(5.0, 3.0)));
    assert_eq!((*multiplier, *angle_offset), (1.0, 0.0));
    assert!(said(&state, "Action \"Stretch1\" added."));
}

#[test]
fn a_flat_frame_and_a_bad_multiplier_are_asked_again() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["Stretch", "3,0", "5,0"]);
    assert!(said(&state, "no width or no height"));
    submit_all(&mut state, &["5,3", "0"]);
    assert!(said(&state, "must be above zero"));
    submit_all(&mut state, &["2", "90", "Wide"]);
    let schema = drafted(&state).unwrap();
    let DynamicActionKind::Stretch {
        multiplier,
        angle_offset,
        ..
    } = &schema.actions[0].kind
    else {
        panic!("not a stretch");
    };
    assert_eq!(*multiplier, 2.0);
    assert!((angle_offset - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert_eq!(schema.actions[0].name, "Wide");
}

#[test]
fn a_move_action_and_bactionset_change_which_objects_an_action_acts_on() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["", "", "", ""]);
    let first = drafted(&state).unwrap();
    assert_eq!(first.actions[0].name, "Move1");
    // Select only the post, then give it to the action.
    let post = state.docs[state.active].session.document().objects()[1].id;
    state.docs[state.active].selection.clear();
    state.select_ids([post], false);
    state.start_command("BACTIONSET", bactionset);
    state.settle_command();
    assert!(!state.command_running());
    let schema = drafted(&state).unwrap();
    let (DynamicActionKind::Move { objects, .. }
    | DynamicActionKind::Stretch { objects, .. }
    | DynamicActionKind::Rotate { objects }) = &schema.actions[0].kind;
    assert_eq!(objects, &[BlockObjectId(post.0)]);
}

#[test]
fn saving_makes_the_block_dynamic_and_a_reference_follows_its_values() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    add_stretch(&mut state);
    assert!(state.bsave());
    state.bclose();
    assert!(!state.in_block_editor());
    let insert = {
        let doc = state.docs[state.active].session.document();
        let Entity::Insert(i) = &doc.objects()[0].entity else {
            panic!("no reference");
        };
        assert!(i.block.is_dynamic());
        (**i).clone()
    };
    assert_eq!(insert.bounds().max, p(14.0, 12.0));
    let longer = insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(7.0))
        .unwrap();
    assert_eq!(longer.bounds().max, p(17.0, 12.0));
    assert_eq!(
        state.docs[state.active].session.document().validate(),
        Ok(())
    );
}

#[test]
fn resetblock_puts_references_back_and_is_one_undo_step() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    add_stretch(&mut state);
    assert!(state.bsave());
    state.bclose();
    let (id, stretched) = {
        let doc = state.docs[state.active].session.document();
        let Entity::Insert(i) = &doc.objects()[0].entity else {
            panic!("no reference");
        };
        let longer = i
            .with_dynamic_value(ParameterId(1), DynamicValue::Distance(7.0))
            .unwrap();
        (doc.objects()[0].id, Entity::Insert(Box::new(longer)))
    };
    state.docs[state.active]
        .session
        .perform(ModifyEntities::new("Stretch", vec![(id, stretched)]));
    state.select_all();
    state.start_command("RESETBLOCK", resetblock);
    state.settle_command();
    assert!(said(&state, "Reset 1 dynamic block reference."));
    let doc = state.docs[state.active].session.document();
    let Entity::Insert(i) = &doc.objects()[0].entity else {
        panic!("no reference");
    };
    assert_eq!(i.dynamic, None);
    assert_eq!(state.undo().as_deref(), Some("Reset dynamic blocks"));
}

#[test]
fn resetblock_ignores_references_that_are_not_dynamic() {
    let mut state = AppState::new();
    state.add_entity(Entity::Line {
        start: p(0.0, 0.0),
        end: p(1.0, 0.0),
    });
    state.select_all();
    state.start_command("RESETBLOCK", resetblock);
    state.settle_command();
    assert!(said(&state, "No dynamic block references selected."));
}

#[test]
fn block_will_not_redefine_a_dynamic_block() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    add_stretch(&mut state);
    assert!(state.bsave());
    state.bclose();
    state.select_all();
    state.start_command("-BLOCK", define_block);
    state.settle_command();
    submit_all(&mut state, &["Bar", "Yes", "0,0"]);
    assert!(said(&state, "is a dynamic block"));
}

#[test]
fn bsave_reports_the_references_it_adjusted_and_the_actions_it_removed() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    add_stretch(&mut state);
    assert!(state.bsave());
    state.bclose();
    let (id, stretched) = {
        let doc = state.docs[state.active].session.document();
        let Entity::Insert(i) = &doc.objects()[0].entity else {
            panic!("no reference");
        };
        let longer = i
            .with_dynamic_value(ParameterId(1), DynamicValue::Distance(7.0))
            .unwrap();
        (doc.objects()[0].id, Entity::Insert(Box::new(longer)))
    };
    state.docs[state.active]
        .session
        .perform(ModifyEntities::new("Stretch", vec![(id, stretched)]));
    state.select_all();
    state.bedit();
    let mut draft = drafted(&state).unwrap();
    let DynamicParameterKind::Linear { value_set, .. } = &mut draft.parameters[0].kind else {
        panic!("a linear parameter");
    };
    *value_set = DynamicValueSet::List(vec![2.0, 4.0, 8.0]);
    state.docs[state.active]
        .session
        .perform(SetBlockEditorDynamic::new("Parameter", Some(draft)));
    assert!(state.bsave());
    assert!(said(&state, "1 reference adjusted"));
    let doc = state.docs[state.active].drawing().document();
    let Entity::Insert(i) = &doc.objects()[0].entity else {
        panic!("no reference");
    };
    assert_eq!(i.dynamic_value(ParameterId(1)), Some(8.0));
}
