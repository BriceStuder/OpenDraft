//! ARRAYPATH: copies of the selected objects along a path curve, worded as
//! AutoCAD 2020's, recorded as associative or not like the other arrays (see
//! `array_cmd.rs`).

use opendraft_commands::{Answer, Cancelled, Editor, NumberOptions, Options, Picked};
use opendraft_core::array::{Path, Placement};
use opendraft_core::{ArrayRecord, ArrayShape, EntityId, Point};
use opendraft_ui::key;

use crate::array_cmd::{ask_count, edit_prompt, fits, ghost, make, read_source, say_type};
use opendraft_commands::Wording;

/// ARRAYPATH: select objects and the path curve; the items are spaced along
/// it (Measure) or spread over it (Divide) and turn with it, changed by the
/// options until Enter.
pub fn array_path(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select(key!("Select objects"))?;
    path_with(ed, ids, None)
}

/// How the items are laid along the path.
#[derive(Clone, Copy, PartialEq)]
enum Method {
    /// Every so far from the start.
    Measure,
    /// Spread evenly over the whole path.
    Divide,
}

/// A path array being made, or (with `editing`) laid out again along the
/// curve it was made along.
pub(crate) fn path_with(
    ed: &mut Editor,
    ids: Vec<EntityId>,
    editing: Option<ArrayRecord>,
) -> Result<(), Cancelled> {
    let Some(source) = read_source(ed, ids.clone())? else {
        return Ok(());
    };
    say_type(ed, "Path");
    let (dx, dy) = source.spacing();
    let (path, path_id, reversed, mut spacing, mut items, mut method, mut align);
    match editing.as_ref().map(|r| r.shape) {
        Some(ArrayShape::Path {
            path: id,
            reversed: back,
            items: count,
            spacing: step,
            align: aligned,
            divide,
        }) => {
            let found = ed.read(move |doc| doc.get(id).and_then(Path::of))?;
            let Some(found) = found else {
                ed.say(key!("The path curve of the array is gone."));
                return Ok(());
            };
            path = if back { found.reversed() } else { found };
            (path_id, reversed, spacing, items, align) = (id, back, step, count, aligned);
            method = if divide {
                Method::Divide
            } else {
                Method::Measure
            };
        }
        _ => {
            let Some((found, id, back)) = ask_path(ed, ids)? else {
                return Ok(());
            };
            (path, path_id, reversed) = (found, id, back);
            spacing = dx.max(dy);
            items = path.items_fitting(spacing);
            (method, align) = (Method::Measure, true);
        }
    }
    let keywords = [("Method", "M"), ("Items", "I"), ("Align items", "A")];
    loop {
        let placed = path
            .array(items, spacing, align)
            .unwrap_or_else(|| vec![Placement::STAY]);
        let Some(option) = edit_prompt(ed, &keywords, ghost(&source, &placed))? else {
            let shape = ArrayShape::Path {
                path: path_id,
                reversed,
                items,
                spacing,
                align,
                divide: method == Method::Divide,
            };
            return make(ed, source, placed, shape, editing);
        };
        match option.as_str() {
            "Method" => {
                let current = if method == Method::Divide {
                    "Divide"
                } else {
                    "Measure"
                };
                let chosen = ed.get_keyword(
                    Options::new(key!("Enter path method"))
                        .keyword("Divide", "D")
                        .keyword("Measure", "M")
                        .default(current.to_owned()),
                )?;
                method = if chosen == "Divide" {
                    Method::Divide
                } else {
                    Method::Measure
                };
                if method == Method::Divide {
                    spacing = path.spacing_dividing(items);
                }
            }
            "Items" if method == Method::Divide => {
                let count = ask_count(ed, key!("Enter number of items along path"), items)?;
                if fits(ed, &source, count) {
                    items = count;
                    spacing = path.spacing_dividing(items);
                }
            }
            "Items" => {
                let options =
                    NumberOptions::new(key!("Specify the distance between items along path"))
                        .default(spacing);
                let distance = ed.get_distance(options)?;
                if distance <= 1e-9 {
                    ed.say(key!("Requires a positive value."));
                    continue;
                }
                let most = path.items_fitting(distance);
                ed.say(Wording::new(
                    key!("Maximum items = %{most}"),
                    &[("most", &most.to_string())],
                ));
                let options = NumberOptions::new(key!("Specify number of items"))
                    .default(most as f64)
                    .keyword("Fill entire path", "F");
                let count = match ed.ask_number(options)? {
                    Answer::Value(n) if n >= 1.0 && n.fract() == 0.0 => (n as usize).min(most),
                    Answer::Value(_) => {
                        ed.say(key!("Requires a positive integer."));
                        continue;
                    }
                    _ => most,
                };
                if fits(ed, &source, count) {
                    (spacing, items) = (distance, count);
                }
            }
            _ => {
                let current = if align { "Yes" } else { "No" };
                let answer = ed.get_keyword(
                    Options::new(key!("Align arrayed items to the path?"))
                        .keyword("Yes", "Y")
                        .keyword("No", "N")
                        .default(current.to_owned()),
                )?;
                align = answer == "Yes";
            }
        }
    }
}

