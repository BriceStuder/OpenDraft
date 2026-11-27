//! FILLET and CHAMFER where a polyline's own segments are joined by more than
//! a shared vertex, as AutoCAD does it:
//!
//! - two segments of one polyline with exactly one segment between them: the
//!   segment between is removed and the two are joined (two parallel ones by a
//!   half circle, as for two lines);
//! - a line or arc and the end segment of an open polyline: the two become one
//!   polyline.
//!
//! The corner itself is worked out on the segments as lines and arcs, as for
//! two objects; the pieces are then chained back into one polyline with
//! `join`. Each polyline segment keeps the side away from the corner (its
//! topological far end), so which way the corner opens never depends on where
//! the segment was clicked.

use crate::corner::{CornerError, Joined};
use crate::polycorner::{segment_entity, Path, Round};
use crate::{bulge, Entity, Point};

/// Ends this close are one point, as in JOIN.
const TOUCH: f64 = 1e-6;

/// A polyline joined to a line or arc.
#[derive(Clone, Debug, PartialEq)]
pub struct EndJoin {
    /// The polyline with the line or arc now part of it (with the joint, when
    /// there is one).
    pub merged: Entity,
    /// The arc or cut between the two, for use when the objects are left
    /// whole (TRIMMODE off).
    pub joint: Option<Entity>,
}

fn corner_of(
    a: &Entity,
    b: &Entity,
    how: Round,
    picked_a: Point,
    picked_b: Point,
) -> Result<Joined, CornerError> {
    match how {
        Round::Fillet(radius) => crate::corner_infinite::fillet(a, b, radius, picked_a, picked_b),
        Round::Chamfer(chamfer) => {
            crate::corner_infinite::chamfer_lines(a, b, chamfer, picked_a, picked_b)
        }
    }
}

fn segment(path: &Path, k: usize) -> Option<Entity> {
    let n = path.points.len();
    segment_entity(
        path.points[k % n],
        path.points[(k + 1) % n],
        bulge::at(&path.bulges, k),
    )
}

/// Whether both are lines running side by side.
fn parallel(a: &Entity, b: &Entity) -> bool {
    let (Entity::Line { start: a0, end: a1 }, Entity::Line { start: b0, end: b1 }) = (a, b) else {
        return false;
    };
    let unit = |s: &Point, e: &Point| {
        let (dx, dy) = (e.x - s.x, e.y - s.y);
        let len = dx.hypot(dy);
        (dx / len, dy / len)
    };
    let ((ux, uy), (vx, vy)) = (unit(a0, a1), unit(b0, b1));
    (ux * vy - uy * vx).abs() <= 1e-9
}

fn is_wide(path: &Path, k: usize) -> bool {
    path.widths.get(k).is_some_and(|w| w.0 != 0.0 || w.1 != 0.0)
}

/// `len` whole segments of `path` from segment `from` (wrapping) as an open
/// polyline.
fn run(path: &Path, from: usize, len: usize) -> Entity {
    let n = path.points.len();
    let at = |k: usize| (from + k) % n;
    let wide = !path.widths.is_empty();
    Entity::Polyline {
        points: (0..=len).map(|k| path.points[at(k)]).collect(),
        closed: false,
        bulges: (0..=len)
            .map(|k| {
                if k < len {
                    bulge::at(&path.bulges, at(k))
                } else {
                    0.0
                }
            })
            .collect(),
        widths: if wide {
            (0..=len)
                .map(|k| {
                    if k < len {
                        path.widths.get(at(k)).copied().unwrap_or_default()
                    } else {
                        (0.0, 0.0)
                    }
                })
                .collect()
        } else {
            Vec::new()
        },
    }
}

