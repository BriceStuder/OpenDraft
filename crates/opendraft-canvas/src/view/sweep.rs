//! A swept answer to an object prompt that allows it (TRIM and EXTEND): a
//! press in empty space is held back until the button comes up. Dragged
//! far enough it is a freehand path; otherwise it is a click where it was
//! pressed.

use egui::{Painter, Pos2, Rect, Shape, Stroke, Ui};
use opendraft_core::Point;

use super::types::CanvasStyle;
use super::{View, BOX_MIN_PX};

/// Pointer moves shorter than this (screen points) add no point to a path.
const PATH_STEP_PX: f32 = 2.0;

/// What a sweep came to this frame.
enum Swept {
    /// Nothing yet: no sweep, or the button is still down.
    Pending,
    /// Released without moving far: a click at this point.
    Click(Point),
    /// Released after a drag: the path, in drawing units.
    Path(Vec<Point>),
}

impl View {
    /// A sweep prompt's part of a frame. A point `prompted` this frame on no
    /// object (`on_nothing`) is held back and starts a sweep; a sweep
    /// released as a click puts its point in `prompted`, and one released
    /// after a drag is returned as a path.
    pub(super) fn sweep_answer(
        &mut self,
        ui: &Ui,
        rect: Rect,
        on_nothing: bool,
        prompted: &mut Option<Point>,
    ) -> Option<Vec<Point>> {
        if on_nothing && prompted.take().is_some() {
            if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
                self.start_sweep(press);
            }
        }
        match self.follow_sweep(ui, rect) {
            Swept::Click(at) => {
                *prompted = Some(at);
                None
            }
            Swept::Path(points) => Some(points),
            Swept::Pending => None,
        }
    }

    /// Start a sweep at `press` (screen), a press of an object prompt that
    /// landed on no object.
    fn start_sweep(&mut self, press: Pos2) {
        self.sweep = Some(vec![press]);
    }

    /// Follow the sweep in progress: the pointer is added to its path while
    /// the button is down, and the release says what it came to.
    fn follow_sweep(&mut self, ui: &Ui, rect: Rect) -> Swept {
        let Some(path) = self.sweep.as_mut() else {
            return Swept::Pending;
        };
        let (down, now) = ui.input(|i| (i.pointer.primary_down(), i.pointer.latest_pos()));
        if let Some(now) = now {
            if path
                .last()
                .is_none_or(|last| last.distance(now) >= PATH_STEP_PX)
            {
                path.push(now);
            }
        }
        if down {
            return Swept::Pending;
        }
        let path = self.sweep.take().unwrap_or_default();
        let Some(&first) = path.first() else {
            return Swept::Pending;
        };
        let reach = path.iter().map(|p| p.distance(first)).fold(0.0, f32::max);
        if reach > BOX_MIN_PX {
            let points = path.iter().map(|p| self.camera.screen_to_world(*p, rect));
            Swept::Path(points.collect())
        } else {
            Swept::Click(self.camera.screen_to_world(first, rect))
        }
    }

    /// Draw the path being swept, in the crossing colour.
    pub(super) fn paint_sweep(&self, painter: &Painter, style: &CanvasStyle) {
        if let Some(path) = self.sweep.as_ref().filter(|p| p.len() >= 2) {
            painter.add(Shape::line(
                path.clone(),
                Stroke::new(1.5_f32, style.crossing),
            ));
        }
    }
}
