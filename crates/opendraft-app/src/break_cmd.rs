//! BREAK: remove the part of an object between two points, and BREAKATPOINT:
//! divide an object in two at one point. The geometry is
//! `Entity::break_between` in `opendraft_core`.

use opendraft_commands::{Answer, Cancelled, Editor, Options, Picked, PointOptions};
use opendraft_core::Point;
use opendraft_ui::key;

/// BREAK: click the object (the click is the first break point), then give
/// the second break point. First point re-asks for the first one. Giving the
/// same point twice (`@0,0`) divides the object without removing anything.
/// One undo step.
pub fn break_object(ed: &mut Editor) -> Result<(), Cancelled> {
    // Like AutoCAD, Break takes one object by clicking it, selected or not.
    let Answer::Value(Picked { id, at, .. }) =
        ed.ask_object(Options::<Picked>::new(key!("Select object")).allow_empty())?
    else {
        return Ok(());
    };
    let Some(object) = ed.read(move |doc| doc.object(id).cloned())? else {
        return Ok(());
    };
    let target = object.entity.clone();
    if matches!(target, opendraft_core::Entity::Spline(_)) {
        ed.say("BREAK does not support splines yet.");
        return Ok(());
    }
    let mut first = at;
    let mut options = break_options(&target, first, key!("Specify second break point"))
        .keyword("First point", "F");
    let second = loop {
        match ed.ask_point(options)? {
            Answer::Value(p) => break p,
            Answer::Keyword(_) => {
                first = ed.get_point(PointOptions::new(key!("Specify first break point")))?;
                options = break_options(&target, first, key!("Specify second break point"));
            }
            Answer::Enter => return Err(Cancelled),
        }
    };
    let Some(left) = target.break_between(first, second) else {
        ed.say("Object cannot be broken.");
        return Ok(());
    };
    if left.len() == 1 && left[0] == target {
        ed.say("Nothing to break there.");
        return Ok(());
    }
    record(ed, "Break", id, object.attrs.clone(), left)
}

/// BREAKATPOINT: click the object, then give the point where it is divided in
/// two (the Midpoint snap gives two equal halves). Nothing is removed. Unlike
/// AutoCAD's BREAK, no `@` is typed: the second prompt is the break point
/// itself. A circle or another closed shape cannot be divided at a single
/// point. One undo step.
pub fn break_at_point(ed: &mut Editor) -> Result<(), Cancelled> {
    let Answer::Value(Picked { id, .. }) =
        ed.ask_object(Options::<Picked>::new(key!("Select object")).allow_empty())?
    else {
        return Ok(());
    };
    let Some(object) = ed.read(move |doc| doc.object(id).cloned())? else {
        return Ok(());
    };
    let target = object.entity.clone();
    if matches!(target, opendraft_core::Entity::Spline(_)) {
        ed.say("BREAKATPOINT does not support splines yet.");
        return Ok(());
    }
    let at = ed.get_point(PointOptions::new(key!("Specify break point")))?;
    let Some(pieces) = target.break_between(at, at) else {
        ed.say("Object cannot be broken.");
        return Ok(());
    };
    if pieces.len() < 2 {
        ed.say(if pieces.first() == Some(&target) && is_closed(&target) {
            "A closed object cannot be broken at a single point."
        } else {
            "Nothing to break there: the point is at an end of the object."
        });
        return Ok(());
    }
    record(ed, "Break at Point", id, object.attrs.clone(), pieces)
}

/// Whether the object closes on itself, so one point cannot open it.
fn is_closed(entity: &opendraft_core::Entity) -> bool {
    use opendraft_core::Entity;
    matches!(
        entity,
        Entity::Circle { .. } | Entity::Rectangle { .. } | Entity::Polyline { closed: true, .. }
    ) || matches!(entity, Entity::Ellipse(e) if e.is_full())
}

/// Put what is left of object `id` in the drawing, as one undo step.
fn record(
    ed: &Editor,
    name: &'static str,
    id: opendraft_core::EntityId,
    attrs: opendraft_core::Attributes,
    mut left: Vec<opendraft_core::Entity>,
) -> Result<(), Cancelled> {
    ed.transaction(name, move |tx| {
        if left.is_empty() {
            tx.remove(vec![id]);
            return Ok(());
        }
        // The first piece keeps the object's identity and place in the draw
        // order; any others are new objects on the same layer.
        let kept = left.remove(0);
        tx.modify(id, move |_| kept);
        if !left.is_empty() {
            tx.add_with(left.into_iter().map(|e| (e, attrs.clone())).collect());
        }
        Ok(())
    })
}

