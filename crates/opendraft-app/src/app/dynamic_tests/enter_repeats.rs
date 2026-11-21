//! Enter with no command running repeats the last command, whatever it was
//! (U, ZOOM, LAYOFF...), but not a switch such as F7 or a zoom run inside
//! another command.
use super::*;

fn circle(app: &mut OpenDraftApp, x: f64) {
    app.run("draw.circle");
    app.state.settle_command();
    for input in [format!("{x},0"), "5".to_string()] {
        app.state.cmd_submit(&input);
        app.state.settle_command();
    }
    assert!(!app.state.command_running());
}

fn circles(app: &OpenDraftApp) -> usize {
    app.state.session().document().objects().len()
}

/// Enter, as the app routes it: the command it gives back is run.
fn enter(app: &mut OpenDraftApp) {
    if let Some(id) = app.state.cmd_submit("") {
        app.run(id);
        app.state.settle_command();
    }
}

#[test]
fn enter_after_u_undoes_again_instead_of_drawing_a_circle() {
    let mut app = app();
    for x in [0.0, 20.0, 40.0] {
        circle(&mut app, x);
    }
    app.run("edit.undo");
    assert_eq!(circles(&app), 2);
    enter(&mut app);
    assert_eq!(circles(&app), 1, "Enter repeats U");
    assert!(!app.state.command_running(), "no CIRCLE was started");
}

#[test]
fn every_kind_of_command_is_the_one_enter_repeats() {
    let mut app = app();
    for id in [
        "edit.redo",
        "edit.copy_clip",
        "layer.hide",
        "view.zoom_extents",
        "file.plot",
        "manage.settings",
    ] {
        app.state.cmdline.note_last(id);
        assert_eq!(app.state.cmdline.last_command, Some(id));
    }
}

#[test]
fn switches_leave_the_command_to_repeat_alone() {
    let mut app = app();
    circle(&mut app, 0.0);
    for id in [
        "view.grid",
        "view.ortho",
        "view.snap_endpoint",
        "tool.select",
    ] {
        app.run(id);
    }
    assert_eq!(app.state.cmdline.last_command, Some("draw.circle"));
}

#[test]
fn a_zoom_inside_a_command_is_not_the_one_enter_repeats() {
    let mut app = app();
    circle(&mut app, 0.0);
    app.run("draw.circle");
    app.state.settle_command();
    app.run("view.zoom_extents");
    assert!(app.state.command_running(), "the zoom did not end CIRCLE");
    assert_eq!(app.state.cmdline.last_command, Some("draw.circle"));
}
