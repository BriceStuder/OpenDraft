//! The Properties rows of a polyline: Closed and Global width can be typed
//! over (the same edits as PEDIT's Close/Open and Width); the rest is derived.
//! A polyline that traces a rectangle also shows its width, height and angle,
//! and the radius or chamfer of rounded or bevelled corners, which can be
//! typed over with the first corner staying put.

use opendraft_core::pedit::PolyData;
use opendraft_core::rect_shape::{self, Cut, Frame};
use opendraft_core::{bulge, Entity};
use opendraft_ui::key;

use super::{oblong, row, Field, Row, Unit};

const ROWS: &[Row] = &[
    row(Field::Vertices, key!("Vertices"), Unit::Count, false),
    row(Field::Closed, key!("Closed"), Unit::Flag, true),
    row(Field::Width, key!("Global width"), Unit::Length, true),
    row(Field::Length, key!("Length"), Unit::Length, false),
    row(Field::Area, key!("Area"), Unit::Area, false),
];

const RECTANGLE_ROWS: &[Row] = &[
    row(Field::Vertices, key!("Vertices"), Unit::Count, false),
    row(Field::Closed, key!("Closed"), Unit::Flag, true),
    row(Field::Width, key!("Global width"), Unit::Length, true),
    row(Field::RectWidth, key!("Width"), Unit::Length, true),
    row(Field::Height, key!("Height"), Unit::Length, true),
    row(Field::Angle, key!("Angle"), Unit::Angle, true),
    row(Field::Length, key!("Length"), Unit::Length, false),
    row(Field::Area, key!("Area"), Unit::Area, false),
];

const ROUNDED_ROWS: &[Row] = &[
    row(Field::Vertices, key!("Vertices"), Unit::Count, false),
    row(Field::Closed, key!("Closed"), Unit::Flag, true),
    row(Field::Width, key!("Global width"), Unit::Length, true),
    row(Field::RectWidth, key!("Width"), Unit::Length, true),
    row(Field::Height, key!("Height"), Unit::Length, true),
    row(Field::Angle, key!("Angle"), Unit::Angle, true),
    row(Field::CornerRadius, key!("Radius"), Unit::Length, true),
    row(Field::Length, key!("Length"), Unit::Length, false),
    row(Field::Area, key!("Area"), Unit::Area, false),
];

const BEVELLED_ROWS: &[Row] = &[
    row(Field::Vertices, key!("Vertices"), Unit::Count, false),
    row(Field::Closed, key!("Closed"), Unit::Flag, true),
    row(Field::Width, key!("Global width"), Unit::Length, true),
    row(Field::RectWidth, key!("Width"), Unit::Length, true),
    row(Field::Height, key!("Height"), Unit::Length, true),
    row(Field::Angle, key!("Angle"), Unit::Angle, true),
    row(Field::Chamfer, key!("Chamfer"), Unit::Length, true),
    row(Field::Length, key!("Length"), Unit::Length, false),
    row(Field::Area, key!("Area"), Unit::Area, false),
];

/// The rectangle the polyline traces, if it is one.
pub(super) fn rectangle(entity: &Entity) -> Option<Frame> {
    match entity {
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => rect_shape::of_polyline(points, bulges, *closed),
        _ => None,
    }
}

pub(super) fn rows(entity: &Entity) -> &'static [Row] {
    if oblong::slot(entity).is_some() {
        return oblong::ROWS;
    }
    match rectangle(entity).map(|f| f.cut) {
        Some(Cut::Square) => RECTANGLE_ROWS,
        Some(Cut::Round(_)) => ROUNDED_ROWS,
        Some(Cut::Bevel(_)) => BEVELLED_ROWS,
        None => ROWS,
    }
}

/// The width every segment has, if they all have the same (a hairline is 0).
fn global_width(widths: &[(f64, f64)]) -> Option<f64> {
    let first = widths.first().copied().unwrap_or((0.0, 0.0));
    (first.0 == first.1 && widths.iter().all(|&w| w == first)).then_some(first.0)
}

