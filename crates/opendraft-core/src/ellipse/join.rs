//! Joining elliptical arcs. Only arcs of one ellipse join, end to end, as
//! circular arcs do in [`crate::join`]; arcs that close the turn become the
//! whole ellipse.

use std::f64::consts::TAU;

use super::Ellipse;
use crate::join::Merged;
use crate::{Entity, Point};

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

impl Ellipse {
    /// Whether `other` lies on the same ellipse (its major axis may point
    /// the other way; a circle's may point anywhere).
    pub fn same_ellipse(&self, other: &Ellipse, tolerance: f64) -> bool {
        let (a, b) = (self.major_radius(), other.major_radius());
        let same_size = (a - b).abs() <= tolerance
            && (self.minor_radius() - other.minor_radius()).abs() <= tolerance;
        if !same_size || distance(self.center, other.center) > tolerance {
            return false;
        }
        let round = (a - self.minor_radius()).abs() <= tolerance;
        let (u, v) = (self.major_axis, other.major_axis);
        // Parallel axes, either way round.
        round || (u.x * v.y - u.y * v.x).abs() / (a * b) <= tolerance / a.max(1.0)
    }
}

/// Every run of elliptical arcs among `items` that meet end to end on one
/// ellipse (ends within `tolerance`), each as one arc or a whole ellipse.
pub(crate) fn join_elliptical_arcs(items: &[Entity], tolerance: f64) -> Vec<Merged> {
    let arcs: Vec<Option<Ellipse>> = items
        .iter()
        .map(|item| match item {
            Entity::Ellipse(e) if !e.is_full() => Some(*e),
            _ => None,
        })
        .collect();
    let mut taken = vec![false; items.len()];
    let mut made = Vec::new();
    for seed in 0..items.len() {
        let Some(mut run) = arcs[seed].filter(|_| !taken[seed]) else {
            continue;
        };
        taken[seed] = true;
        let mut used = vec![seed];
        // Grow the run at either end until nothing more meets it.
        'grow: while run.sweep() < TAU - 1e-9 {
            for (i, other) in arcs.iter().enumerate() {
                let Some(other) = other.filter(|o| !taken[i] && run.same_ellipse(o, tolerance))
                else {
                    continue;
                };
                // Both run counter-clockwise, so one's end meets the other's start.
                let after = distance(run.end_point(), other.start_point()) <= tolerance;
                let before = distance(other.end_point(), run.start_point()) <= tolerance;
                if after || before {
                    let start = if after {
                        run.start
                    } else {
                        run.start - other.sweep()
                    };
                    let sweep = (run.sweep() + other.sweep()).min(TAU);
                    let Ok(longer) = Ellipse::new(
                        run.center,
                        run.major_axis,
                        run.radius_ratio,
                        start,
                        start + sweep,
                    ) else {
                        continue;
                    };
                    run = longer;
                    taken[i] = true;
                    used.push(i);
                    continue 'grow;
                }
            }
            break;
        }
        if used.len() < 2 {
            taken[seed] = false;
            continue;
        }
        used.sort_unstable();
        made.push(Merged {
            used,
            entity: Entity::Ellipse(run),
        });
    }
    made
}

