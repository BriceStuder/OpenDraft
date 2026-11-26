//! What TRIM and EXTEND remember between uses, as AutoCAD's
//! `TRIMEXTENDMODE` (Quick or Standard mode), `EDGEMODE` (whether edges act as
//! if extended past their ends) and `PROJMODE`. Saved with the user's settings, not in
//! the drawing.

use opendraft_commands::{SysVar, Value};

use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    /// Quick mode (AutoCAD's default): every object is an edge without
    /// asking, and TRIM erases what no edge crosses. Standard mode asks for
    /// the edges first.
    pub quick: bool,
    /// The Edge option: cutting and boundary edges act as if they ran on
    /// past their ends along their own paths (`EDGEMODE` 1). Off by default.
    pub edge_extend: bool,
    /// The Project option, `PROJMODE`: 0 None, 1 Ucs, 2 View. Flat drawings
    /// are unaffected.
    pub project: u8,
}

const DEFAULT: Memory = Memory {
    quick: true,
    edge_extend: false,
    project: 1,
};

impl Default for Memory {
    fn default() -> Self {
        DEFAULT
    }
}

const KEY: &str = "trim_extend_quick";
const EDGE_KEY: &str = "trim_extend_edge";
const PROJECT_KEY: &str = "trim_extend_project";

/// Take the saved settings, keeping the defaults when they are missing.
pub fn load(storage: Option<&dyn eframe::Storage>) -> Memory {
    let saved = |key: &str| storage.and_then(|s| s.get_string(key));
    Memory {
        quick: saved(KEY).as_deref() != Some("0"),
        edge_extend: saved(EDGE_KEY).as_deref() == Some("1"),
        project: saved(PROJECT_KEY)
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|p| *p <= 2)
            .unwrap_or(DEFAULT.project),
    }
}

/// Write the settings for the next run.
pub fn save(m: &Memory, storage: &mut dyn eframe::Storage) {
    storage.set_string(KEY, if m.quick { "1" } else { "0" }.to_owned());
    storage.set_string(EDGE_KEY, if m.edge_extend { "1" } else { "0" }.to_owned());
    storage.set_string(PROJECT_KEY, m.project.to_string());
}

/// `TRIMEXTENDMODE` and `EDGEMODE`.
pub fn variables() -> Vec<SysVar<AppState>> {
    vec![
        SysVar::int(
            "TRIMEXTENDMODE",
            "TRIM and EXTEND mode: 0 Standard (choose the edges), 1 Quick (every object is an edge)",
            (0, 1),
            |st| Value::Int(i64::from(st.defaults().trim.get().quick)),
            |st, v| {
                let quick = matches!(v, Value::Int(1));
                st.defaults().trim.change(|m| m.quick = quick);
                Ok(())
            },
        ),
        SysVar::int(
            "EDGEMODE",
            "Cutting and boundary edges for TRIM and EXTEND: 0 as they are, 1 extended along their paths",
            (0, 1),
            |st| Value::Int(i64::from(st.defaults().trim.get().edge_extend)),
            |st, v| {
                let extend = matches!(v, Value::Int(1));
                st.defaults().trim.change(|m| m.edge_extend = extend);
                Ok(())
            },
        ),
        SysVar::int(
            "PROJMODE",
            "Projection for TRIM and EXTEND: 0 None, 1 Ucs, 2 View",
            (0, 2),
            |st| Value::Int(i64::from(st.defaults().trim.get().project)),
            |st, v| {
                if let Value::Int(n) = v {
                    let project = u8::try_from(n).unwrap_or(1);
                    st.defaults().trim.change(|m| m.project = project);
                }
                Ok(())
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_is_the_default_and_the_variable_changes_it() {
        assert!(load(None).quick);
        let (table, mut state) = (crate::sysvars::table(), AppState::new());
        assert_eq!(table.get("TRIMEXTENDMODE", &state), Ok(Value::Int(1)));
        table.set("TRIMEXTENDMODE", "0", &mut state).unwrap();
        assert!(!state.defaults().trim.get().quick);
        assert!(table.set("TRIMEXTENDMODE", "2", &mut state).is_err());
    }

    #[test]
    fn edges_are_not_extended_by_default_and_edgemode_changes_it() {
        assert!(!load(None).edge_extend);
        let (table, mut state) = (crate::sysvars::table(), AppState::new());
        assert_eq!(table.get("EDGEMODE", &state), Ok(Value::Int(0)));
        table.set("EDGEMODE", "1", &mut state).unwrap();
        assert!(state.defaults().trim.get().edge_extend);
        assert_eq!(table.get("EDGEMODE", &state), Ok(Value::Int(1)));
        assert!(table.set("EDGEMODE", "2", &mut state).is_err());
        // The two settings are independent.
        assert!(state.defaults().trim.get().quick);
    }
}
