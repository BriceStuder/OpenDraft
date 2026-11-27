//! Associative arrays: what the drawing remembers of an array so that it can
//! be edited as a whole.
//!
//! The items of an array are ordinary objects. The drawing keeps a record
//! beside them: which objects were arrayed (the sources), which objects are the
//! copies made (the items), and the settings the copies were laid out with.
//! ARRAYEDIT changes the settings and lays the copies out again. Erasing an
//! item or a source does not end the record: an item that is gone is simply not
//! there to replace.

use crate::{Ccs, EntityId, Point};

/// How the items of an array are laid out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArrayShape {
    /// Rows and columns along the axes of the coordinate system it was made in.
    Rectangular {
        columns: usize,
        rows: usize,
        column_spacing: f64,
        row_spacing: f64,
        ccs: Ccs,
    },
    /// Round a centre, `fill` radians in all (negative is clockwise).
    Polar {
        center: Point,
        items: usize,
        fill: f64,
        rotate: bool,
    },
    /// Along a path curve (the object `path`, run from its end when
    /// `reversed`), spaced `spacing` apart, or spread over it when `divide`.
    Path {
        path: EntityId,
        reversed: bool,
        items: usize,
        spacing: f64,
        align: bool,
        divide: bool,
    },
}

/// One array of the drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrayRecord {
    /// Names the array within the drawing.
    pub id: u64,
    /// The objects arrayed, which are the first item.
    pub sources: Vec<EntityId>,
    /// The copies made, in the order they were laid out.
    pub items: Vec<EntityId>,
    pub shape: ArrayShape,
}

impl ArrayRecord {
    /// Whether the object is a source or an item of this array.
    pub fn holds(&self, id: EntityId) -> bool {
        self.sources.contains(&id) || self.items.contains(&id)
    }

    /// The kind of array, as the command that makes it is called.
    pub fn kind(&self) -> &'static str {
        match self.shape {
            ArrayShape::Rectangular { .. } => "Rectangular",
            ArrayShape::Polar { .. } => "Polar",
            ArrayShape::Path { .. } => "Path",
        }
    }
}
