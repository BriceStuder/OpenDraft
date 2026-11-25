//! The custom coordinate system (AutoCAD's UCS, under our own name): an
//! origin and a turn about Z that typed coordinates, readouts, ortho, the
//! grid and tracking work in. Objects are always stored in world
//! coordinates; a system only converts to and from them. 2D only, so there
//! is no tilt about X or Y. Pure geometry.

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::layout::{LayoutId, ViewportId};
use crate::Point;

/// How many earlier systems each space keeps for the Previous option.
pub const PREVIOUS_KEPT: usize = 10;

/// An origin and the direction of the X axis, both in world coordinates.
/// Y is X turned a quarter counter-clockwise.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ccs {
    pub origin: Point,
    /// Radians counter-clockwise from world east, in `0..TAU`.
    pub angle: f64,
}

/// The cosine and sine of `angle`, exact on the quarter turns so a system
/// turned by a right angle keeps whole coordinates whole.
fn cos_sin(angle: f64) -> (f64, f64) {
    let quarters = angle / FRAC_PI_2;
    if (quarters - quarters.round()).abs() < 1e-12 {
        match (quarters.round() as i64).rem_euclid(4) {
            0 => (1.0, 0.0),
            1 => (0.0, 1.0),
            2 => (-1.0, 0.0),
            _ => (0.0, -1.0),
        }
    } else {
        let (s, c) = angle.sin_cos();
        (c, s)
    }
}

impl Ccs {
    /// The world coordinate system.
    pub const WORLD: Ccs = Ccs {
        origin: Point::new(0.0, 0.0),
        angle: 0.0,
    };

    /// A system at `origin` with its X axis at `angle` (brought into range).
    pub fn new(origin: Point, angle: f64) -> Self {
        Self { origin, angle }.sanitized()
    }

    /// The system at `origin` whose X axis points at `on_x`; none when the
    /// two points are the same.
    pub fn through(origin: Point, on_x: Point) -> Option<Self> {
        let (dx, dy) = (on_x.x - origin.x, on_x.y - origin.y);
        (dx.hypot(dy) > 1e-12).then(|| Self::new(origin, dy.atan2(dx)))
    }

    /// The system a file's origin and X and Y directions describe (DXF and
    /// DWG keep a UCS as three points), if it is flat and right-handed: both
    /// directions in the XY plane, Y a quarter turn counter-clockwise from X.
    /// A tilted (3D) or mirrored one is none. The origin's height is dropped.
    pub fn from_axes(origin: [f64; 3], x: [f64; 3], y: [f64; 3]) -> Option<Self> {
        let all = origin.iter().chain(&x).chain(&y);
        if !all.into_iter().all(|v| v.is_finite()) {
            return None;
        }
        let (xl, yl) = (x[0].hypot(x[1]), y[0].hypot(y[1]));
        if x[2].abs() >= 1e-9 || y[2].abs() >= 1e-9 || xl <= 1e-9 || yl <= 1e-9 {
            return None;
        }
        let (ux, uy) = ((x[0] / xl, x[1] / xl), (y[0] / yl, y[1] / yl));
        let turned = (uy.0 + ux.1).abs() < 1e-6 && (uy.1 - ux.0).abs() < 1e-6;
        turned.then(|| Self::new(Point::new(origin[0], origin[1]), ux.1.atan2(ux.0)))
    }

    /// The angle in `0..TAU` and finite numbers throughout; a damaged value
    /// falls back to the world's.
    pub fn sanitized(self) -> Self {
        let finite = |v: f64| if v.is_finite() { v } else { 0.0 };
        let mut angle = finite(self.angle).rem_euclid(TAU);
        if angle >= TAU - 1e-12 {
            angle = 0.0;
        }
        Self {
            origin: Point::new(finite(self.origin.x), finite(self.origin.y)),
            angle,
        }
    }

    /// Whether this is the world system (to within rounding).
    pub fn is_world(&self) -> bool {
        let tiny = |v: f64| v.abs() < 1e-12;
        tiny(self.origin.x) && tiny(self.origin.y) && (tiny(self.angle) || tiny(TAU - self.angle))
    }

    /// The same system with its origin moved.
    pub fn moved_to(self, origin: Point) -> Self {
        Self { origin, ..self }
    }

    /// The same system turned by `turn` radians about its own origin.
    pub fn turned(self, turn: f64) -> Self {
        Self::new(self.origin, self.angle + turn)
    }

    /// The unit vector of the X axis, in world coordinates.
    pub fn x_axis(&self) -> Point {
        let (c, s) = cos_sin(self.angle);
        Point::new(c, s)
    }

    /// The unit vector of the Y axis, in world coordinates.
    pub fn y_axis(&self) -> Point {
        let (c, s) = cos_sin(self.angle);
        Point::new(-s, c)
    }

    /// A direction given in this system, in world coordinates.
    pub fn vector_to_world(&self, v: Point) -> Point {
        let (c, s) = cos_sin(self.angle);
        Point::new(v.x * c - v.y * s, v.x * s + v.y * c)
    }

    /// A world direction, in this system.
    pub fn vector_from_world(&self, v: Point) -> Point {
        let (c, s) = cos_sin(self.angle);
        Point::new(v.x * c + v.y * s, -v.x * s + v.y * c)
    }

    /// A point given in this system, in world coordinates.
    pub fn to_world(&self, p: Point) -> Point {
        let v = self.vector_to_world(p);
        Point::new(self.origin.x + v.x, self.origin.y + v.y)
    }

    /// A world point, in this system.
    pub fn from_world(&self, p: Point) -> Point {
        self.vector_from_world(Point::new(p.x - self.origin.x, p.y - self.origin.y))
    }

