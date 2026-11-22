//! Ordinate dimensions: the X or Y coordinate of a feature, on a leader that
//! runs along the other axis to the text. Coordinates and axes are those of
//! the dimension's frame, the coordinate system in force when it was made
//! (the world's unless a custom one was).

use std::f64::consts::FRAC_PI_2;

use super::font;
use super::sketch::{readable, Sketch};
use super::{Built, DimKind, Dimension};
use crate::Point;

/// The coordinate measured: X for an X datum dimension (a leader running
/// up or down), Y otherwise.
pub(super) fn measurement(kind: &DimKind) -> Option<f64> {
    let DimKind::Ordinate {
        feature,
        x_datum,
        frame,
        ..
    } = kind
    else {
        return None;
    };
    let local = frame.from_world(*feature);
    Some(if *x_datum { local.x } else { local.y })
}

/// Which datum a leader to `leader_end` suggests: the leader runs up or
/// down (X datum) when `leader_end` is further from the feature in Y than
/// in X. Both points are in the frame's coordinates.
pub fn suggested_x_datum(feature: Point, leader_end: Point) -> bool {
    (leader_end.y - feature.y).abs() >= (leader_end.x - feature.x).abs()
}

pub(super) fn build(dim: &Dimension) -> Option<Built> {
    let DimKind::Ordinate {
        feature,
        leader_end,
        x_datum,
        frame,
    } = &dim.kind
    else {
        return None;
    };
    // Worked out in the frame's coordinates, then put back in the world.
    let (feature, leader_end) = (frame.from_world(*feature), frame.from_world(*leader_end));
    let s = dim.style.resolved();
    let label = dim.label();
    let (h, g) = (s.text_height, s.text_gap);
    let contents = dim.contents();
    let width = font::contents_width(&contents, h);
    // The leader runs along `axis` (the unit step of its main direction)
    // from the feature, standing off by the extension offset, to the end.
    let (fx, fy) = (feature.x, feature.y);
    let (ex, ey) = (leader_end.x, leader_end.y);
    let (axis, along_len, across) = if *x_datum {
        let s = if ey >= fy { 1.0 } else { -1.0 };
        ((0.0, s), (ey - fy).abs(), ex - fx)
    } else {
        let s = if ex >= fx { 1.0 } else { -1.0 };
        ((s, 0.0), (ex - fx).abs(), ey - fy)
    };
    let at = |t: f64, side: f64| {
        let n = (-axis.1, axis.0);
        frame.to_world(Point::new(
            fx + axis.0 * t + n.0 * side,
            fy + axis.1 * t + n.1 * side,
        ))
    };
    let start = s.ext_offset.min(along_len);
    let mut out = Sketch::default();
    if across.abs() <= 1e-9 || along_len <= start {
        out.line(at(start, 0.0), at(along_len.max(start), across));
    } else {
        // A jog halfway sideways to the text's line.
        let jog = ((along_len + start) / 2.0).max(start);
        let half = (s.text_height / 2.0).min((along_len - start) / 2.0);
        out.polyline(vec![
            at(start, 0.0),
            at(jog - half, 0.0),
            at(jog + half, across),
            at(along_len, across),
        ]);
    }
    let end = at(along_len, across);
    let run = frame.angle_to_world(if *x_datum { FRAC_PI_2 } else { 0.0 });
    let angle = dim.text_angle.unwrap_or_else(|| readable(run));
    let beyond = g + width / 2.0;
    let step = frame.vector_to_world(Point::new(axis.0, axis.1));
    let centre = dim
        .text_pos
        .unwrap_or_else(|| Point::new(end.x + step.x * beyond, end.y + step.y * beyond));
    if !label.is_empty() {
        out.label(&s, &contents, width, centre, centre, angle);
    }
    Some(Built {
        pieces: out.pieces,
        text_centre: centre,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DimStyle, Entity};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn it_measures_the_coordinate_of_the_datum() {
        let style = DimStyle::default();
        let x = Dimension::ordinate(p(12.5, 3.0), p(12.5, 20.0), true, &style);
        assert_eq!(x.measurement(), Some(12.5));
        assert_eq!(x.label(), "12.50");
        let y = Dimension::ordinate(p(12.5, 3.0), p(30.0, 3.0), false, &style);
        assert_eq!(y.measurement(), Some(3.0));
    }

    #[test]
    fn it_measures_and_draws_in_its_frame() {
        let style = DimStyle::default();
        let frame = crate::Ccs::new(p(10.0, 0.0), FRAC_PI_2);
        // The frame's X runs up the world's Y from (10, 0).
        let feature = frame.to_world(p(4.0, 7.0));
        let up = frame.to_world(p(4.0, 20.0));
        let x = Dimension::ordinate_in(frame, feature, up, true, &style);
        assert!((x.measurement().unwrap() - 4.0).abs() < 1e-9);
        let y = Dimension::ordinate_in(frame, feature, up, false, &style);
        assert!((y.measurement().unwrap() - 7.0).abs() < 1e-9);
        // The leader runs along the frame's Y, the world's -X.
        let leader = x.picture().into_iter().find_map(|e| match e {
            Entity::Line { start, end } => Some((start, end)),
            _ => None,
        });
        let (a, b) = leader.expect("a straight leader");
        assert!((a.y - b.y).abs() < 1e-9 && b.x < a.x);
    }

    #[test]
    fn the_datum_follows_the_leader_unless_forced() {
        assert!(suggested_x_datum(p(0.0, 0.0), p(1.0, 5.0)));
        assert!(!suggested_x_datum(p(0.0, 0.0), p(5.0, 1.0)));
    }

    #[test]
    fn a_straight_leader_is_one_line_and_an_offset_one_jogs() {
        let style = DimStyle::default();
        let lines = |d: &Dimension| {
            d.picture()
                .iter()
                .filter(|e| matches!(e, Entity::Line { .. } | Entity::Polyline { .. }))
                .count()
        };
        let straight = Dimension::ordinate(p(5.0, 5.0), p(5.0, 25.0), true, &style);
        assert!(straight
            .picture()
            .iter()
            .any(|e| matches!(e, Entity::Line { start, end } if start.x == 5.0 && end.x == 5.0)));
        let jogged = Dimension::ordinate(p(5.0, 5.0), p(9.0, 25.0), true, &style);
        assert!(jogged
            .picture()
            .iter()
            .any(|e| matches!(e, Entity::Polyline { points, .. } if points.len() == 4)));
        assert!(lines(&jogged) >= 1);
    }

    #[test]
    fn the_text_sits_beyond_the_end_of_the_leader() {
        let style = DimStyle::default();
        let d = Dimension::ordinate(p(5.0, 5.0), p(5.0, 25.0), true, &style);
        assert!(d.text_centre().unwrap().y > 25.0);
        let d = Dimension::ordinate(p(5.0, 5.0), p(25.0, 5.0), false, &style);
        assert!(d.text_centre().unwrap().x > 25.0);
    }

    #[test]
    fn a_leader_shorter_than_its_offset_still_draws() {
        let style = DimStyle::default();
        let d = Dimension::ordinate(p(5.0, 5.0), p(5.0, 5.1), true, &style);
        assert!(!d.picture().is_empty());
    }
}
