//! The Properties rows of a polyline that traces an oblong: its centre, the
//! distance between the centres of its ends, its width and its angle, typed
//! over with the centre staying put (or moving to the typed centre).

use opendraft_core::rect_shape::{self, Slot};
use opendraft_core::{Entity, Point};
use opendraft_ui::key;

use super::polyline::degrees;
use super::{row, Field, Row, Unit};

pub(super) const ROWS: &[Row] = &[
    row(Field::Vertices, key!("Vertices"), Unit::Count, false),
    row(Field::Closed, key!("Closed"), Unit::Flag, true),
    row(Field::Width, key!("Global width"), Unit::Length, true),
    row(Field::X1, key!("Center X"), Unit::Length, true),
    row(Field::Y1, key!("Center Y"), Unit::Length, true),
    row(
        Field::CenterDistance,
        key!("Center distance"),
        Unit::Length,
        true,
    ),
    row(Field::OblongWidth, key!("Width"), Unit::Length, true),
    row(Field::Angle, key!("Angle"), Unit::Angle, true),
    row(Field::Length, key!("Length"), Unit::Length, false),
    row(Field::Area, key!("Area"), Unit::Area, false),
];

/// The oblong the polyline traces, if it is one.
pub(super) fn slot(entity: &Entity) -> Option<Slot> {
    match entity {
        Entity::Polyline {
            points,
            closed,
            bulges,
            ..
        } => rect_shape::slot_of_polyline(points, bulges, *closed),
        _ => None,
    }
}

/// The value of one of the oblong's own fields, if `field` is one and the
/// polyline is an oblong.
pub(super) fn value(entity: &Entity, field: Field) -> Option<f64> {
    let s = slot(entity)?;
    match field {
        Field::X1 => Some(s.centre.x),
        Field::Y1 => Some(s.centre.y),
        Field::CenterDistance => Some(s.distance),
        Field::OblongWidth => Some(s.width),
        Field::Angle => Some(degrees(s.angle)),
        _ => None,
    }
}

/// The oblong with one of its own fields set to `new`: `None` when `field`
/// is not one of them or the polyline is no oblong, `Some(None)` when the
/// value is refused (a distance or width not above zero).
pub(super) fn with_value(entity: &Entity, field: Field, new: f64) -> Option<Option<Entity>> {
    let mut s = slot(entity)?;
    match field {
        Field::X1 => s.centre = Point::new(new, s.centre.y),
        Field::Y1 => s.centre = Point::new(s.centre.x, new),
        Field::CenterDistance => s.distance = new,
        Field::OblongWidth => s.width = new,
        Field::Angle => s.angle = new.to_radians(),
        _ => return None,
    }
    if !(s.distance > 0.0 && s.width > 0.0) {
        return Some(None);
    }
    let mut out = entity.clone();
    if let Entity::Polyline { points, .. } = &mut out {
        let vertices = s.vertices();
        for (i, p) in points.iter_mut().enumerate() {
            *p = vertices[i % 4];
        }
    }
    Some(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// Centres (0, 0) and (10, 0), 4 wide, line width 0.5.
    fn oblong() -> Entity {
        Entity::Polyline {
            points: vec![p(0.0, -2.0), p(10.0, -2.0), p(10.0, 2.0), p(0.0, 2.0)],
            closed: true,
            bulges: vec![0.0, 1.0, 0.0, 1.0],
            widths: vec![(0.5, 0.5); 4],
        }
    }

    fn points(e: &Entity) -> Vec<(f64, f64)> {
        let Entity::Polyline { points, .. } = e else {
            panic!("a polyline")
        };
        let round = |v: f64| (v * 1e9).round() / 1e9 + 0.0;
        points.iter().map(|p| (round(p.x), round(p.y))).collect()
    }

    #[test]
    fn an_oblong_shows_its_centre_distance_width_and_angle() {
        let o = oblong();
        assert_eq!(value(&o, Field::X1), Some(5.0));
        assert_eq!(value(&o, Field::Y1), Some(0.0));
        assert_eq!(value(&o, Field::CenterDistance), Some(10.0));
        assert_eq!(value(&o, Field::OblongWidth), Some(4.0));
        assert_eq!(value(&o, Field::Angle), Some(0.0));
        assert_eq!(value(&o, Field::Length), None, "the polyline's own row");
    }

    #[test]
    fn typing_keeps_the_centre_and_the_shape() {
        let o = oblong();
        let longer = with_value(&o, Field::CenterDistance, 14.0)
            .unwrap()
            .unwrap();
        assert_eq!(
            points(&longer),
            [(-2.0, -2.0), (12.0, -2.0), (12.0, 2.0), (-2.0, 2.0)]
        );
        let wider = with_value(&o, Field::OblongWidth, 6.0).unwrap().unwrap();
        assert_eq!(
            points(&wider),
            [(0.0, -3.0), (10.0, -3.0), (10.0, 3.0), (0.0, 3.0)]
        );
        let turned = with_value(&o, Field::Angle, 90.0).unwrap().unwrap();
        assert_eq!(
            points(&turned),
            [(7.0, -5.0), (7.0, 5.0), (3.0, 5.0), (3.0, -5.0)]
        );
        let moved = with_value(&o, Field::X1, 0.0).unwrap().unwrap();
        assert_eq!(points(&moved)[0], (-5.0, -2.0));
        assert!(
            matches!(&moved, Entity::Polyline { bulges, widths, closed: true, .. }
            if bulges == &vec![0.0, 1.0, 0.0, 1.0] && widths == &vec![(0.5, 0.5); 4])
        );
    }

    #[test]
    fn the_panel_calls_it_an_oblong_polyline_with_its_own_rows() {
        let o = oblong();
        let title = super::super::title(opendraft_core::EntityKind::Polyline, &[&o]);
        assert_eq!(title, "Oblong polyline");
        let rows = super::super::polyline::rows(&o);
        assert!(rows.iter().any(|r| r.field == Field::CenterDistance));
        assert_eq!(
            super::super::shared_value(&[&o, &o], Field::OblongWidth),
            Some(4.0)
        );
    }

    #[test]
    fn a_distance_or_width_not_above_zero_is_refused() {
        let o = oblong();
        assert_eq!(with_value(&o, Field::CenterDistance, 0.0), Some(None));
        assert_eq!(with_value(&o, Field::OblongWidth, -1.0), Some(None));
        assert_eq!(with_value(&o, Field::Closed, 0.0), None, "not the oblong's");
    }
}