/// The one arc (or whole ellipse) that covers every elliptical arc of
/// `items`, when the objects JOIN can use are at least two elliptical arcs,
/// all of one ellipse, meeting or not. As for circular arcs the first is the
/// source: the result starts where it does and runs counter-clockwise until it
/// has covered the others, across any gap.
pub(crate) fn elliptical_arcs_across_gaps(items: &[Entity], tolerance: f64) -> Option<Merged> {
    let mut arcs: Vec<(usize, Ellipse)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        match item {
            Entity::Ellipse(e) if !e.is_full() => arcs.push((i, *e)),
            Entity::Line { .. } | Entity::Arc { .. } | Entity::Polyline { closed: false, .. } => {
                return None
            }
            _ => {}
        }
    }
    let &(_, source) = arcs.first()?;
    if arcs.len() < 2 || arcs.iter().any(|(_, e)| !source.same_ellipse(e, tolerance)) {
        return None;
    }
    // How far round from the source's start each arc ends, counter-clockwise.
    let reach = arcs
        .iter()
        .map(|(_, e)| {
            (source.parameter_of(e.start_point()) - source.start).rem_euclid(TAU) + e.sweep()
        })
        .fold(0.0, f64::max);
    let made = if reach >= TAU - 1e-9 {
        Ellipse::full(source.center, source.major_axis, source.radius_ratio)
    } else {
        Ellipse::new(
            source.center,
            source.major_axis,
            source.radius_ratio,
            source.start,
            source.start + reach,
        )
    };
    Some(Merged {
        used: arcs.iter().map(|(i, _)| *i).collect(),
        entity: Entity::Ellipse(made.ok()?),
    })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn piece(axis: Point, start: f64, end: f64) -> Entity {
        Entity::Ellipse(Ellipse::new(p(1.0, 1.0), axis, 0.5, start, end).unwrap())
    }

    #[test]
    fn arcs_of_one_ellipse_join_and_close_into_the_whole_ellipse() {
        let axis = p(4.0, 0.0);
        let items = [piece(axis, 0.0, 1.0), piece(axis, 1.0, 2.5)];
        let made = join_elliptical_arcs(&items, 1e-6);
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].used, vec![0, 1]);
        let Entity::Ellipse(e) = made[0].entity else {
            panic!()
        };
        assert!((e.start - 0.0).abs() < 1e-9 && (e.end - 2.5).abs() < 1e-9);
        // The same ellipse described with its axis the other way round.
        let flipped = p(-4.0, 0.0);
        let items = [
            piece(axis, 0.0, PI),
            piece(flipped, 0.0, FRAC_PI_2),
            piece(flipped, FRAC_PI_2, PI),
        ];
        let made = join_elliptical_arcs(&items, 1e-6);
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].used, vec![0, 1, 2]);
        assert!(matches!(made[0].entity, Entity::Ellipse(e) if e.is_full()));
    }

    #[test]
    fn arcs_of_other_ellipses_or_with_a_gap_do_not_meet() {
        let items = [
            piece(p(4.0, 0.0), 0.0, 1.0),
            piece(p(0.0, 4.0), 1.0, 2.0),
            piece(p(4.0, 0.0), 1.5, 2.0),
        ];
        assert!(join_elliptical_arcs(&items, 1e-6).is_empty());
    }

    #[test]
    fn arcs_of_one_ellipse_join_across_a_gap_from_the_source() {
        let axis = p(4.0, 0.0);
        let items = [piece(axis, 0.0, 1.0), piece(axis, 2.0, 3.0)];
        let made = elliptical_arcs_across_gaps(&items, 1e-6).unwrap();
        assert_eq!(made.used, vec![0, 1]);
        let Entity::Ellipse(e) = made.entity else {
            panic!()
        };
        assert!(e.start.abs() < 1e-9 && (e.end - 3.0).abs() < 1e-9, "{e:?}");
        // The other way round: from the second's start, round past the first.
        let made =
            elliptical_arcs_across_gaps(&[items[1].clone(), items[0].clone()], 1e-6).unwrap();
        let Entity::Ellipse(e) = made.entity else {
            panic!()
        };
        assert!((e.start - 2.0).abs() < 1e-9);
        assert!(
            (e.sweep() - (TAU - 2.0 + 1.0)).abs() < 1e-9,
            "{}",
            e.sweep()
        );
        // Another ellipse, or a line among them, is not joined this way.
        let other = piece(p(0.0, 4.0), 2.0, 3.0);
        assert!(elliptical_arcs_across_gaps(&[items[0].clone(), other], 1e-6).is_none());
    }
}
