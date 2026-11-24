//! BPARAMETER for a visibility parameter, BVSTATE, BVHIDE and BVSHOW, and the
//! saved block showing the state a reference is given.

use opendraft_core::block::{DynamicParameterKind, VisibilityState};
use opendraft_core::Entity;

use super::tests::{drafted, editing_bar, said, submit_all};
use crate::dynamic_block_cmd::bparameter;
use crate::dynamic_visibility_cmd::{bvhide, bvshow, bvstate};
use crate::state::AppState;

fn states(state: &AppState) -> Vec<VisibilityState> {
    let schema = drafted(state).unwrap();
    let DynamicParameterKind::Visibility { states, .. } = &schema.parameters[0].kind else {
        panic!("a visibility parameter");
    };
    states.clone()
}

fn add_visibility(state: &mut AppState) {
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    // Type, location, then name, label, description and show as they are.
    submit_all(state, &["Visibility", "0,0", "", "", "", ""]);
}

fn command(
    state: &mut AppState,
    name: &'static str,
    run: fn(&mut opendraft_commands::Editor) -> Result<(), opendraft_commands::Cancelled>,
    answers: &[&str],
) {
    state.start_command(name, run);
    state.settle_command();
    submit_all(state, answers);
}

#[test]
fn a_visibility_parameter_starts_with_one_state_that_hides_nothing() {
    let mut state = editing_bar();
    add_visibility(&mut state);
    let schema = drafted(&state).unwrap();
    assert_eq!(schema.parameters[0].name, "Visibility1");
    let shown = states(&state);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].name, "VisibilityState0");
    assert!(shown[0].hidden.is_empty());
    assert!(said(&state, "Parameter \"Visibility1\" added."));
}

#[test]
fn only_one_visibility_parameter_is_allowed() {
    let mut state = editing_bar();
    add_visibility(&mut state);
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    submit_all(&mut state, &["Visibility"]);
    assert!(said(&state, "visibility parameter already"));
    assert_eq!(drafted(&state).unwrap().parameters.len(), 1);
}

#[test]
fn states_are_added_renamed_and_deleted_and_names_are_not_repeated() {
    let mut state = editing_bar();
    command(&mut state, "BVSTATE", bvstate, &[]);
    assert!(said(&state, "visibility parameter with BPARAMETER first"));
    add_visibility(&mut state);
    command(
        &mut state,
        "BVSTATE",
        bvstate,
        &["New", "Round", "Show all"],
    );
    command(&mut state, "BVSTATE", bvstate, &["New", "round"]);
    assert!(said(&state, "already used"));
    submit_all(&mut state, &["Square", "Hide all"]);
    let now = states(&state);
    assert_eq!(
        now.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["VisibilityState0", "Round", "Square"]
    );
    assert!(now[1].hidden.is_empty());
    assert_eq!(now[2].hidden.len(), 2, "Hide all hides both lines");
    command(
        &mut state,
        "BVSTATE",
        bvstate,
        &["Rename", "Round", "Circle"],
    );
    assert_eq!(states(&state)[1].name, "Circle");
    command(&mut state, "BVSTATE", bvstate, &["Delete", "Square"]);
    assert_eq!(states(&state).len(), 2);
}

#[test]
fn the_last_state_cannot_be_deleted() {
    let mut state = editing_bar();
    add_visibility(&mut state);
    command(&mut state, "BVSTATE", bvstate, &["Delete"]);
    assert!(said(&state, "at least one state"));
    assert_eq!(states(&state).len(), 1);
}

#[test]
fn hiding_and_showing_objects_in_a_state_and_in_all_of_them() {
    let mut state = editing_bar();
    add_visibility(&mut state);
    command(
        &mut state,
        "BVSTATE",
        bvstate,
        &["New", "Round", "Show all"],
    );
    state.select_all();
    command(&mut state, "BVHIDE", bvhide, &["Round"]);
    let now = states(&state);
    assert!(now[0].hidden.is_empty());
    assert_eq!(now[1].hidden.len(), 2);
    state.select_all();
    command(&mut state, "BVSHOW", bvshow, &["Round"]);
    assert!(states(&state)[1].hidden.is_empty());
    state.select_all();
    command(&mut state, "BVHIDE", bvhide, &["All"]);
    assert!(states(&state).iter().all(|s| s.hidden.len() == 2));
    assert!(said(&state, "hidden in all states"));
}

#[test]
fn a_saved_block_shows_the_state_a_reference_is_given() {
    let mut state = editing_bar();
    add_visibility(&mut state);
    command(&mut state, "BVSTATE", bvstate, &["New", "Bare", "Hide all"]);
    assert!(state.bsave());
    state.bclose();
    let insert = {
        let doc = state.docs[state.active].session.document();
        let Entity::Insert(i) = &doc.objects()[0].entity else {
            panic!("no reference");
        };
        (**i).clone()
    };
    assert_eq!(insert.evaluated().len(), 2);
    let bare = insert.with_visibility_state("Bare").unwrap();
    assert!(bare.evaluated().is_empty());
    assert_eq!(bare.visibility_state().map(|s| s.1), Some("Bare"));
}
