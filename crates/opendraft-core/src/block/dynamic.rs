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
    /// A point `end` at a distance and an angle from `base`. Its value is
    /// where that point is (a `Point`, `end` by default); each of the
    /// distance and the angle (radians, from the block's +X) has a value set.
    Polar {
        base: Point,
        end: Point,
        distance_set: DynamicValueSet,
        angle_set: DynamicValueSet,
        /// What the Properties palette calls the angle.
        angle_label: String,
    },
    /// A point `end` at a horizontal and a vertical distance from `base`.
    /// Its value is where `end` is (a `Point`); each distance has a value
    /// set.
    Xy {
        base: Point,
        end: Point,
        x_set: DynamicValueSet,
        y_set: DynamicValueSet,
        /// What the Properties palette calls the vertical distance.
        y_label: String,
    },
    /// A location in the block. Its value is where it is (a `Point`),
    /// `position` by default; it has no value set.
    Point { position: Point },
    /// Which objects show. It has named states, each hiding some objects;
    /// its value is the name of the state shown (`Text`), the first by
    /// default. It applies to the whole block and no action follows it.
    /// `position` is where its grip would be.
    Visibility {
        position: Point,
        states: Vec<VisibilityState>,
    },
    /// A choice among the rows of a table. Picking a row sets each input
    /// parameter to the row's values; what an insert shows is the row whose
    /// values the inputs have now (a reverse lookup), or "Custom" when none
    /// has. Nothing is stored for it in an insert, and no action follows it.
    /// The inputs are linear, rotation or flip parameters of the block.
    Lookup {
        position: Point,
        inputs: Vec<ParameterId>,
        rows: Vec<LookupRow>,
    },
}

/// One row of a lookup table: a label and the value each input parameter
/// takes in it, in the parameter's own units (radians for an angle).
#[derive(Clone, Debug, PartialEq)]
pub struct LookupRow {
    /// Unique in the table, ignoring case; not empty.
    pub label: String,
    /// One value for each of the table's inputs, in order.
    pub values: Vec<f64>,
}

/// One named state of a visibility parameter: the objects it hides. An
/// object it does not list is shown.
#[derive(Clone, Debug, PartialEq)]
pub struct VisibilityState {
    /// Unique in the parameter, ignoring case; not empty.
    pub name: String,
    /// Sorted, without repeats.
    pub hidden: Vec<BlockObjectId>,
}

/// Which parameters and actions go together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicFamily {
    Linear,
    Rotation,
    Flip,
    Polar,
    Xy,
    Point,
    Visibility,
    Lookup,
}

impl DynamicParameterKind {
    pub fn family(&self) -> DynamicFamily {
        match self {
            Self::Linear { .. } => DynamicFamily::Linear,
            Self::Rotation { .. } => DynamicFamily::Rotation,
            Self::Flip { .. } => DynamicFamily::Flip,
            Self::Polar { .. } => DynamicFamily::Polar,
            Self::Xy { .. } => DynamicFamily::Xy,
            Self::Point { .. } => DynamicFamily::Point,
            Self::Visibility { .. } => DynamicFamily::Visibility,
            Self::Lookup { .. } => DynamicFamily::Lookup,
        }
    }
}

impl DynamicActionKind {
    /// Whether this action can follow a parameter of `family`: a move or
    /// stretch follows a distance or any point, a scale a distance or a
    /// polar point, a rotate an angle or a polar point, a flip a flip, and nothing a visibility.
    pub fn follows(&self, family: DynamicFamily) -> bool {
        use DynamicFamily::{Flip, Linear, Point, Polar, Rotation, Xy};
        match self {
            Self::Move { .. } | Self::Stretch { .. } => {
                matches!(family, Linear | Polar | Xy | Point)
            }
            Self::Scale { .. } => matches!(family, Linear | Polar),
            Self::Rotate { .. } => matches!(family, Rotation | Polar),
            Self::Flip { .. } => family == Flip,
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

/// A parameter's value. `Distance`, `Angle` (radians), `Bool` (a flip) and
/// `Text` (a visibility state's name) are used so far; the rest are the types later parameters will need.
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
