//! The selected objects as a list, for panels that read them every frame (the
//! ribbon's property lists and the Properties panel). Finding each selected
//! id in the drawing costs a lookup, and for tens of thousands of objects
//! that was a few milliseconds in each panel in each frame. The positions are
//! kept while neither the drawing nor the selection changes, and the objects
//! are then indexed directly.

use std::sync::Arc;

use eframe::egui;
use opendraft_canvas::layer_cache::selection_hash;
use opendraft_core::Object;

use crate::state::Doc;

/// What was worked out for a state of the drawing and a selection.
type Kept = ((u64, u64), u64, Arc<Vec<usize>>);

/// The objects of the selection of `doc`, in the order of the selection.
pub(crate) fn of<'a>(ctx: &egui::Context, doc: &'a Doc) -> Vec<&'a Object> {
    let document = doc.session.document();
    let objects = document.objects();
    if doc.selection.is_empty() {
        return Vec::new();
    }
    let (token, hash) = (doc.session.draw_token(), selection_hash(&doc.selection));
    let id = egui::Id::new("selected_object_positions");
    let kept: Option<Kept> = ctx.data(|d| d.get_temp(id));
    let positions = match kept.filter(|(t, h, _)| (*t, *h) == (token, hash)) {
        Some((_, _, positions)) => positions,
        None => {
            let found = doc.selection.iter().filter_map(|&i| document.position(i));
            let positions = Arc::new(found.collect::<Vec<_>>());
            ctx.data_mut(|d| d.insert_temp(id, (token, hash, positions.clone())));
            positions
        }
    };
    positions.iter().filter_map(|&at| objects.get(at)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use opendraft_core::{Entity, Point};

    fn line(x: f64) -> Entity {
        Entity::Line {
            start: Point::new(x, 0.0),
            end: Point::new(x + 1.0, 0.0),
        }
    }

    #[test]
    fn the_list_is_the_selection_and_follows_it() {
        let mut state = AppState::new();
        for x in 0..5 {
            state.add_entity(line(f64::from(x)));
        }
        let ctx = egui::Context::default();
        let doc = |s: &AppState| s.docs[s.active].selection.clone();
        let ids = |state: &AppState| -> Vec<_> {
            of(&ctx, &state.docs[state.active])
                .iter()
                .map(|o| o.id)
                .collect()
        };
        assert!(ids(&state).is_empty());
        state.select_all();
        assert_eq!(ids(&state), doc(&state).into_iter().collect::<Vec<_>>());
        assert_eq!(ids(&state).len(), 5, "and again, from what was kept");
        assert_eq!(ids(&state).len(), 5);
        // A smaller selection is not mistaken for the kept one.
        let first = state.docs[state.active].selection.iter().next().copied();
        state.docs[state.active].selection.clear();
        state.docs[state.active].selection.extend(first);
        assert_eq!(ids(&state), first.into_iter().collect::<Vec<_>>());
    }
}
