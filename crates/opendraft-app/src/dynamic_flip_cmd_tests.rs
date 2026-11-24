//! BPARAMETER and BACTION for a flip parameter and the flip action, and its
//! yes-or-no row in Properties.

use opendraft_core::block::{DynamicActionKind, DynamicParameterKind, DynamicValue, ParameterId};

use super::tests::{add_parameter, drafted, editing_bar, p, said, submit_all};
use super::*;

fn add_flip(state: &mut crate::state::AppState) {
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    // Type, the mirror line (up through the base point), then name, label,
    // description, show and grips left as they are.
    submit_all(state, &["Flip", "0,0", "0,4", "", "", "", "", ""]);
}

#[test]
fn a_flip_parameter_and_action_are_drafted_saved_and_mirror_a_reference() {
    let mut state = editing_bar();
    add_flip(&mut state);
    let schema = drafted(&state).unwrap();
    let param = &schema.parameters[0];
    assert_eq!((param.name.as_str(), param.grips), ("Flip1", 1));
    assert_eq!(
        param.kind,
        DynamicParameterKind::Flip {
            base: p(0.0, 0.0),
            end: p(0.0, 4.0)
        }
    );
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["Flip", ""]);
    let schema = drafted(&state).unwrap();
    assert!(matches!(
        &schema.actions[0].kind,
        DynamicActionKind::Flip { objects } if objects.len() == 2
    ));
    assert!(said(&state, "Action \"Flip1\" added."));
    assert!(state.bsave());
    state.bclose();
    let insert = {
        let doc = state.docs[state.active].session.document();
        let Entity::Insert(i) = &doc.objects()[0].entity else {
            panic!("no reference");
        };
        (**i).clone()
    };
    assert_eq!(insert.bounds().max, p(14.0, 12.0));
    let mirrored = insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Bool(true))
        .unwrap();
    let b = mirrored.bounds();
    assert!(
        (b.min.x - 6.0).abs() < 1e-9 && (b.max.x - 10.0).abs() < 1e-9,
        "{b:?}"
    );
}

#[test]
fn a_flip_action_needs_a_flip_parameter_and_a_flip_has_a_line() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["Flip"]);
    assert!(said(&state, "needs a flip parameter"));

    let mut state = editing_bar();
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    submit_all(&mut state, &["Flip", "1,1", "1,1"]);
    assert!(said(&state, "no length"));
    submit_all(&mut state, &["1,5", "", "", "", "", ""]);
    let kind = &drafted(&state).unwrap().parameters[0].kind;
    assert!(matches!(kind, DynamicParameterKind::Flip { .. }));
}
