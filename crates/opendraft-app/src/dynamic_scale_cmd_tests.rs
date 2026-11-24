//! BACTION for a scale action.

use opendraft_core::block::{DynamicActionKind, DynamicValue, ParameterId};

use super::tests::{add_parameter, drafted, editing_bar, p, said, submit_all};
use super::*;

#[test]
fn a_scale_action_is_drafted_saved_and_enlarges_a_reference_about_the_parameters_start() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["Scale", ""]);
    let schema = drafted(&state).unwrap();
    assert_eq!(schema.actions[0].name, "Scale1");
    assert!(matches!(
        &schema.actions[0].kind,
        DynamicActionKind::Scale { objects } if objects.len() == 2
    ));
    assert!(said(&state, "Action \"Scale1\" added."));
    assert!(state.bsave());
    state.bclose();
    let insert = {
        let doc = state.docs[state.active].session.document();
        let Entity::Insert(i) = &doc.objects()[0].entity else {
            panic!("no reference");
        };
        (**i).clone()
    };
    // The parameter is 4 long, so a value of 8 doubles everything.
    let doubled = insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Distance(8.0))
        .unwrap();
    let b = doubled.bounds();
    assert_eq!((b.min, b.max), (p(10.0, 10.0), p(18.0, 14.0)));
}