/// `path` with its segments `first` to `last` (counting on from `first`,
/// wrapping when the path is closed) replaced by `middle`, as one polyline.
fn stitch(
    path: &Path,
    first: usize,
    last: usize,
    middle: Vec<Entity>,
) -> Result<Entity, CornerError> {
    let segments = bulge::segment_count(path.points.len(), path.closed);
    let replaced = (last + segments - first) % segments + 1;
    let mut items = Vec::new();
    if path.closed {
        items.extend(middle);
        if segments > replaced {
            items.push(run(path, (last + 1) % segments, segments - replaced));
        }
    } else {
        if first > 0 {
            items.push(run(path, 0, first));
        }
        items.extend(middle);
        if last + 1 < segments {
            items.push(run(path, last + 1, segments - last - 1));
        }
    }
    let mut merged = crate::join::join(&items, TOUCH);
    match (merged.pop(), merged.is_empty()) {
        (Some(m), true) if m.used.len() == items.len() => Ok(m.entity),
        _ => Err(CornerError::Unsupported),
    }
}

/// Join segments `i` and `j` of `path` when exactly one segment lies between
/// them (either way round on a closed path): the segment between goes and the
/// two meet at a fillet or chamfer. Returns the polyline that replaces `path`.
pub fn join_apart_segments(
    path: &Path,
    i: usize,
    j: usize,
    how: Round,
) -> Result<Entity, CornerError> {
    let segments = bulge::segment_count(path.points.len(), path.closed);
    let ahead =
        |from: usize, to: usize| (from + 2) % segments == to && (path.closed || to == from + 2);
    let (first, last) = if ahead(i, j) {
        (i, j)
    } else if ahead(j, i) {
        (j, i)
    } else {
        return Err(CornerError::NotAdjacent);
    };
    if is_wide(path, first) || is_wide(path, last) {
        return Err(CornerError::Unsupported);
    }
    let n = path.points.len();
    let (Some(a), Some(b)) = (segment(path, first), segment(path, last)) else {
        return Err(CornerError::Unsupported);
    };
    // Each keeps the part away from the segment that goes. Two parallel lines
    // are joined at the end of the first that the pick is nearest, which is
    // the end beside the segment that goes.
    let pick_first = if parallel(&a, &b) {
        path.points[(first + 1) % n]
    } else {
        path.points[first]
    };
    let joined = corner_of(&a, &b, how, pick_first, path.points[(last + 1) % n])?;
    let mut middle = vec![joined.a];
    middle.extend(joined.joint);
    middle.push(joined.b);
    stitch(path, first, last, middle)
}