/// A point prompt that previews what is left when the break ends at the
/// pointer.
fn break_options(target: &opendraft_core::Entity, first: Point, message: &str) -> PointOptions {
    let shown = target.clone();
    PointOptions::new(message)
        .base(first)
        .ghost(move |at| shown.break_between(first, at).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use opendraft_core::Entity;

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

    fn start(state: &mut AppState) -> opendraft_core::EntityId {
        let id = state.session().document().objects()[0].id;
        state.start_command("Break", break_object);
        state.settle_command();
        id
    }

    #[test]
    fn a_spline_is_refused_by_name() {
        let mut state = AppState::new();
        let s = Entity::Spline(Box::new(
            opendraft_core::Spline::from_control_points(1, vec![p(-5.0, 0.0), p(5.0, 0.0)])
                .unwrap(),
        ));
        state.add_entity(s.clone());
        let id = start(&mut state);
        state.answer_object(p(0.0, 0.0), Some(id), false);
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(entities(&state), [s]);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("BREAK does not support splines yet.")));
    }

    #[test]
    fn click_then_second_point_removes_the_part_between_in_one_undo_step() {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        let id = start(&mut state);
        state.answer_object(p(3.0, 0.0), Some(id), false);
        state.answer_point(p(7.0, 0.0));
        state.settle_command();
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(3.0, 0.0)),
                line(p(7.0, 0.0), p(10.0, 0.0))
            ]
        );
        state.undo();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(10.0, 0.0))]);
    }

    fn start_at_point(state: &mut AppState) -> opendraft_core::EntityId {
        let id = state.session().document().objects()[0].id;
        state.start_command("Break at Point", break_at_point);
        state.settle_command();
        id
    }

    #[test]
    fn break_at_point_divides_a_line_in_two_in_one_undo_step() {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        let id = start_at_point(&mut state);
        // The click on the object does not count: only the break point does.
        state.answer_object(p(8.0, 0.0), Some(id), false);
        state.answer_point(p(5.0, 0.0));
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(5.0, 0.0)),
                line(p(5.0, 0.0), p(10.0, 0.0))
            ]
        );
        let objects = state.session().document().objects();
        assert_eq!(objects[0].id, id, "the first half keeps the object");
        assert_eq!(objects[0].attrs, objects[1].attrs, "same layer and colour");
        state.undo();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(10.0, 0.0))]);
    }

    #[test]
    fn break_at_point_takes_the_nearest_point_of_the_object() {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        let id = start_at_point(&mut state);
        state.answer_object(p(1.0, 0.0), Some(id), false);
        state.answer_point(p(4.0, 3.0));
        state.settle_command();
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(4.0, 0.0)),
                line(p(4.0, 0.0), p(10.0, 0.0))
            ]
        );
    }

    #[test]
    fn break_at_point_leaves_a_circle_and_an_end_point_alone() {
        let mut state = AppState::new();
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        };
        state.add_entity(circle.clone());
        let id = start_at_point(&mut state);
        state.answer_object(p(5.0, 0.0), Some(id), false);
        state.answer_point(p(0.0, 5.0));
        state.settle_command();
        assert_eq!(entities(&state), [circle]);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("A closed object cannot be broken at a single point.")));

        let mut state = AppState::new();
        let whole = line(p(0.0, 0.0), p(10.0, 0.0));
        state.add_entity(whole.clone());
        let id = start_at_point(&mut state);
        state.answer_object(p(5.0, 0.0), Some(id), false);
        state.answer_point(p(10.0, 0.0));
        state.settle_command();
        assert_eq!(entities(&state), [whole]);
    }

    #[test]
    fn the_first_point_option_replaces_the_click() {
        let mut state = AppState::new();
        state.add_entity(line(p(0.0, 0.0), p(10.0, 0.0)));
        let id = start(&mut state);
        state.answer_object(p(9.0, 0.0), Some(id), false);
        state.answer_keyword("First point");
        state.answer_point(p(2.0, 0.0));
        state.answer_point(p(5.0, 0.0));
        state.settle_command();
        assert_eq!(
            entities(&state),
            [
                line(p(0.0, 0.0), p(2.0, 0.0)),
                line(p(5.0, 0.0), p(10.0, 0.0))
            ]
        );
    }
}
