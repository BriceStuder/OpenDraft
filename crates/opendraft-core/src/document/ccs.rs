//! The drawing's custom coordinate systems: the Model tab's, each layout
//! viewport's, the ones saved by name, and the earlier ones the Previous
//! option goes back to.

use super::Document;
use crate::ccs::{Ccs, CcsSpace, NamedCcs, PREVIOUS_KEPT};
use crate::layout::{LayoutId, ViewportId};

impl Document {
    /// The layout viewport being worked through, if one is (see
    /// `set_active_viewport`).
    pub fn active_viewport(&self) -> Option<(LayoutId, ViewportId)> {
        self.active_viewport
    }

    /// Say which layout viewport is being worked through (none: the Model
    /// tab or a layout's sheet, as `active_layout` says). While one is, the
    /// active layout is none, as new objects go to model space. Not an edit:
    /// neither undone nor saved.
    pub fn set_active_viewport(&mut self, viewport: Option<(LayoutId, ViewportId)>) {
        self.active_viewport = viewport;
    }

    /// The space whose coordinate system is in force: the viewport being
    /// worked through, or else the Model tab. A layout's sheet has none.
    pub fn active_ccs_space(&self) -> Option<CcsSpace> {
        match (self.active_viewport, self.active_layout) {
            (Some((l, v)), _) => Some(CcsSpace::Viewport(l, v)),
            (None, None) => Some(CcsSpace::Model),
            (None, Some(_)) => None,
        }
    }

    /// The coordinate system in force: the world on a layout's sheet.
    pub fn ccs(&self) -> Ccs {
        self.active_ccs_space()
            .and_then(|space| self.ccs_of(space))
            .unwrap_or(Ccs::WORLD)
    }

    /// The coordinate system of `space`; none for a viewport that is gone.
    pub fn ccs_of(&self, space: CcsSpace) -> Option<Ccs> {
        match space {
            CcsSpace::Model => Some(self.model_ccs),
            CcsSpace::Viewport(l, v) => Some(self.layout(l)?.viewport(v)?.ccs),
        }
    }

    /// Replace the coordinate system of `space`, returning the old one (none,
    /// and nothing changed, for a viewport that is gone).
    pub fn set_ccs_of(&mut self, space: CcsSpace, ccs: Ccs) -> Option<Ccs> {
        let ccs = ccs.sanitized();
        match space {
            CcsSpace::Model => Some(std::mem::replace(&mut self.model_ccs, ccs)),
            CcsSpace::Viewport(l, v) => {
                let layout = self.layouts.iter_mut().find(|x| x.id == l)?;
                let vp = layout.viewports.iter_mut().find(|x| x.id == v)?;
                Some(std::mem::replace(&mut vp.ccs, ccs))
            }
        }
    }

    /// The systems saved by name, in the order they were saved.
    pub fn named_ccs(&self) -> &[NamedCcs] {
        &self.named_ccs
    }

    /// The saved system called `name`, ignoring case.
    pub fn named_ccs_called(&self, name: &str) -> Option<&NamedCcs> {
        let name = name.trim();
        self.named_ccs
            .iter()
            .find(|n| n.name.eq_ignore_ascii_case(name))
    }

    /// Swap in the whole list of saved systems, returning the old one (what
    /// undo and the file readers use). The caller keeps names unique.
    pub fn set_named_ccs(&mut self, named: Vec<NamedCcs>) -> Vec<NamedCcs> {
        let named = named
            .into_iter()
            .map(|n| NamedCcs {
                ccs: n.ccs.sanitized(),
                ..n
            })
            .collect();
        std::mem::replace(&mut self.named_ccs, named)
    }

    /// The system `space` had before its current one, if any is kept.
    pub fn previous_ccs(&self, space: CcsSpace) -> Option<Ccs> {
        let mut kept = self.ccs_previous.iter().rev();
        kept.find(|(s, _)| *s == space).map(|(_, c)| *c)
    }

    /// Keep `ccs` as the latest earlier system of `space`, forgetting the
    /// oldest beyond [`PREVIOUS_KEPT`].
    pub fn push_previous_ccs(&mut self, space: CcsSpace, ccs: Ccs) {
        self.ccs_previous.push((space, ccs));
        let count = self
            .ccs_previous
            .iter()
            .filter(|(s, _)| *s == space)
            .count();
        if count > PREVIOUS_KEPT {
            if let Some(oldest) = self.ccs_previous.iter().position(|(s, _)| *s == space) {
                self.ccs_previous.remove(oldest);
            }
        }
    }

    /// Take back the latest earlier system of `space`.
    pub fn pop_previous_ccs(&mut self, space: CcsSpace) -> Option<Ccs> {
        let at = self.ccs_previous.iter().rposition(|(s, _)| *s == space)?;
        Some(self.ccs_previous.remove(at).1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point;

    fn turned(angle: f64) -> Ccs {
        Ccs::new(Point::new(1.0, 2.0), angle)
    }

    #[test]
    fn model_and_each_viewport_keep_their_own() {
        let mut doc = Document::default();
        let mut layout = doc.new_layout("Sheet", 297.0, 210.0);
        let vp = layout.viewports[0].id;
        let lid = layout.id;
        layout.viewports[0].ccs = turned(0.5);
        doc.set_layouts(vec![layout]);
        doc.set_ccs_of(CcsSpace::Model, turned(1.0));
        assert_eq!(doc.ccs(), turned(1.0), "the Model tab");
        doc.set_active_layout(Some(lid));
        assert_eq!(doc.ccs(), Ccs::WORLD, "a sheet has none");
        // Inside a viewport the active layout is none (objects go to model
        // space), and the viewport is told apart.
        doc.set_active_layout(None);
        doc.set_active_viewport(Some((lid, vp)));
        assert_eq!(doc.ccs(), turned(0.5), "the viewport's own");
        doc.set_ccs_of(CcsSpace::Viewport(lid, vp), turned(0.25));
        assert_eq!(doc.ccs_of(CcsSpace::Model), Some(turned(1.0)));
        assert_eq!(doc.ccs(), turned(0.25));
        doc.set_active_viewport(None);
        assert_eq!(doc.ccs(), turned(1.0));
        let gone = CcsSpace::Viewport(lid, ViewportId(999));
        assert_eq!(doc.set_ccs_of(gone, turned(2.0)), None);
    }

    #[test]
    fn previous_systems_are_kept_per_space_up_to_ten() {
        let mut doc = Document::default();
        for k in 0..12 {
            doc.push_previous_ccs(CcsSpace::Model, turned(f64::from(k) * 0.1));
        }
        let other = CcsSpace::Viewport(crate::LayoutId(1), ViewportId(1));
        doc.push_previous_ccs(other, turned(3.0));
        assert_eq!(doc.previous_ccs(other), Some(turned(3.0)));
        let mut back = 0;
        while doc.pop_previous_ccs(CcsSpace::Model).is_some() {
            back += 1;
        }
        assert_eq!(back, PREVIOUS_KEPT);
        assert_eq!(doc.previous_ccs(other), Some(turned(3.0)));
    }

    #[test]
    fn named_systems_are_found_ignoring_case() {
        let mut doc = Document::default();
        doc.set_named_ccs(vec![NamedCcs {
            name: "Wing A".into(),
            ccs: turned(0.3),
        }]);
        assert_eq!(
            doc.named_ccs_called(" wing a ").map(|n| n.ccs),
            Some(turned(0.3))
        );
        assert!(doc.named_ccs_called("Wing B").is_none());
    }
}
