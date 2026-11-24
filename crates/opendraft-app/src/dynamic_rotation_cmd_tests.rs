//! BPARAMETER and BACTION for a rotation parameter and the rotate action.

use opendraft_core::block::{
    DynamicActionKind, DynamicParameterKind, DynamicValue, DynamicValueSet, ParameterId,
};

use super::tests::{add_parameter, drafted, editing_bar, p, said, submit_all};
use super::*;
use crate::state::AppState;

fn add_rotation(state: &mut AppState, answers: &[&str]) {
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    submit_all(state, &["Rotation", "0,0", "4"]);
    submit_all(state, answers);
}

#[test]
fn a_rotation_parameter_and_action_are_drafted_saved_and_turn_a_reference() {
    let mut state = editing_bar();
    add_rotation(&mut state, &["", "", "", "", "", ""]);
    let schema = drafted(&state).unwrap();
    let param = &schema.parameters[0];
    assert_eq!(
        (param.name.as_str(), param.label.as_str(), param.grips),
        ("Angle1", "Angle1", 1)
    );
    assert_eq!(
        param.kind,
        DynamicParameterKind::Rotation {
            base: p(0.0, 0.0),
            radius: 4.0,
            value_set: DynamicValueSet::Any,
        }
    );
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["Rotate", ""]);
    let schema = drafted(&state).unwrap();
    assert_eq!(schema.actions[0].name, "Rotate1");
    assert!(matches!(
        &schema.actions[0].kind,
        DynamicActionKind::Rotate { objects } if objects.len() == 2
    ));
    assert!(said(&state, "Action \"Rotate1\" added."));
    assert!(state.bsave());
    state.bclose();
    let insert = {
        let doc = state.docs[state.active].session.document();
        let Entity::Insert(i) = &doc.objects()[0].entity else {
            panic!("no reference");
        };
        (**i).clone()
    };
    let turned = insert
        .with_dynamic_value(
            ParameterId(1),
            DynamicValue::Angle(std::f64::consts::FRAC_PI_2),
        )
        .unwrap();
    let b = turned.bounds();
    assert!(
        (b.max.x - 10.0).abs() < 1e-9 && (b.max.y - 14.0).abs() < 1e-9,
        "{b:?}"
    );
    assert!((b.min.x - 8.0).abs() < 1e-9, "{b:?}");
}

#[test]
fn angle_values_are_typed_in_degrees_and_kept_in_radians() {
    let mut state = editing_bar();
    add_rotation(&mut state, &["", "", "", "", "", "List", "90, 180"]);
    let DynamicParameterKind::Rotation { value_set, .. } =
        &drafted(&state).unwrap().parameters[0].kind
    else {
        panic!("a rotation");
    };
    // The block as drawn (0) is always one of the choices.
    assert_eq!(
        *value_set,
        DynamicValueSet::List(vec![0.0, std::f64::consts::FRAC_PI_2, std::f64::consts::PI])
    );
}

#[test]
fn an_action_needs_the_kind_of_parameter_it_follows() {
    let mut state = editing_bar();
    add_parameter(&mut state);
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["Rotate"]);
    assert!(said(&state, "needs a rotation or polar parameter"));
    assert!(drafted(&state).unwrap().actions.is_empty());

    let mut state = editing_bar();
    add_rotation(&mut state, &["", "", "", "", "", ""]);
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["Stretch"]);
    assert!(said(&state, "needs a linear, polar, XY or point parameter"));
}
