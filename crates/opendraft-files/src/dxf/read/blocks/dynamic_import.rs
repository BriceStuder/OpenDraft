//! Dynamic blocks of a DXF built into the drawing, from what `dynamic`
//! decoded. A definition keeps its linear, polar, XY, point, rotation and flip parameters and its
//! stretch, move, scale, rotate and flip actions; what the app cannot represent (polar,
//! lookup, visibility, scale and the rest) is left out and named in a note. A reference to
//! an anonymous `*U` block becomes a reference to its definition with the
//! parameter values it recorded, but only when everything that moves geometry
//! in the definition is understood (a lookup table changes no geometry, so it
//! does not count): otherwise the anonymous block's picture is what the reference
//! keeps, as before, because that is the one thing AutoCAD certainly drew.
//!
//! The actions name objects by DXF handle; the importer cannot know which
//! objects survive until a block is read, so a [`Draft`] holds the handles and
//! is turned into a schema at the end of each block.

use std::collections::HashMap;
use std::f64::consts::TAU;

use opendraft_core::block::{
    BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, DynamicParameterKind,
    DynamicValue, ParameterId, VisibilityState,
};

use self::definition::{distance, read, upper, Draft, Kept, Step};
use super::super::dynamic::{decode, DynamicBlocks, NodeState};
use super::super::Record;

mod definition;

/// The values a modified reference records, and the block it is a reference to.
pub(super) struct Mapped {
    /// The lower-cased name of the definition.
    pub definition: String,
    pub values: Vec<(ParameterId, DynamicValue)>,
}

/// What the file's dynamic blocks come to, for the importer.
#[derive(Default)]
pub(super) struct Plan {
    drafts: HashMap<String, Draft>,
    mapped: HashMap<String, Mapped>,
    /// Names of definitions that come in with parameters and actions.
    imported: Vec<String>,
    /// Names of definitions that come in as their default picture only.
    pictures: Vec<String>,
    /// What was not understood, as "polar parameter" and the like.
    left_out: Vec<String>,
    /// Definitions of `imported` whose objects turned out not to be there.
    failed: Vec<String>,
}

#[cfg(test)]
mod kinds_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod visibility_tests;

impl Plan {
    /// What the file's records hold of dynamic blocks.
    pub(super) fn scan(records: &[Record]) -> Plan {
        Plan::from(&decode(records))
    }

    fn from(found: &DynamicBlocks) -> Plan {
        let mut plan = Plan::default();
        let mut whole: HashMap<String, (HashMap<u32, Kept>, bool)> = HashMap::new();
        for def in &found.definitions {
            let key = def.name.to_lowercase();
            let read = read(def);
            plan.left_out.extend(read.left_out.iter().cloned());
            match read.draft {
                Some(draft) => {
                    plan.imported.push(def.name.clone());
                    plan.drafts.insert(key.clone(), draft);
                    // A flip's state in a modified reference is not read, so a
                    // definition with one cannot rebuild its references.
                    let exact = read.left_out.iter().all(|k| k.starts_with("lookup "))
                        && !read.parameters.values().any(|k| k.unread);
                    whole.insert(key, (read.parameters, exact));
                }
                None => plan.pictures.push(def.name.clone()),
            }
        }
        plan.left_out.sort();
        plan.left_out.dedup();
        for instance in &found.instances {
            let key = instance.definition.to_lowercase();
            let Some((parameters, true)) = whole.get(&key) else {
                continue;
            };
            // A lookup's text changes no geometry; any other state must be
            // of a parameter that came in.
            let values: Option<Vec<(ParameterId, DynamicValue)>> = instance
                .states
                .iter()
                .filter(|(_, state)| !matches!(state, NodeState::Lookup(_)))
                .map(|(node, state)| match (parameters.get(node), state) {
                    (Some(kept), NodeState::Parameter { start, end }) => {
                        let value = match kept.zero_angle {
                            // A rotation is recorded as where its grip is.
                            Some(zero) => DynamicValue::Angle(
                                ((end.y - start.y).atan2(end.x - start.x) - zero).rem_euclid(TAU),
                            ),
                            // So is a polar parameter: its value is the end.
                            None if kept.polar => DynamicValue::Point(*end),
                            None => DynamicValue::Distance(distance(*start, *end)),
                        };
                        let number = match value {
                            DynamicValue::Angle(v) | DynamicValue::Distance(v) => v,
                            DynamicValue::Point(p) => p.x + p.y,
                            _ => f64::NAN,
                        };
                        number.is_finite().then_some((kept.id, value))
                    }
                    // A visibility shows the state it names, if it has it.
                    (Some(kept), NodeState::Visibility(name))
                        if kept.states.contains(&name.trim().to_lowercase()) =>
                    {
                        Some((kept.id, DynamicValue::Text(name.trim().to_owned())))
                    }
                    _ => None,
                })
                .collect();
            if let Some(values) = values.filter(|v| !v.is_empty()) {
                plan.mapped.insert(
                    instance.block.to_lowercase(),
                    Mapped {
                        definition: key,
                        values,
                    },
                );
            }
        }
        plan
    }

