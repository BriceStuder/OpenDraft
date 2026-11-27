//! What PEDIT keeps beside a polyline that the polyline itself cannot hold:
//! the tangent direction set at its vertices (Edit vertex, Tangent), and the
//! frame a Fit or Spline curve was made from, so that Decurve can bring the
//! original vertices back.
//!
//! A record belongs to one polyline by id and describes it as it was left
//! (`shape`). It applies only while the polyline is still exactly that: any
//! other edit (moving it, closing it, widening it...) makes it stop applying
//! and it is then ignored and not saved, so a record can never contradict the
//! drawing.

use crate::pedit::PolyData;
use crate::{EntityId, Point};

/// The polyline a Fit or Spline curve was made from.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveFrame {
    /// The vertices, bulges, widths and closing as they were.
    pub shape: PolyData,
    /// The tangent direction (radians) set at each vertex, `None` where none.
    pub tangents: Vec<Option<f64>>,
}

/// One polyline's tangents and frame. See the module notes.
#[derive(Clone, Debug, PartialEq)]
pub struct PolylineCurve {
    pub id: EntityId,
    /// The polyline the record was made for.
    pub shape: PolyData,
    /// The tangent direction (radians) at each vertex of `shape`, `None`
    /// where none was set; one per vertex.
    pub tangents: Vec<Option<f64>>,
    /// Set for a polyline made by Fit or Spline.
    pub frame: Option<CurveFrame>,
}

impl PolylineCurve {
    /// Whether the record still describes `current`.
    pub fn applies_to(&self, current: &PolyData) -> bool {
        self.shape == *current
    }

    /// Whether the record holds anything worth keeping.
    pub fn is_empty(&self) -> bool {
        self.frame.is_none() && self.tangents.iter().all(Option::is_none)
    }

    /// Whether the numbers in the record can be used: one tangent per vertex
    /// and finite angles, on the frame too.
    pub fn is_valid(&self) -> bool {
        let tangents_ok =
            |t: &[Option<f64>], n: usize| t.len() == n && t.iter().flatten().all(|a| a.is_finite());
        tangents_ok(&self.tangents, self.shape.points.len())
            && self.frame.as_ref().is_none_or(|f| {
                tangents_ok(&f.tangents, f.shape.points.len()) && f.shape.points.len() >= 2
            })
    }
}

/// The direction (a unit vector) a tangent angle stands for.
pub fn direction_of(angle: f64) -> Point {
    Point::new(angle.cos(), angle.sin())
}

/// The polyline as the drawing holds it: what `into_entity` keeps (no bulge
/// or width list when nothing in it is curved or wide), so that a record made
/// for it is found again.
pub fn normalized(data: PolyData) -> PolyData {
    PolyData::of(&data.into_entity()).unwrap_or_else(|| PolyData {
        points: Vec::new(),
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    })
}

/// The curve Fit (`fit`) or Spline makes of `data`, and the frame it was made
/// from. A polyline that is already a curve of this record is curved again from
/// its frame (so Spline after Fit goes back to the frame's vertices), with the
/// tangents set there; otherwise from `data` and the tangents set on it.
/// `None` when it cannot be curved (fewer than three vertices...).
pub fn curved(
    data: &PolyData,
    record: Option<&PolylineCurve>,
    fit: bool,
) -> Option<(PolyData, CurveFrame)> {
    let frame = match record {
        Some(r) if r.applies_to(data) => match &r.frame {
            Some(frame) => frame.clone(),
            None => CurveFrame {
                shape: data.clone(),
                tangents: r.tangents.clone(),
            },
        },
        _ => CurveFrame {
            shape: data.clone(),
            tangents: vec![None; data.points.len()],
        },
    };
    let made = if fit {
        frame.shape.fitted_with(&frame.tangents)?
    } else {
        frame.shape.splined()?
    };
    Some((made, frame))
}

