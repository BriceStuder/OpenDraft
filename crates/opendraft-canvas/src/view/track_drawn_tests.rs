//! Object snap tracking on what a command has drawn but not yet added (the
//! segments of a polyline in progress), as on objects of the drawing.

use std::collections::BTreeSet;

use egui::{Context, Event, RawInput};
use opendraft_core::{Document, Entity, Point};

use super::input_tests::{style, SCREEN};
use super::{CanvasOutput, GridSettings, View};
use crate::snap::SnapModes;
use crate::PointPrompt;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A frame at `time` seconds with the pointer at `at` (in drawing units).
fn frame(view: &mut View, ctx: &Context, time: f64, at: Point) -> CanvasOutput {
    let pos = view.camera.world_to_screen(at, SCREEN);
    let modes = SnapModes {
        grid: false,
        ..SnapModes::default()
    };
    let mut result = None;
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(SCREEN),
            time: Some(time),
            events: vec![Event::PointerMoved(pos)],
            ..RawInput::default()
        },
        |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    result = Some(view.show(
                        ui,
                        &style(),
                        &GridSettings {
                            visible: false,
                            spacing: 10.0,
                        },
                        &Document::default(),
                        &BTreeSet::new(),
                        &modes,
                    ));
                });
        },
    );
    result.expect("the canvas was shown")
}

/// PLINE after three clicks: the two segments drawn so far are not in the
/// drawing, only offered to the pointer.
fn polyline_in_progress() -> View {
    let mut view = View::default();
    view.set_point_prompt(Some(PointPrompt {
        base: Some(p(250.0, 150.0)),
        rubber_band: true,
        snap_geometry: vec![Entity::Polyline {
            points: vec![p(50.0, 50.0), p(250.0, 50.0), p(250.0, 150.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        }]
        .into(),
        ..Default::default()
    }));
    view
}

#[test]
fn the_end_of_a_polyline_in_progress_snaps() {
    let ctx = Context::default();
    let mut view = polyline_in_progress();
    let out = frame(&mut view, &ctx, 0.0, p(50.5, 50.5));
    assert_eq!(out.preview_cursor, Some(p(50.0, 50.0)), "closing by hand");
}

#[test]
fn a_point_of_a_polyline_in_progress_is_kept_and_tracked_from() {
    let ctx = Context::default();
    let mut view = polyline_in_progress();
    let first = p(50.0, 50.0);
    // The pointer rests on the first point long enough to keep it...
    for time in [0.0, 0.2, 0.4, 0.5, 0.6] {
        frame(&mut view, &ctx, time, p(50.5, 50.5));
    }
    let kept: Vec<Point> = view.snap_acquire.origins().iter().map(|o| o.at).collect();
    assert_eq!(kept, [first], "the first point stays kept");
    // ...and straight above it, the pointer is held to the path from it.
    let out = frame(&mut view, &ctx, 0.7, p(50.8, 120.0));
    assert_eq!(out.preview_cursor, Some(p(50.0, 120.0)));
    // Paused on again, it is let go, as on an object of the drawing.
    for time in [1.0, 1.2, 1.4, 1.5] {
        frame(&mut view, &ctx, time, p(50.5, 50.5));
    }
    assert!(view.snap_acquire.origins().is_empty());
}
