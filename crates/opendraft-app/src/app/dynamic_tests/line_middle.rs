use super::*;

#[test]
fn tab_reaches_the_whole_length_of_a_line_from_its_middle() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.state
        .start_command("Line", crate::line_modes::draw_line_middle);
    frame(&mut app, &ctx, Vec::new(), 0.0);
    app.state.settle_command();
    app.state.answer_dynamic("0,0").unwrap();
    app.state.settle_command();
    frame(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(Pos2::new(450.0, 300.0))],
        0.05,
    );
    frame(&mut app, &ctx, vec![Event::Text("10".into())], 0.06);
    frame(&mut app, &ctx, key(Key::Tab), 0.5);
    frame(&mut app, &ctx, Vec::new(), 0.51);
    let input = &app.state.docs[0].dynamic_input;
    assert_eq!(input.active, Some(Field::Length));
    assert!(input.editing, "the whole length lost the keyboard");
    frame(&mut app, &ctx, vec![Event::Text("30".into())], 0.52);
    assert_eq!(app.state.docs[0].dynamic_input.draft, "30");
    frame(&mut app, &ctx, key(Key::Enter), 0.53);
    app.state.settle_command();
    let objects = app.state.session().document().objects();
    let opendraft_core::Entity::Line { start, end } = &objects[0].entity else {
        panic!("not a line");
    };
    assert!(((start.x - end.x).hypot(start.y - end.y) - 30.0).abs() < 1e-9);
    assert!((start.x + end.x).abs() < 1e-9 && (start.y + end.y).abs() < 1e-9);
}
