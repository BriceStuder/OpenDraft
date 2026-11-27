//! The parts of JOIN that go beyond ends that meet: arcs of one circle joined
//! across a gap, and the Close option (a lone arc, elliptical arc or open
//! polyline made whole).

use std::f64::consts::TAU;

use crate::join::Merged;
use crate::{arc, Ellipse, Entity, Point};

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// The one arc (or circle) that covers every arc of `items`, when the objects
/// JOIN can use are at least two arcs, all on one circle (within
/// `tolerance`), meeting or not. As in AutoCAD the first arc is the source:
/// the result starts where it does and runs counter-clockwise until it has
/// covered the others, across any gap. Objects JOIN cannot use do not count.
pub(crate) fn arcs_on_one_circle(items: &[Entity], tolerance: f64) -> Option<Merged> {
    let mut arcs: Vec<(usize, Point, f64, f64, f64)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        match item {
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } if *radius > 0.0 && arc::sweep(*start, *end) < TAU - 1e-9 => {
                arcs.push((i, *center, *radius, *start, *end));
            }
            Entity::Line { .. } | Entity::Polyline { closed: false, .. } => return None,
            Entity::Ellipse(e) if !e.is_full() => return None,
            _ => {}
        }
    }
    let &(_, center, radius, source_start, _) = arcs.first()?;
    if arcs.len() < 2 {
        return None;
    }
    if arcs
        .iter()
        .any(|a| distance(a.1, center) > tolerance || (a.2 - radius).abs() > tolerance)
    {
        return None;
    }
    // How far round from the source's start each arc ends, going counter-clockwise.
    let reach = arcs
        .iter()
        .map(|&(_, _, _, start, end)| arc::normalize(start - source_start) + arc::sweep(start, end))
        .fold(0.0, f64::max);
    let entity = if reach >= TAU - 1e-9 {
        Entity::Circle { center, radius }
    } else {
        Entity::Arc {
            center,
            radius,
            start: source_start,
            end: arc::normalize(source_start + reach),
        }
    };
    Some(Merged {
        used: arcs.iter().map(|a| a.0).collect(),
        entity,
    })
}

/// The object made whole, for JOIN's Close option: an arc becomes its circle,
/// an elliptical arc its ellipse, and an open polyline is closed with a
/// straight segment. `None` for anything else.
pub fn closed(entity: &Entity) -> Option<Entity> {
    match entity {
        Entity::Arc { center, radius, .. } if *radius > 0.0 => Some(Entity::Circle {
            center: *center,
            radius: *radius,
        }),
        Entity::Ellipse(e) if !e.is_full() => Ellipse::full(e.center, e.major_axis, e.radius_ratio)
            .ok()
            .map(Entity::Ellipse),
        Entity::Polyline {
            points,
            closed: false,
            bulges,
            widths,
        } if points.len() >= 3 => {
            // An end that already sits on the start is the same corner.
            let mut points = points.clone();
            let mut bulges = bulges.clone();
            let mut widths = widths.clone();
            if points.len() > 3 && distance(points[0], points[points.len() - 1]) <= 1e-9 {
                points.pop();
                bulges.truncate(points.len());
                widths.truncate(points.len());
            }
            // The last segment of an open polyline is straight; it now closes.
            if let Some(last) = bulges.last_mut() {
                *last = 0.0;
            }
            Some(Entity::Polyline {
                points,
                closed: true,
                bulges,
                widths,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "join_gap_tests.rs"]
mod tests;
