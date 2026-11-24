//! The structural check of a schema: ids, kinds that go together, and
//! selections.

use std::collections::HashSet;

use crate::block::{BlockObjectId, DynamicActionKind, DynamicBlock};

impl DynamicBlock {
    /// Why this schema could not exist in a block whose objects have ids
    /// `objects`, or `Ok`. Checks the structure: the version, parameter and
    /// action ids (nonzero, unique, already issued), that each action names
    /// a parameter, and that its selection is non-empty, sorted, without
    /// repeats and of objects the block has.
    pub fn check(&self, objects: &HashSet<BlockObjectId>) -> Result<(), String> {
        if self.version != Self::VERSION {
            return Err(format!("unknown schema version {}", self.version));
        }
        if self.parameters.len() > Self::MAX_PARAMETERS || self.actions.len() > Self::MAX_ACTIONS {
            return Err("it has too many parameters or actions".to_owned());
        }
        let mut parameters = HashSet::new();
        let mut work = 0usize;
        for p in &self.parameters {
            if p.id.0 == 0 || p.id.0 >= self.next_parameter_id || !parameters.insert(p.id) {
                return Err(format!(
                    "parameter id {} is zero, repeated or not issued",
                    p.id.0
                ));
            }
            if p.grips > 2 || p.name.is_empty() || p.is_degenerate() || !p.sets_valid() {
                return Err(format!(
                    "parameter {} has no name, too many grips, no length or an unusable value set",
                    p.id.0
                ));
            }
        }
        self.check_visibility(objects)?;
        self.check_lookups()?;
        let mut actions = HashSet::new();
        for a in &self.actions {
            if a.id.0 == 0 || a.id.0 >= self.next_action_id || !actions.insert(a.id) {
                return Err(format!(
                    "action id {} is zero, repeated or not issued",
                    a.id.0
                ));
            }
            let Some(p) = self.parameters.iter().find(|p| p.id == a.parameter) else {
                return Err(format!("action {} names a missing parameter", a.id.0));
            };
            let suits = a.kind.follows(p.kind.family());
            let (selected, usable) = match &a.kind {
                DynamicActionKind::Move {
                    objects,
                    multiplier,
                    angle_offset,
                } => (
                    objects,
                    suits
                        && multiplier.is_finite()
                        && *multiplier > 0.0
                        && angle_offset.is_finite(),
                ),
                DynamicActionKind::Stretch {
                    objects,
                    frame,
                    multiplier,
                    angle_offset,
                } => {
                    let (w, h) = (frame.max.x - frame.min.x, frame.max.y - frame.min.y);
                    (
                        objects,
                        suits
                            && multiplier.is_finite()
                            && *multiplier > 0.0
                            && angle_offset.is_finite()
                            && w.is_finite()
                            && h.is_finite()
                            && w > 0.0
                            && h > 0.0,
                    )
                }
                DynamicActionKind::Rotate { objects }
                | DynamicActionKind::Flip { objects }
                | DynamicActionKind::Scale { objects } => (objects, suits),
            };
            if !usable {
                return Err(format!(
                    "action {} does not suit its parameter, or has a multiplier or angle that is not usable, or an empty frame",
                    a.id.0
                ));
            }
            work = work.saturating_add(selected.len());
            if work > Self::MAX_ACTION_WORK {
                return Err("its actions select too many objects".to_owned());
            }
            if selected.is_empty() || selected.windows(2).any(|w| w[0] >= w[1]) {
                return Err(format!(
                    "action {} selects nothing, or its selection is unsorted or repeats",
                    a.id.0
                ));
            }
            if let Some(missing) = selected.iter().find(|o| !objects.contains(o)) {
                return Err(format!(
                    "action {} selects object {}, which the block lacks",
                    a.id.0, missing.0
                ));
            }
        }
        Ok(())
    }
}
