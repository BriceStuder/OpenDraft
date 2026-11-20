//! A command waiting for the user asks for no frames; one working between
//! prompts does, so its next prompt shows as soon as it is there.
use std::sync::mpsc;

use super::*;

#[test]
fn a_command_waiting_for_a_point_is_not_working() {
    let mut app = app();
    app.state.start_command("Line", crate::draw::draw_line);
    app.state.settle_command();
    assert!(app.state.command_running());
    assert!(app.state.waiting().is_some());
    assert!(!app.state.command_working());
}

#[test]
fn a_command_busy_between_prompts_is_working_until_it_asks() {
    let mut app = app();
    let (go, wait_for_go) = mpsc::channel::<()>();
    app.state.start_command("Busy", move |ed| {
        // Stands for a long calculation before the first prompt.
        let _ = wait_for_go.recv();
        ed.get_point("Specify point")?;
        Ok(())
    });
    app.state.pump_command();
    assert!(app.state.command_working());

    let _ = go.send(());
    app.state.settle_command();
    assert!(app.state.waiting().is_some());
    assert!(!app.state.command_working());
}

#[test]
fn no_command_is_not_working() {
    let app = app();
    assert!(!app.state.command_working());
}
