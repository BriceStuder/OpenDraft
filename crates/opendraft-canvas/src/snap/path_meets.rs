//! Where a polar or object snap tracking path crosses an object under the
//! pointer, as in AutoCAD: following a path onto an object snaps to the
//! crossing, an intersection on the path, while the Intersection (or
//! Apparent Intersection) snap is on.

use opendraft_core::{Bounds, Curve, Point};

use super::context::{Piece, Scene};
use super::otrack::{guides_near, Guide, TrackInput};
use super::{Class, Ranked, SnapKind, SnapPoint};

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn at(g: &Guide, t: f64) -> Point {
    Point::new(g.origin.x + g.dir.x * t, g.origin.y + g.dir.y * t)
}

/// The crossings of the paths near `cursor` with the pieces in `capture`,
/// within `tolerance` of the pointer, as `kind`. Each keeps its path, so the
/// path is drawn and the tooltip gives its distance and angle. Where a path
/// starts is that point's own snap, not a crossing.
pub(super) fn crossings(
    scene: &Scene<'_>,
    capture: &Bounds,
    track: &TrackInput,
    cursor: Point,
    tolerance: f64,
    kind: SnapKind,
) -> Vec<Ranked> {
    let guides = guides_near(track, cursor, tolerance);
    if guides.is_empty() {
        return Vec::new();
    }
    let items = scene.items(capture);
    let mut found = Vec::new();
    for g in &guides {
        // The stretch of the path that passes the pointer.
        let t = g.along(cursor);
        let reach = 2.0 * tolerance;
        let path = Piece::Curve(Curve::Segment {
            a: at(g, t - reach),
            b: at(g, t + reach),
        });
        for item in &items {
            for point in path.meets(&item.piece, capture) {
                let dist = distance(point, cursor);
                if dist > tolerance || distance(point, g.origin) < tolerance {
                    continue;
                }
                let mut snap = SnapPoint::new(point, kind).from(item.object.id);
                snap.guides = [Some(*g), None];
                found.push(Ranked {
                    snap,
                    dist,
                    class: Class::TrackCross,
                });
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use opendraft_core::Entity;

    use super::super::{directions, snap_all, Probe, SnapModes};
    use super::*;
    use crate::testing::objs;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    /// Polar tracking at 90° steps from `base`, and the running snaps
    /// `modes`.
    fn snapped(
        entities: &[Entity],
        base: Point,
        cursor: Point,
        modes: &SnapModes,
    ) -> Vec<SnapPoint> {
        let track = TrackInput {
            origins: Vec::new(),
            base: Some(base),
            track_dirs: Vec::new(),
            polar_dirs: directions(&[0.0, 90.0]),
            view: Bounds::from_points(p(-1000.0, -1000.0), p(1000.0, 1000.0)),
        };
        let probe = Probe {
            cursor,
            tolerance: 1.0,
            grid_step: None,
            grid_frame: opendraft_core::Ccs::WORLD,
            modes,
            from: Some(base),
            along: None,
            defer: false,
            tracks: &[],
            track: Some(&track),
        };
        snap_all(&objs(entities), &probe)
    }

    fn running(intersection: bool) -> SnapModes {
        let mut modes = SnapModes {
            grid: false,
            ..SnapModes::default()
        };
        for k in SnapKind::OBJECT {
            modes.set(k, k == SnapKind::Intersection && intersection);
        }
        modes
    }

    #[test]
    fn a_polar_path_snaps_to_where_it_crosses_a_line() {
        // As in the user's report: drawing up from (0, 0) at 90°, the
        // pointer arrives on a horizontal line a little off the path.
        let wall = line(p(-50.0, 40.0), p(50.0, 40.0));
        let found = snapped(&[wall], p(0.0, 0.0), p(0.4, 40.3), &running(true));
        let first = found[0];
        assert_eq!(first.kind, SnapKind::Intersection);
        assert!(distance(first.point, p(0.0, 40.0)) < 1e-9, "{first:?}");
        assert_eq!(first.guides[0].map(|g| g.origin), Some(p(0.0, 0.0)));
        assert_eq!(first.label(), "Intersection | Polar: 40.0000 < 90\u{b0}");
    }

    #[test]
    fn a_polar_path_crosses_a_circle_too() {
        let ring = Entity::Circle {
            center: p(0.0, 50.0),
            radius: 10.0,
        };
        let found = snapped(&[ring], p(0.0, 0.0), p(0.3, 40.5), &running(true));
        assert_eq!(found[0].kind, SnapKind::Intersection);
        assert!(distance(found[0].point, p(0.0, 40.0)) < 1e-9);
    }

    #[test]
    fn without_the_intersection_snap_the_path_alone_is_followed() {
        let wall = line(p(-50.0, 40.0), p(50.0, 40.0));
        let found = snapped(&[wall], p(0.0, 0.0), p(0.4, 40.3), &running(false));
        assert_eq!(found[0].kind, SnapKind::Polar);
        assert!(distance(found[0].point, p(0.0, 40.3)) < 1e-9);
    }

    #[test]
    fn a_crossing_far_from_the_pointer_is_not_offered() {
        let wall = line(p(-50.0, 40.0), p(50.0, 40.0));
        let found = snapped(&[wall], p(0.0, 0.0), p(0.2, 30.0), &running(true));
        assert!(found.iter().all(|s| s.kind != SnapKind::Intersection));
    }
}
