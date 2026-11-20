//! Which rows of the Layers panel are selected. A click picks one row,
//! Ctrl+click adds or removes a row, Shift+click picks the run from the last
//! clicked row, as in AutoCAD's Layer Properties Manager. A change made on a
//! selected row applies to every selected layer.

use eframe::egui;
use opendraft_core::{Document, LayerId};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayerSelection {
    /// In the order they were picked; no duplicates.
    ids: Vec<LayerId>,
    /// Where a Shift+click run starts: the last row clicked.
    anchor: Option<LayerId>,
}

impl LayerSelection {
    /// The selection kept for the panel `key`, minus layers that are gone.
    pub fn load(ctx: &egui::Context, key: egui::Id, document: &Document) -> Self {
        let mut selection: Self = ctx.data(|d| d.get_temp(key)).unwrap_or_default();
        selection.ids.retain(|&id| document.layer(id).is_some());
        if selection
            .anchor
            .is_some_and(|id| document.layer(id).is_none())
        {
            selection.anchor = None;
        }
        selection
    }

    pub fn store(self, ctx: &egui::Context, key: egui::Id) {
        ctx.data_mut(|d| d.insert_temp(key, self));
    }

    pub fn ids(&self) -> &[LayerId] {
        &self.ids
    }

    pub fn contains(&self, id: LayerId) -> bool {
        self.ids.contains(&id)
    }

    /// The one selected layer, if exactly one is.
    pub fn single(&self) -> Option<LayerId> {
        match self.ids.as_slice() {
            [id] => Some(*id),
            _ => None,
        }
    }

    /// The layers a change made on row `id` applies to: the whole selection
    /// when the row is part of it, the row alone otherwise.
    pub fn targets(&self, id: LayerId) -> Vec<LayerId> {
        if self.contains(id) {
            self.ids.clone()
        } else {
            vec![id]
        }
    }

    /// A click on row `id`. `order` is every layer as the panel lists them.
    pub fn click(&mut self, id: LayerId, order: &[LayerId], ctrl: bool, shift: bool) {
        let run = self
            .anchor
            .filter(|_| shift)
            .and_then(|anchor| run(order, anchor, id));
        if let Some(run) = run {
            if !ctrl {
                self.ids.clear();
            }
            for layer in run {
                if !self.ids.contains(&layer) {
                    self.ids.push(layer);
                }
            }
            return;
        }
        if ctrl {
            if self.contains(id) {
                self.ids.retain(|&l| l != id);
            } else {
                self.ids.push(id);
            }
        } else {
            self.ids = vec![id];
        }
        self.anchor = Some(id);
    }

    /// A right-click on row `id`: a row outside the selection becomes the
    /// selection, so the menu acts on what the user pointed at.
    pub fn point_at(&mut self, id: LayerId) {
        if !self.contains(id) {
            self.ids = vec![id];
            self.anchor = Some(id);
        }
    }

    pub fn select_all(&mut self, order: &[LayerId]) {
        self.ids = order.to_vec();
    }

    pub fn select_all_but(&mut self, order: &[LayerId], current: LayerId) {
        self.ids = order.iter().copied().filter(|&id| id != current).collect();
    }

    pub fn invert(&mut self, order: &[LayerId]) {
        self.ids = order
            .iter()
            .copied()
            .filter(|id| !self.ids.contains(id))
            .collect();
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.anchor = None;
    }
}

/// The layers from `a` to `b` inclusive, in list order.
fn run(order: &[LayerId], a: LayerId, b: LayerId) -> Option<Vec<LayerId>> {
    let i = order.iter().position(|&id| id == a)?;
    let j = order.iter().position(|&id| id == b)?;
    let (from, to) = (i.min(j), i.max(j));
    Some(order[from..=to].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> Vec<LayerId> {
        (0..6).map(LayerId).collect()
    }

    fn ids(selection: &LayerSelection) -> Vec<u64> {
        let mut ids: Vec<u64> = selection.ids().iter().map(|id| id.0).collect();
        ids.sort_unstable();
        ids
    }

    #[test]
    fn a_plain_click_picks_one_row() {
        let mut s = LayerSelection::default();
        s.click(LayerId(2), &order(), false, false);
        s.click(LayerId(4), &order(), false, false);
        assert_eq!(ids(&s), [4]);
        assert_eq!(s.single(), Some(LayerId(4)));
    }

    #[test]
    fn ctrl_click_adds_and_removes() {
        let mut s = LayerSelection::default();
        s.click(LayerId(1), &order(), false, false);
        s.click(LayerId(3), &order(), true, false);
        s.click(LayerId(5), &order(), true, false);
        assert_eq!(ids(&s), [1, 3, 5]);
        assert_eq!(s.single(), None);
        s.click(LayerId(3), &order(), true, false);
        assert_eq!(ids(&s), [1, 5]);
    }

    #[test]
    fn shift_click_picks_the_run_from_the_last_click_either_way() {
        let mut s = LayerSelection::default();
        s.click(LayerId(4), &order(), false, false);
        s.click(LayerId(1), &order(), false, true);
        assert_eq!(ids(&s), [1, 2, 3, 4]);
        // The anchor stays on 4: a second Shift+click reshapes the run.
        s.click(LayerId(5), &order(), false, true);
        assert_eq!(ids(&s), [4, 5]);
    }

    #[test]
    fn ctrl_shift_click_adds_a_run_to_the_selection() {
        let mut s = LayerSelection::default();
        s.click(LayerId(0), &order(), false, false);
        s.click(LayerId(3), &order(), true, false);
        s.click(LayerId(5), &order(), true, true);
        assert_eq!(ids(&s), [0, 3, 4, 5]);
    }

    #[test]
    fn shift_click_without_an_anchor_is_a_plain_click() {
        let mut s = LayerSelection::default();
        s.click(LayerId(2), &order(), false, true);
        assert_eq!(ids(&s), [2]);
    }

    #[test]
    fn a_change_on_a_selected_row_reaches_the_whole_selection() {
        let mut s = LayerSelection::default();
        s.click(LayerId(1), &order(), false, false);
        s.click(LayerId(2), &order(), true, false);
        assert_eq!(s.targets(LayerId(2)).len(), 2);
        assert_eq!(
            s.targets(LayerId(4)),
            [LayerId(4)],
            "outside: that row only"
        );
    }

    #[test]
    fn right_click_keeps_a_selection_it_lands_in() {
        let mut s = LayerSelection::default();
        s.select_all(&order()[1..4]);
        s.point_at(LayerId(2));
        assert_eq!(ids(&s), [1, 2, 3]);
        s.point_at(LayerId(5));
        assert_eq!(ids(&s), [5]);
    }

    #[test]
    fn select_all_but_current_invert_and_clear() {
        let mut s = LayerSelection::default();
        s.select_all_but(&order(), LayerId(0));
        assert_eq!(ids(&s), [1, 2, 3, 4, 5]);
        s.invert(&order());
        assert_eq!(ids(&s), [0]);
        s.clear();
        assert!(s.ids().is_empty());
    }
}
