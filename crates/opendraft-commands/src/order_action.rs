//! Changing the draw order: which objects are drawn over which.

use opendraft_core::{Document, EntityId, Object};

use crate::{Action, Changes};

/// Where the objects go in the draw order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Drawn last, over everything.
    Front,
    /// Drawn first, under everything.
    Back,
    /// Just over this object.
    Above(EntityId),
    /// Just under this object.
    Under(EntityId),
}

/// Move objects in the draw order, keeping their ids and their order among
/// themselves.
pub struct ReorderObjects {
    ids: Vec<EntityId>,
    place: Place,
    /// Where the objects were, ascending, kept for the way back.
    before: Vec<usize>,
    /// What the last apply moved.
    moved: Vec<EntityId>,
}

impl ReorderObjects {
    pub fn new(mut ids: Vec<EntityId>, place: Place) -> Self {
        ids.sort_unstable();
        ids.dedup();
        Self {
            ids,
            place,
            before: Vec::new(),
            moved: Vec::new(),
        }
    }
}

impl Action for ReorderObjects {
    fn name(&self) -> String {
        match self.place {
            Place::Front => "Bring to Front",
            Place::Back => "Send to Back",
            Place::Above(_) => "Bring Above Objects",
            Place::Under(_) => "Send Under Objects",
        }
        .to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        // An object cannot go over or under itself.
        let target = match self.place {
            Place::Above(id) | Place::Under(id) => Some(id),
            _ => None,
        };
        self.ids.retain(|id| Some(*id) != target);
        let taken = doc.remove_many(&self.ids);
        self.before = taken.iter().map(|(position, _)| *position).collect();
        self.moved = taken.iter().map(|(_, o)| o.id).collect();
        let at = match self.place {
            Place::Front => doc.len(),
            Place::Back => 0,
            Place::Above(id) => doc.position(id).map_or(doc.len(), |p| p + 1),
            Place::Under(id) => doc.position(id).unwrap_or(0),
        };
        doc.insert_many(
            taken
                .into_iter()
                .enumerate()
                .map(|(k, (_, object))| (at + k, object))
                .collect(),
        );
    }

    fn revert(&mut self, doc: &mut Document) {
        let objects: Vec<Object> = doc
            .remove_many(&self.moved)
            .into_iter()
            .map(|(_, o)| o)
            .collect();
        // They come out in the order they went in, which is the order they
        // had, so each goes back to its own old place.
        doc.insert_many(self.before.iter().copied().zip(objects).collect());
    }

    fn changes(&self) -> Changes {
        Changes::changed(self.moved.clone())
    }
}

#[cfg(test)]
mod tests {
    use opendraft_core::{Entity, Point};

    use super::*;
    use crate::AddEntity;

    fn drawing(n: usize) -> (Document, Vec<EntityId>) {
        let mut doc = Document::default();
        for i in 0..n {
            let mut add = AddEntity::new(Entity::Point {
                at: Point::new(i as f64, 0.0),
            });
            add.apply(&mut doc);
        }
        let ids = doc.objects().iter().map(|o| o.id).collect();
        (doc, ids)
    }

    fn order(doc: &Document) -> Vec<EntityId> {
        doc.objects().iter().map(|o| o.id).collect()
    }

    #[test]
    fn front_and_back_keep_the_objects_among_themselves_and_undo_restores_the_order() {
        let (mut doc, ids) = drawing(5);
        let mut front = ReorderObjects::new(vec![ids[3], ids[1]], Place::Front);
        front.apply(&mut doc);
        assert_eq!(order(&doc), [ids[0], ids[2], ids[4], ids[1], ids[3]]);
        front.revert(&mut doc);
        assert_eq!(order(&doc), ids);
        let mut back = ReorderObjects::new(vec![ids[4], ids[2]], Place::Back);
        back.apply(&mut doc);
        assert_eq!(order(&doc), [ids[2], ids[4], ids[0], ids[1], ids[3]]);
        back.revert(&mut doc);
        assert_eq!(order(&doc), ids);
        // And redo.
        back.apply(&mut doc);
        assert_eq!(order(&doc), [ids[2], ids[4], ids[0], ids[1], ids[3]]);
    }

    #[test]
    fn above_and_under_put_the_objects_beside_the_reference() {
        let (mut doc, ids) = drawing(5);
        let mut above = ReorderObjects::new(vec![ids[0], ids[1]], Place::Above(ids[3]));
        above.apply(&mut doc);
        assert_eq!(order(&doc), [ids[2], ids[3], ids[0], ids[1], ids[4]]);
        above.revert(&mut doc);
        assert_eq!(order(&doc), ids);
        let mut under = ReorderObjects::new(vec![ids[4]], Place::Under(ids[1]));
        under.apply(&mut doc);
        assert_eq!(order(&doc), [ids[0], ids[4], ids[1], ids[2], ids[3]]);
        under.revert(&mut doc);
        assert_eq!(order(&doc), ids);
    }

    #[test]
    fn an_object_is_never_placed_over_itself() {
        let (mut doc, ids) = drawing(3);
        let mut same = ReorderObjects::new(vec![ids[1]], Place::Above(ids[1]));
        same.apply(&mut doc);
        assert_eq!(order(&doc), ids);
        same.revert(&mut doc);
        assert_eq!(order(&doc), ids);
    }
}
