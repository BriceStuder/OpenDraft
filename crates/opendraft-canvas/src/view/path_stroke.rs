//! Stroking an outline through egui without its join spikes.
//!
//! egui joins two segments of a path by averaging their sides. Where a path
//! turns back on itself that average is next to nothing, its direction is
//! rounding noise, and the corner it gives can be thrown across the screen: a
//! streak that is not in the drawing, comes and goes as the view moves (the
//! noise is in the screen positions) and cannot be picked. Paths meet it at a
//! reversal (a polyline that runs out and straight back) and at points closer
//! together than rounding can tell apart, whose direction is noise too. So
//! points on top of the one before are dropped, and a path is cut where it
//! turns back, each part stroked on its own.

use egui::{Painter, Pos2, Shape, Stroke};

/// Points closer than this (screen points) to the one before are dropped: a
/// segment that short shows nothing, and its direction is unreliable.
const SAME_POINT: f32 = 0.05;
/// Two segments whose directions' dot product is below this turn back on
/// each other (within about a degree), and the path is cut between them.
const TURNS_BACK: f32 = -0.9998;

/// Stroke `path` (closed back to its start if `closed`), as
/// `Shape::line`/`Shape::closed_line` would, but cut where it turns back.
pub(super) fn stroke_path(painter: &Painter, path: Vec<Pos2>, closed: bool, stroke: Stroke) {
    for (part, closed) in parts(path, closed) {
        painter.add(if closed {
            Shape::closed_line(part, stroke)
        } else {
            Shape::line(part, stroke)
        });
    }
}

/// The pieces `path` is stroked as, each with whether it is closed.
fn parts(path: Vec<Pos2>, closed: bool) -> Vec<(Vec<Pos2>, bool)> {
    let mut kept: Vec<Pos2> = Vec::with_capacity(path.len());
    for p in path {
        if kept.last().is_none_or(|q| q.distance(p) >= SAME_POINT) {
            kept.push(p);
        }
    }
    if closed {
        while kept.len() > 1 && kept[0].distance(kept[kept.len() - 1]) < SAME_POINT {
            kept.pop();
        }
    }
    if kept.len() < 3 {
        return vec![(kept, false)];
    }
    // A closed path that turns back is stroked open, start to start again.
    let open = if closed {
        let mut round = kept.clone();
        round.push(kept[0]);
        let n = kept.len();
        let at_start = turns_back(kept[n - 1], kept[0], kept[1]);
        if turns(&round).is_empty() && !at_start {
            return vec![(kept, true)];
        }
        round
    } else {
        kept
    };
    let cuts = turns(&open);
    if cuts.is_empty() {
        return vec![(open, false)];
    }
    let mut out = Vec::with_capacity(cuts.len() + 1);
    let mut from = 0;
    for at in cuts.into_iter().chain([open.len() - 1]) {
        out.push((open[from..=at].to_vec(), false));
        from = at;
    }
    out
}

/// The points at which `path` turns back.
fn turns(path: &[Pos2]) -> Vec<usize> {
    let at = |i: usize| turns_back(path[i - 1], path[i], path[i + 1]);
    (1..path.len().saturating_sub(1))
        .filter(|&i| at(i))
        .collect()
}

/// Whether the path `a` to `b` to `c` turns back at `b`.
fn turns_back(a: Pos2, b: Pos2, c: Pos2) -> bool {
    let (d0, d1) = ((b - a).normalized(), (c - b).normalized());
    d0.dot(d1) < TURNS_BACK
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn a_plain_path_is_stroked_as_it_is() {
        let path = vec![pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(10.0, 10.0)];
        assert_eq!(parts(path.clone(), false), vec![(path.clone(), false)]);
        assert_eq!(parts(path.clone(), true), vec![(path, true)]);
    }

    #[test]
    fn a_path_that_turns_back_is_cut_there() {
        let path = vec![pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(3.0, 0.0)];
        assert_eq!(
            parts(path, false),
            vec![
                (vec![pos2(0.0, 0.0), pos2(10.0, 0.0)], false),
                (vec![pos2(10.0, 0.0), pos2(3.0, 0.0)], false),
            ]
        );
    }

    #[test]
    fn points_on_top_of_each_other_are_one() {
        let path = vec![
            pos2(0.0, 0.0),
            pos2(10.0, 0.0),
            pos2(10.0, 0.000_01),
            pos2(10.0, 10.0),
        ];
        let want = vec![pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(10.0, 10.0)];
        assert_eq!(parts(path, false), vec![(want, false)]);
    }

    /// The outline that showed the streak (`render_issue_1`): out along an
    /// arc, a hair across, straight back, and a hair across to the start.
    /// Its hairs, whose direction is noise, are what threw the corner; without
    /// them the turns at the ends are sharp but true, which egui joins well.
    #[test]
    fn a_closed_sliver_that_runs_out_and_back_loses_its_hairs() {
        let arc = [pos2(0.0, 0.0), pos2(40.0, 60.0), pos2(100.0, 200.0)];
        let mut path = arc.to_vec();
        path.push(pos2(100.0, 200.000_02));
        path.push(pos2(0.0, 0.000_01));
        assert_eq!(parts(path, true), vec![(arc.to_vec(), true)]);
    }

    #[test]
    fn a_closed_path_turning_back_at_its_start_is_cut_there() {
        // Out and back along a line, closed: the start is where it turns.
        let path = vec![pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(20.0, 0.0)];
        let pieces = parts(path, true);
        assert_eq!(
            pieces,
            vec![
                (
                    vec![pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(20.0, 0.0)],
                    false
                ),
                (vec![pos2(20.0, 0.0), pos2(0.0, 0.0)], false),
            ]
        );
    }
}
