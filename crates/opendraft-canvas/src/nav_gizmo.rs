//! The geometry of the navigation gizmo in the corner of the drawing: a home
//! button in the middle, a ring of eight sectors round it, and an arrow outside
//! the ring. The arrow points in from where the drawing's up (world +Y) lies on
//! the screen, so it is at the top while the view is not turned. Clicking a
//! sector turns the view so that up lies there; dragging the arrow turns it
//! freely. This module only works out where things are and what a point is
//! over; the app paints it and acts on the answers.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

use egui::{pos2, Pos2};

/// Sectors in the ring, 45 degrees each.
pub const SECTORS: usize = 8;

/// Radius of the whole gizmo, arrow included, in screen points.
pub const RADIUS: f32 = 46.0;
/// Outer edge of the ring.
const RING_OUTER: f32 = 36.0;
/// Inner edge of the ring.
const RING_INNER: f32 = 23.0;
/// The home button.
const HOME: f32 = 17.0;
/// How far the arrow's point stands off the ring, and its length and half
/// width.
const ARROW_GAP: f32 = 3.0;
const ARROW_LENGTH: f32 = 7.0;
const ARROW_HALF_WIDTH: f32 = 5.5;
/// The level button below the ring: its radius and the space between.
const LEVEL_RADIUS: f32 = 10.0;
const LEVEL_GAP: f32 = 4.0;
/// Slack round the arrow where a press still takes it, as a distance in points
/// along the circle it travels on.
const ARROW_REACH: f32 = 9.0;

/// What a point on the gizmo is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The button in the middle: the plan view of the world.
    Home,
    /// A sector of the ring, counted counter-clockwise from the right
    /// (0 right, 2 top, 4 left, 6 bottom).
    Sector(usize),
    /// The arrow outside the ring.
    Arrow,
    /// The button below the ring: level the view on two picked points.
    Level,
}

/// Where the gizmo is on the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gizmo {
    pub centre: Pos2,
}

/// The twist, in radians, that puts up at the sector's side of the screen.
pub fn sector_twist(sector: usize) -> f64 {
    let turned = sector.rem_euclid(SECTORS) as f64 * FRAC_PI_4 - FRAC_PI_2;
    turned.rem_euclid(TAU)
}

/// The sector the view is turned to exactly, if it is (to within a hair).
pub fn sector_of_twist(twist: f64) -> Option<usize> {
    let steps = (twist + FRAC_PI_2) / FRAC_PI_4;
    let nearest = steps.round();
    ((steps - nearest).abs() < 1e-6).then(|| (nearest as i64).rem_euclid(SECTORS as i64) as usize)
}

/// Where up lies on the screen: an angle counter-clockwise from the right.
pub fn arrow_angle(twist: f64) -> f64 {
    FRAC_PI_2 + twist
}

/// The angle of `p` round `centre`, counter-clockwise on the screen from the
/// right (screen y runs down).
fn angle_round(centre: Pos2, p: Pos2) -> f64 {
    f64::from(centre.y - p.y).atan2(f64::from(p.x - centre.x))
}

impl Gizmo {
    /// The gizmo in the top right corner of `canvas`, `margin` in from its edges.
    pub fn in_corner(canvas: egui::Rect, margin: f32) -> Self {
        Self {
            centre: pos2(
                canvas.right() - margin - RADIUS,
                canvas.top() + margin + RADIUS,
            ),
        }
    }

    /// The point at `angle` (counter-clockwise from the right) and `radius`
    /// from the middle.
    pub fn at(&self, angle: f64, radius: f32) -> Pos2 {
        pos2(
            self.centre.x + radius * angle.cos() as f32,
            self.centre.y - radius * angle.sin() as f32,
        )
    }

    /// The twist that puts up under the pointer at `p`, as dragging the arrow
    /// does.
    pub fn twist_at(&self, p: Pos2) -> f64 {
        (angle_round(self.centre, p) - FRAC_PI_2).rem_euclid(TAU)
    }

    /// What `p` is over, with the view turned to `twist`. A press in the thin
    /// gap between two sectors goes to the nearer one.
    pub fn part_at(&self, p: Pos2, twist: f64) -> Option<Part> {
        let d = (p - self.centre).length();
        if d <= HOME {
            return Some(Part::Home);
        }
        if (RING_INNER..=RING_OUTER).contains(&d) {
            let sector = (angle_round(self.centre, p) / FRAC_PI_4).round() as i64;
            return Some(Part::Sector(sector.rem_euclid(SECTORS as i64) as usize));
        }
        if (p - self.level_centre()).length() <= LEVEL_RADIUS {
            return Some(Part::Level);
        }
        // The arrow: near the point it stands at, a little beyond the ring.
        let tip = self.at(arrow_angle(twist), RING_OUTER + ARROW_GAP + ARROW_LENGTH);
        let near_ring =
            (RING_OUTER - 2.0..=RING_OUTER + ARROW_GAP + ARROW_LENGTH + 5.0).contains(&d);
        (near_ring && (p - tip).length() <= ARROW_REACH + ARROW_HALF_WIDTH).then_some(Part::Arrow)
    }

    /// The outline of a sector of the ring, as points round it, with
    /// `gap` degrees left out between it and each neighbour.
    pub fn sector_outline(&self, sector: usize, gap: f64) -> Vec<Pos2> {
        let middle = sector as f64 * FRAC_PI_4;
        let half = FRAC_PI_4 / 2.0 - gap.to_radians() / 2.0;
        let steps = 8;
        let arc = |radius: f32, reverse: bool| {
            (0..=steps)
                .map(move |i| {
                    let i = if reverse { steps - i } else { i };
                    middle - half + 2.0 * half * f64::from(i) / f64::from(steps)
                })
                .map(move |a| (a, radius))
        };
        arc(RING_OUTER, false)
            .chain(arc(RING_INNER, true))
            .map(|(a, r)| self.at(a, r))
            .collect()
    }

