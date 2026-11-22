//! What an `ask_*` call gives back to the command.

use opendraft_core::{EntityId, Point};

/// The answer to `Editor::ask_object`: the object that was clicked, the point
/// it was clicked at, and whether Shift was held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picked {
    pub id: EntityId,
    pub at: Point,
    pub shift: bool,
}

/// The answer to `Editor::ask_object_or_sweep`: an object clicked, a click
/// in empty space, or a path dragged from there; each says whether Shift
/// was held.
#[derive(Clone, Debug, PartialEq)]
pub enum Swept {
    Object(Picked),
    Missed { at: Point, shift: bool },
    Path { points: Vec<Point>, shift: bool },
}

/// The answer to `Editor::ask_angle_or_point` and `ask_distance_or_point`: a
/// typed number (radians for an angle), or a point that was picked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumberOrPoint {
    Number(f64),
    Point(Point),
}

/// An answer to an `ask_*` call: a value, a keyword, or a bare Enter.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer<T> {
    Value(T),
    /// The label of the keyword chosen.
    Keyword(String),
    Enter,
}
