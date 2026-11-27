use super::tests::{at, click, enter, entities, fresh, key, p, polyline, said, started};
use super::*;
use crate::state::AppState;
use opendraft_core::Spline;

fn zigzag() -> Entity {
    polyline(&[(0.0, 0.0), (4.0, 3.0), (8.0, 0.0), (12.0, 3.0)])
}

fn straight() -> Entity {
    polyline(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)])
}

fn points_of(state: &AppState) -> usize {
    match &entities(state)[0] {
        Entity::Polyline { points, .. } => points.len(),
        other => panic!("a polyline, got {other:?}"),
    }
}

fn arcs_in(state: &AppState) -> bool {
    match &entities(state)[0] {
        Entity::Polyline { bulges, .. } => bulges.iter().any(|b| b.abs() > 1e-6),
        other => panic!("a polyline, got {other:?}"),
    }
}

#[test]
fn decurve_brings_back_the_vertices_a_fit_or_a_spline_started_from() {
    for curve in ["Fit", "Spline"] {
        let mut state = started(&[zigzag()]);
        click(&mut state, 0);
        key(&mut state, curve);
        assert!(points_of(&state) > 4, "{curve}");
        key(&mut state, "Decurve");
        assert_eq!(entities(&state)[0], zigzag(), "{curve}");
        // One undo step takes the decurving back to the curve.
        key(&mut state, "Undo");
        assert!(points_of(&state) > 4, "{curve}");
    }
}

#[test]
fn a_second_curve_starts_again_from_the_frame_and_decurve_still_finds_it() {
    let mut state = started(&[zigzag()]);
    click(&mut state, 0);
    key(&mut state, "Fit");
    key(&mut state, "Spline");
    let after = entities(&state)[0].clone();
    key(&mut state, "Decurve");
    assert_eq!(entities(&state)[0], zigzag());
    // The spline is the one of the original vertices, not of the fit.
    let mut other = started(&[zigzag()]);
    click(&mut other, 0);
    key(&mut other, "Spline");
    assert_eq!(entities(&other)[0], after);
}

#[test]
fn an_edit_after_the_curve_leaves_decurve_to_straighten_what_is_there() {
    let mut state = started(&[zigzag()]);
    click(&mut state, 0);
    key(&mut state, "Fit");
    key(&mut state, "Close");
    key(&mut state, "Decurve");
    let Entity::Polyline {
        points,
        bulges,
        closed,
        ..
    } = &entities(&state)[0]
    else {
        panic!("a polyline")
    };
    assert!(*closed && bulges.is_empty());
    assert_eq!(points.len(), 7, "the fit's own vertices, straightened");
}

fn set_tangent_at(state: &mut AppState, vertex: usize, degrees: &str) {
    for _ in 0..vertex {
        key(state, "Next");
    }
    key(state, "Tangent");
    state.cmd_submit(degrees);
    state.settle_command();
}

#[test]
fn a_tangent_set_at_a_vertex_bends_the_next_fit() {
    // Three points in a line fit to a straight line...
    let mut plain = started(&[straight()]);
    click(&mut plain, 0);
    key(&mut plain, "Fit");
    assert!(!arcs_in(&plain));
    // ...until the middle vertex is told to run straight up.
    let mut state = started(&[straight()]);
    click(&mut state, 0);
    key(&mut state, "Edit vertex");
    set_tangent_at(&mut state, 1, "90");
    key(&mut state, "eXit");
    assert_eq!(
        entities(&state)[0],
        straight(),
        "the polyline itself is unchanged"
    );
    key(&mut state, "Fit");
    assert!(arcs_in(&state));
    // The tangent belongs to the vertices of the frame: Decurve keeps it.
    key(&mut state, "Decurve");
    assert_eq!(entities(&state)[0], straight());
    key(&mut state, "Fit");
    assert!(arcs_in(&state));
}

#[test]
fn a_tangent_is_one_undo_step_and_survives_moving_a_vertex() {
    let mut state = started(&[straight()]);
    click(&mut state, 0);
    key(&mut state, "Edit vertex");
    set_tangent_at(&mut state, 1, "90");
    key(&mut state, "Next");
    key(&mut state, "Move");
    at(&mut state, 11.0, 0.0);
    key(&mut state, "eXit");
    assert_eq!(
        entities(&state)[0],
        polyline(&[(0.0, 0.0), (5.0, 0.0), (11.0, 0.0)])
    );
    key(&mut state, "Fit");
    assert!(arcs_in(&state), "the tangent was kept through the move");
    // Undo takes the fit back, then the move, then the tangent.
    key(&mut state, "Undo");
    key(&mut state, "Undo");
    key(&mut state, "Undo");
    assert_eq!(entities(&state)[0], straight());
    key(&mut state, "Fit");
    assert!(!arcs_in(&state), "no tangent left");
}

