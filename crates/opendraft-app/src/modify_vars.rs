//! `FILLETRAD`, `CHAMFERA` to `CHAMFERD`, `CHAMMODE`, `TRIMMODE` and
//! `OFFSETDIST`: what FILLET, CHAMFER and OFFSET remember. They are saved in
//! the drawing (see `ModifySettings`), so setting one is not an undo step, as
//! in AutoCAD.

use opendraft_commands::{SysVar, Value};
use opendraft_core::ModifySettings;

use crate::state::AppState;

impl AppState {
    /// What the active drawing remembers for the modify commands.
    pub fn modify_settings(&self) -> ModifySettings {
        self.session().document().modify_settings()
    }

    /// Change it. Not an undo step.
    pub fn change_modify_settings(&mut self, change: impl FnOnce(&mut ModifySettings)) {
        let doc = &mut self.docs[self.active];
        doc.session.change_modify_settings(change);
    }
}

fn flag(on: bool) -> Value {
    Value::Int(i64::from(on))
}

/// A real variable that is one field of the settings.
macro_rules! real_var {
    ($name:literal, $help:literal, $field:ident) => {
        SysVar::real(
            $name,
            $help,
            (0.0, 1.0e9),
            |st| Value::Real(st.modify_settings().$field),
            |st, v| {
                if let Value::Real(n) = v {
                    st.change_modify_settings(|s| s.$field = n);
                }
                Ok(())
            },
        )
    };
}

pub fn variables() -> Vec<SysVar<AppState>> {
    vec![
        real_var!("FILLETRAD", "Fillet radius", fillet_radius),
        real_var!("CHAMFERA", "First chamfer distance", chamfer_first),
        real_var!("CHAMFERB", "Second chamfer distance", chamfer_second),
        real_var!(
            "CHAMFERC",
            "Chamfer length when measured by length and angle",
            chamfer_length
        ),
        SysVar::real(
            "CHAMFERD",
            "Chamfer angle in degrees when measured by length and angle",
            (0.0, 180.0),
            |st| Value::Real(st.modify_settings().chamfer_angle),
            |st, v| {
                if let Value::Real(n) = v {
                    st.change_modify_settings(|s| s.chamfer_angle = n);
                }
                Ok(())
            },
        ),
        SysVar::int(
            "CHAMMODE",
            "Chamfer measured by: 0 two distances, 1 length and angle",
            (0, 1),
            |st| flag(st.modify_settings().chamfer_by_angle),
            |st, v| {
                st.change_modify_settings(|s| s.chamfer_by_angle = matches!(v, Value::Int(1)));
                Ok(())
            },
        ),
        SysVar::int(
            "TRIMMODE",
            "Fillet and chamfer trim the lines: 0 no, 1 yes",
            (0, 1),
            |st| flag(st.modify_settings().trim),
            |st, v| {
                st.change_modify_settings(|s| s.trim = matches!(v, Value::Int(1)));
                Ok(())
            },
        ),
        SysVar::real(
            "OFFSETDIST",
            "OFFSET's default distance; negative for Through",
            (-1.0e9, 1.0e9),
            |st| Value::Real(st.modify_settings().offset_distance),
            |st, v| match v {
                Value::Real(n) if n.abs() < 1e-9 => Err("Requires a non-zero value.".to_owned()),
                Value::Real(n) => {
                    st.change_modify_settings(|s| s.offset_distance = n);
                    Ok(())
                }
                _ => Ok(()),
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variables_read_and_write_the_drawings_settings() {
        let (table, mut state) = (crate::sysvars::table(), AppState::new());
        assert_eq!(table.get("FILLETRAD", &state), Ok(Value::Real(10.0)));
        table.set("FILLETRAD", "4.5", &mut state).unwrap();
        table.set("CHAMMODE", "1", &mut state).unwrap();
        table.set("CHAMFERD", "30", &mut state).unwrap();
        table.set("TRIMMODE", "0", &mut state).unwrap();
        let s = state.modify_settings();
        assert_eq!((s.fillet_radius, s.chamfer_angle), (4.5, 30.0));
        assert!(s.chamfer_by_angle && !s.trim);
        assert!(table.set("FILLETRAD", "-1", &mut state).is_err());
        assert!(table.set("CHAMFERD", "200", &mut state).is_err());
    }

    #[test]
    fn offsetdist_starts_at_one_and_a_negative_number_is_through() {
        let (table, mut state) = (crate::sysvars::table(), AppState::new());
        assert_eq!(table.get("OFFSETDIST", &state), Ok(Value::Real(1.0)));
        table.set("OFFSETDIST", "2.5", &mut state).unwrap();
        assert_eq!(state.modify_settings().offset_distance, 2.5);
        assert!(table.set("OFFSETDIST", "0", &mut state).is_err());
        table.set("OFFSETDIST", "-3", &mut state).unwrap();
        assert_eq!(state.modify_settings().offset_distance, -3.0);
    }

    #[test]
    fn each_drawing_has_its_own_settings() {
        let mut state = AppState::new();
        state.change_modify_settings(|s| s.fillet_radius = 7.0);
        state.new_document();
        assert_eq!(state.modify_settings().fillet_radius, 10.0);
    }
}
