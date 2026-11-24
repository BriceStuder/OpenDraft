//! Dynamic blocks: a block definition that carries parameters and actions,
//! and the values one insert of it overrides.
//!
//! These are the data types only. Nothing evaluates a schema or reads a
//! state yet: a [`BlockDef`](super::BlockDef) has `dynamic: None`, an
//! [`Insert`](super::Insert) has `dynamic: None`, and every picture, bound,
//! grip and file is what it was. The types exist so the places that build
//! blocks and inserts (importers, the Block Editor, paste) are made to carry
//! them before any behaviour depends on them.
//!
//! The schema ([`DynamicBlock`]) belongs to the definition and is shared by
//! every insert. An insert stores only what it overrides
//! ([`DynamicInstanceState`]); a parameter it does not name has the
//! definition's default. Evaluated geometry is derived and is never stored
//! here.

use std::collections::HashSet;

use crate::{Bounds, Point};

/// Stable identity of one object inside one block definition. It is not a
/// drawing-wide `EntityId`: two blocks may both have an object 0. An action's
/// selection names objects by it, so a schema never refers to a list
/// position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockObjectId(pub u64);

/// Identity of a parameter inside one definition: monotonic, never reused
/// while the definition exists, and never zero (zero means "none").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParameterId(pub u64);

/// Identity of an action inside one definition, with the same rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId(pub u64);

/// What makes a block dynamic: its parameters and the actions they drive.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicBlock {
    /// Schema version, 1 for the first. A reader refuses one it does not know.
    pub version: u16,
    pub parameters: Vec<DynamicParameter>,
    pub actions: Vec<DynamicAction>,
    pub next_parameter_id: u64,
    pub next_action_id: u64,
}

impl DynamicBlock {
    /// The schema version written today.
    pub const VERSION: u16 = 1;
    /// Most parameters, and most actions, a block may have.
    pub const MAX_PARAMETERS: usize = 256;
    pub const MAX_ACTIONS: usize = 1024;
    /// Most objects all the actions together may select: the work of one
    /// evaluation, so a hostile schema cannot make drawing it slow.
    pub const MAX_ACTION_WORK: usize = 100_000;

    /// A schema with no parameters or actions. Ids start at 1, so zero is
    /// never a valid parameter or action id.
    pub fn new() -> Self {
        Self {
            version: Self::VERSION,
            parameters: Vec::new(),
            actions: Vec::new(),
            next_parameter_id: 1,
            next_action_id: 1,
        }
    }
}

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
            if p.grips > 2 || p.name.is_empty() || p.is_degenerate() || !p.value_set().is_valid() {
                return Err(format!(
                    "parameter {} has no name, too many grips, no length or an unusable value set",
                    p.id.0
                ));
            }
        }
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
            let suits = p.kind.family() == a.kind.family();
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

impl Default for DynamicBlock {
    fn default() -> Self {
        Self::new()
    }
}

/// A value an insert can change, with how it is named and shown.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicParameter {
    pub id: ParameterId,
    /// Unique in the block; an identifier such as `Distance1`.
    pub name: String,
    /// What the Properties palette calls it, such as `Distance`.
    pub label: String,
    pub description: String,
    /// Whether the Properties palette lists it.
    pub show: bool,
    /// How many grips it shows on an insert: 0, 1 or 2.
    pub grips: u8,
    pub kind: DynamicParameterKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DynamicParameterKind {
    /// A distance between two points of the block; its default is their
    /// distance apart.
    Linear {
        start: Point,
        end: Point,
        value_set: DynamicValueSet,
    },
    /// An angle about a point of the block. Its value is the turn from the
    /// block as drawn, in radians, so the default is 0; the grip sits
    /// `radius` from `base`, along the turned direction.
    Rotation {
        base: Point,
        radius: f64,
        /// In radians.
        value_set: DynamicValueSet,
    },
    /// A mirror line from `base` to `end`; the value is whether the block is
    /// flipped about it (a `Bool`, not flipped by default).
    Flip { base: Point, end: Point },
}

/// Which parameters and actions go together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicFamily {
    Linear,
    Rotation,
    Flip,
}

impl DynamicParameterKind {
    pub fn family(&self) -> DynamicFamily {
        match self {
            Self::Linear { .. } => DynamicFamily::Linear,
            Self::Rotation { .. } => DynamicFamily::Rotation,
            Self::Flip { .. } => DynamicFamily::Flip,
        }
    }
}

impl DynamicActionKind {
    /// The kind of parameter this action follows.
    pub fn family(&self) -> DynamicFamily {
        match self {
            Self::Move { .. } | Self::Stretch { .. } | Self::Scale { .. } => DynamicFamily::Linear,
            Self::Rotate { .. } => DynamicFamily::Rotation,
            Self::Flip { .. } => DynamicFamily::Flip,
        }
    }
}

