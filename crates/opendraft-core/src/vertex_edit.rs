//! Editing the vertices of a polyline from its grips: add a vertex, remove
//! one, and turn a segment into an arc or back into a line. A polyline's
//! grips are its vertices (indexes `0..n`) and then the middle of each segment
//! (`n + segment`), see [`Entity::grips`].

use crate::{bulge, Entity, Point};

/// What a polyline grip is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexGrip {
    Vertex(usize),
    /// The middle of this segment, which is an arc when `arc`.
    Segment {
        index: usize,
        arc: bool,
    },
}

/// A polyline's parts, with a bulge and a width pair for every point.
struct Parts {
    points: Vec<Point>,
    bulges: Vec<f64>,
    widths: Vec<(f64, f64)>,
    closed: bool,
    /// Whether the polyline had widths at all (so none are written if not).
    wide: bool,
}

impl Parts {
    fn of(entity: &Entity) -> Option<Parts> {
        let Entity::Polyline {
            points,
            closed,
            bulges,
            widths,
        } = entity
        else {
            return None;
        };
        Some(Parts {
            points: points.clone(),
            bulges: (0..points.len()).map(|i| bulge::at(bulges, i)).collect(),
            widths: if widths.is_empty() {
                vec![(0.0, 0.0); points.len()]
            } else {
                widths.clone()
            },
            closed: *closed,
            wide: !widths.is_empty(),
        })
    }

    fn into_entity(self) -> Entity {
        Entity::Polyline {
            bulges: if bulge::has_arcs(&self.bulges) {
                self.bulges
            } else {
                Vec::new()
            },
            widths: if self.wide { self.widths } else { Vec::new() },
            points: self.points,
            closed: self.closed,
        }
    }

    fn segments(&self) -> usize {
        bulge::segment_count(self.points.len(), self.closed)
    }
}

impl Entity {
    /// What grip `grip` of a polyline is, or `None` for another kind of object
    /// or no such grip.
    pub fn vertex_grip(&self, grip: usize) -> Option<VertexGrip> {
        let parts = Parts::of(self)?;
        let n = parts.points.len();
        if grip < n {
            return Some(VertexGrip::Vertex(grip));
        }
        let index = grip - n;
        (index < parts.segments()).then(|| VertexGrip::Segment {
            index,
            arc: parts.bulges[index].abs() > bulge::STRAIGHT,
        })
    }

    /// The polyline with a vertex added at `at` on the segment of grip `grip`
    /// (the segment that starts at a vertex grip, or the one a middle grip is
    /// on). On the end vertex of an open polyline, it is added beyond the end.
    /// On an arc segment the vertex goes on the arc, nearest `at`, and the arc
    /// is split in two.
    pub fn with_vertex_added(&self, grip: usize, at: Point) -> Option<Entity> {
        let mut parts = Parts::of(self)?;
        let n = parts.points.len();
        let segment = match self.vertex_grip(grip)? {
            VertexGrip::Segment { index, .. } => index,
            VertexGrip::Vertex(i) if i < parts.segments() => i,
            VertexGrip::Vertex(_) => {
                // Beyond the end of an open polyline.
                parts.bulges[n - 1] = 0.0;
                parts.points.push(at);
                parts.bulges.push(0.0);
                parts.widths[n - 1] = (0.0, 0.0);
                parts.widths.push((0.0, 0.0));
                return Some(parts.into_entity());
            }
        };
        let (a, b) = (parts.points[segment], parts.points[(segment + 1) % n]);
        let curve = parts.bulges[segment];
        let (on, t) = bulge::nearest(a, b, curve, at);
        let arc = curve.abs() > bulge::STRAIGHT;
        let new = if arc { on } else { at };
        let (first, second) = if arc {
            (bulge::part(curve, 0.0, t), bulge::part(curve, t, 1.0))
        } else {
            (0.0, 0.0)
        };
        let (w0, w1) = parts.widths[segment];
        let across = w0 + (w1 - w0) * t;
        parts.bulges[segment] = first;
        parts.widths[segment] = (w0, across);
        parts.points.insert(segment + 1, new);
        parts.bulges.insert(segment + 1, second);
        parts.widths.insert(segment + 1, (across, w1));
        if !parts.closed {
            // The last vertex's own entries stay unused.
            if let Some(last) = parts.bulges.last_mut() {
                *last = 0.0;
            }
        }
        Some(parts.into_entity())
    }

    /// The polyline without vertex `vertex`, the two segments beside it
    /// becoming one straight one. `None` when too few vertices would be left,
    /// or when this is not a polyline.
    pub fn with_vertex_removed(&self, vertex: usize) -> Option<Entity> {
        let mut parts = Parts::of(self)?;
        let n = parts.points.len();
        let enough = if parts.closed { n > 3 } else { n > 2 };
        if vertex >= n || !enough {
            return None;
        }
        let before = if vertex == 0 && !parts.closed {
            None
        } else {
            Some((vertex + n - 1) % n)
        };
        let after_width = parts.widths[vertex].1;
        parts.points.remove(vertex);
        parts.bulges.remove(vertex);
        parts.widths.remove(vertex);
        if let Some(before) = before {
            // `before` is now the last index when the final vertex went.
            let before = before.min(parts.points.len() - 1);
            parts.bulges[before] = 0.0;
            let start = parts.widths[before].0;
            parts.widths[before] = (start, after_width);
        }
        if !parts.closed {
            if let Some(last) = parts.bulges.last_mut() {
                *last = 0.0;
            }
            if let Some(last) = parts.widths.last_mut() {
                *last = (0.0, 0.0);
            }
        }
        Some(parts.into_entity())
    }

    /// The polyline with the segment `segment` bent into the arc through
    /// `through`. `None` when the three points are in a line.
    pub fn with_segment_arc(&self, segment: usize, through: Point) -> Option<Entity> {
        let mut parts = Parts::of(self)?;
        let n = parts.points.len();
        if segment >= parts.segments() {
            return None;
        }
        let curve = bulge::through(
            parts.points[segment],
            through,
            parts.points[(segment + 1) % n],
        )?;
        parts.bulges[segment] = curve;
        Some(parts.into_entity())
    }

    /// The polyline with the segment `segment` made straight.
    pub fn with_segment_straight(&self, segment: usize) -> Option<Entity> {
        let mut parts = Parts::of(self)?;
        if segment >= parts.segments() {
            return None;
        }
        parts.bulges[segment] = 0.0;
        Some(parts.into_entity())
    }
}

#[cfg(test)]
#[path = "vertex_edit_tests.rs"]
mod tests;
