//! The plain-text rules (specification 5.1, 5.2): every string that is shown
//! has no control characters, no bidirectional override characters and is in
//! Unicode normal form NFC. Lengths count characters, not bytes.

use unicode_normalization::is_nfc;

/// Why a string is not acceptable as shown text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextError {
    Empty,
    TooLong { max: usize },
    Control,
    Bidi,
    NotNfc,
}

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextError::Empty => write!(f, "is empty"),
            TextError::TooLong { max } => write!(f, "is longer than {max} characters"),
            TextError::Control => write!(f, "has a control character"),
            TextError::Bidi => write!(f, "has a bidirectional override character"),
            TextError::NotNfc => write!(f, "is not in Unicode normal form NFC"),
        }
    }
}

/// Embeddings, overrides and isolates: what can make text read differently
/// from how it is stored.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Checks `text` as one line of shown text, of 1 to `max` characters.
pub fn check_line(text: &str, max: usize) -> Result<(), TextError> {
    check(text, max, false)
}

/// As `check_line`, but `\n` is allowed (paragraphs and lists).
pub fn check_paragraphs(text: &str, max: usize) -> Result<(), TextError> {
    check(text, max, true)
}

fn check(text: &str, max: usize, newlines: bool) -> Result<(), TextError> {
    if text.is_empty() {
        return Err(TextError::Empty);
    }
    let mut count = 0usize;
    for c in text.chars() {
        count += 1;
        if count > max {
            return Err(TextError::TooLong { max });
        }
        if c.is_control() && !(newlines && c == '\n') {
            return Err(TextError::Control);
        }
        if is_bidi_control(c) {
            return Err(TextError::Bidi);
        }
    }
    if !is_nfc(text) {
        return Err(TextError::NotNfc);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_passes() {
        assert_eq!(check_line("Piping valves, 50 mm", 40), Ok(()));
        assert_eq!(check_line("Café", 4), Ok(()));
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        assert_eq!(check_line("éééé", 4), Ok(()));
        assert_eq!(check_line("ééééé", 4), Err(TextError::TooLong { max: 4 }));
    }

    #[test]
    fn empty_control_and_bidi_are_refused() {
        assert_eq!(check_line("", 10), Err(TextError::Empty));
        assert_eq!(check_line("a\u{0}b", 10), Err(TextError::Control));
        assert_eq!(check_line("a\tb", 10), Err(TextError::Control));
        assert_eq!(check_line("a\nb", 10), Err(TextError::Control));
        assert_eq!(check_line("a\u{85}b", 10), Err(TextError::Control));
        for bidi in ['\u{202A}', '\u{202E}', '\u{2066}', '\u{2069}'] {
            assert_eq!(check_line(&format!("a{bidi}b"), 10), Err(TextError::Bidi));
        }
    }

    #[test]
    fn paragraphs_allow_newlines_only() {
        assert_eq!(check_paragraphs("one\n\n- two", 20), Ok(()));
        assert_eq!(check_paragraphs("a\rb", 20), Err(TextError::Control));
    }

    #[test]
    fn decomposed_text_is_not_nfc() {
        // "e" followed by a combining acute accent.
        assert_eq!(check_line("e\u{301}", 10), Err(TextError::NotNfc));
        assert_eq!(check_line("\u{e9}", 10), Ok(()));
    }
}