#[test]
fn a_vertex_edit_that_changes_the_vertices_drops_the_tangents() {
    let mut state = started(&[straight()]);
    click(&mut state, 0);
    key(&mut state, "Edit vertex");
    set_tangent_at(&mut state, 1, "90");
    key(&mut state, "Insert");
    at(&mut state, 7.0, 0.0);
    key(&mut state, "eXit");
    key(&mut state, "Fit");
    assert!(!arcs_in(&state));
}

#[test]
fn tangent_is_refused_on_a_polyline_that_is_a_curve() {
    let mut state = started(&[zigzag()]);
    click(&mut state, 0);
    key(&mut state, "Fit");
    key(&mut state, "Edit vertex");
    key(&mut state, "Tangent");
    assert!(said(&state, "Decurve it to set tangents"));
}

fn spline() -> Entity {
    Entity::Spline(Box::new(
        Spline::from_control_points(3, vec![p(0.0, 0.0), p(2.0, 3.0), p(5.0, 3.0), p(7.0, 0.0)])
            .unwrap(),
    ))
}

#[test]
fn plineconvertmode_one_turns_a_spline_into_arcs() {
    let table = crate::sysvars::table();
    let mut state = fresh();
    table.set("PLINECONVERTMODE", "1", &mut state).unwrap();
    state.add_entity(spline());
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    click(&mut state, 0);
    key(&mut state, "Yes");
    state.cmd_submit("");
    state.settle_command();
    assert!(arcs_in(&state));
    // With the variable at 0 it is straight pieces, as before.
    let mut state = fresh();
    state.add_entity(spline());
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    click(&mut state, 0);
    key(&mut state, "Yes");
    state.cmd_submit("");
    state.settle_command();
    assert!(!arcs_in(&state));
}

#[test]
fn multiple_turns_the_splines_among_the_selection_into_polylines() {
    let mut state = fresh();
    state.add_entity(spline());
    state.add_entity(zigzag());
    state.add_entity(spline());
    state.select_all();
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    assert!(state.cmd_prompt().text.contains("Convert"));
    key(&mut state, "Yes");
    assert!(state.cmd_prompt().text.contains("precision"));
    state.cmd_submit("");
    state.settle_command();
    assert!(state.cmd_prompt().text.contains("Enter an option"));
    assert!(entities(&state)
        .iter()
        .all(|e| matches!(e, Entity::Polyline { .. })));
    // Close them all, then Enter ends; one undo step per option.
    key(&mut state, "Close");
    enter(&mut state);
    assert!(entities(&state)
        .iter()
        .all(|e| matches!(e, Entity::Polyline { closed: true, .. })));
}

#[test]
fn multiple_declined_leaves_the_splines_and_edits_the_polylines() {
    let mut state = fresh();
    state.add_entity(spline());
    state.add_entity(zigzag());
    state.select_all();
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    key(&mut state, "No");
    assert!(matches!(entities(&state)[0], Entity::Spline(_)));
    key(&mut state, "Close");
    assert!(matches!(
        entities(&state)[1],
        Entity::Polyline { closed: true, .. }
    ));
}

#[test]
fn multiple_decurve_restores_every_curved_polyline() {
    let mut state = fresh();
    state.add_entity(zigzag());
    state.add_entity(polyline(&[(0.0, 5.0), (3.0, 9.0), (6.0, 5.0), (9.0, 9.0)]));
    // Curve each by itself, then decurve both together.
    for index in 0..2 {
        state.start_command("PEDIT", pedit_objects);
        state.settle_command();
        click(&mut state, index);
        key(&mut state, "Fit");
        enter(&mut state);
    }
    assert!(entities(&state)
        .iter()
        .all(|e| matches!(e, Entity::Polyline { points, .. } if points.len() == 7)));
    state.select_all();
    state.start_command("PEDIT", pedit_objects);
    state.settle_command();
    key(&mut state, "Decurve");
    enter(&mut state);
    assert_eq!(entities(&state)[0], zigzag());
    assert!(entities(&state)
        .iter()
        .all(|e| matches!(e, Entity::Polyline { points, bulges, .. } if points.len() == 4 && bulges.is_empty())));
}
