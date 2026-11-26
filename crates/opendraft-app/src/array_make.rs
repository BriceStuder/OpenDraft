//! What the array commands share to make an array: the objects arrayed, the
//! preview of the copies, the check that an array is not too big, and adding
//! the copies (with the record that makes the array associative).

use std::sync::Arc;

use opendraft_commands::{Cancelled, Editor, SetArrays};
use opendraft_core::array::{Placement, MAX_ITEMS};
use opendraft_core::{ArrayRecord, ArrayShape, Attributes, Bounds, Entity, EntityId, Point};
use opendraft_ui::key;

use crate::defaults::EditorDefaults;
use opendraft_commands::Wording;

/// The most objects one array may add.
const MAX_COPIES: usize = 200_000;
/// The most objects the preview draws.
const PREVIEW_LIMIT: usize = 2_000;

/// The objects an array copies.
pub(crate) struct Source {
    /// The objects, which are still in the drawing.
    pub ids: Vec<EntityId>,
    pub objects: Vec<(Entity, Attributes)>,
    pub bounds: Bounds,
}

impl Source {
    /// The width and height of the objects together, for default spacings:
    /// one and a half times each (a side with no size takes the other's).
    pub fn spacing(&self) -> (f64, f64) {
        let (w, h) = (
            self.bounds.max.x - self.bounds.min.x,
            self.bounds.max.y - self.bounds.min.y,
        );
        let side = |a: f64, b: f64| match (a > 1e-9, b > 1e-9) {
            (true, _) => a,
            (false, true) => b,
            _ => 1.0,
        };
        (1.5 * side(w, h), 1.5 * side(h, w))
    }

    pub fn center(&self) -> Point {
        Point::new(
            (self.bounds.min.x + self.bounds.max.x) / 2.0,
            (self.bounds.min.y + self.bounds.max.y) / 2.0,
        )
    }
}

/// The objects `ids` names, with their size together; `None` when none are
/// left.
pub(crate) fn read_source(ed: &Editor, ids: Vec<EntityId>) -> Result<Option<Source>, Cancelled> {
    ed.read(move |doc| {
        let found: Vec<_> = ids.iter().filter_map(|id| doc.object(*id)).collect();
        let bounds = (found.iter().map(|o| o.bounds()))
            .filter(Bounds::is_finite)
            .reduce(Bounds::union)?;
        let objects = found
            .iter()
            .map(|o| (o.entity.clone(), o.attrs.clone()))
            .collect();
        let ids = found.iter().map(|o| o.id).collect();
        Some(Source {
            ids,
            objects,
            bounds,
        })
    })
}

/// What the preview shows: the copies, up to [`PREVIEW_LIMIT`] objects.
pub(crate) fn ghost(source: &Source, items: &[Placement]) -> impl Fn(Point) -> Vec<Entity> {
    let shown: Vec<Entity> = items
        .iter()
        .skip(1)
        .flat_map(|item| source.objects.iter().map(|(e, _)| item.apply(e)))
        .take(PREVIEW_LIMIT)
        .collect();
    let shown = Arc::new(shown);
    move |_| shown.as_ref().clone()
}

/// Whether an array of `items` copies of the objects is small enough; says
/// why not.
pub(crate) fn fits(ed: &Editor, source: &Source, items: usize) -> bool {
    let copies = items.saturating_sub(1).saturating_mul(source.objects.len());
    if items > MAX_ITEMS || copies > MAX_COPIES {
        let said = Wording::new(
            key!("The array would have %{items}; the most is %{most}."),
            &[("most", &MAX_ITEMS.to_string())],
        );
        ed.say(said.part("items", crate::text::items(items)));
        return false;
    }
    true
}

/// Say what is being made, with whether it will be associative.
pub(crate) fn say_type(ed: &Editor, kind: &str) {
    let associative = !ed.defaults().array_plain.get();
    ed.say(match (kind, associative) {
        ("Rectangular", true) => key!("Type = Rectangular  Associative = Yes"),
        ("Rectangular", false) => key!("Type = Rectangular  Associative = No"),
        ("Polar", true) => key!("Type = Polar  Associative = Yes"),
        ("Polar", false) => key!("Type = Polar  Associative = No"),
        (_, true) => key!("Type = Path  Associative = Yes"),
        (_, false) => key!("Type = Path  Associative = No"),
    });
}

/// Add every item but the first (the objects themselves) as one undo step,
/// and, when the array is associative, record it. With `editing` the items of
/// that array are replaced instead, and its record updated.
pub(crate) fn make(
    ed: &Editor,
    source: Source,
    items: Vec<Placement>,
    shape: ArrayShape,
    editing: Option<ArrayRecord>,
) -> Result<(), Cancelled> {
    let count = items.len();
    let keep = editing.is_some() || !ed.defaults().array_plain.get();
    let copies: Vec<(Entity, Attributes)> = items
        .iter()
        .skip(1)
        .flat_map(|item| {
            source
                .objects
                .iter()
                .map(move |(e, attrs)| (item.apply(e), attrs.clone()))
        })
        .collect();
    if copies.is_empty() && editing.is_none() {
        return Ok(());
    }
    let sources = source.ids;
    ed.transaction("Array", move |tx| {
        if let Some(old) = &editing {
            tx.remove(old.items.clone());
        }
        let before = tx.document().objects().len();
        tx.add_with(copies);
        let made: Vec<EntityId> = tx.document().objects()[before..]
            .iter()
            .map(|o| o.id)
            .collect();
        if keep {
            let mut records = tx.document().arrays().to_vec();
            match &editing {
                Some(old) => {
                    if let Some(slot) = records.iter_mut().find(|r| r.id == old.id) {
                        *slot = ArrayRecord {
                            items: made,
                            shape,
                            ..old.clone()
                        };
                    }
                }
                None => records.push(ArrayRecord {
                    id: tx.document().next_array_id(),
                    sources,
                    items: made,
                    shape,
                }),
            }
            tx.perform(SetArrays::new(records));
        }
        Ok(())
    })?;
    ed.say(Wording::new(key!("Array of %{items}."), &[]).part("items", crate::text::items(count)));
    Ok(())
}
