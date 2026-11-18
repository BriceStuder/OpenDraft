//! One kept layer per place the drawing is painted: the model tab, each
//! viewport of a sheet, and the sheet itself. They are made, drawn and let go
//! on their own; they share the memory limit.

use std::collections::HashMap;

use opendraft_core::{LayoutId, ViewportId};

use super::{CacheStats, LayerCache};

/// Where a layer is painted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Slot {
    /// The model tab, or model space reached through a viewport.
    Model,
    /// The model as one viewport of a sheet shows it.
    Viewport(ViewportId),
    /// What is drawn on the sheet itself.
    Sheet(LayoutId),
}

/// A slot not painted for this many passes is let go, so a sheet that is no
/// longer shown does not keep its memory.
const IDLE_PASSES: u64 = 600;

#[derive(Default)]
pub(crate) struct LayerCaches {
    slots: HashMap<Slot, (LayerCache, u64)>,
}

impl LayerCaches {
    /// The cache for `slot`, made if there is none, used in egui pass `pass`;
    /// slots that have been idle for long are let go.
    pub fn slot(&mut self, slot: Slot, pass: u64) -> &mut LayerCache {
        self.slots
            .retain(|s, (_, used)| *s == slot || pass.saturating_sub(*used) <= IDLE_PASSES);
        let (cache, used) = self.slots.entry(slot).or_default();
        *used = pass;
        cache
    }

    /// Bytes kept in the slots but `slot`.
    pub fn bytes_elsewhere(&self, slot: Slot) -> usize {
        let others = self.slots.iter().filter(|(s, _)| **s != slot);
        others.map(|(_, (cache, _))| cache.stats().bytes).sum()
    }

    /// What all the slots have done together.
    pub fn stats(&self) -> CacheStats {
        let all = self.slots.values().map(|(cache, _)| cache.stats());
        all.fold(CacheStats::default(), CacheStats::merged)
    }

    /// Forget every kept layer (the counts of what was done stay).
    pub fn clear(&mut self) {
        for (cache, _) in self.slots.values_mut() {
            cache.clear();
        }
    }

    #[cfg(test)]
    pub fn slots(&self) -> usize {
        self.slots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendraft_core::{LayoutId, ViewportId};

    #[test]
    fn a_slot_not_used_for_long_is_let_go_and_the_rest_stay() {
        let mut caches = LayerCaches::default();
        let (sheet, viewport) = (Slot::Sheet(LayoutId(1)), Slot::Viewport(ViewportId(1)));
        caches.slot(Slot::Model, 1);
        caches.slot(sheet, 1);
        caches.slot(viewport, 2);
        assert_eq!(caches.slots(), 3);
        // Only the viewport is painted from now on.
        caches.slot(viewport, 300);
        assert_eq!(caches.slots(), 3, "idle, but not for long yet");
        caches.slot(viewport, 700);
        assert_eq!(caches.slots(), 1, "the unused ones are gone");
    }

    #[test]
    fn what_the_other_slots_keep_is_counted_against_the_limit() {
        let mut caches = LayerCaches::default();
        let (a, b) = (Slot::Model, Slot::Sheet(LayoutId(1)));
        caches.slot(a, 1);
        caches.slot(b, 1);
        assert_eq!(caches.bytes_elsewhere(a), 0);
        assert_eq!(caches.stats(), CacheStats::default());
    }
}