    /// Whether the visibility parameter of block `key` governs the object
    /// with this DXF handle.
    pub(super) fn governs(&self, key: &str, handle: &str) -> bool {
        self.drafts
            .get(key)
            .and_then(|d| d.visibility.as_ref())
            .is_some_and(|v| v.governed.contains(&upper(handle)))
    }

    /// The reference to the anonymous block `key`, as a reference to its
    /// definition.
    pub(super) fn instance(&self, key: &str) -> Option<&Mapped> {
        self.mapped.get(key)
    }

    /// The schema of block `key` (a definition's lower-cased name), its
    /// objects found among `handles`: the DXF handle of each object of the
    /// block, in the order the block's objects are listed.
    pub(super) fn schema(
        &mut self,
        key: &str,
        handles: &[Option<String>],
    ) -> Option<Box<DynamicBlock>> {
        let draft = self.drafts.get(key)?;
        let mut schema = DynamicBlock::new();
        schema.parameters = draft.parameters.clone();
        for a in &draft.actions {
            let mut objects: Vec<BlockObjectId> = handles
                .iter()
                .enumerate()
                .filter(|(_, h)| h.as_ref().is_some_and(|h| a.handles.contains(&upper(h))))
                .map(|(i, _)| BlockObjectId(i as u64))
                .collect();
            objects.sort();
            objects.dedup();
            if objects.is_empty() {
                continue;
            }
            schema.actions.push(DynamicAction {
                id: a.id,
                name: a.name.clone(),
                parameter: a.parameter,
                kind: match a.step {
                    Step::Stretch(frame) => DynamicActionKind::Stretch {
                        objects,
                        frame,
                        multiplier: a.multiplier,
                        angle_offset: a.angle_offset,
                    },
                    Step::Move => DynamicActionKind::Move {
                        objects,
                        multiplier: a.multiplier,
                        angle_offset: a.angle_offset,
                    },
                    Step::Rotate => DynamicActionKind::Rotate { objects },
                    Step::Flip => DynamicActionKind::Flip { objects },
                    Step::Scale => DynamicActionKind::Scale { objects },
                },
            });
        }
        if let Some(v) = &draft.visibility {
            let object_of = |handle: &String| {
                handles
                    .iter()
                    .position(|h| h.as_ref().is_some_and(|h| upper(h) == *handle))
                    .map(|i| BlockObjectId(i as u64))
            };
            // An object it governs and a state does not show is hidden there.
            let mut governed: Vec<BlockObjectId> =
                v.governed.iter().filter_map(object_of).collect();
            governed.sort();
            governed.dedup();
            let states: Vec<VisibilityState> = v
                .states
                .iter()
                .map(|(name, shown)| {
                    let shown: Vec<BlockObjectId> = shown.iter().filter_map(object_of).collect();
                    VisibilityState {
                        name: name.clone(),
                        hidden: governed
                            .iter()
                            .copied()
                            .filter(|o| !shown.contains(o))
                            .collect(),
                    }
                })
                .collect();
            for p in &mut schema.parameters {
                if p.id == v.parameter {
                    if let DynamicParameterKind::Visibility { states: slot, .. } = &mut p.kind {
                        *slot = states.clone();
                    }
                }
            }
        }
        schema.next_parameter_id = draft.parameters.len() as u64 + 1;
        schema.next_action_id = draft.actions.len() as u64 + 1;
        if schema.actions.is_empty() && draft.visibility.is_none() {
            self.failed.push(key.to_owned());
            return None;
        }
        Some(Box::new(schema))
    }

    /// What the import tells the user about the file's dynamic blocks; none
    /// when it has none.
    pub(super) fn warnings(&self) -> Vec<String> {
        let list = |names: &[String]| {
            let first: Vec<&str> = names.iter().take(5).map(String::as_str).collect();
            match names.len() - first.len() {
                0 => first.join(", "),
                more => format!("{} and {more} more", first.join(", ")),
            }
        };
        let mut out = Vec::new();
        let failed = |name: &String| self.failed.contains(&name.to_lowercase());
        let imported: Vec<String> = self
            .imported
            .iter()
            .filter(|n| !failed(n))
            .cloned()
            .collect();
        let pictures: Vec<String> = self
            .pictures
            .iter()
            .chain(self.imported.iter().filter(|n| failed(n)))
            .cloned()
            .collect();
        if !imported.is_empty() {
            let mut line = format!(
                "Imported {} with their linear, polar, XY, point, rotation and flip parameters and stretch, move, scale, rotate and flip actions: {}",
                crate::plural(imported.len() as u64, "dynamic block"),
                list(&imported)
            );
            if !self.mapped.is_empty() {
                line.push_str(&format!(
                    "; {} rebuilt from recorded values",
                    crate::plural(self.mapped.len() as u64, "modified reference")
                ));
            }
            line.push('.');
            out.push(line);
        }
        if !pictures.is_empty() {
            out.push(format!(
                "Kept the default picture of {}: {}. Their parameters and actions were not imported.",
                crate::plural(pictures.len() as u64, "dynamic block"),
                list(&pictures)
            ));
        }
        if !self.left_out.is_empty() {
            out.push(format!(
                "Left out of dynamic blocks (not supported yet): {}.",
                self.left_out.join(", ")
            ));
        }
        out
    }
}