/// A field's value; not a number when the polyline has none (segments of
/// different widths have no global width).
pub(super) fn value(entity: &Entity, field: Field) -> f64 {
    if let Some(value) = oblong::value(entity, field) {
        return value;
    }
    let Entity::Polyline {
        points,
        closed,
        bulges,
        widths,
    } = entity
    else {
        return f64::NAN;
    };
    match field {
        Field::Vertices => points.len() as f64,
        Field::Closed => f64::from(u8::from(*closed)),
        Field::Width => {
            // The unused last slot of an open polyline does not count.
            let active = if *closed || widths.is_empty() {
                widths
            } else {
                &widths[..widths.len() - 1]
            };
            global_width(active).unwrap_or(f64::NAN)
        }
        Field::Length => bulge::path_length(points, bulges, *closed),
        Field::Area => bulge::path_area(points, bulges),
        Field::RectWidth | Field::Height | Field::Angle | Field::CornerRadius | Field::Chamfer => {
            match (rectangle(entity), field) {
                (Some(f), Field::RectWidth) => f.width,
                (Some(f), Field::Height) => f.height,
                (Some(f), Field::Angle) => degrees(f.angle),
                (
                    Some(Frame {
                        cut: Cut::Round(r), ..
                    }),
                    Field::CornerRadius,
                ) => r,
                (
                    Some(Frame {
                        cut: Cut::Bevel(d), ..
                    }),
                    Field::Chamfer,
                ) => d,
                _ => f64::NAN,
            }
        }
        _ => f64::NAN,
    }
}

/// The polyline with `field` set to `new`, or `None` when it is derived or
/// the value is not allowed.
pub(super) fn with_value(entity: &Entity, field: Field, new: f64) -> Option<Entity> {
    if let Some(changed) = oblong::with_value(entity, field, new) {
        return changed;
    }
    let data = PolyData::of(entity)?;
    match field {
        Field::Closed => Some(data.with_closed(new != 0.0).into_entity()),
        Field::Width if new >= 0.0 => Some(data.with_width(new).into_entity()),
        Field::RectWidth | Field::Height | Field::Angle | Field::CornerRadius | Field::Chamfer => {
            let mut frame = rectangle(entity)?;
            match (field, frame.cut) {
                (Field::RectWidth, _) => frame.width = new,
                (Field::Height, _) => frame.height = new,
                (Field::Angle, _) => frame.angle = new.to_radians(),
                (Field::CornerRadius, Cut::Round(_)) => frame.cut = Cut::Round(new),
                (Field::Chamfer, Cut::Bevel(_)) => frame.cut = Cut::Bevel(new),
                _ => return None,
            }
            // The sides must stay longer than the corners take.
            frame.fits().then(|| moved_corners(entity, &frame))
        }
        _ => None,
    }
}

/// An angle in degrees from 0 up to 360, with a side a hair below east read
/// as 0 rather than 360.
pub(super) fn degrees(angle: f64) -> f64 {
    let d = angle.to_degrees().rem_euclid(360.0);
    if 360.0 - d < 1e-9 {
        0.0
    } else {
        d
    }
}

