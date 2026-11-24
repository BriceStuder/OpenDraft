//! One decoded definition read for what the app keeps of it: linear and
//! rotation parameters, and the stretch, move and rotate actions that follow
//! them. Everything else is named for the import note.

use std::collections::{HashMap, HashSet};

use opendraft_core::block::{
    ActionId, DynamicParameter, DynamicParameterKind, DynamicValueSet, ParameterId,
};
use opendraft_core::{Bounds, Point};

use super::super::super::dynamic::{Action, DynamicDefinition, NodeKind};

/// What an action does, before its objects are known.
pub(super) enum Step {
    Move,
    Stretch(Bounds),
    Rotate,
}

pub(super) struct DraftAction {
    pub id: ActionId,
    pub name: String,
    pub parameter: ParameterId,
    /// Upper-cased handles of the objects it acts on.
    pub handles: HashSet<String>,
    pub step: Step,
    pub multiplier: f64,
    pub angle_offset: f64,
}

/// A definition's schema before its objects are known.
pub(super) struct Draft {
    pub parameters: Vec<DynamicParameter>,
    pub actions: Vec<DraftAction>,
}

/// A parameter that came in: its id, and for a rotation the direction its
/// angle is measured from (the file records a modified reference's angle as
/// the point its grip is at).
pub(super) struct Kept {
    pub id: ParameterId,
    pub zero_angle: Option<f64>,
}

/// A definition read for what the app keeps of it.
pub(super) struct Read {
    pub draft: Option<Draft>,
    pub left_out: Vec<String>,
    /// Parameters kept, by the file's node id.
    pub parameters: HashMap<u32, Kept>,
}

pub(super) fn upper(handle: &str) -> String {
    handle.trim().to_ascii_uppercase()
}

pub(super) fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

fn has_length(a: Point, b: Point) -> bool {
    distance(a, b).partial_cmp(&1e-9) == Some(std::cmp::Ordering::Greater)
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
        NodeKind::Rotation(p) if p.restricted => {
            Some("rotation parameter with a list or increment of angles".to_owned())
        }
        NodeKind::Rotation(p) if !has_length(p.start, p.end) => {
            Some("rotation parameter with no radius".to_owned())
        }
        NodeKind::Other(name) => named(name),
        _ => None,
    }
}

/// The corners of `points` as the rectangle that holds them.
fn bounding(points: &[Point]) -> Option<Bounds> {
    let first = *points.first()?;
    let (mut lo, mut hi) = (first, first);
    for p in points {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    (points.len() >= 2).then_some(Bounds { min: lo, max: hi })
}

pub(super) fn read(def: &DynamicDefinition) -> Read {
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
    let mut parameters: Vec<(u32, DynamicParameter)> = Vec::new();
    let mut kept: HashMap<u32, Kept> = HashMap::new();
    let mut names = HashSet::new();
    for node in &def.nodes {
        let (p, linear) = match &node.kind {
            NodeKind::Linear(p) => (p, true),
            NodeKind::Rotation(p) if !p.restricted => (p, false),
            _ => continue,
        };
        if !has_length(p.start, p.end) {
            if linear {
                left_out.push("linear parameter with no length".to_owned());
            }
            continue;
        }
        let id = ParameterId(parameters.len() as u64 + 1);
        let mut name = match p.name.trim() {
            "" if linear => format!("Distance{}", id.0),
            "" => format!("Angle{}", id.0),
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
        let value_set = match value_set.is_valid() && linear {
            true => value_set,
            false => DynamicValueSet::Any,
        };
        let grips = def
            .nodes
            .iter()
            .filter(|g| matches!(g.kind, NodeKind::Grip(_)) && linked(node.id, g.id))
            .count()
            .min(if linear { 2 } else { 1 }) as u8;
        let zero_angle = (!linear).then(|| (p.end.y - p.start.y).atan2(p.end.x - p.start.x));
        kept.insert(node.id, Kept { id, zero_angle });
        let kind = if linear {
            DynamicParameterKind::Linear {
                start: p.start,
                end: p.end,
                value_set,
            }
        } else {
            DynamicParameterKind::Rotation {
                base: p.start,
                radius: distance(p.start, p.end),
                value_set,
            }
        };
        parameters.push((
            node.id,
            DynamicParameter {
                id,
                name: name.clone(),
                label: name,
                description: String::new(),
                show: true,
                grips,
                kind,
            },
        ));
    }
    let mut actions: Vec<DraftAction> = Vec::new();
    let mut action_names = HashSet::new();
    for node in &def.nodes {
        let (action, word): (&Action, &str) = match &node.kind {
            NodeKind::Stretch(a) => (a, "Stretch"),
            NodeKind::Move(a) => (a, "Move"),
            NodeKind::Rotate(a) => (a, "Rotate"),
            _ => continue,
        };
        let angular = word == "Rotate";
        let followed = parameters
            .iter()
            .find(|(id, p)| linked(*id, node.id) && p.is_angle() == angular);
        let step = match word {
            "Stretch" => bounding(&action.frame).map(Step::Stretch),
            "Move" => Some(Step::Move),
            // It turns about its parameter's base point; one about a point
            // of its own is a different action.
            _ => followed
                .and_then(|(_, p)| match &p.kind {
                    DynamicParameterKind::Rotation { base, .. } => Some(*base),
                    DynamicParameterKind::Linear { .. } => None,
                })
                .filter(|base| action.frame.first().is_some_and(|b| !has_length(*b, *base)))
                .map(|_| Step::Rotate),
        };
        let (Some((_, parameter)), Some(step)) = (followed, step) else {
            let why = match (followed.is_some(), angular) {
                (true, true) => "about a point of its own",
                _ => "with nothing to follow",
            };
            left_out.push(format!("{} action {why}", word.to_lowercase()));
            continue;
        };
        let id = ActionId(actions.len() as u64 + 1);
        let mut name = match node.label.trim() {
            "" => word.to_owned(),
            n => n.to_owned(),
        };
        if !action_names.insert(name.to_lowercase()) {
            name = format!("{name}{}", id.0);
            action_names.insert(name.to_lowercase());
        }
        actions.push(DraftAction {
            id,
            name,
            parameter: parameter.id,
            handles: action.selection.iter().map(|h| upper(h)).collect(),
            step,
            multiplier: action
                .multiplier
                .filter(|m| m.is_finite() && *m > 0.0)
                .unwrap_or(1.0),
            angle_offset: action.angle_offset.filter(|a| a.is_finite()).unwrap_or(0.0),
        });
    }
    // A parameter nothing follows changes no picture.
    let used: HashSet<ParameterId> = actions.iter().map(|a| a.parameter).collect();
    kept.retain(|_, k| used.contains(&k.id));
    parameters.retain(|(_, p)| used.contains(&p.id));
    left_out.sort();
    left_out.dedup();
    Read {
        draft: (!actions.is_empty()).then(|| Draft {
            parameters: parameters.into_iter().map(|(_, p)| p).collect(),
            actions,
        }),
        left_out,
        parameters: kept,
    }
}
