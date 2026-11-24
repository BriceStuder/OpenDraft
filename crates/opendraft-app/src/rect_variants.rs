//! The other ways of drawing a rectangle (from its centre, by three points)
//! and the oblong (by the centres of its ends, or from its centre). The
//! rectangles take RECTANGLE's remembered corners and line width, and offer
//! its Chamfer, Fillet and Width options at their first prompt.

use opendraft_commands::{Answer, Cancelled, Editor, NumberOptions, NumberOrPoint, PointOptions};
use opendraft_core::rect::{by_three_points, centred, outline};
use opendraft_core::{oblong, Point};
use opendraft_ui::key;

use crate::defaults::EditorDefaults;
use crate::draw::{add, MIN_SIZE};
use crate::rectangle::{corners, extent, first_point, positive, rotation, size_from_area, Size};

/// Half of a full size, for a rectangle measured from its centre.
fn half(size: Option<Size>) -> Option<Size> {
    size.map(|(length, width)| (length / 2.0, width / 2.0))
}

/// RECTANGLE from its centre: the centre, then a corner (or a size given by
/// Area or Dimensions, the pointer choosing the quadrant). Rotation turns the
/// rectangle about its centre.
pub fn draw_rectangle_center(ed: &mut Editor) -> Result<(), Cancelled> {
    let centre = first_point(ed, key!("Specify center of rectangle"))?;
    let m = ed.defaults().rect.get();
    let (style, width) = (corners(&m), m.width);
    let level = ed.read(|doc| doc.ccs().angle)?;
    let (mut angle, mut size): (f64, Option<Size>) = (level, None);
    let reach = loop {
        let options = PointOptions::new(key!("Specify corner point"))
            .keyword("Area", "A")
            .keyword("Dimensions", "D")
            .keyword("Rotation", "R")
            .base(centre)
            .corner()
            .ghost(move |at| {
                let (origin, dx, dy) =
                    centred(centre, angle, extent(centre, angle, half(size), at));
                vec![outline(origin, angle, dx, dy, style, width)]
            });
        match ed.ask_point(options)? {
            Answer::Value(at) => {
                let reach = extent(centre, angle, half(size), at);
                if reach.0.abs() > MIN_SIZE && reach.1.abs() > MIN_SIZE {
                    break reach;
                }
                ed.say("Requires a non-zero value.");
            }
            Answer::Keyword(k) if k == "Area" => size = Some(size_from_area(ed)?),
            Answer::Keyword(k) if k == "Dimensions" => {
                let length = positive(ed, key!("Specify length for rectangles"))?;
                let width = positive(ed, key!("Specify width for rectangles"))?;
                size = Some((length, width));
            }
            Answer::Keyword(_) => angle = rotation(ed, centre, angle)?,
            Answer::Enter => return Err(Cancelled),
        }
    };
    let (origin, dx, dy) = centred(centre, angle, reach);
    add(
        ed,
        outline(origin, angle, dx, dy, style, width),
        "Rectangle",
    )
}

/// RECTANGLE by three points: two corners give the first side, at any angle,
/// and a third point (or a typed height, on the left of the first side when
/// positive) the opposite side.
pub fn draw_rectangle_3p(ed: &mut Editor) -> Result<(), Cancelled> {
    let first = first_point(ed, key!("Specify first corner point"))?;
    let m = ed.defaults().rect.get();
    let (style, width) = (corners(&m), m.width);
    let second = other_point(ed, first, key!("Specify second corner point"))?;
    // The first side; `other_point` made sure it has a length.
    let (angle, length, _) = by_three_points(first, second, second).ok_or(Cancelled)?;
    loop {
        let options = NumberOptions::new(key!("Specify height or a point on the opposite side"))
            .ghost(move |at| {
                by_three_points(first, second, at)
                    .map(|(angle, length, height)| {
                        outline(first, angle, length, height, style, width)
                    })
                    .into_iter()
                    .collect()
            });
        let height = match ed.ask_distance_or_point(options)? {
            Answer::Value(NumberOrPoint::Number(height)) => height,
            Answer::Value(NumberOrPoint::Point(at)) => {
                by_three_points(first, second, at).map_or(0.0, |(_, _, height)| height)
            }
            _ => return Err(Cancelled),
        };
        if height.abs() > MIN_SIZE {
            let entity = outline(first, angle, length, height, style, width);
            return add(ed, entity, "Rectangle");
        }
        ed.say("Requires a non-zero value.");
    }
}

