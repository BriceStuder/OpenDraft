//! Coordinate systems in the records the reader meets: a UCS table record
//! (a system saved by name) and a VIEWPORT's own system. See `dxf/ccs.rs`.

use opendraft_core::ccs::name_refusal;
use opendraft_core::{Ccs, NamedCcs};

use super::{decode, Importer, Record};
use crate::dxf::ccs::from_axes;

/// What a record's origin and X and Y directions (groups `base`, `base + 1`
/// and `base + 2`) say: none when it has no origin, `Some(None)` when it
/// has one that is 3D or mirrored.
pub(super) fn axes(record: &Record, base: i32) -> Option<Option<Ccs>> {
    let point = |code: i32, default: [f64; 3]| {
        [
            record.number(code).unwrap_or(default[0]),
            record.number(code + 10).unwrap_or(default[1]),
            record.number(code + 20).unwrap_or(default[2]),
        ]
    };
    record.number(base)?;
    let origin = point(base, [0.0; 3]);
    let x = point(base + 1, [1.0, 0.0, 0.0]);
    let y = point(base + 2, [0.0, 1.0, 0.0]);
    Some(from_axes(origin, x, y))
}

impl Importer {
    /// A record of the UCS table: a system saved by name. A name that cannot
    /// name one, or one already read, is left out.
    pub(super) fn ucs_record(&mut self, record: &Record) {
        let name = decode(record.text(2).unwrap_or(""));
        let taken = self.document.named_ccs_called(&name).is_some();
        if name_refusal(&name).is_some() || taken {
            return;
        }
        match axes(record, 10) {
            Some(Some(ccs)) => {
                let mut named = self.document.named_ccs().to_vec();
                named.push(NamedCcs { name, ccs });
                self.document.set_named_ccs(named);
            }
            Some(None) => self.tilted_ccs += 1,
            None => {}
        }
    }
}
