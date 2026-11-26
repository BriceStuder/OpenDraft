//! PEDIT's curve options: Fit and Spline for a polyline, and turning a spline
//! into a polyline. The geometry is `opendraft_core::pedit` (`curve.rs`).

use opendraft_commands::{Cancelled, Editor, NumberOptions, Options};
use opendraft_core::pedit::PolyData;
use opendraft_core::{Entity, EntityId};
use opendraft_ui::key;

use crate::defaults::EditorDefaults;
use crate::pedit_cmd::store;

/// Fit or Spline: replace the polyline by its curve. `true` when it changed.
pub(crate) fn curve(
    ed: &Editor,
    id: EntityId,
    data: &PolyData,
    fit: bool,
) -> Result<bool, Cancelled> {
    let made = if fit { data.fitted() } else { data.splined() };
    match made {
        Some(made) => {
            store(ed, id, made)?;
            Ok(true)
        }
        None => {
            ed.say(key!(
                "The polyline needs at least three vertices that are apart."
            ));
            Ok(false)
        }
    }
}

/// A spline picked for PEDIT: offer to turn it into a polyline (without
/// asking when PEDITACCEPT is 1), by the precision asked for (0 to 99: the
/// higher, the closer to the curve). `true` when the spline became a polyline.
pub(crate) fn convert_spline(ed: &Editor, id: EntityId) -> Result<bool, Cancelled> {
    if !ed.defaults().pedit.get().accept {
        let answer = ed.get_keyword(
            Options::new(key!(
                "Object selected is not a polyline. Do you want to turn it into one?"
            ))
            .keyword("Yes", "Y")
            .keyword("No", "N")
            .default("Yes".to_owned()),
        )?;
        if answer == "No" {
            return Ok(false);
        }
    }
    let precision = ed
        .get_number(
            NumberOptions::new(key!("Specify precision for polyline conversion (0 to 99)"))
                .default(10.0),
        )?
        .clamp(0.0, 99.0);
    let spline = ed.read(move |doc| match doc.get(id) {
        Some(Entity::Spline(s)) => Some((**s).clone()),
        _ => None,
    })?;
    let Some(spline) = spline else {
        return Ok(false);
    };
    let size = {
        let b = spline.bounds();
        (b.max.x - b.min.x).hypot(b.max.y - b.min.y)
    };
    // 10 gives a thousandth of the size; each ten more is ten times closer.
    let tolerance = (size * 10f64.powf(-2.0 - precision / 10.0)).max(1e-9);
    match PolyData::from_spline(&spline, tolerance) {
        Some(data) => {
            store(ed, id, data)?;
            Ok(true)
        }
        None => {
            ed.say(key!("The spline could not be turned into a polyline."));
            Ok(false)
        }
    }
}
