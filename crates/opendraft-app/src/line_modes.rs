//! The two other ways of drawing lines, reached from LINE's first prompt or
//! as commands of their own: single lines (each line its own two points, not
//! chained to the last) and lines from their middle (the middle, then one
//! end; the other end mirrors it). Each line is added the moment it is
//! complete; Undo at the next first prompt takes back the last one; Enter or
//! Esc ends the command and keeps what was drawn.

use opendraft_commands::{Answer, Cancelled, DynamicDimensions, Editor, PointOptions};
use opendraft_core::Point;
use opendraft_ui::key;

use crate::draw::{add, distance, line, MIN_SIZE};

/// How each line is given.
#[derive(Clone, Copy)]
enum Mode {
    /// Two ends.
    Single,
    /// The middle, then one end.
    Middle,
}

/// LINE SINGLE (SL): lines that do not chain, each given by its two ends.
pub fn draw_line_single(ed: &mut Editor) -> Result<(), Cancelled> {
    lines(ed, Mode::Single)
}

/// LINE MIDDLE (ML): lines given by their middle, then one end.
pub fn draw_line_middle(ed: &mut Editor) -> Result<(), Cancelled> {
    lines(ed, Mode::Middle)
}

/// The line from `from` with its other end at `to`: for a line given by its
/// middle, `to` is mirrored through `from`.
fn segment(mode: Mode, from: Point, to: Point) -> opendraft_core::Entity {
    match mode {
        Mode::Single => line(from, to),
        Mode::Middle => line(Point::new(2.0 * from.x - to.x, 2.0 * from.y - to.y), to),
    }
}

fn lines(ed: &mut Editor, mode: Mode) -> Result<(), Cancelled> {
    let (first_prompt, end_prompt) = match mode {
        Mode::Single => (key!("Specify first point"), key!("Specify second point")),
        Mode::Middle => (
            key!("Specify midpoint of line"),
            key!("Specify endpoint of line"),
        ),
    };
    let mut drawn = 0usize;
    loop {
        let mut options = PointOptions::new(first_prompt);
        if drawn > 0 {
            options = options.keyword("Undo", "U");
        }
        let from = match ed.ask_point(options) {
            Err(Cancelled) | Ok(Answer::Enter) => return Ok(()),
            Ok(Answer::Value(point)) => point,
            Ok(Answer::Keyword(_)) => {
                ed.undo()?;
                drawn -= 1;
                continue;
            }
        };
        let mut options = PointOptions::new(end_prompt)
            .base(from)
            .keyword("Undo", "U")
            .ghost(move |at| vec![segment(mode, from, at)]);
        options = match mode {
            Mode::Single => options.dimensions(DynamicDimensions::Polar),
            Mode::Middle => options.dimensions(DynamicDimensions::Mirrored),
        };
        match ed.ask_point(options) {
            Err(Cancelled) | Ok(Answer::Enter) => return Ok(()),
            // Undo before the line is made goes back to its first point.
            Ok(Answer::Keyword(_)) => {}
            Ok(Answer::Value(to)) => {
                if distance(from, to) > MIN_SIZE {
                    add(ed, segment(mode, from, to), "Line")?;
                    drawn += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use opendraft_core::Entity;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn run(command: fn(&mut Editor) -> Result<(), Cancelled>, inputs: &[&str]) -> AppState {
        let mut state = AppState::new();
        state.start_command("Test", command);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
        state
    }

    #[test]
    fn single_lines_do_not_chain() {
        let state = run(draw_line_single, &["0,0", "5,0", "10,0", "10,5"]);
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(5.0, 0.0)),
                line(p(10.0, 0.0), p(10.0, 5.0))
            ]
        );
        assert!(state.waiting().is_some_and(|ask| ask.base.is_none()));
    }

    #[test]
    fn a_line_from_its_middle_reaches_both_ways() {
        let state = run(draw_line_middle, &["10,10", "@5,0", "0,0", "@0,2"]);
        assert_eq!(
            entities(&state),
            [
                line(p(5.0, 10.0), p(15.0, 10.0)),
                line(p(0.0, -2.0), p(0.0, 2.0))
            ]
        );
    }

    #[test]
    fn a_bare_distance_from_the_middle_is_half_the_line() {
        let mut state = run(draw_line_middle, &["0,0"]);
        state.cursor = Some(p(100.0, 0.0));
        state.cmd_submit("25");
        state.settle_command();
        assert_eq!(entities(&state), [line(p(-25.0, 0.0), p(25.0, 0.0))]);
    }

    #[test]
    fn undo_takes_back_the_last_line_then_the_first_point() {
        let mut state = run(draw_line_single, &["0,0", "5,0", "1,1", "2,2"]);
        state.cmd_submit("u");
        state.settle_command();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(5.0, 0.0))]);
        state.cmd_submit("3,3");
        state.settle_command();
        state.cmd_submit("u");
        state.settle_command();
        // Back at the first point: nothing more was taken back.
        assert_eq!(entities(&state).len(), 1);
        assert!(state.waiting().is_some_and(|ask| ask.base.is_none()));
    }

    #[test]
    fn enter_ends_and_keeps_the_lines() {
        let mut state = run(draw_line_single, &["0,0", "5,0"]);
        state.cmd_submit("");
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(entities(&state).len(), 1);
    }

    #[test]
    fn line_offers_both_ways_at_its_first_prompt() {
        let state = run(crate::draw::draw_line, &["s", "0,0", "5,0", "1,1", "2,1"]);
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(5.0, 0.0)),
                line(p(1.0, 1.0), p(2.0, 1.0))
            ]
        );
        let state = run(crate::draw::draw_line, &["m", "0,0", "1,0"]);
        assert_eq!(entities(&state), [line(p(-1.0, 0.0), p(1.0, 0.0))]);
    }
}
