//! BPARAMETER for a lookup parameter and BLOOKUPTABLE, and the saved block
//! taking a row's values.

use opendraft_core::block::{DynamicParameterKind, LookupRow, ParameterId};
use opendraft_core::Entity;

use super::tests::{add_parameter, drafted, editing_bar, said, submit_all};
use crate::dynamic_block_cmd::bparameter;
use crate::dynamic_lookup_cmd::blookuptable;
use crate::state::AppState;

fn rows(state: &AppState) -> Vec<LookupRow> {
    let schema = drafted(state).unwrap();
    let DynamicParameterKind::Lookup { rows, .. } = &schema.parameters[1].kind else {
        panic!("a lookup parameter");
    };
    rows.clone()
}

fn add_lookup(state: &mut AppState) {
    add_parameter(state);
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    // Type, location, name, label, description, show, the one input is taken
    // by itself, then the first row.
    submit_all(
        state,
        &["Lookup", "0,2", "", "", "", "", "Distance1", "Short", "2"],
    );
}

fn table(state: &mut AppState, answers: &[&str]) {
    state.start_command("BLOOKUPTABLE", blookuptable);
    state.settle_command();
    submit_all(state, answers);
}

#[test]
fn a_lookup_needs_a_parameter_to_take_values_from() {
    let mut state = editing_bar();
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    submit_all(&mut state, &["Lookup"]);
    assert!(said(&state, "linear, rotation or flip parameter"));
    assert!(drafted(&state).is_none());
    table(&mut state, &[]);
    assert!(said(&state, "lookup parameter with BPARAMETER first"));
}

#[test]
fn a_lookup_parameter_starts_with_its_first_row() {
    let mut state = editing_bar();
    add_lookup(&mut state);
    let schema = drafted(&state).unwrap();
    assert_eq!(schema.parameters[1].name, "Lookup1");
    let DynamicParameterKind::Lookup { inputs, .. } = &schema.parameters[1].kind else {
        panic!("a lookup parameter");
    };
    assert_eq!(inputs, &[ParameterId(1)]);
    assert_eq!(
        rows(&state),
        [LookupRow {
            label: "Short".into(),
            values: vec![2.0]
        }]
    );
}

#[test]
fn rows_are_added_listed_and_deleted_and_bad_ones_are_asked_again() {
    let mut state = editing_bar();
    add_lookup(&mut state);
    table(&mut state, &["Add", "short"]);
    assert!(said(&state, "already used"));
    submit_all(&mut state, &["Long", "6"]);
    assert_eq!(rows(&state).len(), 2);
    // The same values twice cannot be told apart.
    table(&mut state, &["Add", "Twin", "6"]);
    assert!(said(&state, "two rows with the same values"));
    assert_eq!(rows(&state).len(), 2);
    table(&mut state, &["List"]);
    assert!(said(&state, "Short: Distance1=2; Long: Distance1=6"));
    table(&mut state, &["Delete", "Short"]);
    assert_eq!(rows(&state)[0].label, "Long");
    table(&mut state, &["Delete"]);
    assert!(said(&state, "at least one row"));
}

#[test]
fn a_saved_block_takes_the_values_of_the_row_picked() {
    let mut state = editing_bar();
    add_lookup(&mut state);
    table(&mut state, &["Add", "Long", "6"]);
    assert!(state.bsave());
    state.bclose();
    let doc = state.docs[state.active].session.document();
    let Entity::Insert(i) = &doc.objects()[0].entity else {
        panic!("no reference");
    };
    let long = i.with_choice(ParameterId(2), 1).unwrap();
    assert_eq!(long.dynamic_value(ParameterId(1)), Some(6.0));
    assert_eq!(long.choice(ParameterId(2)).unwrap().1, 1);
}
