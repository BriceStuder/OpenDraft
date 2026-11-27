//! What a DXF header sets on the drawing when it is read.

use opendraft_core::Document;

/// Apply the header's groups (code, value) to `document`: its units, linetype
/// scale, point style and the Model tab's coordinate system. Returns whether
/// that system was 3D or mirrored and so left as World.
pub(super) fn apply<'a>(
    groups: impl Iterator<Item = (i32, &'a str)> + Clone,
    document: &mut Document,
) -> bool {
    document.set_units(super::units::read_header(groups.clone()));
    super::linetypes::apply_header(groups.clone(), document);
    super::point_style::apply_header(groups.clone(), document);
    super::modify_settings::apply_header(groups.clone(), document);
    super::ccs::apply_header(groups, document)
}