    /// A direction angle measured in this system, as a world angle.
    pub fn angle_to_world(&self, angle: f64) -> f64 {
        angle + self.angle
    }

    /// A world direction angle, measured in this system.
    pub fn angle_from_world(&self, angle: f64) -> f64 {
        angle - self.angle
    }
}

/// A system saved under a name (the Named option). Names are unique,
/// ignoring case.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedCcs {
    pub name: String,
    pub ccs: Ccs,
}

/// The longest name a saved system may have (AutoCAD's limit for table names).
pub const NAME_MAX: usize = 255;

/// Why `name` cannot name a saved system, if it cannot.
pub fn name_refusal(name: &str) -> Option<&'static str> {
    const BAD: &[char] = &[
        '<', '>', '/', '\\', '"', ':', ';', '?', '*', '|', ',', '=', '`',
    ];
    if name.trim().is_empty() {
        Some(key!("A name is needed."))
    } else if name.chars().count() > NAME_MAX {
        Some(key!("The name is longer than 255 characters."))
    } else if name.contains(BAD) {
        Some(key!(
            "The name may not contain < > / \\ \" : ; ? * | , = or `."
        ))
    } else if name.eq_ignore_ascii_case("World") {
        Some(key!(
            "World is the world coordinate system and cannot be saved over."
        ))
    } else {
        None
    }
}

/// Where a system belongs: the Model tab, or one layout viewport (worked
/// through from its layout). Each has its own, and changing one never
/// changes another. A layout's sheet has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CcsSpace {
    Model,
    Viewport(LayoutId, ViewportId),
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_4, PI};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    #[test]
    fn world_changes_nothing() {
        let w = Ccs::WORLD;
        assert!(w.is_world());
        assert_eq!(w.to_world(p(3.0, -4.0)), p(3.0, -4.0));
        assert_eq!(w.from_world(p(3.0, -4.0)), p(3.0, -4.0));
    }

    #[test]
    fn points_go_there_and_back() {
        let c = Ccs::new(p(10.0, 5.0), 0.7);
        for q in [p(0.0, 0.0), p(3.0, -4.0), p(-120.5, 1e4)] {
            assert!(near(c.from_world(c.to_world(q)), q));
            assert!(near(c.to_world(c.from_world(q)), q));
        }
        assert!(near(c.to_world(p(0.0, 0.0)), p(10.0, 5.0)));
    }

    #[test]
    fn a_quarter_turn_is_exact() {
        let c = Ccs::new(p(1.0, 2.0), FRAC_PI_2);
        assert_eq!(c.to_world(p(3.0, 0.0)), p(1.0, 5.0));
        assert_eq!(c.to_world(p(0.0, 3.0)), p(-2.0, 2.0));
        assert_eq!(c.from_world(p(1.0, 5.0)), p(3.0, 0.0));
        assert_eq!(c.x_axis(), p(0.0, 1.0));
        assert_eq!(c.y_axis(), p(-1.0, 0.0));
    }

    #[test]
    fn two_points_give_the_origin_and_x_axis() {
        let c = Ccs::through(p(1.0, 1.0), p(2.0, 2.0)).unwrap();
        assert!((c.angle - FRAC_PI_4).abs() < 1e-12);
        assert!(near(c.to_world(p(2.0_f64.sqrt(), 0.0)), p(2.0, 2.0)));
        assert!(Ccs::through(p(1.0, 1.0), p(1.0, 1.0)).is_none());
    }

    #[test]
    fn angles_are_kept_in_range_and_damage_falls_back() {
        assert!((Ccs::new(p(0.0, 0.0), -FRAC_PI_2).angle - 1.5 * PI).abs() < 1e-12);
        assert_eq!(Ccs::new(p(0.0, 0.0), TAU).angle, 0.0);
        let bad = Ccs::new(p(f64::NAN, 1.0), f64::INFINITY);
        assert_eq!(bad, Ccs::new(p(0.0, 1.0), 0.0));
        assert!(Ccs::new(p(0.0, 0.0), TAU - 1e-14).is_world());
    }

    #[test]
    fn angles_convert_both_ways() {
        let c = Ccs::new(p(0.0, 0.0), 0.5);
        assert_eq!(c.angle_to_world(0.0), 0.5);
        assert_eq!(c.angle_from_world(0.5), 0.0);
        assert!((c.turned(0.25).angle - 0.75).abs() < 1e-12);
        assert_eq!(c.moved_to(p(4.0, 4.0)).origin, p(4.0, 4.0));
    }

    #[test]
    fn flat_right_handed_axes_are_a_system_and_others_are_not() {
        let c = Ccs::from_axes([1.0, 2.0, 7.0], [0.0, 2.0, 0.0], [-3.0, 0.0, 0.0]).unwrap();
        assert_eq!(c.origin, p(1.0, 2.0));
        assert!((c.angle - FRAC_PI_2).abs() < 1e-12);
        let mirrored = Ccs::from_axes([0.0; 3], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]);
        let tilted = Ccs::from_axes([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        assert!(mirrored.is_none() && tilted.is_none());
        assert!(Ccs::from_axes([0.0; 3], [0.0; 3], [0.0, 1.0, 0.0]).is_none());
    }

    #[test]
    fn names_are_checked() {
        assert!(name_refusal("Wing A").is_none());
        assert!(name_refusal(" ").is_some());
        assert!(name_refusal("a/b").is_some());
        assert!(name_refusal("world").is_some());
        assert!(name_refusal(&"x".repeat(256)).is_some());
    }
}
