//! Dynamic blocks of a DXF built into the drawing, from what `dynamic`
//! decoded. A definition keeps its linear parameters and its stretch and move
//! actions; what the app cannot represent (polar, lookup, visibility, rotation,
//! flip, scale and the rest) is left out and named in a note. A reference to
//! an anonymous `*U` block becomes a reference to its definition with the
//! parameter values it recorded, but only when everything that moves geometry
//! in the definition is understood (a lookup table changes no geometry, so it
//! does not count): otherwise the anonymous block's picture is what the reference
//! keeps, as before, because that is the one thing AutoCAD certainly drew.
//!
//! The actions name objects by DXF handle; the importer cannot know which
//! objects survive until a block is read, so a [`Draft`] holds the handles and
//! is turned into a schema at the end of each block.

use std::collections::{HashMap, HashSet};

use opendraft_core::block::{
    ActionId, BlockObjectId, DynamicAction, DynamicActionKind, DynamicBlock, DynamicParameter,
    DynamicParameterKind, DynamicValueSet, ParameterId,
};
use opendraft_core::{Bounds, Point};

use super::super::dynamic::{decode, DynamicBlocks, DynamicDefinition, NodeKind, NodeState};
use super::super::Record;

/// The values a modified reference records, and the block it is a reference to.
pub(super) struct Mapped {
    /// The lower-cased name of the definition.
    pub definition: String,
    pub values: Vec<(ParameterId, f64)>,
}

struct DraftAction {
    id: ActionId,
    name: String,
    parameter: ParameterId,
    /// Upper-cased handles of the objects it acts on.
    handles: HashSet<String>,
    /// `Some` for a stretch.
    frame: Option<Bounds>,
    multiplier: f64,
    angle_offset: f64,
}

