//! What OFFSET remembers between uses, as AutoCAD's `OFFSETDIST`: the distance
//! offered as the default, or a negative number for Through. Saved with the
//! user's settings, not in the drawing.

use opendraft_commands::{SysVar, Value};

use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    /// The distance the last offset used, offered by Enter at the distance
    /// prompt; negative when it was Through (the size is then not used).
    pub distance: f64,
}

const DEFAULT: Memory = Memory { distance: 1.0 };

impl Default for Memory {
    fn default() -> Self {
        DEFAULT
    }
}

const KEY: &str = "offset_distance";

/// Take the saved distance, keeping the default when it is missing or zero.
pub fn load(storage: Option<&dyn eframe::Storage>) -> Memory {
    let distance = storage
        .and_then(|s| s.get_string(KEY))
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d != 0.0)
        .unwrap_or(DEFAULT.distance);
    Memory { distance }
}

/// Write the distance for the next run.
pub fn save(m: &Memory, storage: &mut dyn eframe::Storage) {
    storage.set_string(KEY, m.distance.to_string());
}

/// `OFFSETDIST`.
pub fn variables() -> Vec<SysVar<AppState>> {
    vec![SysVar::real(
        "OFFSETDIST",
        "OFFSET's default distance; negative for Through",
        (-1.0e9, 1.0e9),
        |st| Value::Real(st.defaults().offset.get().distance),
        |st, v| match v {
            Value::Real(n) if n.abs() < 1e-9 => Err("Requires a non-zero value.".to_owned()),
            Value::Real(n) => {
                st.defaults().offset.change(|m| m.distance = n);
                Ok(())
            }
            _ => Ok(()),
        },
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_distance_starts_at_one_and_offsetdist_changes_it() {
        assert_eq!(load(None).distance, 1.0);
        let (table, mut state) = (crate::sysvars::table(), AppState::new());
        assert_eq!(table.get("OFFSETDIST", &state), Ok(Value::Real(1.0)));
        table.set("OFFSETDIST", "2.5", &mut state).unwrap();
        assert_eq!(state.defaults().offset.get().distance, 2.5);
        assert!(table.set("OFFSETDIST", "0", &mut state).is_err());
        // A negative number is Through.
        table.set("OFFSETDIST", "-3", &mut state).unwrap();
        assert_eq!(state.defaults().offset.get().distance, -3.0);
    }
}
