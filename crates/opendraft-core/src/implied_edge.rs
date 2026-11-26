//! The implied extension of a cutting or boundary edge: TRIM and EXTEND's Edge
//! option (`EDGEMODE` 1) lets an edge act as if it ran on past its ends along
//! its own path, so an object can be trimmed to, or extended up to, a point
//! where the edge would be but is not.
//!
//! A line becomes the infinite line through it, an arc its whole circle, an
//! elliptical arc its whole ellipse, and an open polyline keeps its shape and
//! gains a ray (or the whole circle of an arc segment) beyond each end.
//! Anything else stays as it is.

use crate::infinite::unit;
use crate::{bulge, Ellipse, Entity, Point};

impl Entity {
    /// What stands for this edge when it is extended along its path: the
    /// edge itself, or the longer thing it implies, as one or more entities.
    pub fn with_implied_extension(&self) -> Vec<Entity> {
        match self {
            Entity::Line { start, end } => {
                let direction = unit(Point::new(end.x - start.x, end.y - start.y));
                vec![direction.map_or_else(
                    || self.clone(),
                    |direction| Entity::Xline {
                        origin: *start,
                        direction,
                    },
                )]
            }
            Entity::Arc { center, radius, .. } => vec![Entity::Circle {
                center: *center,
                radius: *radius,
            }],
            Entity::Ellipse(e) if !e.is_full() => {
                vec![Ellipse::full(e.center, e.major_axis, e.radius_ratio)
                    .map_or_else(|_| self.clone(), Entity::Ellipse)]
            }
            Entity::Polyline {
                points,
                closed: false,
                bulges,
                ..
            } if points.len() >= 2 => {
                let n = points.len();
                let mut made = vec![self.clone()];
                made.extend(beyond(
                    points[0],
                    points[1],
                    bulge::at(bulges, 0),
                    Side::Start,
                ));
                made.extend(beyond(
                    points[n - 2],
                    points[n - 1],
                    bulge::at(bulges, n - 2),
                    Side::End,
                ));
                made
            }
            _ => vec![self.clone()],
        }
    }
}

/// `edges`, each as what it implies.
pub fn implied_edges(edges: &[Entity]) -> Vec<Entity> {
    edges
        .iter()
        .flat_map(Entity::with_implied_extension)
        .collect()
}

/// Which end of the segment `a` to `b` is the end of the polyline.
#[derive(Clone, Copy)]
enum Side {
    Start,
    End,
}

/// What continues the segment `a` to `b` past the polyline's end: a ray for a
/// straight segment, the whole circle for an arc.
fn beyond(a: Point, b: Point, bulge_value: f64, side: Side) -> Option<Entity> {
    if let Some(arc) = bulge::arc_of(a, b, bulge_value) {
        return Some(Entity::Circle {
            center: arc.center,
            radius: arc.radius,
        });
    }
    let (origin, toward) = match side {
        Side::Start => (a, Point::new(a.x - b.x, a.y - b.y)),
        Side::End => (b, Point::new(b.x - a.x, b.y - a.y)),
    };
    unit(toward).map(|direction| Entity::Ray { origin, direction })
}

#[cfg(test)]
#[path = "implied_edge_tests.rs"]
mod tests;
