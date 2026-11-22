//! Commands that orient what they make follow the custom coordinate system:
//! text, rectangles, construction lines, linear dimensions and rectangular
//! arrays are square to its axes, not the world's.

use std::f64::consts::FRAC_PI_6;

use opendraft_commands::{Editor, SetCcs};
use opendraft_core::{Ccs, CcsSpace, Entity, Point};

use crate::state::AppState;

fn turned() -> AppState {
    let mut state = AppState::new();
    state.perform(SetCcs::new(
        CcsSpace::Model,
        Ccs::new(Point::new(0.0, 0.0), FRAC_PI_6),
    ));
    state
}

fn run(
    state: &mut AppState,
    name: &str,
    command: fn(&mut Editor) -> Result<(), opendraft_commands::Cancelled>,
    inputs: &[&str],
) {
    state.start_command(name, command);
    state.settle_command();
    for input in inputs {
        state.cmd_submit(input);
        state.settle_command();
    }
}

fn last(state: &AppState) -> Entity {
    let objects = state.session().document().objects();
    objects[objects.len() - 1].entity.clone()
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

#[test]
fn text_is_level_with_the_system_by_default() {
    let mut state = turned();
    let inputs = ["0,0", "2.5", "", "Hi"];
    run(
        &mut state,
        "-TEXT",
        crate::single_line_text_cmd::draw_text_line,
        &inputs,
    );
    let Entity::Text(text) = last(&state) else {
        panic!("expected text");
    };
    assert!((text.placement.rotation - FRAC_PI_6).abs() < 1e-12);
}

#[test]
fn a_rectangle_is_square_to_the_system() {
    let mut state = turned();
    run(
        &mut state,
        "Rectangle",
        crate::rectangle::draw_rectangle,
        &["0,0", "@10,5"],
    );
    let Entity::Polyline { points, .. } = last(&state) else {
        panic!("a turned rectangle is a polyline");
    };
    let ccs = state.session().document().ccs();
    let local: Vec<Point> = points.iter().map(|p| ccs.from_world(*p)).collect();
    for corner in [(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)] {
        let corner = Point::new(corner.0, corner.1);
        assert!(
            local.iter().any(|p| near(*p, corner)),
            "{corner:?} in {local:?}"
        );
    }
}

#[test]
fn horizontal_construction_lines_run_along_the_system() {
    let mut state = turned();
    run(
        &mut state,
        "XLINE",
        crate::xline_cmd::draw_xline,
        &["h", "0,0", ""],
    );
    let Entity::Xline { direction, .. } = last(&state) else {
        panic!("expected a construction line");
    };
    assert!(near(direction, state.session().document().ccs().x_axis()));
}

#[test]
fn a_linear_dimension_measures_along_the_system() {
    let mut state = turned();
    let inputs = ["0,0", "10,0", "5,5"];
    run(
        &mut state,
        "DIMLINEAR",
        crate::dimension_cmd::dim_linear,
        &inputs,
    );
    let Entity::Dimension(dimension) = last(&state) else {
        panic!("expected a dimension");
    };
    let measured = dimension.measurement().unwrap_or_default();
    assert!((measured - 10.0).abs() < 1e-9, "{measured}");
}

#[test]
fn a_rectangular_array_steps_along_the_system() {
    let mut state = turned();
    run(
        &mut state,
        "Line",
        crate::draw::draw_line,
        &["0,0", "1,0", ""],
    );
    state.select_all();
    let inputs = ["cou", "2", "1", "s", "10", "1", ""];
    run(
        &mut state,
        "ARRAYRECT",
        crate::array_cmd::array_rect,
        &inputs,
    );
    let ccs = state.session().document().ccs();
    let Entity::Line { start, .. } = last(&state) else {
        panic!("expected the copied line");
    };
    assert!(
        near(ccs.from_world(start), Point::new(10.0, 0.0)),
        "{:?}",
        ccs.from_world(start)
    );
}

#[test]
fn a_hatch_pattern_lies_along_the_system_from_its_origin() {
    let mut state = AppState::new();
    let frame = Ccs::new(Point::new(5.0, 5.0), FRAC_PI_6);
    state.perform(SetCcs::new(CcsSpace::Model, frame));
    state.add_entity(Entity::Rectangle {
        min: Point::new(0.0, 0.0),
        max: Point::new(10.0, 10.0),
    });
    state.select_all();
    state.start_command("Hatch", crate::hatch_cmd::hatch_objects);
    state.settle_command();
    state.answer_enter();
    state.settle_command();
    let hatch = state
        .session()
        .document()
        .objects()
        .iter()
        .find_map(|o| match &o.entity {
            Entity::Hatch(h) => Some((**h).clone()),
            _ => None,
        })
        .expect("a hatch");
    let opendraft_core::hatch::HatchFill::Pattern(pattern) = hatch.fill else {
        panic!("the remembered fill is a pattern");
    };
    // Angle 0 and origin 0,0 in the system: its X axis and its origin.
    assert!((pattern.angle - FRAC_PI_6).abs() < 1e-12);
    assert!(near(pattern.origin, Point::new(5.0, 5.0)));
}
