use super::*;

fn middle(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Middle,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// One event a frame, a fiftieth of a second apart.
fn play(app: &mut OpenDraftApp, ctx: &egui::Context, events: Vec<Event>, time: &mut f64) {
    for event in events {
        *time += 0.02;
        frame(app, ctx, vec![event], *time);
    }
    *time += 0.02;
    frame(app, ctx, Vec::new(), *time);
}

fn double_click(app: &mut OpenDraftApp, ctx: &egui::Context, at: Pos2, time: &mut f64) {
    let events = vec![
        Event::PointerMoved(at),
        middle(at, true),
        middle(at, false),
        middle(at, true),
        middle(at, false),
    ];
    play(app, ctx, events, time);
}

/// A drawing with one line far from the view, and LINE waiting for its
/// second point.
fn drawing_a_line() -> (OpenDraftApp, egui::Context, f64) {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.state.start_command("Line", crate::draw::draw_line);
    app.state.settle_command();
    for answer in ["5000,3000", "9000,6000", ""] {
        app.state.cmd_submit(answer);
        app.state.settle_command();
    }
    assert!(!app.state.command_running());
    line(&mut app, &ctx);
    app.state.cmd_submit("5000,3000");
    app.state.settle_command();
    let mut time = 0.02;
    play(&mut app, &ctx, Vec::new(), &mut time);
    (app, ctx, time)
}

fn objects(app: &OpenDraftApp) -> usize {
    app.state.docs[0].session.document().objects().len()
}

#[test]
fn double_clicking_the_wheel_zooms_to_the_extents_without_touching_the_command() {
    let (mut app, ctx, mut time) = drawing_a_line();
    let doc = &app.state.docs[0];
    let (canvas, before) = (doc.dynamic_input.canvas.unwrap(), doc.view.camera);
    let mut extents = before;
    extents.fit(doc.session.document().bounds(), canvas);
    assert_ne!(before, extents, "the line is out of view to begin with");
    let prompt = app.state.cmd_prompt();
    let selected = app.state.docs[0].selection.clone();

    double_click(&mut app, &ctx, canvas.center(), &mut time);

    assert_eq!(app.state.docs[0].view.camera, extents);
    // LINE still waits for the same point: nothing was picked or cancelled.
    assert!(app.state.command_running());
    assert_eq!(app.state.cmd_prompt(), prompt);
    assert_eq!(objects(&app), 1);
    assert_eq!(app.state.docs[0].selection, selected);
}

#[test]
fn double_clicking_the_wheel_with_no_command_selects_nothing() {
    let (mut app, ctx, mut time) = drawing_a_line();
    app.state.cmd_submit("");
    app.state.settle_command();
    play(&mut app, &ctx, Vec::new(), &mut time);
    assert!(!app.state.command_running());
    let doc = &app.state.docs[0];
    let canvas = doc.dynamic_input.canvas.unwrap();
    let mut extents = doc.view.camera;
    extents.fit(doc.session.document().bounds(), canvas);

    // Once fitted, the line runs through the middle of the view.
    double_click(&mut app, &ctx, canvas.center(), &mut time);
    double_click(&mut app, &ctx, canvas.center(), &mut time);

    assert_eq!(app.state.docs[0].view.camera, extents);
    assert!(app.state.docs[0].selection.is_empty());
    assert!(!app.state.command_running());
    assert_eq!(objects(&app), 1);
}

#[test]
fn dragging_with_the_wheel_still_pans_and_is_no_double_click() {
    let (mut app, ctx, mut time) = drawing_a_line();
    let doc = &app.state.docs[0];
    let (canvas, before) = (doc.dynamic_input.canvas.unwrap(), doc.view.camera);
    let from = canvas.center();
    let to = from + Vec2::new(120.0, 60.0);

    // Two drags in quick succession from the same place.
    for _ in 0..2 {
        let mut events = vec![Event::PointerMoved(from), middle(from, true)];
        events.extend((1..=6).map(|i| Event::PointerMoved(from.lerp(to, i as f32 / 6.0))));
        events.push(middle(to, false));
        play(&mut app, &ctx, events, &mut time);
    }

    let after = app.state.docs[0].view.camera;
    assert_eq!(after.zoom, before.zoom, "panned, not zoomed");
    // What was under the pointer followed it, twice.
    let moved = after.world_to_screen(before.screen_to_world(from, canvas), canvas) - from;
    assert!((moved - 2.0 * (to - from)).length() < 1.0, "{moved:?}");
    assert!(app.state.command_running());
    assert_eq!(objects(&app), 1);
}
