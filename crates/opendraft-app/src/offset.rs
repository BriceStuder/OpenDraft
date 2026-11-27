//! OFFSET: a parallel copy of an object at a distance, or through a point.
//! Written against the `Editor`; the geometry is `Entity::offset` in
//! `opendraft_core`.

use opendraft_commands::{Answer, Cancelled, Editor, NumberOptions, Options, Picked, PointOptions};
use opendraft_core::{Entity, Point};
use opendraft_ui::key;

/// Distances at or below this are refused.
const MIN_DISTANCE: f64 = 1e-9;

/// How far the copy is from the object.
#[derive(Clone, Copy)]
enum Gap {
    /// A fixed distance, on the side the user points to.
    Distance(f64),
    /// As far as the point the user gives.
    Through,
}

/// Which layer the copies go on.
#[derive(Clone, Copy, PartialEq)]
enum CopyLayer {
    /// The object's own layer.
    Source,
    /// The current layer.
    Current,
}

/// The copy of `target` for a click at `at`, if there is one.
fn copy_for(target: &Entity, gap: Gap, at: Point) -> Option<Entity> {
    let distance = match gap {
        Gap::Distance(d) => d,
        Gap::Through => target.distance_to(at),
    };
    target.offset(distance, at)
}

/// OFFSET: ask for a distance (or Through), then offset one object after
/// another: click the object, then the side. Enter or Esc ends. Each copy is
/// its own undo step and takes the object's attributes, and its layer unless
/// the Layer option said Current. At the side prompt, Multiple keeps offsetting
/// the same object with clicks: by a distance each copy is measured from the
/// last one; through points are each measured from the object. Exit ends the
/// command and Undo takes back the last copy, at either prompt. A distance
/// is remembered (`OFFSETDIST`) and offered by Enter.
pub fn offset_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let (mut erase, mut layer) = (false, CopyLayer::Source);
    let gap = loop {
        let remembered = ed.read(|doc| doc.modify_settings().offset_distance)?;
        // After Through, Enter is Through again.
        let through_next = remembered < 0.0;
        let message = if through_next {
            key!("Specify offset distance, Enter for Through")
        } else {
            key!("Specify offset distance")
        };
        let mut options = NumberOptions::new(message);
        if !through_next {
            options = options.default(remembered);
        }
        let options = options
            .keyword("Through", "T")
            .keyword("Erase", "E")
            .keyword("Layer", "L");
        match ed.ask_distance(options)? {
            Answer::Value(d) if d > MIN_DISTANCE => {
                ed.change_modify_settings(move |s| s.offset_distance = d.abs())?;
                break Gap::Distance(d);
            }
            Answer::Value(_) => ed.say(key!("Requires a positive value.")),
            Answer::Keyword(k) if k == "Through" => {
                ed.change_modify_settings(|s| s.offset_distance = -s.offset_distance.abs())?;
                break Gap::Through;
            }
            Answer::Keyword(k) if k == "Layer" => {
                let answer = ed.get_keyword(
                    Options::<String>::new(key!("Enter layer option for offset objects"))
                        .keyword("Current", "C")
                        .keyword("Source", "S")
                        .default("Source".to_owned()),
                )?;
                layer = if answer == "Current" {
                    CopyLayer::Current
                } else {
                    CopyLayer::Source
                };
            }
            Answer::Keyword(_) => {
                let answer = ed.get_keyword(
                    Options::<String>::new(key!("Erase source object after offsetting?"))
                        .keyword("Yes", "Y")
                        .keyword("No", "N")
                        .default("No".to_owned()),
                )?;
                erase = answer == "Yes";
            }
            Answer::Enter if through_next => break Gap::Through,
            Answer::Enter => return Err(Cancelled),
        }
    };
    // The copies made so far that Undo can still take back.
    let mut made = 0_usize;
    loop {
        let mut options =
            Options::<Picked>::new(key!("Select object to offset or <exit>")).keyword("Exit", "E");
        if made > 0 {
            options = options.keyword("Undo", "U");
        }
        let id = match ed.ask_object(options) {
            Ok(Answer::Value(Picked { id, .. })) => id,
            Ok(Answer::Keyword(k)) if k == "Undo" => {
                ed.undo()?;
                made -= 1;
                continue;
            }
            _ => return Ok(()),
        };
        let Some(object) = ed.read(move |doc| doc.object(id).cloned())? else {
            continue;
        };
        let target = object.entity.clone();
        let message = match gap {
            Gap::Distance(_) => key!("Specify point on side to offset"),
            Gap::Through => key!("Specify through point"),
        };
        let (mut source, mut multiple, mut erase_now) = (target, false, erase);
        // What this object was before each of its copies, for Undo.
        let mut before: Vec<(Entity, bool)> = Vec::new();
        loop {
            let shown = source.clone();
            let mut options = PointOptions::new(message)
                .ghost(move |at| copy_for(&shown, gap, at).into_iter().collect())
                .keyword("Exit", "E");
            if !multiple {
                options = options.keyword("Multiple", "M");
            }
            if made > 0 {
                options = options.keyword("Undo", "U");
            }
            let at = match ed.ask_point(options) {
                Ok(Answer::Value(at)) => at,
                Ok(Answer::Keyword(k)) if k == "Exit" => return Ok(()),
                Ok(Answer::Keyword(k)) if k == "Undo" => {
                    ed.undo()?;
                    made -= 1;
                    match before.pop() {
                        // A copy of this object: its next copy starts again from there.
                        Some((was, erased)) => (source, erase_now) = (was, erased),
                        // One of an earlier object: back to choosing.
                        None => break,
                    }
                    continue;
                }
                Ok(Answer::Keyword(_)) => {
                    multiple = true;
                    continue;
                }
                Ok(Answer::Enter) => break,
                Err(Cancelled) => return Ok(()),
            };
            let Some(copy) = copy_for(&source, gap, at) else {
                ed.say(key!("Cannot offset the object that way."));
                if multiple {
                    continue;
                }
                break;
            };
            let mut attrs = object.attrs.clone();
            if layer == CopyLayer::Current {
                attrs.layer = ed.read(|doc| doc.current_layer())?;
            }
            let next = copy.clone();
            before.push((source.clone(), erase_now));
            let remove = std::mem::take(&mut erase_now);
            ed.transaction("Offset", move |tx| {
                tx.add_with(vec![(copy, attrs)]);
                if remove {
                    tx.remove(vec![id]);
                }
                Ok(())
            })?;
            made += 1;
            if !multiple {
                break;
            }
            // By a distance, the next copy is measured from this one.
            if matches!(gap, Gap::Distance(_)) {
                source = next;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn offsetting(entities: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in entities {
            state.add_entity(e.clone());
        }
        state.start_command("Offset", offset_objects);
        state.settle_command();
        state
    }

    fn click(state: &mut AppState, index: usize, at: Point) {
        let id = state.session().document().objects()[index].id;
        state.answer_object(at, Some(id), false);
        state.settle_command();
    }

    #[test]
    fn a_distance_then_an_object_then_a_side_makes_a_copy_each_time() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("2");
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select object to offset"));
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,9");
        state.settle_command();
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(10.0, 0.0)),
                line(p(0.0, 2.0), p(10.0, 2.0))
            ]
        );
        // Another object straight away, on the other side this time.
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,-9");
        state.settle_command();
        assert_eq!(entities(&state).len(), 3);
        assert_eq!(entities(&state)[2], line(p(0.0, -2.0), p(10.0, -2.0)));
        assert!(state.command_running());
        state.cmd_submit("");
        state.settle_command();
        assert!(!state.command_running(), "Enter ends it");
        // Each copy was one undo step.
        state.undo();
        assert_eq!(entities(&state).len(), 2);
    }

    #[test]
    fn through_takes_the_distance_from_a_point() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("t");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        assert!(state.cmd_prompt().text.starts_with("Specify through point"));
        state.cmd_submit("3,7");
        state.settle_command();
        assert_eq!(entities(&state)[1], line(p(0.0, 7.0), p(10.0, 7.0)));
    }

    fn layers(state: &AppState) -> Vec<opendraft_core::LayerId> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.attrs.layer).collect()
    }

    #[test]
    fn layer_current_puts_the_copy_on_the_current_layer() {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        let source = layers(&state)[0];
        let walls = state.new_layer();
        state.set_current_layer(walls);
        state.start_command("Offset", offset_objects);
        state.settle_command();
        state.cmd_submit("l");
        state.settle_command();
        state.cmd_submit("c");
        state.settle_command();
        assert!(
            state
                .cmd_prompt()
                .text
                .starts_with("Specify offset distance"),
            "back to asking for the distance"
        );
        state.cmd_submit("2");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,9");
        state.settle_command();
        assert_eq!(layers(&state), [source, walls]);
        assert_ne!(source, walls);
    }

    #[test]
    fn layer_source_and_the_default_keep_the_objects_layer() {
        for answers in [&["l", "s"][..], &["l", ""][..], &[][..]] {
            let mut state = AppState::new();
            state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
            let source = layers(&state)[0];
            let walls = state.new_layer();
            state.set_current_layer(walls);
            state.start_command("Offset", offset_objects);
            state.settle_command();
            for a in answers {
                state.cmd_submit(a);
                state.settle_command();
            }
            state.cmd_submit("2");
            state.settle_command();
            click(&mut state, 0, p(5.0, 0.0));
            state.cmd_submit("5,9");
            state.settle_command();
            assert_eq!(layers(&state), [source, source], "{answers:?}");
        }
    }

    /// Whether the waiting prompt offers the Multiple option.
    fn offers_multiple(state: &mut AppState) -> bool {
        state
            .cmd_prompt()
            .options
            .iter()
            .any(|o| o.value == "Multiple")
    }

    /// Offset by 2 and answer Multiple at the side prompt.
    fn multiple(entities: &[Entity], distance: &str) -> AppState {
        let mut state = offsetting(entities);
        state.cmd_submit(distance);
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("m");
        state.settle_command();
        state
    }

    #[test]
    fn multiple_measures_each_copy_from_the_last_by_a_distance() {
        let mut state = multiple(&[line(p(0.0, 0.0), p(10.0, 0.0))], "2");
        for _ in 0..3 {
            assert!(state
                .cmd_prompt()
                .text
                .starts_with("Specify point on side to offset"));
            state.cmd_submit("5,100");
            state.settle_command();
        }
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(10.0, 0.0)),
                line(p(0.0, 2.0), p(10.0, 2.0)),
                line(p(0.0, 4.0), p(10.0, 4.0)),
                line(p(0.0, 6.0), p(10.0, 6.0)),
            ]
        );
        assert!(state.command_running(), "still at the side prompt");
        // Each copy is its own undo step.
        state.undo();
        assert_eq!(entities(&state).len(), 3);
    }

    #[test]
    fn enter_leaves_multiple_and_asks_for_the_next_object() {
        let mut state = multiple(&[line(p(0.0, 0.0), p(10.0, 0.0))], "2");
        state.cmd_submit("5,9");
        state.settle_command();
        state.cmd_submit("");
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select object to offset"));
        // Multiple is for the one object: the next one starts again.
        click(&mut state, 0, p(5.0, 0.0));
        assert!(offers_multiple(&mut state));
    }

    #[test]
    fn multiple_through_measures_each_point_from_the_object() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("t");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("m");
        state.settle_command();
        state.cmd_submit("3,7");
        state.settle_command();
        state.cmd_submit("3,-4");
        state.settle_command();
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(10.0, 0.0)),
                line(p(0.0, 7.0), p(10.0, 7.0)),
                line(p(0.0, -4.0), p(10.0, -4.0)),
            ]
        );
    }

    #[test]
    fn multiple_with_erase_removes_only_the_original() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        for answer in ["e", "y", "2"] {
            state.cmd_submit(answer);
            state.settle_command();
        }
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("m");
        state.settle_command();
        for _ in 0..2 {
            state.cmd_submit("5,100");
            state.settle_command();
        }
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 2.0), p(10.0, 2.0)),
                line(p(0.0, 4.0), p(10.0, 4.0))
            ]
        );
    }

    #[test]
    fn multiple_is_offered_once_and_a_failed_copy_stays_in_multiple() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        };
        let mut state = offsetting(&[circle]);
        state.cmd_submit("3");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        assert!(offers_multiple(&mut state));
        state.cmd_submit("m");
        state.settle_command();
        assert!(!offers_multiple(&mut state), "once only");
        // 3 inside is fine (radius 2); then 3 more would vanish, so it is refused
        // and the same prompt asks again.
        state.cmd_submit("0,0");
        state.settle_command();
        state.cmd_submit("0,0");
        state.settle_command();
        assert_eq!(entities(&state).len(), 2);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("Cannot offset")));
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Specify point on side to offset"));
    }

    #[test]
    fn erase_removes_the_source() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("e");
        state.settle_command();
        state.cmd_submit("y");
        state.settle_command();
        assert!(
            state
                .cmd_prompt()
                .text
                .starts_with("Specify offset distance"),
            "back to asking for the distance"
        );
        state.cmd_submit("2");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,9");
        state.settle_command();
        assert_eq!(entities(&state), [line(p(0.0, 2.0), p(10.0, 2.0))]);
    }

    #[test]
    fn an_ellipse_is_offset_into_a_spline() {
        let e = opendraft_core::Ellipse::full(p(0.0, 0.0), p(6.0, 0.0), 0.5).unwrap();
        let mut state = offsetting(&[Entity::Ellipse(e)]);
        state.cmd_submit("1");
        state.settle_command();
        click(&mut state, 0, p(6.0, 0.0));
        state.cmd_submit("9,0");
        state.settle_command();
        let all = entities(&state);
        assert_eq!(all.len(), 2, "{all:?}");
        assert_eq!(all[0], Entity::Ellipse(e));
        let Entity::Spline(copy) = &all[1] else {
            panic!("a spline, got {:?}", all[1])
        };
        assert!(copy.closed);
        assert!((copy.nearest(p(30.0, 0.0)).0.x - 7.0).abs() < 1e-3);
        state.undo();
        assert_eq!(entities(&state).len(), 1);
    }

    #[test]
    fn an_offset_that_would_fold_a_curve_is_refused_and_the_next_object_asked_for() {
        let e = opendraft_core::Ellipse::full(p(0.0, 0.0), p(6.0, 0.0), 0.5).unwrap();
        let mut state = offsetting(&[Entity::Ellipse(e)]);
        // The tightest curve of this ellipse has radius 1.5.
        state.cmd_submit("2");
        state.settle_command();
        click(&mut state, 0, p(6.0, 0.0));
        state.cmd_submit("0,0");
        state.settle_command();
        assert_eq!(entities(&state), [Entity::Ellipse(e)]);
        assert!(state.cmdline.log.last().unwrap().contains("Cannot offset"));
    }

    #[test]
    fn a_spline_is_offset_to_the_side_clicked() {
        let s = Entity::Spline(Box::new(
            opendraft_core::Spline::from_control_points(
                3,
                vec![p(0.0, 0.0), p(2.0, 6.0), p(8.0, 6.0), p(10.0, 0.0)],
            )
            .unwrap(),
        ));
        let mut state = offsetting(std::slice::from_ref(&s));
        state.cmd_submit("1");
        state.settle_command();
        click(&mut state, 0, p(5.0, 4.5));
        state.cmd_submit("5,12");
        state.settle_command();
        let all = entities(&state);
        assert_eq!(all.len(), 2, "{all:?}");
        let (Entity::Spline(base), Entity::Spline(copy)) = (&all[0], &all[1]) else {
            panic!("{all:?}")
        };
        let (a, b) = copy.domain();
        for i in 0..=20 {
            let q = copy.point_at(a + (b - a) * f64::from(i) / 20.0);
            assert!((base.distance_to(q) - 1.0).abs() < 1e-3, "{q:?}");
        }
        assert!(copy.nearest(p(5.0, 30.0)).0.y > base.nearest(p(5.0, 30.0)).0.y);
    }

    #[test]
    fn a_circle_that_would_vanish_is_refused_and_asks_for_the_next_object() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 2.0,
        };
        let mut state = offsetting(&[circle]);
        state.cmd_submit("5");
        state.settle_command();
        click(&mut state, 0, p(2.0, 0.0));
        state.cmd_submit("0,0");
        state.settle_command();
        assert_eq!(entities(&state).len(), 1);
        assert!(state.cmdline.log.last().unwrap().contains("Cannot offset"));
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select object to offset"));
    }

    #[test]
    fn a_zero_distance_is_refused() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("0");
        state.settle_command();
        assert!(state.cmdline.log.last().unwrap().contains("positive"));
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Specify offset distance"));
    }

    fn offers(state: &mut AppState, label: &str) -> bool {
        state.cmd_prompt().options.iter().any(|o| o.label == label)
    }

    #[test]
    fn undo_in_multiple_takes_back_the_last_copy_and_carries_on_from_the_one_before() {
        let mut state = multiple(&[line(p(0.0, 0.0), p(10.0, 0.0))], "2");
        assert!(!offers(&mut state, "Undo"), "nothing to undo yet");
        for _ in 0..2 {
            state.cmd_submit("5,100");
            state.settle_command();
        }
        assert!(offers(&mut state, "Undo"));
        state.cmd_submit("u");
        state.settle_command();
        assert_eq!(entities(&state).len(), 2);
        assert!(state.command_running());
        // The next copy is measured from the one that is left.
        state.cmd_submit("5,100");
        state.settle_command();
        assert_eq!(entities(&state)[2], line(p(0.0, 4.0), p(10.0, 4.0)));
    }

    #[test]
    fn undo_after_a_single_copy_returns_to_choosing_an_object() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("2");
        state.settle_command();
        assert!(!offers(&mut state, "Undo"));
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,9");
        state.settle_command();
        assert_eq!(entities(&state).len(), 2);
        // Back at the object prompt, Undo takes the copy away.
        assert!(offers(&mut state, "Undo") && offers(&mut state, "Exit"));
        state.cmd_submit("u");
        state.settle_command();
        assert_eq!(entities(&state).len(), 1);
        assert!(!offers(&mut state, "Undo"));
        // And from the side prompt, Undo reaches the copy of the object before.
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,9");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        assert!(offers(&mut state, "Undo"));
        state.cmd_submit("u");
        state.settle_command();
        assert_eq!(entities(&state).len(), 1);
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select object to offset"));
    }

    #[test]
    fn undo_puts_back_an_object_erased_by_its_copy() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("e");
        state.settle_command();
        state.cmd_submit("y");
        state.settle_command();
        state.cmd_submit("2");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,9");
        state.settle_command();
        assert_eq!(entities(&state), [line(p(0.0, 2.0), p(10.0, 2.0))]);
        state.cmd_submit("u");
        state.settle_command();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(10.0, 0.0))]);
    }

    #[test]
    fn exit_ends_the_command_at_either_prompt() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("2");
        state.settle_command();
        assert!(offers(&mut state, "Exit"));
        state.cmd_submit("e");
        state.settle_command();
        assert!(!state.command_running());
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("2");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        assert!(offers(&mut state, "Exit"));
        state.cmd_submit("e");
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(entities(&state).len(), 1);
    }

    #[test]
    fn the_last_distance_is_offered_next_time_and_enter_takes_it() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        assert!(
            state.cmd_prompt().text.contains("<1"),
            "{}",
            state.cmd_prompt().text
        );
        state.cmd_submit("3");
        state.settle_command();
        assert_eq!(state.modify_settings().offset_distance, 3.0);
        state.cmd_submit("");
        state.settle_command();
        state.cmd_submit("");
        state.settle_command();
        // A second run starts with 3 as the default, and Enter uses it.
        state.start_command("Offset", offset_objects);
        state.settle_command();
        assert!(
            state.cmd_prompt().text.contains("<3"),
            "{}",
            state.cmd_prompt().text
        );
        state.cmd_submit("");
        state.settle_command();
        click(&mut state, 0, p(5.0, 0.0));
        state.cmd_submit("5,9");
        state.settle_command();
        assert_eq!(entities(&state)[1], line(p(0.0, 3.0), p(10.0, 3.0)));
    }

    #[test]
    fn through_is_remembered_and_enter_takes_it_until_a_distance_replaces_it() {
        let mut state = offsetting(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        state.cmd_submit("t");
        state.settle_command();
        assert!(state.modify_settings().offset_distance < 0.0);
        state.cmd_submit("");
        state.settle_command();
        state.start_command("Offset", offset_objects);
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Specify offset distance, Enter for Through"));
        state.cmd_submit("");
        state.settle_command();
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Select object to offset"));
        click(&mut state, 0, p(5.0, 0.0));
        assert!(state.cmd_prompt().text.starts_with("Specify through point"));
        state.cmd_submit("");
        state.settle_command();
        state.cmd_submit("");
        state.settle_command();
        // A typed distance goes back to offering a number.
        state.start_command("Offset", offset_objects);
        state.settle_command();
        state.cmd_submit("4");
        state.settle_command();
        assert_eq!(state.modify_settings().offset_distance, 4.0);
    }
}