    /// The three corners of the arrow for a view turned to `twist`: its point
    /// first, towards the middle.
    pub fn arrow_corners(&self, twist: f64) -> [Pos2; 3] {
        let angle = arrow_angle(twist);
        let base = RING_OUTER + ARROW_GAP + ARROW_LENGTH;
        let side = self.at(angle + FRAC_PI_2, ARROW_HALF_WIDTH);
        let across = side - self.centre;
        let foot = self.at(angle, base);
        [
            self.at(angle, RING_OUTER + ARROW_GAP),
            foot + across,
            foot - across,
        ]
    }

    /// The middle of the level button, below the ring.
    pub fn level_centre(&self) -> Pos2 {
        pos2(
            self.centre.x,
            self.centre.y + RADIUS + LEVEL_GAP + LEVEL_RADIUS,
        )
    }

    /// The radius of the level button.
    pub fn level_radius(&self) -> f32 {
        LEVEL_RADIUS
    }

    /// The size of the whole gizmo, level button included.
    pub fn size() -> egui::Vec2 {
        egui::vec2(2.0 * RADIUS, 2.0 * RADIUS + LEVEL_GAP + 2.0 * LEVEL_RADIUS)
    }

    /// The radius of the home button.
    pub fn home_radius(&self) -> f32 {
        HOME
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gizmo() -> Gizmo {
        Gizmo {
            centre: pos2(100.0, 100.0),
        }
    }

    #[test]
    fn the_middle_is_home_the_ring_is_sectors_and_far_away_is_nothing() {
        let g = gizmo();
        assert_eq!(g.part_at(pos2(100.0, 100.0), 0.0), Some(Part::Home));
        assert_eq!(g.part_at(pos2(100.0, 70.0), 0.0), Some(Part::Sector(2)));
        assert_eq!(g.part_at(pos2(130.0, 100.0), 0.0), Some(Part::Sector(0)));
        assert_eq!(g.part_at(pos2(70.0, 100.0), 0.0), Some(Part::Sector(4)));
        assert_eq!(g.part_at(pos2(100.0, 130.0), 0.0), Some(Part::Sector(6)));
        // Up and to the right on the screen is the sector between right and top.
        assert_eq!(g.part_at(pos2(122.0, 78.0), 0.0), Some(Part::Sector(1)));
        assert_eq!(g.part_at(pos2(100.0, 40.0), 0.0), None);
    }

    #[test]
    fn the_level_button_is_below_the_ring_and_inside_the_gizmo() {
        let g = gizmo();
        let at = g.level_centre();
        assert_eq!(g.part_at(at, 0.0), Some(Part::Level));
        assert_eq!(g.part_at(pos2(at.x + LEVEL_RADIUS + 3.0, at.y), 0.0), None);
        assert!(at.y + LEVEL_RADIUS <= g.centre.y + Gizmo::size().y - RADIUS + 0.01);
        assert!(
            at.y - LEVEL_RADIUS > g.centre.y + RADIUS,
            "clear of the arrow's circle"
        );
    }

    #[test]
    fn the_arrow_is_over_the_ring_where_up_lies() {
        let g = gizmo();
        let out = RING_OUTER + ARROW_GAP + 3.0;
        assert_eq!(g.part_at(pos2(100.0, 100.0 - out), 0.0), Some(Part::Arrow));
        // Turned a quarter, up lies to the left.
        assert_eq!(
            g.part_at(pos2(100.0 - out, 100.0), FRAC_PI_2),
            Some(Part::Arrow)
        );
        assert_eq!(g.part_at(pos2(100.0, 100.0 - out), FRAC_PI_2), None);
    }

    #[test]
    fn a_sector_puts_up_on_its_side() {
        assert_eq!(sector_twist(2), 0.0);
        assert!((sector_twist(0) - 3.0 * FRAC_PI_2).abs() < 1e-12, "right");
        assert!((sector_twist(4) - FRAC_PI_2).abs() < 1e-12, "left");
        assert!((sector_twist(1) - 7.0 * FRAC_PI_4).abs() < 1e-12);
        for sector in 0..SECTORS {
            assert_eq!(sector_of_twist(sector_twist(sector)), Some(sector));
        }
        assert_eq!(sector_of_twist(0.3), None);
    }

    #[test]
    fn the_arrow_follows_the_twist_and_the_pointer_sets_it() {
        let g = gizmo();
        for twist in [0.0, 0.7, 2.0, 4.5] {
            let a = arrow_angle(twist);
            let p = g.at(a, 40.0);
            assert!((g.twist_at(p) - twist).abs() < 1e-5, "twist {twist}");
        }
        // The pointer to the right of the middle puts up on the right.
        assert!((g.twist_at(pos2(150.0, 100.0)) - 3.0 * FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn the_corner_holds_the_whole_gizmo_inside_the_canvas() {
        let canvas = egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0));
        let g = Gizmo::in_corner(canvas, 10.0);
        assert!((g.centre.x + RADIUS - 790.0).abs() < 1e-4);
        assert!((g.centre.y - RADIUS - 10.0).abs() < 1e-4);
    }

    #[test]
    fn a_sector_outline_stays_inside_its_ring() {
        let g = gizmo();
        for p in g.sector_outline(3, 4.0) {
            let d = (p - g.centre).length();
            assert!((RING_INNER - 0.01..=RING_OUTER + 0.01).contains(&d));
        }
    }
}