/// The path curve picked, run from the end nearer the pick, with its id and
/// whether it was turned round to do that. The objects being arrayed cannot be
/// their own path.
fn ask_path(
    ed: &Editor,
    arrayed: Vec<EntityId>,
) -> Result<Option<(Path, EntityId, bool)>, Cancelled> {
    loop {
        let options = Options::<Picked>::new(key!("Select path curve")).allow_empty();
        let Answer::Value(Picked { id, at, .. }) = ed.ask_object(options)? else {
            return Ok(None);
        };
        if arrayed.contains(&id) {
            ed.say(key!("The path cannot be one of the objects arrayed."));
            continue;
        }
        let path = ed.read(move |doc| doc.get(id).and_then(Path::of))?;
        let Some(path) = path else {
            ed.say(key!("Object is not a path curve."));
            continue;
        };
        let near = |p: Point| (p.x - at.x).hypot(p.y - at.y);
        let nearer_end = !path.is_closed() && near(path.end()) < near(path.start());
        let path = if nearer_end { path.reversed() } else { path };
        return Ok(Some((path, id, nearer_end)));
    }
}

#[cfg(test)]
mod tests {
    use opendraft_commands::Reply;
    use opendraft_core::Entity;

    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// A unit square at the origin and a 20-long line along y = 10.
    fn drawing() -> (AppState, EntityId, EntityId) {
        let mut state = AppState::new();
        state.add_entity(Entity::Rectangle {
            min: p(0.0, 0.0),
            max: p(1.0, 1.0),
        });
        state.add_entity(Entity::Line {
            start: p(0.0, 10.0),
            end: p(20.0, 10.0),
        });
        let objects = state.session().document().objects();
        let ids = (objects[0].id, objects[1].id);
        (state, ids.0, ids.1)
    }

    fn answer(state: &mut AppState, replies: &[Reply]) {
        for reply in replies {
            state.reply(reply.clone());
            state.settle_command();
        }
    }

    fn square_mins(state: &AppState) -> Vec<f64> {
        let objects = state.session().document().objects();
        let mut xs: Vec<f64> = objects
            .iter()
            .filter(|o| !matches!(o.entity, Entity::Line { .. }))
            .map(|o| (o.bounds().min.x * 1e6).round() / 1e6)
            .collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs
    }

    #[test]
    fn items_are_measured_along_the_path_from_the_end_picked() {
        let (mut state, square, line) = drawing();
        state.click_select(Some(square), false);
        state.start_command("ARRAYPATH", array_path);
        state.settle_command();
        state.answer_object(p(1.0, 10.0), Some(line), false);
        state.settle_command();
        answer(
            &mut state,
            &[
                Reply::Keyword("Items".into()),
                Reply::Number(5.0),
                Reply::Enter,
                Reply::Enter,
            ],
        );
        assert!(!state.command_running());
        assert_eq!(square_mins(&state), [0.0, 5.0, 10.0, 15.0, 20.0]);
    }

    #[test]
    fn divide_spreads_the_items_over_the_path_and_the_far_end_reverses_it() {
        let (mut state, square, line) = drawing();
        state.click_select(Some(square), false);
        state.start_command("ARRAYPATH", array_path);
        state.settle_command();
        state.answer_object(p(19.0, 10.0), Some(line), false);
        state.settle_command();
        answer(
            &mut state,
            &[
                Reply::Keyword("Method".into()),
                Reply::Keyword("Divide".into()),
                Reply::Keyword("Items".into()),
                Reply::Number(3.0),
                Reply::Enter,
            ],
        );
        assert_eq!(
            square_mins(&state),
            [-20.0, -10.0, 0.0],
            "running back from x = 20"
        );
    }

    #[test]
    fn the_path_must_be_a_curve_and_not_one_of_the_objects() {
        let (mut state, square, _) = drawing();
        state.click_select(Some(square), false);
        state.start_command("ARRAYPATH", array_path);
        state.settle_command();
        state.answer_object(p(0.0, 0.0), Some(square), false);
        state.settle_command();
        let said = state.cmdline.log.last().cloned().unwrap_or_default();
        assert_eq!(said, "The path cannot be one of the objects arrayed.");
        assert!(state.command_running());
    }
}
