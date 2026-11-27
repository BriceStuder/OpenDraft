//! What FILLET, CHAMFER, OFFSET and LENGTHEN remember, always written. A file
//! without them (one saved before they were kept) gets the defaults, and a
//! value that cannot be used is put back to its default.

use opendraft_core::{Document, ModifySettings};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ModifySettingsDto {
    fillet_radius: f64,
    chamfer_first: f64,
    chamfer_second: f64,
    chamfer_length: f64,
    chamfer_angle: f64,
    chamfer_by_angle: bool,
    corner_trim: bool,
    offset_distance: f64,
    lengthen_delta: Option<f64>,
}

impl Default for ModifySettingsDto {
    fn default() -> Self {
        Self::from(ModifySettings::default())
    }
}

impl From<ModifySettings> for ModifySettingsDto {
    fn from(s: ModifySettings) -> Self {
        Self {
            fillet_radius: s.fillet_radius,
            chamfer_first: s.chamfer_first,
            chamfer_second: s.chamfer_second,
            chamfer_length: s.chamfer_length,
            chamfer_angle: s.chamfer_angle,
            chamfer_by_angle: s.chamfer_by_angle,
            corner_trim: s.trim,
            offset_distance: s.offset_distance,
            lengthen_delta: s.lengthen_delta,
        }
    }
}

impl ModifySettingsDto {
    pub(super) fn of(document: &Document) -> Self {
        document.modify_settings().into()
    }

    pub(super) fn load(self, document: &mut Document) {
        document.set_modify_settings(ModifySettings {
            fillet_radius: self.fillet_radius,
            chamfer_first: self.chamfer_first,
            chamfer_second: self.chamfer_second,
            chamfer_length: self.chamfer_length,
            chamfer_angle: self.chamfer_angle,
            chamfer_by_angle: self.chamfer_by_angle,
            trim: self.corner_trim,
            offset_distance: self.offset_distance,
            lengthen_delta: self.lengthen_delta,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::tests::roundtrip;

    #[test]
    fn the_settings_round_trip() {
        let mut document = Document::default();
        document.set_modify_settings(ModifySettings {
            fillet_radius: 2.5,
            chamfer_first: 1.0,
            chamfer_second: 2.0,
            chamfer_by_angle: true,
            trim: false,
            offset_distance: -3.0,
            lengthen_delta: Some(4.5),
            ..ModifySettings::default()
        });
        let loaded = roundtrip(&document);
        assert_eq!(loaded.modify_settings(), document.modify_settings());
    }

    #[test]
    fn a_file_without_them_gets_the_defaults_and_a_bad_value_is_repaired() {
        let from_old: ModifySettingsDto = serde_json::from_str("{}").unwrap();
        let mut document = Document::default();
        from_old.load(&mut document);
        assert_eq!(document.modify_settings(), ModifySettings::default());
        let bad: ModifySettingsDto =
            serde_json::from_str(r#"{"fillet_radius": -5, "chamfer_first": 2}"#).unwrap();
        bad.load(&mut document);
        assert_eq!(document.modify_settings().fillet_radius, 10.0);
        assert_eq!(document.modify_settings().chamfer_first, 2.0);
    }
}
