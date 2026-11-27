//! What FILLET and CHAMFER remember between uses, like AutoCAD's `FILLETRAD`,
//! `CHAMFERA`, `CHAMFERB`, `CHAMFERC`, `CHAMFERD`, `CHAMMODE` and `TRIMMODE`.
//! They are kept in the drawing (see `ModifySettings`), so each drawing has
//! its own, and they are the system variables of those names.

use opendraft_commands::{Cancelled, Editor};
use opendraft_core::{Chamfer, ModifySettings};

/// The corner commands' view of the drawing's [`ModifySettings`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    pub radius: f64,
    /// The two chamfer distances (`CHAMFERA`, `CHAMFERB`).
    pub first: f64,
    pub second: f64,
    /// The chamfer length and angle in degrees (`CHAMFERC`, `CHAMFERD`).
    pub length: f64,
    pub angle: f64,
    /// Chamfer by length and angle rather than two distances (`CHAMMODE`).
    pub by_angle: bool,
    /// Cut the lines back to the joint (`TRIMMODE`).
    pub trim: bool,
}

impl Default for Memory {
    fn default() -> Self {
        Self::of(&ModifySettings::default())
    }
}

impl Memory {
    pub fn of(s: &ModifySettings) -> Self {
        Self {
            radius: s.fillet_radius,
            first: s.chamfer_first,
            second: s.chamfer_second,
            length: s.chamfer_length,
            angle: s.chamfer_angle,
            by_angle: s.chamfer_by_angle,
            trim: s.trim,
        }
    }

    /// Put these values back into the drawing's settings.
    pub fn store(&self, s: &mut ModifySettings) {
        s.fillet_radius = self.radius;
        s.chamfer_first = self.first;
        s.chamfer_second = self.second;
        s.chamfer_length = self.length;
        s.chamfer_angle = self.angle;
        s.chamfer_by_angle = self.by_angle;
        s.trim = self.trim;
    }

    /// What the active drawing holds now.
    pub fn read(ed: &Editor) -> Result<Self, Cancelled> {
        ed.read(|doc| Self::of(&doc.modify_settings()))
    }

    /// Change what the active drawing holds (not an undo step).
    pub fn change(
        ed: &Editor,
        change: impl FnOnce(&mut Memory) + Send + 'static,
    ) -> Result<(), Cancelled> {
        ed.change_modify_settings(move |s| {
            let mut m = Self::of(s);
            change(&mut m);
            m.store(s);
        })
    }

    pub fn chamfer(&self) -> Chamfer {
        if self.by_angle {
            Chamfer::Angle {
                length: self.length,
                angle: self.angle.to_radians(),
            }
        } else {
            Chamfer::Distances(self.first, self.second)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_and_the_settings_agree_both_ways() {
        let mut s = ModifySettings::default();
        let mut m = Memory::of(&s);
        assert_eq!(m, Memory::default());
        m.radius = 3.5;
        m.by_angle = true;
        m.trim = false;
        m.store(&mut s);
        assert_eq!(
            (s.fillet_radius, s.chamfer_by_angle, s.trim),
            (3.5, true, false)
        );
        assert_eq!(Memory::of(&s), m);
    }
}
