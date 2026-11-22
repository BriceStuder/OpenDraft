//! Coordinate interpretation shared by the command line and cursor input.
//! Lengths and angles are read in the drawing's units, and coordinates and
//! direction angles in its custom coordinate system; a `*` after any `@` or
//! `#` reads the point in world coordinates instead (`*10,20`, `@*5<0`).
use opendraft_core::units::{format_angle, format_point, parse_angle, parse_length};
use opendraft_core::{Ccs, Document, Point, Units};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CoordinateMode {
    #[default]
    CommandLine,
    DynamicPointer {
        default_relative: bool,
    },
}

/// How typed values are read and shown: the drawing's units, in the
/// coordinate system in force. Plain units read in world coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Reading {
    pub units: Units,
    pub ccs: Ccs,
}

impl From<&Units> for Reading {
    fn from(units: &Units) -> Self {
        Self {
            units: *units,
            ccs: Ccs::WORLD,
        }
    }
}

impl Reading {
    /// The drawing's units and the system of the space being edited.
    pub fn of(document: &Document) -> Self {
        Self {
            units: *document.units(),
            ccs: document.ccs(),
        }
    }

    /// A typed direction (measured from the system's X axis, in the
    /// drawing's base angle and direction), as a world angle.
    pub fn direction(&self, text: &str) -> Option<f64> {
        parse_angle(&self.units, text).map(|a| self.ccs.angle_to_world(a))
    }

    /// A typed amount to turn by: the base angle does not apply (the angle
    /// direction does) and neither does the system's turn.
    pub fn turn(&self, text: &str) -> Option<f64> {
        parse_angle(&self.units, text).map(|a| a - self.units.from_display_angle(0.0))
    }

    /// A world direction as it is shown: measured in the system.
    pub fn show_direction(&self, angle: f64) -> String {
        format_angle(&self.units, self.ccs.angle_from_world(angle))
    }

    /// An amount to turn by as it is shown.
    pub fn show_turn(&self, angle: f64) -> String {
        format_angle(&self.units, angle + self.units.from_display_angle(0.0))
    }

    /// A world point as it is shown: its coordinates in the system.
    pub fn show_point(&self, point: Point) -> String {
        format_point(&self.units, self.ccs.from_world(point))
    }
}

pub fn parse_point_with_mode(
    text: &str,
    origin: Point,
    mode: CoordinateMode,
    reading: impl Into<Reading>,
) -> Option<Point> {
    let reading = reading.into();
    let text = text.trim();
    let (relative, body) = if let Some(body) = text.strip_prefix('@') {
        (true, body)
    } else if let Some(body) = text.strip_prefix('#') {
        (false, body)
    } else {
        (
            matches!(
                mode,
                CoordinateMode::DynamicPointer {
                    default_relative: true
                }
            ),
            text,
        )
    };
    let (ccs, body) = match body.strip_prefix('*') {
        Some(body) => (Ccs::WORLD, body),
        None => (reading.ccs, body),
    };
    let (x, y) = if let Some((a, b)) = body.split_once(',') {
        (parse_length(a)?, parse_length(b)?)
    } else if let Some((d, a)) = body.split_once('<') {
        let (d, a) = (parse_length(d)?, parse_angle(&reading.units, a)?);
        (d * a.cos(), d * a.sin())
    } else {
        return None;
    };
    let point = if relative {
        let v = ccs.vector_to_world(Point::new(x, y));
        Point::new(origin.x + v.x, origin.y + v.y)
    } else {
        ccs.to_world(Point::new(x, y))
    };
    (point.x.is_finite() && point.y.is_finite()).then_some(point)
}

#[cfg(test)]
mod tests {
    use opendraft_core::{AngleFormat, LengthFormat};

    use super::*;

