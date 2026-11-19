//! A layer being made a few chunks at a time, while the old one is drawn
//! moved. The chunks are independent, so each frame makes some and the
//! rest wait; if the camera moves before the last is made, the work is thrown
//! away, since it was for a view nobody is looking at.

use std::time::{Duration, Instant};

use super::key::View;
use super::{LayerCache, LayerKey, Piece};

/// The new layer so far.
pub(super) struct Pending {
    key: LayerKey,
    pieces: Vec<Piece>,
    total: usize,
    bytes: usize,
}

/// Where a slice of work left the new layer.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Advance {
    /// Not started: the view has not stood still long enough.
    Waiting(Duration),
    /// Started or going on; more chunks are to be made.
    Going,
    /// All made and the new layer is the kept one.
    Done,
    /// The old and the new together would be over the memory limit, so there
    /// is no room to show the old one: the new one is to be made at once.
    NoRoom,
}

impl LayerCache {
    /// How long the view has stood still at `view`. A view that is not the
    /// one last seen starts the clock again, and drops work made for another.
    pub fn stood_still(&mut self, view: View, now: Instant) -> Duration {
        match self.settle {
            Some((seen, since)) if seen == view => now.saturating_duration_since(since),
            _ => {
                self.settle = Some((view, now));
                if self.pending.as_ref().is_some_and(|p| p.key.view() != view) {
                    self.drop_pending();
                }
                Duration::ZERO
            }
        }
    }

    /// Make pictures that show what `key` shows on the screen's thread: the
    /// background could not make one.
    pub fn refuse_background(&mut self, key: LayerKey) {
        self.refused = Some(key);
    }

    /// Whether pictures that show what `key` shows are made on the screen's
    /// thread (see `refuse_background`).
    pub fn refused(&self, key: &LayerKey) -> bool {
        self.refused.as_ref().is_some_and(|k| k.same_content(key))
    }

    pub fn drop_pending(&mut self) {
        self.pending = None;
        self.stats.rebuilding = None;
    }

    /// Start the new layer for `key` of `total` chunks, unless it is going.
    pub fn begin_pending(&mut self, key: &LayerKey, total: usize) {
        if self.pending.as_ref().is_some_and(|p| &p.key == key) {
            return;
        }
        self.pending = Some(Pending {
            key: key.clone(),
            pieces: Vec::with_capacity(total),
            total,
            bytes: 0,
        });
        self.stats.rebuilding = Some(0);
    }

    /// The chunk to make next, if any is left.
    pub fn pending_next(&self) -> Option<usize> {
        let p = self.pending.as_ref()?;
        (p.pieces.len() < p.total).then_some(p.pieces.len())
    }

    /// Add the next chunk. `Advance::NoRoom` if that leaves the old and new
    /// layers over `limit` together.
    pub fn pending_push(&mut self, piece: Piece, limit: usize) -> Advance {
        let Some(p) = self.pending.as_mut() else {
            return Advance::Going;
        };
        p.bytes += piece.bytes();
        p.pieces.push(piece);
        if p.bytes + self.stats.bytes > limit {
            self.drop_pending();
            return Advance::NoRoom;
        }
        let percent = (p.pieces.len() * 100 / p.total.max(1)) as u8;
        self.stats.rebuilding = Some(percent);
        Advance::Going
    }

    /// If every chunk is made, make the new layer the kept one, by `keep`
    /// (given the cache, the key and the pieces).
    pub fn finish_pending(
        &mut self,
        keep: impl FnOnce(&mut LayerCache, LayerKey, Vec<Piece>),
    ) -> bool {
        if self.pending_next().is_some() {
            return false;
        }
        let Some(p) = self.pending.take() else {
            return false;
        };
        self.stats.rebuilding = None;
        keep(self, p.key, p.pieces);
        true
    }
}