/// A point away from `from`, with a rubber band from it.
fn other_point(ed: &Editor, from: Point, message: &str) -> Result<Point, Cancelled> {
    loop {
        let at = ed.get_point(PointOptions::new(message).base(from))?;
        if (at.x - from.x).hypot(at.y - from.y) > MIN_SIZE {
            return Ok(at);
        }
        ed.say("Requires a non-zero value.");
    }
}

/// OBLONG by the centres of its two ends, then its width.
pub fn draw_oblong_2p(ed: &mut Editor) -> Result<(), Cancelled> {
    let a = ed.get_point(key!("Specify center of first end"))?;
    let b = other_point(ed, a, key!("Specify center of second end"))?;
    oblong_of_width(ed, a, b)
}

/// OBLONG from its centre: the centre, the centre of one end (the other end
/// mirrors it), then the width.
pub fn draw_oblong_center(ed: &mut Editor) -> Result<(), Cancelled> {
    let centre = ed.get_point(key!("Specify center of oblong"))?;
    let b = other_point(ed, centre, key!("Specify center of one end"))?;
    let a = Point::new(2.0 * centre.x - b.x, 2.0 * centre.y - b.y);
    oblong_of_width(ed, a, b)
}

/// Asks the width, typed or as a point on a side, and draws the oblong.
fn oblong_of_width(ed: &Editor, a: Point, b: Point) -> Result<(), Cancelled> {
    loop {
        let options = NumberOptions::new(key!("Specify width of oblong")).ghost(move |at| {
            oblong::outline(a, b, oblong::width_through(a, b, at))
                .into_iter()
                .collect()
        });
        let width = match ed.ask_distance_or_point(options)? {
            Answer::Value(NumberOrPoint::Number(width)) => width,
            Answer::Value(NumberOrPoint::Point(at)) => oblong::width_through(a, b, at),
            _ => return Err(Cancelled),
        };
        match oblong::outline(a, b, width) {
            Some(entity) if width > MIN_SIZE => return add(ed, entity, "Oblong"),
            _ => ed.say("Requires a positive value."),
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

    fn run(
        run: fn(&mut Editor) -> Result<(), Cancelled>,
        inputs: &[&str],
    ) -> (AppState, Vec<Entity>) {
        let mut state = AppState::new();
        state.start_command("Test", run);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
        let objects = state.session().document().objects();
        let entities = objects.iter().map(|o| o.entity.clone()).collect();
        (state, entities)
    }

    fn polyline(e: &Entity) -> (Vec<Point>, Vec<f64>) {
        let Entity::Polyline { points, bulges, .. } = e else {
            panic!("a polyline, got {e:?}")
        };
        (points.clone(), bulges.clone())
    }

    fn near(a: &[Point], b: &[Point]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(a, b)| (a.x - b.x).hypot(a.y - b.y) < 1e-9)
    }

    fn bounds(e: &Entity) -> (Point, Point) {
        match e {
            Entity::Rectangle { min, max } => (*min, *max),
            e => panic!("a rectangle, got {e:?}"),
        }
    }

    #[test]
    fn a_rectangle_from_its_centre_reaches_the_corner_on_both_sides() {
        let (state, drawn) = run(draw_rectangle_center, &["5,5", "8,7"]);
        assert!(!state.command_running());
        assert_eq!(bounds(&drawn[0]), (p(2.0, 3.0), p(8.0, 7.0)));
    }

    #[test]
    fn a_rectangle_from_its_centre_takes_the_options_of_rectangle() {
        let (mut state, _) = run(draw_rectangle_center, &[]);
        let labels = |s: &mut AppState| -> Vec<String> {
            s.cmd_prompt()
                .options
                .iter()
                .map(|o| o.label.to_string())
                .collect()
        };
        assert_eq!(labels(&mut state), ["Chamfer", "Fillet", "Width"]);
        let (_, drawn) = run(draw_rectangle_center, &["0,0", "d", "10", "4", "1,1"]);
        assert_eq!(bounds(&drawn[0]), (p(-5.0, -2.0), p(5.0, 2.0)));
        let (_, drawn) = run(draw_rectangle_center, &["0,0", "r", "90", "2,5"]);
        let (points, _) = polyline(&drawn[0]);
        // Turned a quarter: the corner at (2, 5) is 5 along its x axis, -2 along y.
        assert!(near(
            &points,
            &[p(-2.0, -5.0), p(-2.0, 5.0), p(2.0, 5.0), p(2.0, -5.0)]
        ));
        let (_, drawn) = run(draw_rectangle_center, &["f", "1", "0,0", "5,3"]);
        assert_eq!(polyline(&drawn[0]).0.len(), 8, "rounded corners");
    }

    #[test]
    fn a_rectangle_by_three_points_lies_along_its_first_side() {
        let (state, drawn) = run(draw_rectangle_3p, &["0,0", "4,4", "0,8"]);
        assert!(!state.command_running());
        let (points, _) = polyline(&drawn[0]);
        assert!(near(
            &points,
            &[p(0.0, 0.0), p(4.0, 4.0), p(0.0, 8.0), p(-4.0, 4.0)]
        ));
        let (_, drawn) = run(draw_rectangle_3p, &["0,0", "6,0", "2"]);
        assert_eq!(bounds(&drawn[0]), (p(0.0, 0.0), p(6.0, 2.0)));
        let (_, drawn) = run(draw_rectangle_3p, &["0,0", "6,0", "-2"]);
        assert_eq!(bounds(&drawn[0]), (p(0.0, -2.0), p(6.0, 0.0)));
    }

    #[test]
    fn a_flat_rectangle_by_three_points_is_refused() {
        let (state, drawn) = run(draw_rectangle_3p, &["0,0", "0,0"]);
        assert!(drawn.is_empty() && state.command_running());
        let (state, drawn) = run(draw_rectangle_3p, &["0,0", "6,0", "9,0"]);
        assert!(drawn.is_empty() && state.command_running());
    }

    #[test]
    fn an_oblong_by_the_centres_of_its_ends() {
        let (state, drawn) = run(draw_oblong_2p, &["0,0", "10,0", "4"]);
        assert!(!state.command_running());
        let (points, bulges) = polyline(&drawn[0]);
        assert!(near(
            &points,
            &[p(0.0, -2.0), p(10.0, -2.0), p(10.0, 2.0), p(0.0, 2.0)]
        ));
        assert_eq!(bulges, [0.0, 1.0, 0.0, 1.0]);
        let (_, drawn) = run(draw_oblong_2p, &["0,0", "10,0", "3,1.5"]);
        assert!(near(
            &polyline(&drawn[0]).0,
            &[p(0.0, -1.5), p(10.0, -1.5), p(10.0, 1.5), p(0.0, 1.5)]
        ));
    }

    #[test]
    fn an_oblong_from_its_centre_mirrors_the_end_given() {
        let (_, drawn) = run(draw_oblong_center, &["5,5", "5,9", "2"]);
        assert!(near(
            &polyline(&drawn[0]).0,
            &[p(6.0, 1.0), p(6.0, 9.0), p(4.0, 9.0), p(4.0, 1.0)]
        ));
    }

    #[test]
    fn an_oblong_needs_two_ends_and_a_width() {
        let (state, drawn) = run(draw_oblong_2p, &["0,0", "0,0"]);
        assert!(drawn.is_empty() && state.command_running());
        let (state, drawn) = run(draw_oblong_2p, &["0,0", "10,0", "0"]);
        assert!(drawn.is_empty() && state.command_running());
    }
}
