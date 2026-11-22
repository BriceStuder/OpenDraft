//! Undoable changes of the custom coordinate systems: one space's system,
//! going back to its previous one, and the list saved by name.

use opendraft_core::{Ccs, CcsSpace, Document, NamedCcs};

use crate::{Action, Changes};

/// What a [`SetCcs`] does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    /// Put in this system, keeping the old one for Previous.
    To(Ccs),
    /// Go back to the space's previous system.
    Back,
}

/// Change the coordinate system of one space (the CCS command). The space
/// is fixed when the action is made, so undo and redo act on it wherever
/// the user is working by then.
pub struct SetCcs {
    space: CcsSpace,
    step: Step,
    /// What `apply` replaced and, for `Back`, the system it went back to.
    done: Option<(Ccs, Ccs)>,
}

impl SetCcs {
    /// Put `ccs` in force in `space`.
    pub fn new(space: CcsSpace, ccs: Ccs) -> Self {
        Self {
            space,
            step: Step::To(ccs.sanitized()),
            done: None,
        }
    }

    /// Go back to the previous system of `space` (nothing happens when it
    /// has none: check `Document::previous_ccs` first).
    pub fn previous(space: CcsSpace) -> Self {
        Self {
            space,
            step: Step::Back,
            done: None,
        }
    }
}

impl Action for SetCcs {
    fn name(&self) -> String {
        "Coordinate system".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        let space = self.space;
        self.done = match self.step {
            Step::To(ccs) => doc.set_ccs_of(space, ccs).map(|old| {
                doc.push_previous_ccs(space, old);
                (old, ccs)
            }),
            Step::Back => doc.pop_previous_ccs(space).and_then(|back| {
                let old = doc.set_ccs_of(space, back);
                if old.is_none() {
                    doc.push_previous_ccs(space, back);
                }
                old.map(|old| (old, back))
            }),
        };
    }

    fn revert(&mut self, doc: &mut Document) {
        let Some((old, new)) = self.done.take() else {
            return;
        };
        let space = self.space;
        doc.set_ccs_of(space, old);
        match self.step {
            Step::To(_) => {
                doc.pop_previous_ccs(space);
            }
            Step::Back => doc.push_previous_ccs(space, new),
        }
    }

    fn changes(&self) -> Changes {
        Changes::ccs()
    }
}

/// Replace the list of systems saved by name (Save and Delete).
pub struct SetNamedCcs {
    /// Applying and reverting both swap.
    named: Vec<NamedCcs>,
}

impl SetNamedCcs {
    pub fn new(named: Vec<NamedCcs>) -> Self {
        Self { named }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.named = doc.set_named_ccs(std::mem::take(&mut self.named));
    }
}

impl Action for SetNamedCcs {
    fn name(&self) -> String {
        "Named coordinate systems".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::ccs()
    }
}

#[cfg(test)]
mod tests {
    use opendraft_core::Point;

    use super::*;

    fn turned(angle: f64) -> Ccs {
        Ccs::new(Point::new(5.0, 0.0), angle)
    }

    #[test]
    fn a_change_keeps_the_old_system_for_previous_and_undo_forgets_it() {
        let mut doc = Document::default();
        let mut set = SetCcs::new(CcsSpace::Model, turned(0.5));
        set.apply(&mut doc);
        assert_eq!(doc.ccs(), turned(0.5));
        assert_eq!(doc.previous_ccs(CcsSpace::Model), Some(Ccs::WORLD));
        set.revert(&mut doc);
        assert_eq!(doc.ccs(), Ccs::WORLD);
        assert_eq!(doc.previous_ccs(CcsSpace::Model), None);
        set.apply(&mut doc);
        assert_eq!(doc.ccs(), turned(0.5), "redo");
    }

    #[test]
    fn previous_goes_back_and_undo_restores() {
        let mut doc = Document::default();
        SetCcs::new(CcsSpace::Model, turned(0.5)).apply(&mut doc);
        SetCcs::new(CcsSpace::Model, turned(1.0)).apply(&mut doc);
        let mut back = SetCcs::previous(CcsSpace::Model);
        back.apply(&mut doc);
        assert_eq!(doc.ccs(), turned(0.5));
        assert_eq!(doc.previous_ccs(CcsSpace::Model), Some(Ccs::WORLD));
        back.revert(&mut doc);
        assert_eq!(doc.ccs(), turned(1.0));
        assert_eq!(doc.previous_ccs(CcsSpace::Model), Some(turned(0.5)));
        back.apply(&mut doc);
        assert_eq!(doc.ccs(), turned(0.5), "redo");
    }

    #[test]
    fn previous_with_nothing_kept_changes_nothing() {
        let mut doc = Document::default();
        let mut back = SetCcs::previous(CcsSpace::Model);
        back.apply(&mut doc);
        back.revert(&mut doc);
        assert_eq!(doc.ccs(), Ccs::WORLD);
    }

    #[test]
    fn the_saved_list_swaps_both_ways() {
        let mut doc = Document::default();
        let saved = vec![NamedCcs {
            name: "A".into(),
            ccs: turned(0.2),
        }];
        let mut set = SetNamedCcs::new(saved.clone());
        set.apply(&mut doc);
        assert_eq!(doc.named_ccs(), saved.as_slice());
        set.revert(&mut doc);
        assert!(doc.named_ccs().is_empty());
        assert_eq!(set.changes(), Changes::ccs());
    }
}
