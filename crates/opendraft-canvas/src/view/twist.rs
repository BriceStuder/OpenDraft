//! Turning the view: Ctrl+middle-drag turns the drawing on the screen about
//! the middle of the canvas (of the viewport, in a layout), following the
//! pointer round it; Shift as well holds the turn to 15 degree steps; Esc
//! during the drag puts the view back. Commands turn it with
//! [`View::request_twist`]. A layout's sheet never turns.

use egui::{Key, Modifiers, PointerButton, Pos2, Response, Ui};

use super::{Request, View};

/// The steps Shift holds the turn to.
const STEP: f64 = std::f64::consts::PI / 12.0;

/// A Ctrl+middle-drag under way.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TwistDrag {
    /// The twist when the drag began, which Esc goes back to.
    from: f64,
    /// The pointer's angle round the pivot when the drag began.
    grab: f64,
    /// Esc was pressed: the rest of the drag does nothing.
    dropped: bool,
}

/// The angle of `p` round `pivot`, counter-clockwise on the screen.
fn angle_round(pivot: Pos2, p: Pos2) -> f64 {
    f64::from(pivot.y - p.y).atan2(f64::from(p.x - pivot.x))
}

/// `twist` held to the nearest step when `steps`.
fn stepped(twist: f64, steps: bool) -> f64 {
    if steps {
        (twist / STEP).round() * STEP
    } else {
        twist
    }
}

impl View {
    /// Turn the drawing on the screen to `twist` (radians counter-clockwise)
    /// on the next frame: the Model tab's view, or the view of the viewport
    /// being worked through (not a locked one). A layout's sheet does not turn.
    pub fn request_twist(&mut self, twist: f64) {
        if twist.is_finite() {
            self.request(Request::Twist(twist, false));
        }
    }

    /// Like [`request_twist`](Self::request_twist), for a turn that goes on over
    /// many frames (the navigation gizmo's arrow being dragged): it is not a
    /// step of its own for ZOOM Previous, which takes the pauses between the
    /// frames as the steps, as it does for Ctrl+middle-drag.
    pub fn request_twist_live(&mut self, twist: f64) {
        if twist.is_finite() {
            self.request = Some(Request::Twist(twist, false));
        }
    }

    /// [`request_twist`](Self::request_twist), then show the extents (PLAN).
    pub fn request_plan(&mut self, twist: f64) {
        if twist.is_finite() {
            self.request(Request::Twist(twist, true));
        }
    }

    /// Whether a Ctrl+middle-drag is turning the view.
    pub fn is_twisting(&self) -> bool {
        self.twist_drag.is_some_and(|d| !d.dropped)
    }

    /// The twist a Ctrl+middle-drag about `pivot` asks for this frame, if
    /// one is under way; none when the middle button is not dragging, or
    /// the drag pans.
    pub(super) fn twist_by_drag(
        &mut self,
        ui: &Ui,
        response: &Response,
        pivot: Pos2,
    ) -> Option<f64> {
        if !response.dragged_by(PointerButton::Middle) {
            self.twist_drag = None;
            return None;
        }
        let (ctrl, shift, at) =
            ui.input(|i| (i.modifiers.ctrl, i.modifiers.shift, i.pointer.latest_pos()));
        let at = at?;
        if response.drag_started_by(PointerButton::Middle) && ctrl {
            self.twist_drag = Some(TwistDrag {
                from: self.camera.twist,
                grab: angle_round(pivot, ui.input(|i| i.pointer.press_origin()).unwrap_or(at)),
                dropped: false,
            });
        }
        let drag = self.twist_drag?;
        if drag.dropped {
            // Held still: neither turning nor panning until the button is let go.
            return Some(drag.from);
        }
        Some(stepped(
            drag.from + angle_round(pivot, at) - drag.grab,
            shift,
        ))
    }

    /// Esc during a Ctrl+middle-drag: the view goes back to where it was,
    /// and the key goes no further (it does not cancel a running command).
    pub(super) fn take_twist_keys(&mut self, ctx: &egui::Context) {
        let Some(drag) = &mut self.twist_drag else {
            return;
        };
        if !drag.dropped && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            drag.dropped = true;
            let from = drag.from;
            self.request(Request::Twist(from, false));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_run_counter_clockwise_on_the_screen() {
        let pivot = Pos2::new(100.0, 100.0);
        assert!(angle_round(pivot, Pos2::new(200.0, 100.0)).abs() < 1e-9);
        let up = angle_round(pivot, Pos2::new(100.0, 0.0));
        assert!(
            (up - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "screen y runs down"
        );
    }

    #[test]
    fn shift_holds_the_turn_to_fifteen_degree_steps() {
        let t = stepped(20.0_f64.to_radians(), true);
        assert!((t - 15.0_f64.to_radians()).abs() < 1e-12);
        assert_eq!(stepped(0.3, false), 0.3);
    }
}
