//! BPARAMETER for an XY parameter and a point parameter, and their two rows in
//! Properties.

use opendraft_core::block::{DynamicParameterKind, DynamicValue, ParameterId};

use super::tests::{drafted, editing_bar, p, submit_all};
use opendraft_core::Insert;

use super::*;
use crate::dynamic_rows::{rows, with_display_value};
use crate::state::AppState;

/// The block saved with `answers` to BPARAMETER and a move of everything.
fn saved(answers: &[&str]) -> (AppState, Insert) {
    let mut state = editing_bar();
    state.start_command("BPARAMETER", bparameter);
    state.settle_command();
    submit_all(&mut state, answers);
    state.select_all();
    state.start_command("BACTION", baction);
    state.settle_command();
    submit_all(&mut state, &["Move", "", "", ""]);
    assert!(state.bsave());
    state.bclose();
    let insert = {
        let doc = state.docs[state.active].session.document();
        let Entity::Insert(i) = &doc.objects()[0].entity else {
            panic!("no reference");
        };
        (**i).clone()
    };
    (state, insert)
}

#[test]
fn an_xy_parameter_is_drafted_and_a_move_follows_its_corner() {
    let (state, insert) = saved(&["XY", "0,0", "4,2", "", "", "", "", "", "", "", ""]);
    drop(state);
    let kind = &insert.block.dynamic.as_deref().unwrap().parameters[0].kind;
    assert!(matches!(
        kind,
        DynamicParameterKind::Xy { y_label, .. } if y_label == "Y Distance1"
    ));
    let moved = insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Point(p(6.0, 2.0)))
        .unwrap();
    let b = moved.bounds();
    assert_eq!((b.min, b.max), (p(12.0, 10.0), p(16.0, 12.0)));
    let shown = rows(&[&insert]);
    let labels: Vec<(&str, u8)> = shown.iter().map(|r| (r.label.as_str(), r.part)).collect();
    assert_eq!(labels, [("Distance1", 0), ("Y Distance1", 1)]);
    assert_eq!((shown[0].value, shown[1].value), (Some(4.0), Some(2.0)));
    // Typing a vertical distance keeps the horizontal one.
    let taller = with_display_value(&insert, ParameterId(1), 1, 5.0).unwrap();
    let after = rows(&[&taller]);
    assert_eq!((after[0].value, after[1].value), (Some(4.0), Some(5.0)));
}

#[test]
fn a_point_parameter_is_drafted_and_a_move_follows_its_location() {
    let (state, insert) = saved(&["Point", "0,0", "", "", "", "", ""]);
    drop(state);
    let kind = &insert.block.dynamic.as_deref().unwrap().parameters[0].kind;
    assert_eq!(
        *kind,
        DynamicParameterKind::Point {
            position: p(0.0, 0.0)
        }
    );
    let moved = insert
        .with_dynamic_value(ParameterId(1), DynamicValue::Point(p(1.0, 3.0)))
        .unwrap();
    let b = moved.bounds();
    assert_eq!((b.min, b.max), (p(11.0, 13.0), p(15.0, 15.0)));
    let labels: Vec<String> = rows(&[&moved]).into_iter().map(|r| r.label).collect();
    assert_eq!(labels, ["Position1 X", "Position1 Y"]);
    assert!(drafted(&editing_bar()).is_none());
}