/// The polyline with its vertices moved to `frame`'s, keeping their order,
/// the widths, the arcs and a last vertex back on the first.
fn moved_corners(entity: &Entity, frame: &Frame) -> Entity {
    let mut out = entity.clone();
    if let Entity::Polyline { points, .. } = &mut out {
        let vertices = frame.vertices();
        for (i, p) in points.iter_mut().enumerate() {
            *p = vertices[i % vertices.len()];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendraft_core::rect::{self, Corners};
    use opendraft_core::Point;

    fn polyline(widths: Vec<(f64, f64)>) -> Entity {
        Entity::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(4.0, 0.0),
                Point::new(4.0, 3.0),
            ],
            closed: false,
            bulges: Vec::new(),
            widths,
        }
    }

    #[test]
    fn closed_and_global_width_are_typed_over() {
        let open = polyline(Vec::new());
        assert_eq!(value(&open, Field::Closed), 0.0);
        assert_eq!(value(&open, Field::Width), 0.0);
        let closed = with_value(&open, Field::Closed, 1.0).unwrap();
        assert_eq!(value(&closed, Field::Closed), 1.0);
        assert!(value(&closed, Field::Length) > value(&open, Field::Length));
        let wide = with_value(&closed, Field::Width, 2.0).unwrap();
        assert_eq!(value(&wide, Field::Width), 2.0);
        let hairline = with_value(&wide, Field::Width, 0.0).unwrap();
        assert!(matches!(hairline, Entity::Polyline { widths, .. } if widths.is_empty()));
    }

    #[test]
    fn segments_of_different_widths_have_no_global_width() {
        let mixed = polyline(vec![(1.0, 1.0), (2.0, 2.0), (0.0, 0.0)]);
        assert!(value(&mixed, Field::Width).is_nan());
        let same = polyline(vec![(1.0, 1.0), (1.0, 1.0), (0.0, 0.0)]);
        assert_eq!(
            value(&same, Field::Width),
            1.0,
            "the unused slot is ignored"
        );
        let tapered = polyline(vec![(1.0, 2.0), (2.0, 2.0), (0.0, 0.0)]);
        assert!(value(&tapered, Field::Width).is_nan());
    }

    fn rectangle(points: &[(f64, f64)], closed: bool) -> Entity {
        Entity::Polyline {
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed,
            bulges: Vec::new(),
            widths: vec![(0.5, 0.5); points.len()],
        }
    }

    fn corners(e: &Entity) -> Vec<(f64, f64)> {
        let Entity::Polyline { points, .. } = e else {
            panic!("a polyline")
        };
        // Rounded so that a turned rectangle compares exactly.
        let round = |v: f64| (v * 1e9).round() / 1e9 + 0.0;
        points.iter().map(|p| (round(p.x), round(p.y))).collect()
    }

    #[test]
    fn a_polyline_tracing_a_rectangle_shows_its_sides_and_angle() {
        let r = rectangle(&[(1.0, 1.0), (1.0, 5.0), (-1.0, 5.0), (-1.0, 1.0)], true);
        assert_eq!(rows(&r).len(), 8);
        assert_eq!(value(&r, Field::RectWidth), 4.0);
        assert_eq!(value(&r, Field::Height), 2.0);
        assert!((value(&r, Field::Angle) - 90.0).abs() < 1e-9);
        assert_eq!(
            value(&r, Field::Width),
            0.5,
            "the global width is kept apart"
        );
        let open = polyline(Vec::new());
        assert_eq!(rows(&open).len(), 5);
        assert!(value(&open, Field::RectWidth).is_nan());
        assert_eq!(with_value(&open, Field::RectWidth, 3.0), None);
    }

    #[test]
    fn a_side_just_below_east_reads_as_zero_degrees() {
        let r = rectangle(&[(0.0, 0.0), (4.0, -1e-15), (4.0, 2.0), (0.0, 2.0)], true);
        assert_eq!(value(&r, Field::Angle), 0.0);
    }

    #[test]
    fn typing_a_side_or_the_angle_keeps_the_first_vertex_and_the_turn() {
        // Clockwise, starting up the left side.
        let r = rectangle(&[(0.0, 0.0), (0.0, 4.0), (10.0, 4.0), (10.0, 0.0)], true);
        let wider = with_value(&r, Field::RectWidth, 6.0).unwrap();
        assert_eq!(
            corners(&wider),
            [(0.0, 0.0), (0.0, 6.0), (10.0, 6.0), (10.0, 0.0)]
        );
        let higher = with_value(&r, Field::Height, 3.0).unwrap();
        assert_eq!(
            corners(&higher),
            [(0.0, 0.0), (0.0, 4.0), (3.0, 4.0), (3.0, 0.0)]
        );
        let turned = with_value(&r, Field::Angle, 0.0).unwrap();
        assert_eq!(
            corners(&turned),
            [(0.0, 0.0), (4.0, 0.0), (4.0, -10.0), (0.0, -10.0)]
        );
        assert!(
            matches!(&turned, Entity::Polyline { widths, closed: true, .. }
            if widths == &vec![(0.5, 0.5); 4])
        );
        assert_eq!(with_value(&r, Field::RectWidth, 0.0), None);
        assert_eq!(with_value(&r, Field::Height, -2.0), None);
    }

    #[test]
    fn a_fifth_vertex_back_on_the_first_follows_it() {
        let r = rectangle(
            &[(0.0, 0.0), (5.0, 0.0), (5.0, 2.0), (0.0, 2.0), (0.0, 0.0)],
            false,
        );
        let turned = with_value(&r, Field::Angle, 90.0).unwrap();
        assert_eq!(
            corners(&turned),
            [(0.0, 0.0), (0.0, 5.0), (-2.0, 5.0), (-2.0, 0.0), (0.0, 0.0)]
        );
    }

    fn cut(dx: f64, corners: Corners) -> Entity {
        let at = Point::new(0.0, 0.0);
        rect::outline(at, 0.0, dx, 4.0, corners, 0.25)
    }

    #[test]
    fn rounded_corners_show_their_radius_and_keep_it_when_the_sides_change() {
        let r = cut(10.0, Corners::Fillet(1.0));
        assert_eq!(rows(&r).len(), 9);
        assert!(rows(&r).iter().any(|row| row.field == Field::CornerRadius));
        assert!((value(&r, Field::CornerRadius) - 1.0).abs() < 1e-9);
        assert!((value(&r, Field::RectWidth) - 10.0).abs() < 1e-9);
        assert!(value(&r, Field::Chamfer).is_nan());
        let wider = with_value(&r, Field::RectWidth, 12.0).unwrap();
        assert_eq!(corners(&wider), corners(&cut(12.0, Corners::Fillet(1.0))));
        let rounder = with_value(&r, Field::CornerRadius, 1.5).unwrap();
        assert_eq!(corners(&rounder), corners(&cut(10.0, Corners::Fillet(1.5))));
        assert!(matches!(&rounder, Entity::Polyline { bulges, widths, .. }
            if bulges.iter().filter(|b| **b != 0.0).count() == 4 && widths[0] == (0.25, 0.25)));
        assert_eq!(
            with_value(&r, Field::CornerRadius, 2.0),
            None,
            "no straight part left"
        );
        assert_eq!(with_value(&r, Field::CornerRadius, 0.0), None);
        assert_eq!(with_value(&r, Field::Height, 2.0), None);
        assert_eq!(with_value(&r, Field::Chamfer, 1.0), None);
    }

    #[test]
    fn bevelled_corners_show_their_chamfer() {
        let b = cut(10.0, Corners::Chamfer(0.5, 0.5));
        assert!(rows(&b).iter().any(|row| row.field == Field::Chamfer));
        assert!(!rows(&b).iter().any(|row| row.field == Field::CornerRadius));
        assert!((value(&b, Field::Chamfer) - 0.5).abs() < 1e-9);
        let deeper = with_value(&b, Field::Chamfer, 1.0).unwrap();
        assert_eq!(
            corners(&deeper),
            corners(&cut(10.0, Corners::Chamfer(1.0, 1.0)))
        );
        assert_eq!(with_value(&b, Field::CornerRadius, 1.0), None);
        let square = rectangle(&[(0.0, 0.0), (5.0, 0.0), (5.0, 2.0), (0.0, 2.0)], true);
        assert!(value(&square, Field::Chamfer).is_nan());
        assert_eq!(with_value(&square, Field::Chamfer, 1.0), None);
    }

    #[test]
    fn a_negative_width_and_derived_fields_are_refused() {
        let open = polyline(Vec::new());
        assert_eq!(with_value(&open, Field::Width, -1.0), None);
        assert_eq!(with_value(&open, Field::Length, 5.0), None);
        assert_eq!(with_value(&open, Field::Vertices, 5.0), None);
    }
}
