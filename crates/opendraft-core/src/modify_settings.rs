//! What the modify commands remember, saved in the drawing as AutoCAD saves
//! them: `FILLETRAD`, `CHAMFERA` to `CHAMFERD`, `CHAMMODE`, `TRIMMODE` and
//! `OFFSETDIST`, plus LENGTHEN's last delta (which AutoCAD keeps only for the
//! session, so DXF has no variable for it).

/// The drawing-wide defaults of FILLET, CHAMFER, OFFSET and LENGTHEN.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModifySettings {
    /// `FILLETRAD`.
    pub fillet_radius: f64,
    /// `CHAMFERA` and `CHAMFERB`.
    pub chamfer_first: f64,
    pub chamfer_second: f64,
    /// `CHAMFERC` and `CHAMFERD` (the angle in degrees).
    pub chamfer_length: f64,
    pub chamfer_angle: f64,
    /// `CHAMMODE`: chamfer by length and angle rather than two distances.
    pub chamfer_by_angle: bool,
    /// `TRIMMODE`: cut the objects back to the corner.
    pub trim: bool,
    /// `OFFSETDIST`: the distance offered as the default, negative for Through.
    pub offset_distance: f64,
    /// The last delta LENGTHEN was given.
    pub lengthen_delta: Option<f64>,
}

impl Default for ModifySettings {
    fn default() -> Self {
        Self {
            fillet_radius: 10.0,
            chamfer_first: 10.0,
            chamfer_second: 10.0,
            chamfer_length: 10.0,
            chamfer_angle: 45.0,
            chamfer_by_angle: false,
            trim: true,
            offset_distance: 1.0,
            lengthen_delta: None,
        }
    }
}

impl ModifySettings {
    /// Whether every value is one the commands can use: sizes finite and not
    /// negative, the angle from 0 to 180, the offset distance finite and not
    /// zero (a negative one means Through).
    pub fn is_valid(&self) -> bool {
        let size = |n: f64| n.is_finite() && n >= 0.0;
        size(self.fillet_radius)
            && size(self.chamfer_first)
            && size(self.chamfer_second)
            && size(self.chamfer_length)
            && self.chamfer_angle.is_finite()
            && (0.0..=180.0).contains(&self.chamfer_angle)
            && self.offset_distance.is_finite()
            && self.offset_distance != 0.0
            && self.lengthen_delta.is_none_or(f64::is_finite)
    }

    /// The same settings with every value that is not usable put back to its
    /// default, so a damaged file spoils only the value it damaged.
    pub fn sanitized(self) -> Self {
        let d = Self::default();
        let size = |n: f64, default: f64| {
            if n.is_finite() && n >= 0.0 {
                n
            } else {
                default
            }
        };
        Self {
            fillet_radius: size(self.fillet_radius, d.fillet_radius),
            chamfer_first: size(self.chamfer_first, d.chamfer_first),
            chamfer_second: size(self.chamfer_second, d.chamfer_second),
            chamfer_length: size(self.chamfer_length, d.chamfer_length),
            chamfer_angle: if (0.0..=180.0).contains(&self.chamfer_angle) {
                self.chamfer_angle
            } else {
                d.chamfer_angle
            },
            offset_distance: if self.offset_distance.is_finite() && self.offset_distance != 0.0 {
                self.offset_distance
            } else {
                d.offset_distance
            },
            lengthen_delta: self.lengthen_delta.filter(|n| n.is_finite()),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_usable() {
        assert!(ModifySettings::default().is_valid());
    }

    #[test]
    fn sanitizing_repairs_only_what_is_broken() {
        let broken = ModifySettings {
            fillet_radius: -1.0,
            chamfer_first: 3.0,
            chamfer_angle: 500.0,
            offset_distance: 0.0,
            lengthen_delta: Some(f64::NAN),
            trim: false,
            ..ModifySettings::default()
        };
        assert!(!broken.is_valid());
        let fixed = broken.sanitized();
        assert!(fixed.is_valid());
        assert_eq!(fixed.fillet_radius, 10.0);
        assert_eq!(fixed.chamfer_first, 3.0);
        assert_eq!(fixed.chamfer_angle, 45.0);
        assert_eq!(fixed.offset_distance, 1.0);
        assert_eq!(fixed.lengthen_delta, None);
        assert!(!fixed.trim);
    }

    #[test]
    fn a_negative_offset_distance_is_through_and_is_kept() {
        let through = ModifySettings {
            offset_distance: -2.5,
            ..ModifySettings::default()
        };
        assert!(through.is_valid());
        assert_eq!(through.sanitized(), through);
    }
}
