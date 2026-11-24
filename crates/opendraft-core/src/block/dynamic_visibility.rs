//! A visibility parameter: the named states of a block, which objects each
//! hides, the state an insert shows, and the checks of a schema that has one.

use std::collections::HashSet;

use crate::block::{
    BlockObjectId, DynamicBlock, DynamicInstanceState, DynamicParameter, DynamicParameterKind,
    DynamicValue, Insert, ParameterId, VisibilityState,
};

use super::dynamic_eval::DynamicBlockError;

impl DynamicParameter {
    /// The value to store for the state called `name`: `None` for the first
    /// (the default) or a name no state has.
    pub(super) fn visibility_value(&self, name: &str) -> Option<DynamicValue> {
        let found = self.visibility_state_named(name)?;
        let first = self.visibility_states()?.first()?;
        (found != first).then(|| DynamicValue::Text(found.name.clone()))
    }

    /// The states of a visibility parameter.
    pub fn visibility_states(&self) -> Option<&[VisibilityState]> {
        match &self.kind {
            DynamicParameterKind::Visibility { states, .. } => Some(states),
            _ => None,
        }
    }

    /// Whether this is a visibility parameter.
    pub fn is_visibility(&self) -> bool {
        matches!(self.kind, DynamicParameterKind::Visibility { .. })
    }

    /// The state an insert with `state` shows: the one its value names, or the
    /// first when it names none.
    pub(super) fn shown_state(
        &self,
        state: Option<&DynamicInstanceState>,
    ) -> Result<&VisibilityState, DynamicBlockError> {
        let states = self
            .visibility_states()
            .ok_or(DynamicBlockError::BadValue(self.id))?;
        let first = states
            .first()
            .ok_or(DynamicBlockError::Degenerate(self.id))?;
        match state.and_then(|s| s.get(self.id)) {
            None => Ok(first),
            Some(DynamicValue::Text(name)) => states
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(name))
                .ok_or(DynamicBlockError::BadValue(self.id)),
            Some(_) => Err(DynamicBlockError::BadValue(self.id)),
        }
    }

    /// The state named `name` (ignoring case), as the name this parameter
    /// spells it.
    pub fn visibility_state_named(&self, name: &str) -> Option<&VisibilityState> {
        self.visibility_states()?
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name.trim()))
    }
}

impl DynamicBlock {
    /// The visibility parameter, if the block has one.
    pub fn visibility(&self) -> Option<&DynamicParameter> {
        self.parameters.iter().find(|p| p.is_visibility())
    }

    /// The objects hidden in an insert with `state`.
    pub(super) fn hidden_objects(
        &self,
        state: Option<&DynamicInstanceState>,
    ) -> Result<HashSet<BlockObjectId>, DynamicBlockError> {
        let Some(p) = self.visibility() else {
            return Ok(HashSet::new());
        };
        Ok(p.shown_state(state)?.hidden.iter().copied().collect())
    }

    /// Why the visibility parameter could not be in a block whose objects
    /// have ids `objects`, or `Ok`: one at most, with at least one state,
    /// names that are not empty and not repeated, and hidden objects sorted
    /// and in the block.
    pub(super) fn check_visibility(&self, objects: &HashSet<BlockObjectId>) -> Result<(), String> {
        let mut all = self.parameters.iter().filter(|p| p.is_visibility());
        let (Some(p), None) = (all.next(), all.next()) else {
            return if self.visibility().is_none() {
                Ok(())
            } else {
                Err("it has more than one visibility parameter".to_owned())
            };
        };
        let states = p.visibility_states().unwrap_or(&[]);
        if states.is_empty() || states.len() > Self::MAX_VISIBILITY_STATES {
            return Err(format!("visibility parameter {} has no states", p.id.0));
        }
        let mut names = HashSet::new();
        for s in states {
            if s.name.trim().is_empty() || !names.insert(s.name.to_ascii_lowercase()) {
                return Err(format!(
                    "visibility parameter {} has a state with no name or a name that repeats",
                    p.id.0
                ));
            }
            if s.hidden.windows(2).any(|w| w[0] >= w[1]) {
                return Err(format!(
                    "state \"{}\" lists its hidden objects unsorted or repeated",
                    s.name
                ));
            }
            if let Some(o) = s.hidden.iter().find(|o| !objects.contains(o)) {
                return Err(format!(
                    "state \"{}\" hides object {}, which the block lacks",
                    s.name, o.0
                ));
            }
        }
        Ok(())
    }

    /// Most states a visibility parameter may have.
    pub const MAX_VISIBILITY_STATES: usize = 256;
}

impl Insert {
    /// The name of the visibility state this insert shows, if its block has a
    /// visibility parameter.
    pub fn visibility_state(&self) -> Option<(ParameterId, &str)> {
        let p = self.block.dynamic.as_ref()?.visibility()?;
        let shown = p.shown_state(self.dynamic.as_deref()).ok()?;
        Some((p.id, shown.name.as_str()))
    }

    /// This insert showing the state called `name`, or `None` when the block
    /// has no visibility parameter or no such state. The first state is the
    /// default and is not stored.
    pub fn with_visibility_state(&self, name: &str) -> Option<Insert> {
        let p = self.block.dynamic.as_ref()?.visibility()?;
        p.visibility_state_named(name)?;
        let value = p.visibility_value(name);
        let mut state = self.dynamic.as_deref().cloned().unwrap_or_default();
        match value {
            Some(v) => state.set(p.id, v),
            None => state.values.retain(|e| e.parameter != p.id),
        }
        Some(Insert {
            dynamic: (!state.is_default()).then(|| Box::new(state)),
            ..self.clone()
        })
    }
}
