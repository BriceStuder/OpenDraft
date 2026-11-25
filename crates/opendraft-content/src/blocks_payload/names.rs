//! The names a package's drawing carries that the app shows or looks up
//! (specification 5.2, 7.1): plain text, and none that names a file. A font is
//! named, never located, and the app resolves it through its own font lookup.

use opendraft_core::text_style::FontDescriptor;
use opendraft_core::Document;

use crate::envelope::text::check_line;

const MAX_NAME: usize = 255;

/// A name that looks like a path, a drive or a URL.
fn looks_like_location(name: &str) -> bool {
    name.contains(['/', '\\', ':']) || name.starts_with('~') || name.contains("..")
}

fn check_name(what: &str, name: &str) -> Result<(), String> {
    check_line(name, MAX_NAME).map_err(|e| format!("{what} {name:?} {e}"))
}

fn check_not_location(what: &str, name: &str) -> Result<(), String> {
    check_name(what, name)?;
    if looks_like_location(name) {
        return Err(format!("{what} {name:?} looks like a path or an address"));
    }
    Ok(())
}

pub(super) fn check_shown_names(doc: &Document) -> Result<(), String> {
    for layer in doc.layers() {
        check_name("layer name", &layer.name)?;
    }
    for record in doc.linetypes().records() {
        check_not_location("linetype name", &record.name)?;
    }
    for style in doc.text_styles() {
        check_name("text style name", &style.name)?;
        match &style.font {
            FontDescriptor::TrueType { family, .. } => check_not_location("font", family)?,
            FontDescriptor::Shx { file_name } => check_not_location("font", file_name)?,
            _ => {}
        }
        if let Some(big) = &style.big_font {
            check_not_location("big font", big)?;
        }
    }
    Ok(())
}