    #[test]
    fn typed_coordinates_use_the_drawings_units() {
        let units = Units {
            length: LengthFormat::Architectural,
            angle: AngleFormat::Gradians,
            ..Units::default()
        };
        let origin = Point::new(0.0, 0.0);
        let point = |text| parse_point_with_mode(text, origin, CoordinateMode::CommandLine, &units);
        assert_eq!(point("5'-3 1/2\",1'"), Some(Point::new(63.5, 12.0)));
        let polar = point("1'<100g").unwrap();
        assert!(polar.x.abs() < 1e-9 && (polar.y - 12.0).abs() < 1e-9);
        let base = Units {
            base_angle: std::f64::consts::FRAC_PI_2,
            ..Units::default()
        };
        let north =
            parse_point_with_mode("10<0", origin, CoordinateMode::CommandLine, &base).unwrap();
        assert!(
            north.x.abs() < 1e-9 && (north.y - 10.0).abs() < 1e-9,
            "angle 0 points along the base angle"
        );
    }

    #[test]
    fn cursor_defaults_and_overrides_do_not_change_command_line() {
        let base = Point::new(100.0, 200.0);
        let mode = CoordinateMode::DynamicPointer {
            default_relative: true,
        };
        assert_eq!(
            parse_point_with_mode("10,20", base, mode, &Units::default()),
            Some(Point::new(110.0, 220.0))
        );
        for text in ["#10,20", "10,20"] {
            assert_eq!(
                parse_point_with_mode(text, base, CoordinateMode::CommandLine, &Units::default()),
                Some(Point::new(10.0, 20.0))
            );
        }
        assert_eq!(
            parse_point_with_mode("#10,20", base, mode, &Units::default()),
            Some(Point::new(10.0, 20.0))
        );
        assert_eq!(
            parse_point_with_mode(
                "@10,20",
                base,
                CoordinateMode::CommandLine,
                &Units::default()
            ),
            Some(Point::new(110.0, 220.0))
        );
        let point = parse_point_with_mode("10<90", base, mode, &Units::default()).unwrap();
        assert!((point.x - 100.0).abs() < 1e-9 && (point.y - 210.0).abs() < 1e-9);
        for text in [
            "NaN,0",
            "inf<0",
            "1,2,3",
            "#3",
            "@#1,2",
            "1e308,1e308",
            "**1,2",
            "*@1,2",
        ] {
            let origin = Point::new(1e308, 1e308);
            assert!(
                parse_point_with_mode(text, origin, mode, &Units::default()).is_none(),
                "{text}"
            );
        }
    }

    #[test]
    fn coordinates_are_read_in_the_custom_system_and_star_means_world() {
        let reading = Reading {
            units: Units::default(),
            ccs: Ccs::new(Point::new(10.0, 0.0), std::f64::consts::FRAC_PI_2),
        };
        let base = Point::new(1.0, 1.0);
        let read = |text| parse_point_with_mode(text, base, CoordinateMode::CommandLine, reading);
        assert_eq!(
            read("5,0"),
            Some(Point::new(10.0, 5.0)),
            "along the turned X"
        );
        assert_eq!(read("@5,0"), Some(Point::new(1.0, 6.0)));
        assert_eq!(read("*5,0"), Some(Point::new(5.0, 0.0)), "world");
        assert_eq!(read("@*5,0"), Some(Point::new(6.0, 1.0)));
        assert_eq!(read("#*5,0"), Some(Point::new(5.0, 0.0)));
        let polar = read("@2<90").unwrap();
        assert!((polar.x - -1.0).abs() < 1e-9 && (polar.y - 1.0).abs() < 1e-9);
        let pointer = CoordinateMode::DynamicPointer {
            default_relative: true,
        };
        let moved = parse_point_with_mode("3,0", base, pointer, reading).unwrap();
        assert_eq!(moved, Point::new(1.0, 4.0));
    }

    #[test]
    fn angles_are_read_as_directions_in_the_system_or_as_amounts() {
        let reading = Reading {
            units: Units {
                base_angle: std::f64::consts::FRAC_PI_2,
                ..Units::default()
            },
            ccs: Ccs::new(Point::new(0.0, 0.0), 0.5),
        };
        let direction = reading.direction("0").unwrap();
        assert!((direction - (0.5 + std::f64::consts::FRAC_PI_2)).abs() < 1e-12);
        let turn = reading.turn("30").unwrap();
        assert!((turn - 30.0_f64.to_radians()).abs() < 1e-12);
        assert_eq!(reading.show_turn(turn), "30");
        assert_eq!(reading.show_direction(direction), "0");
        assert_eq!(reading.show_point(Point::new(0.0, 0.0)), "0.0000,0.0000");
    }
}