/// The values a parameter may take.
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicValueSet {
    Any,
    /// Sorted, unique, finite, in the block's units.
    List(Vec<f64>),
    /// `min <= max`; the increment, when there is one, is positive.
    Range {
        min: f64,
        max: f64,
        increment: Option<f64>,
    },
}

/// What a parameter does to objects of the block when its value changes.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicAction {
    pub id: ActionId,
    /// Unique in the block; an identifier such as `Stretch1`.
    pub name: String,
    pub parameter: ParameterId,
    pub kind: DynamicActionKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DynamicActionKind {
    Move {
        objects: Vec<BlockObjectId>,
        /// Finite and positive; 1 moves by the parameter's change.
        multiplier: f64,
        /// Radians.
        angle_offset: f64,
    },
    Stretch {
        objects: Vec<BlockObjectId>,
        /// The part of the block that stretches, in the block's coordinates.
        frame: Bounds,
        multiplier: f64,
        angle_offset: f64,
    },
    /// Turns the objects about its rotation parameter's base point by the
    /// parameter's value.
    Rotate { objects: Vec<BlockObjectId> },
    /// Mirrors the objects about its flip parameter's line when flipped.
    Flip { objects: Vec<BlockObjectId> },
    /// Enlarges the objects about the start of its linear parameter by the
    /// parameter's value over its value as drawn.
    Scale { objects: Vec<BlockObjectId> },
}

/// The values one insert overrides. A parameter it does not list has the
/// definition's default, so an untouched insert has no state at all.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DynamicInstanceState {
    /// Sorted by parameter id, one entry per parameter.
    pub values: Vec<DynamicValueEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicValueEntry {
    pub parameter: ParameterId,
    pub value: DynamicValue,
}

/// A parameter's value. `Distance`, `Angle` (radians) and `Bool` (a flip) are
/// used so far; the rest are the types later parameters will need.
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicValue {
    Distance(f64),
    Angle(f64),
    Point(Point),
    Integer(i64),
    Text(String),
    Bool(bool),
}

impl DynamicInstanceState {
    /// The overriding value of `parameter`, if there is one.
    pub fn get(&self, parameter: ParameterId) -> Option<&DynamicValue> {
        self.values
            .binary_search_by_key(&parameter, |e| e.parameter)
            .ok()
            .map(|i| &self.values[i].value)
    }

    /// Override `parameter`, replacing an earlier value and keeping the list
    /// sorted. (Dropping a value equal to the default needs the schema.)
    pub fn set(&mut self, parameter: ParameterId, value: DynamicValue) {
        match self
            .values
            .binary_search_by_key(&parameter, |e| e.parameter)
        {
            Ok(i) => self.values[i].value = value,
            Err(i) => self
                .values
                .insert(i, DynamicValueEntry { parameter, value }),
        }
    }

    /// Whether every value is finite and the parameter ids are strictly
    /// increasing (sorted, one entry each).
    pub fn is_well_formed(&self) -> bool {
        let finite = |v: &DynamicValue| match v {
            DynamicValue::Distance(x) | DynamicValue::Angle(x) => x.is_finite(),
            DynamicValue::Point(p) => p.x.is_finite() && p.y.is_finite(),
            DynamicValue::Integer(_) | DynamicValue::Text(_) | DynamicValue::Bool(_) => true,
        };
        self.values.iter().all(|e| finite(&e.value))
            && self
                .values
                .windows(2)
                .all(|w| w[0].parameter < w[1].parameter)
    }

    /// Whether nothing is overridden, which is the same as no state.
    pub fn is_default(&self) -> bool {
        self.values.is_empty()
    }
}

/// A grip a parameter shows on an insert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DynamicGrip {
    pub parameter: ParameterId,
    /// 0 is the end grip (a rotation's only grip). 1 is the start grip,
    /// reserved for the linear parameter's Startpoint/Midpoint base
    /// behaviour.
    pub index: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_schema_starts_ids_at_one() {
        let schema = DynamicBlock::new();
        assert_eq!(schema.version, DynamicBlock::VERSION);
        assert_eq!((schema.next_parameter_id, schema.next_action_id), (1, 1));
        assert!(schema.parameters.is_empty() && schema.actions.is_empty());
        assert_eq!(schema, DynamicBlock::default());
    }

    #[test]
    fn a_state_keeps_its_values_sorted_and_one_per_parameter() {
        let mut state = DynamicInstanceState::default();
        assert!(state.is_default());
        state.set(ParameterId(3), DynamicValue::Distance(1.0));
        state.set(ParameterId(1), DynamicValue::Angle(0.5));
        state.set(ParameterId(3), DynamicValue::Distance(2.0));
        let ids: Vec<u64> = state.values.iter().map(|e| e.parameter.0).collect();
        assert_eq!(ids, [1, 3]);
        assert_eq!(
            state.get(ParameterId(3)),
            Some(&DynamicValue::Distance(2.0))
        );
        assert_eq!(state.get(ParameterId(2)), None);
        assert!(!state.is_default());
    }
}
