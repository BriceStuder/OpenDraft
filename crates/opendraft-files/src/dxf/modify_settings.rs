//! What FILLET and CHAMFER remember, in a DXF header: `$FILLETRAD`,
//! `$CHAMFERA` to `$CHAMFERD` (the angle in degrees) and `$TRIMMODE`. DXF has
//! no header variable for `CHAMMODE`, `OFFSETDIST` or LENGTHEN's delta, so
//! those stay in `.odraft` only.

use opendraft_core::{Document, ModifySettings};

use super::write::Out;

/// The header variables to write, as (name, group code, value).
pub(super) fn header(s: ModifySettings) -> [(&'static str, i32, String); 6] {
    [
        ("$FILLETRAD", 40, s.fillet_radius.to_string()),
        ("$CHAMFERA", 40, s.chamfer_first.to_string()),
        ("$CHAMFERB", 40, s.chamfer_second.to_string()),
        ("$CHAMFERC", 40, s.chamfer_length.to_string()),
        ("$CHAMFERD", 40, s.chamfer_angle.to_string()),
        ("$TRIMMODE", 70, i32::from(s.trim).to_string()),
    ]
}

/// Write the variables into a header.
pub(super) fn write_header(out: &mut Out, document: &Document) {
    for (name, code, value) in header(document.modify_settings()) {
        out.pair(9, name);
        out.pair(code, value);
    }
}

/// Set what the header holds from its groups (code, value); what is missing
/// or cannot be used keeps the default, and what DXF cannot hold is left
/// as it is.
pub(super) fn apply_header<'a>(
    groups: impl Iterator<Item = (i32, &'a str)>,
    document: &mut Document,
) {
    let mut s = ModifySettings::default();
    let mut variable = "";
    for (code, value) in groups {
        if code == 9 {
            variable = value;
            continue;
        }
        let number = value.parse::<f64>().ok();
        match (variable, code, number) {
            ("$FILLETRAD", 40, Some(n)) => s.fillet_radius = n,
            ("$CHAMFERA", 40, Some(n)) => s.chamfer_first = n,
            ("$CHAMFERB", 40, Some(n)) => s.chamfer_second = n,
            ("$CHAMFERC", 40, Some(n)) => s.chamfer_length = n,
            ("$CHAMFERD", 40, Some(n)) => s.chamfer_angle = n,
            ("$TRIMMODE", 70, Some(n)) => s.trim = n != 0.0,
            _ => {}
        }
    }
    document.set_modify_settings(s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_sets_the_settings_and_repairs_what_cannot_be_used() {
        let mut doc = Document::default();
        let groups = [
            (9, "$FILLETRAD"),
            (40, "2.5"),
            (9, "$CHAMFERA"),
            (40, "-1"),
            (9, "$CHAMFERB"),
            (40, "4"),
            (9, "$CHAMFERD"),
            (40, "30"),
            (9, "$TRIMMODE"),
            (70, "0"),
        ];
        apply_header(groups.into_iter(), &mut doc);
        let s = doc.modify_settings();
        assert_eq!(
            (s.fillet_radius, s.chamfer_second, s.chamfer_angle),
            (2.5, 4.0, 30.0)
        );
        assert_eq!(s.chamfer_first, 10.0, "a negative distance is repaired");
        assert!(!s.trim);
    }

    #[test]
    fn a_header_without_them_leaves_the_defaults() {
        let mut doc = Document::default();
        apply_header([(9, "$LTSCALE"), (40, "2")].into_iter(), &mut doc);
        assert_eq!(doc.modify_settings(), ModifySettings::default());
    }

    fn changed() -> Document {
        let mut doc = Document::default();
        doc.set_modify_settings(ModifySettings {
            fillet_radius: 2.5,
            chamfer_first: 1.5,
            chamfer_second: 3.0,
            chamfer_length: 4.0,
            chamfer_angle: 30.0,
            trim: false,
            // DXF has no variable for these two.
            chamfer_by_angle: true,
            offset_distance: -2.0,
            ..ModifySettings::default()
        });
        doc
    }

    #[test]
    fn both_dxf_writers_keep_what_dxf_can_hold() {
        for bytes in [
            super::super::write::write(&changed()).unwrap().bytes,
            super::super::write2000::write(&changed()).unwrap().bytes,
        ] {
            let back = super::super::read::read(&bytes).unwrap().document;
            let s = back.modify_settings();
            assert_eq!(
                (s.fillet_radius, s.chamfer_first, s.chamfer_second),
                (2.5, 1.5, 3.0)
            );
            assert_eq!(
                (s.chamfer_length, s.chamfer_angle, s.trim),
                (4.0, 30.0, false)
            );
            // What DXF cannot hold comes back as the default.
            assert!(!s.chamfer_by_angle);
            assert_eq!(s.offset_distance, 1.0);
        }
    }
}