/// Join `line` (a line or an arc) to the end segment of the open polyline
/// `polyline` at a fillet or chamfer, into one polyline. `picked_line` is where
/// the line was clicked (it keeps that side); `picked_polyline` says which
/// segment, which must be an end one.
pub fn join_to_end_segment(
    line: &Entity,
    polyline: &Entity,
    how: Round,
    picked_line: Point,
    picked_polyline: Point,
) -> Result<EndJoin, CornerError> {
    if !matches!(line, Entity::Line { .. } | Entity::Arc { .. }) {
        return Err(CornerError::Unsupported);
    }
    let Entity::Polyline { closed: false, .. } = polyline else {
        return Err(CornerError::Unsupported);
    };
    let (Some(path), Some(k)) = (
        polyline.polyline_path(),
        polyline.segment_at(picked_polyline),
    ) else {
        return Err(CornerError::Unsupported);
    };
    let segments = bulge::segment_count(path.points.len(), false);
    if k != 0 && k + 1 != segments {
        return Err(CornerError::NotEndSegment);
    }
    if is_wide(&path, k) {
        return Err(CornerError::Unsupported);
    }
    let Some(seg) = segment(&path, k) else {
        return Err(CornerError::Unsupported);
    };
    // The end away from the free end of the polyline stays where it is.
    let far = if segments == 1 {
        picked_polyline
    } else if k == 0 {
        path.points[1]
    } else {
        path.points[k]
    };
    let joined = corner_of(line, &seg, how, picked_line, far)?;
    let mut middle = vec![joined.b];
    middle.extend(joined.joint.clone());
    middle.push(joined.a);
    Ok(EndJoin {
        merged: stitch(&path, k, k, middle)?,
        joint: joined.joint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn open(points: &[Point]) -> Entity {
        Entity::Polyline {
            points: points.to_vec(),
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        }
    }

    fn parts(e: &Entity) -> (Vec<Point>, Vec<f64>, bool) {
        match e {
            Entity::Polyline {
                points,
                bulges,
                closed,
                ..
            } => (
                points.clone(),
                (0..points.len()).map(|i| bulge::at(bulges, i)).collect(),
                *closed,
            ),
            other => panic!("expected a polyline, got {other:?}"),
        }
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).hypot(a.y - b.y) < 1e-6
    }

    #[test]
    fn a_line_joins_the_end_of_a_polyline_through_a_fillet() {
        let poly = open(&[p(0.0, 0.0), p(10.0, 0.0)]);
        let line = Entity::Line {
            start: p(14.0, -8.0),
            end: p(14.0, 8.0),
        };
        let done = join_to_end_segment(&line, &poly, Round::Fillet(2.0), p(14.0, 6.0), p(3.0, 0.0))
            .unwrap();
        let (points, bulges, closed) = parts(&done.merged);
        assert!(!closed);
        assert_eq!(points.len(), 4, "{points:?}");
        assert!(near(points[0], p(0.0, 0.0)) && near(points[1], p(12.0, 0.0)));
        assert!(near(points[2], p(14.0, 2.0)) && near(points[3], p(14.0, 8.0)));
        // A quarter circle turning left: bulge tan(pi / 8).
        assert!((bulges[1] - (std::f64::consts::PI / 8.0).tan()).abs() < 1e-9);
        assert!(done.joint.is_some());
    }

    #[test]
    fn it_works_at_the_start_of_the_polyline_and_with_either_way_round_line() {
        // The free end is at the start this time, and the line is drawn
        // downwards.
        let poly = open(&[p(10.0, 0.0), p(0.0, 0.0)]);
        let line = Entity::Line {
            start: p(14.0, 8.0),
            end: p(14.0, -8.0),
        };
        let done = join_to_end_segment(&line, &poly, Round::Fillet(2.0), p(14.0, 6.0), p(3.0, 0.0))
            .unwrap();
        let (points, _, _) = parts(&done.merged);
        assert_eq!(points.len(), 4);
        let ends = [points[0], points[3]];
        assert!(ends.iter().any(|e| near(*e, p(0.0, 0.0))));
        assert!(ends.iter().any(|e| near(*e, p(14.0, 8.0))));
    }

    #[test]
    fn a_sharp_corner_has_no_joint_and_a_chamfer_cuts_it() {
        let poly = open(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 4.0)]);
        let line = Entity::Line {
            start: p(6.0, 12.0),
            end: p(14.0, 4.0),
        };
        let sharp =
            join_to_end_segment(&line, &poly, Round::Fillet(0.0), p(7.0, 11.0), p(10.0, 3.0))
                .unwrap();
        assert!(sharp.joint.is_none());
        let (points, _, _) = parts(&sharp.merged);
        assert_eq!(points.len(), 4, "{points:?}");
        assert!(near(points[2], p(10.0, 8.0)), "{points:?}");
        let cut = join_to_end_segment(
            &line,
            &poly,
            Round::Chamfer(crate::Chamfer::Distances(1.0, 1.0)),
            p(7.0, 11.0),
            p(10.0, 3.0),
        );
        assert!(cut.unwrap().joint.is_some());
    }

    #[test]
    fn a_middle_segment_wide_segments_and_other_objects_are_refused() {
        let poly = open(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 5.0), p(0.0, 5.0)]);
        let line = Entity::Line {
            start: p(-3.0, 8.0),
            end: p(5.0, 8.0),
        };
        let r = Round::Fillet(1.0);
        assert_eq!(
            join_to_end_segment(&line, &poly, r, p(0.0, 8.0), p(10.0, 2.0)),
            Err(CornerError::NotEndSegment)
        );
        let wide = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0)],
            closed: false,
            bulges: Vec::new(),
            widths: vec![(1.0, 1.0), (0.0, 0.0)],
        };
        let up = Entity::Line {
            start: p(14.0, -8.0),
            end: p(14.0, 8.0),
        };
        assert_eq!(
            join_to_end_segment(&up, &wide, r, p(14.0, 6.0), p(3.0, 0.0)),
            Err(CornerError::Unsupported)
        );
        let circle = Entity::Circle {
            center: p(20.0, 0.0),
            radius: 3.0,
        };
        assert_eq!(
            join_to_end_segment(&circle, &poly, r, p(20.0, 3.0), p(5.0, 0.0)),
            Err(CornerError::Unsupported)
        );
    }

    fn path_of(e: &Entity) -> Path {
        e.polyline_path().unwrap()
    }

    #[test]
    fn two_parallel_segments_with_one_between_are_joined_by_a_half_circle() {
        let u = open(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 4.0), p(0.0, 4.0)]);
        let done = join_apart_segments(&path_of(&u), 0, 2, Round::Fillet(1.0)).unwrap();
        let (points, bulges, closed) = parts(&done);
        assert!(!closed);
        assert_eq!(points.len(), 4, "{points:?}");
        assert!(
            (bulges[1].abs() - 1.0).abs() < 1e-9,
            "a half circle: {bulges:?}"
        );
        let middle = bulge::point_at(points[1], points[2], bulges[1], 0.5);
        assert!(near(middle, p(12.0, 2.0)), "bulges outwards: {middle:?}");
        // Picking them the other way round gives the same drawing.
        let swapped = join_apart_segments(&path_of(&u), 2, 0, Round::Fillet(1.0)).unwrap();
        assert_eq!(parts(&swapped).0, points);
    }

    #[test]
    fn segments_that_meet_when_extended_are_cut_back_or_extended_to_the_fillet() {
        let z = open(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 3.0), p(4.0, 9.0)]);
        let done = join_apart_segments(&path_of(&z), 0, 2, Round::Fillet(1.0)).unwrap();
        let (points, _, _) = parts(&done);
        assert_eq!(points.len(), 4, "{points:?}");
        // The lines meet at (13, 0) at 45 degrees: the arc starts
        // r / tan(22.5 deg) back from it, past the end of the first segment.
        let reach = 1.0 / (std::f64::consts::PI / 8.0).tan();
        assert!(near(points[1], p(13.0 - reach, 0.0)), "{points:?}");
        assert!(near(points[3], p(4.0, 9.0)));
    }

    #[test]
    fn a_closed_polyline_loses_the_segment_between_and_stays_closed() {
        let rect = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 4.0), p(0.0, 4.0)],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        let done = join_apart_segments(&path_of(&rect), 0, 2, Round::Fillet(1.0)).unwrap();
        let (points, bulges, closed) = parts(&done);
        assert!(closed);
        assert_eq!(points.len(), 4, "{points:?}");
        assert!((bulges[1].abs() - 1.0).abs() < 1e-9, "{bulges:?}");
    }

    #[test]
    fn segments_not_separated_by_exactly_one_are_refused() {
        let five = open(&[
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 4.0),
            p(0.0, 4.0),
            p(0.0, 8.0),
            p(10.0, 8.0),
        ]);
        let path = path_of(&five);
        for (i, j) in [(0, 3), (0, 4), (1, 4)] {
            assert_eq!(
                join_apart_segments(&path, i, j, Round::Fillet(1.0)),
                Err(CornerError::NotAdjacent),
                "{i} {j}"
            );
        }
        // Segments 0 and 2 can be joined; what follows the second stays.
        let done = join_apart_segments(&path, 0, 2, Round::Fillet(1.0)).unwrap();
        assert_eq!(parts(&done).0.len(), 6);
        // Segments 1 and 3 run the same way up: the half circle would land on
        // a segment that carries on beyond it.
        assert_eq!(
            join_apart_segments(&path, 1, 3, Round::Fillet(1.0)),
            Err(CornerError::PastTheEnd)
        );
    }
}