/// What Decurve makes of `data`: the frame its record holds (a polyline made by
/// Fit or Spline), else every segment straight. Also the tangents that go on
/// applying to the result (none when it was only straightened).
pub fn decurved(data: &PolyData, record: Option<&PolylineCurve>) -> (PolyData, Vec<Option<f64>>) {
    match record {
        Some(r) if r.applies_to(data) => match &r.frame {
            Some(frame) => (frame.shape.clone(), frame.tangents.clone()),
            None => (data.decurved(), Vec::new()),
        },
        _ => (data.decurved(), Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(points: &[(f64, f64)]) -> PolyData {
        PolyData {
            points: points.iter().map(|p| Point::new(p.0, p.1)).collect(),
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        }
    }

    #[test]
    fn a_record_applies_only_to_the_polyline_it_was_made_for() {
        let s = shape(&[(0.0, 0.0), (1.0, 0.0), (2.0, 1.0)]);
        let record = PolylineCurve {
            id: EntityId(1),
            shape: s.clone(),
            tangents: vec![None, Some(0.5), None],
            frame: None,
        };
        assert!(record.applies_to(&s));
        assert!(record.is_valid() && !record.is_empty());
        assert!(!record.applies_to(&s.with_closed(true)));
        assert!(!record.applies_to(&s.with_width(1.0)));
        assert!(!record.applies_to(&shape(&[(0.0, 0.0), (1.0, 0.0), (2.0, 2.0)])));
    }

    #[test]
    fn a_record_with_the_wrong_number_of_tangents_or_a_bad_angle_is_not_valid() {
        let s = shape(&[(0.0, 0.0), (1.0, 0.0)]);
        let mut record = PolylineCurve {
            id: EntityId(1),
            shape: s,
            tangents: vec![None],
            frame: None,
        };
        assert!(!record.is_valid());
        record.tangents = vec![Some(f64::NAN), None];
        assert!(!record.is_valid());
        record.tangents = vec![None, None];
        assert!(record.is_valid() && record.is_empty());
    }

    fn zigzag() -> PolyData {
        shape(&[(0.0, 0.0), (4.0, 3.0), (8.0, 0.0), (12.0, 3.0)])
    }

    #[test]
    fn a_curve_remembers_its_frame_and_decurve_brings_it_back() {
        let frame = zigzag();
        let (made, kept) = curved(&frame, None, true).unwrap();
        assert!(made.points.len() > frame.points.len());
        assert_eq!(kept.shape, frame);
        let record = PolylineCurve {
            id: EntityId(1),
            shape: normalized(made.clone()),
            tangents: vec![None; made.points.len()],
            frame: Some(kept),
        };
        let now = normalized(made);
        let (back, tangents) = decurved(&now, Some(&record));
        assert_eq!(back, frame);
        assert_eq!(tangents.len(), frame.points.len());
        // Edited since (here: closed), the frame no longer applies and Decurve
        // only straightens what is there.
        let closed = now.with_closed(true);
        let (plain, none) = decurved(&closed, Some(&record));
        assert_eq!(plain, closed.decurved());
        assert!(none.is_empty());
    }

    #[test]
    fn spline_after_fit_starts_again_from_the_frame_and_uses_its_tangents() {
        let frame = zigzag();
        let set = vec![None, Some(0.5), None, None];
        let record = PolylineCurve {
            id: EntityId(1),
            shape: frame.clone(),
            tangents: set.clone(),
            frame: None,
        };
        let (fitted, kept) = curved(&frame, Some(&record), true).unwrap();
        assert_eq!(kept.tangents, set);
        assert_ne!(
            fitted,
            frame.fitted().unwrap(),
            "the tangent changed the fit"
        );
        let after = PolylineCurve {
            id: EntityId(1),
            shape: normalized(fitted.clone()),
            tangents: vec![None; fitted.points.len()],
            frame: Some(kept),
        };
        let (splined, again) = curved(&normalized(fitted), Some(&after), false).unwrap();
        assert_eq!(
            again.shape, frame,
            "the spline is of the frame, not of the fit"
        );
        assert_eq!(splined, frame.splined().unwrap());
    }

    #[test]
    fn a_polyline_that_cannot_be_curved_gives_nothing() {
        assert!(curved(&shape(&[(0.0, 0.0), (1.0, 1.0)]), None, true).is_none());
    }
}
