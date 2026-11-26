//! Bringing a definition into a table from outside (a `.lin` file): add it,
//! or replace the record of the same name as AutoCAD's Reload does.

use super::{Linetype, LinetypeError, LinetypeId, LinetypeTable, StrokeElement};

/// What loading one definition did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loaded {
    /// A new record.
    Added(LinetypeId),
    /// The record of that name took the new pattern and description, keeping
    /// its id, so every object that uses it follows.
    Replaced(LinetypeId),
    /// The record already had exactly this pattern and description.
    Unchanged(LinetypeId),
}

impl LinetypeTable {
    /// Whether loading `elements` under `name` would change the table: it is
    /// new, or the record of that name has another pattern.
    pub fn would_change(&self, name: &str, elements: &[StrokeElement]) -> bool {
        self.named(name).is_none_or(|r| r.elements != elements)
    }

    /// Load a definition under `name`. A record of that name (ignoring case)
    /// is replaced in place and keeps its id and its own spelling of the name;
    /// the three special records are refused.
    pub fn load_definition(
        &mut self,
        name: &str,
        description: &str,
        elements: Vec<StrokeElement>,
    ) -> Result<Loaded, LinetypeError> {
        let Some(existing) = self.named(name) else {
            return self.add(name, description, elements).map(Loaded::Added);
        };
        if existing.id.is_special() {
            return Err(LinetypeError::Special(existing.name.clone()));
        }
        if existing.elements == elements && existing.description == description {
            return Ok(Loaded::Unchanged(existing.id));
        }
        let id = existing.id;
        self.replace(Linetype {
            id,
            name: existing.name.clone(),
            description: description.to_owned(),
            elements,
            flags: existing.flags,
        })?;
        Ok(Loaded::Replaced(id))
    }
}
