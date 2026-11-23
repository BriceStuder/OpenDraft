//! Showing texts in the language of the interface (issue #138).
//!
//! A text written in the code is wrapped in `rust_i18n::t!("…")`. A text
//! that is also data, compared or stored in English (a table of menu rows,
//! a property's name, an entity kind's title), stays English where it is
//! defined, marked with [`key!`], and is translated with [`tr`] where it is
//! drawn.

use std::borrow::Cow;

/// Marks an English text that is translated where it is drawn, with
/// [`tr`]: it gives the text unchanged, and the translation test reads it
/// as a key that is still in use.
#[macro_export]
macro_rules! key {
    ($text:literal) => {
        $text
    };
}

/// `text` in the language of the interface, on one line: a translation may
/// break a long ribbon label with `\n`, which is shown as a space here.
pub fn tr(text: &str) -> Cow<'_, str> {
    let shown = rust_i18n::t!(text);
    if shown.contains('\n') {
        Cow::Owned(shown.replace('\n', " "))
    } else {
        shown
    }
}