/// A definition's schema before its objects are known.
struct Draft {
    parameters: Vec<DynamicParameter>,
    actions: Vec<DraftAction>,
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

fn upper(handle: &str) -> String {
    handle.trim().to_ascii_uppercase()
}

fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

/// The kind of an object the app cannot represent, if it is a parameter or
/// an action.
fn unsupported(kind: &NodeKind) -> Option<String> {
    let named = |n: &str| {
        let n = n.strip_prefix("BLOCK").unwrap_or(n);
        for (end, word) in [("PARAMETER", "parameter"), ("ACTION", "action")] {
            if let Some(head) = n.strip_suffix(end) {
                return Some(format!("{} {word}", head.to_lowercase()));
            }
        }
        None
    };
    match kind {
        NodeKind::Polar(_) => Some("polar parameter".to_owned()),
        NodeKind::Lookup { .. } => Some("lookup parameter".to_owned()),
        NodeKind::PolarStretch(_) => Some("polar stretch action".to_owned()),
        NodeKind::Scale(_) => Some("scale action".to_owned()),
        NodeKind::LookupTable(_) => Some("lookup action".to_owned()),
        NodeKind::Other(name) => named(name),
        _ => None,
    }
}

/// A definition read for what the app keeps of it.
struct Read {
    draft: Option<Draft>,
    left_out: Vec<String>,
    /// Parameters kept, by the file's node id.
    parameters: HashMap<u32, ParameterId>,
}

fn read(def: &DynamicDefinition) -> Read {
    let mut left_out: Vec<String> = def
        .nodes
        .iter()
        .filter_map(|n| unsupported(&n.kind))
        .collect();
    let linked = |a: u32, b: u32| {
        def.links
            .iter()
            .any(|&(f, t)| (f, t) == (a, b) || (f, t) == (b, a))
    };
    let mut parameters = Vec::new();
    let mut ids = HashMap::new();
    let mut names = HashSet::new();
    for node in &def.nodes {
        let NodeKind::Linear(p) = &node.kind else {
            continue;
        };
        if distance(p.start, p.end).partial_cmp(&1e-9) != Some(std::cmp::Ordering::Greater) {
            left_out.push("linear parameter with no length".to_owned());
            continue;
        }
        let id = ParameterId(parameters.len() as u64 + 1);
        let mut name = match p.name.trim() {
            "" => format!("Distance{}", id.0),
            n => n.to_owned(),
        };
        if !names.insert(name.to_lowercase()) {
            name = format!("{name}{}", id.0);
            names.insert(name.to_lowercase());
        }
        let mut allowed = p.values.clone();
        allowed.sort_by(f64::total_cmp);
        allowed.dedup();
        let value_set = DynamicValueSet::List(allowed);
        let value_set = match value_set.is_valid() {
            true => value_set,
            false => DynamicValueSet::Any,
        };
        let grips = def
            .nodes
            .iter()
            .filter(|g| matches!(g.kind, NodeKind::Grip(_)) && linked(node.id, g.id))
            .count()
            .min(2) as u8;
        ids.insert(node.id, id);
        parameters.push((
            node.id,
            DynamicParameter {
                id,
                name: name.clone(),
                label: name,
                description: String::new(),
                show: true,
                grips,
                kind: DynamicParameterKind::Linear {
                    start: p.start,
                    end: p.end,
                    value_set,
                },
            },
        ));
    }
    let mut actions: Vec<DraftAction> = Vec::new();
    let mut action_names = HashSet::new();
    for node in &def.nodes {
        let (action, stretch) = match &node.kind {
            NodeKind::Stretch(a) => (a, true),
            NodeKind::Move(a) => (a, false),
            _ => continue,
        };
        let parameter = parameters
            .iter()
            .find(|(id, _)| linked(*id, node.id))
            .map(|(_, p)| p.id);
        let frame = (stretch && action.frame.len() >= 2).then(|| {
            let (mut lo, mut hi) = (action.frame[0], action.frame[0]);
            for p in &action.frame {
                lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
            }
            Bounds { min: lo, max: hi }
        });
        let (Some(parameter), true) = (parameter, !stretch || frame.is_some()) else {
            left_out.push(format!(
                "{} action with nothing to follow",
                if stretch { "stretch" } else { "move" }
            ));
            continue;
        };
        let id = ActionId(actions.len() as u64 + 1);
        let mut name = match node.label.trim() {
            "" if stretch => "Stretch".to_owned(),
            "" => "Move".to_owned(),
            n => n.to_owned(),
        };
        if !action_names.insert(name.to_lowercase()) {
            name = format!("{name}{}", id.0);
            action_names.insert(name.to_lowercase());
        }
        actions.push(DraftAction {
            id,
            name,
            parameter,
            handles: action.selection.iter().map(|h| upper(h)).collect(),
            frame,
            multiplier: action
                .multiplier
                .filter(|m| m.is_finite() && *m > 0.0)
                .unwrap_or(1.0),
            angle_offset: action.angle_offset.filter(|a| a.is_finite()).unwrap_or(0.0),
        });
    }
    // A parameter nothing follows changes no picture.
    let used: HashSet<ParameterId> = actions.iter().map(|a| a.parameter).collect();
    for (node, p) in &parameters {
        if !used.contains(&p.id) {
            ids.remove(node);
        }
    }
    parameters.retain(|(_, p)| used.contains(&p.id));
    left_out.sort();
    left_out.dedup();
    Read {
        draft: (!actions.is_empty()).then(|| Draft {
            parameters: parameters.into_iter().map(|(_, p)| p).collect(),
            actions,
        }),
        left_out,
        parameters: ids,
    }
}

#[cfg(test)]
mod tests;

impl Plan {
    /// What the file's records hold of dynamic blocks.
    pub(super) fn scan(records: &[Record]) -> Plan {
        Plan::from(&decode(records))
    }

    fn from(found: &DynamicBlocks) -> Plan {
        let mut plan = Plan::default();
        let mut whole: HashMap<String, (HashMap<u32, ParameterId>, bool)> = HashMap::new();
        for def in &found.definitions {
            let key = def.name.to_lowercase();
            let read = read(def);
            plan.left_out.extend(read.left_out.iter().cloned());
            match read.draft {
                Some(draft) => {
                    plan.imported.push(def.name.clone());
                    plan.drafts.insert(key.clone(), draft);
                    let exact = read.left_out.iter().all(|k| k.starts_with("lookup "));
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
            let values: Option<Vec<(ParameterId, f64)>> = instance
                .states
                .iter()
                .filter(|(_, state)| !matches!(state, NodeState::Lookup(_)))
                .map(|(node, state)| match (parameters.get(node), state) {
                    (Some(id), NodeState::Parameter { start, end }) => {
                        Some((*id, distance(*start, *end))).filter(|(_, d)| d.is_finite())
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
                kind: match a.frame {
                    Some(frame) => DynamicActionKind::Stretch {
                        objects,
                        frame,
                        multiplier: a.multiplier,
                        angle_offset: a.angle_offset,
                    },
                    None => DynamicActionKind::Move {
                        objects,
                        multiplier: a.multiplier,
                        angle_offset: a.angle_offset,
                    },
                },
            });
        }
        schema.next_parameter_id = draft.parameters.len() as u64 + 1;
        schema.next_action_id = draft.actions.len() as u64 + 1;
        if schema.actions.is_empty() {
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
                "Imported {} with their linear parameters and stretch and move actions: {}",
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
