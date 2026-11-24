//! BPARAMETER and BACTION for a polar parameter, and its two rows in
//! Properties.

use opendraft_core::block::{
    DynamicActionKind, DynamicParameterKind, DynamicValue, DynamicValueSet, ParameterId,
};

use super::tests::{drafted, editing_bar, p, submit_all};
use super::*;
use crate::dynamic_rows::{rows, with_display_value};

fn add_polar(state: &mut crate::state::AppState) {
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    // Type, base and end; then name, label, description, show, the angle's
    // label, grips, and the value set of the distance and of the angle.
    submit_all(
        state,
        &["Polar", "0,0", "4,0", "", "", "", "", "", "", "", ""],
    );
}

#[test]
fn a_polar_parameter_and_a_stretch_are_drafted_saved_and_follow_the_end() {
    let mut state = editing_bar();
    add_polar(&mut state);
    let schema = drafted(&state).unwrap();
    let param = &schema.parameters[0];
    assert_eq!((param.name.as_str(), param.grips), ("Distance1", 1));
    assert!(matches!(
        &param.kind,
        DynamicParameterKind::Polar { angle_label, distance_set, angle_set, .. }
            if angle_label == "Angle1"
                && *distance_set == DynamicValueSet::Any
                && *angle_set == DynamicValueSet::Any
    ));
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["Stretch", "3,-1", "5,3", "", "", ""]);
    assert!(matches!(
        drafted(&state).unwrap().actions[0].kind,
        DynamicActionKind::Stretch { .. }
    ));
    assert!(state.bsave());
    state.bclose();
    let insert = {
        let doc = state.docs[state.active].session.document();
        let Entity::Insert(i) = &doc.objects()[0].entity else {
            panic!("no reference");
        };
        (**i).clone()
    };
    // The end of the bar goes from (4, 0) up to (4, 3): the post comes with it.
    let up = insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Point(p(4.0, 3.0)))
        .unwrap();
    assert_eq!(up.bounds().max, p(14.0, 15.0));
}

#[test]
fn a_polar_parameter_has_a_row_for_its_distance_and_one_for_its_angle() {
    let mut state = editing_bar();
    add_polar(&mut state);
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["Stretch", "3,-1", "5,3", "", "", ""]);
    assert!(state.bsave());
    state.bclose();
    let insert = {
        let doc = state.docs[state.active].session.document();
        let Entity::Insert(i) = &doc.objects()[0].entity else {
            panic!("no reference");
        };
        (**i).clone()
    };
    let shown = rows(&[&insert]);
    let labels: Vec<(&str, u8, bool)> = shown
        .iter()
        .map(|r| (r.label.as_str(), r.part, r.angle))
        .collect();
    assert_eq!(
        labels,
        [("Distance1", 0, false), ("Angle1", 1, true)],
        "{shown:?}"
    );
    assert_eq!((shown[0].value, shown[1].value), (Some(4.0), Some(0.0)));
    // Typing 90 degrees turns the end to straight up, keeping the distance.
    let turned = with_display_value(&insert, ParameterId(1), 1, 90.0).unwrap();
    let after = rows(&[&turned]);
    assert!((after[0].value.unwrap() - 4.0).abs() < 1e-9);
    assert!((after[1].value.unwrap() - 90.0).abs() < 1e-9);
    // Typing a distance keeps the angle.
    let longer = with_display_value(&insert, ParameterId(1), 0, 6.0).unwrap();
    assert!((rows(&[&longer])[0].value.unwrap() - 6.0).abs() < 1e-9);
}
