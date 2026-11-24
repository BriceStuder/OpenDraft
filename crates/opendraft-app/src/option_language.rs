//! A prompt's options in the language of the interface (issue #138, step 5).
//!
//! The command offers its options in English and is answered with the
//! English label, whatever the language. In French each option is shown
//! translated, with its shortcut written in capitals in the word, as AutoCAD
//! writes it ("annUler"), and typing the French shortcut or the start of the
//! French word picks it. The English option, with `_` in front (`_Undo`,
//! `_U`), always works, so scripts, macros and outside callers do not depend
//! on the language.

use std::borrow::Cow;

use opendraft_commands::Keyword;

use crate::cmdline::CmdOption;

/// The options of a prompt as they are shown, in the language of the
/// interface.
pub fn options(keywords: &[Keyword]) -> Vec<CmdOption> {
    options_in(keywords, &rust_i18n::locale())
}

pub(crate) fn options_in(keywords: &[Keyword], locale: &str) -> Vec<CmdOption> {
    keywords.iter().map(|k| option_in(k, locale)).collect()
}

fn option_in(keyword: &Keyword, locale: &str) -> CmdOption {
    let (label, key) = match translated(keyword, locale) {
        Some(label) => {
            let key = shortcut(&label);
            (label, key)
        }
        None => (keyword.label.clone(), keyword.key.clone()),
    };
    CmdOption {
        label: label.into(),
        key: key.into(),
        value: keyword.label.clone().into(),
        english_key: keyword.key.clone().into(),
    }
}

/// A prompt's default as shown: an option's label in the language of the
/// interface (SPLINE's `<Fit>` reads `<Lissage>`), anything else as it is.
pub fn shown_default(options: &[CmdOption], default: String) -> String {
    match options.iter().find(|o| o.value == default) {
        Some(option) => option.label.to_string(),
        None => default,
    }
}

/// The key an option is translated by in `locales/`: its English label in
/// square brackets, after its context when it has one ("[spline: Fit]").
pub fn translation_key(keyword: &Keyword) -> String {
    match keyword.context {
        Some(context) => format!("[{context}: {}]", keyword.label),
        None => format!("[{}]", keyword.label),
    }
}

fn translated(keyword: &Keyword, locale: &str) -> Option<String> {
    let key = translation_key(keyword);
    let shown = rust_i18n::t!(&key, locale = locale);
    (shown != key).then(|| shown.into_owned())
}

/// The shortcut written in a translated option: its first run of capitals
/// ("annUler" gives "U", "LArgeur" "LA", "2P" "2P"), else its first letter.
pub fn shortcut(label: &str) -> String {
    let is_key = |c: char| c.is_uppercase() || c.is_ascii_digit() || c == '?';
    let run: String = label
        .chars()
        .skip_while(|c| !is_key(*c))
        .take_while(|c| is_key(*c))
        .collect();
    if run.is_empty() {
        label.chars().take(1).collect()
    } else {
        run
    }
}

/// The option `text` picks: with `_` in front, by its English shortcut or
/// the start of its English label; otherwise by the shortcut or label shown.
/// An exact shortcut wins over the start of a label ("L" is Length even when
/// "LArgeur" is offered too).
pub fn pick<'a>(options: &'a [CmdOption], text: &str) -> Option<&'a CmdOption> {
    let text = text.trim();
    let (english, typed) = match text.strip_prefix('_') {
        Some(rest) => (true, fold(rest)),
        None => (false, fold(text)),
    };
    if typed.is_empty() {
        return None;
    }
    let names = |o: &'a CmdOption| -> (&'a str, &'a str) {
        if english {
            (&o.english_key, &o.value)
        } else {
            (&o.key, &o.label)
        }
    };
    options
        .iter()
        .find(|o| fold(names(o).0) == typed)
        .or_else(|| {
            options
                .iter()
                .find(|o| fold(names(o).1).starts_with(typed.as_ref()))
        })
}

/// Upper case without accents, so "e" picks "Étirer" and "noeuds" "Nœuds".
fn fold(text: &str) -> Cow<'_, str> {
    if text
        .chars()
        .all(|c| c.is_ascii_uppercase() || !c.is_alphabetic())
    {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_uppercase) {
        match c {
            'À' | 'Â' | 'Ä' => out.push('A'),
            'É' | 'È' | 'Ê' | 'Ë' => out.push('E'),
            'Î' | 'Ï' => out.push('I'),
            'Ô' | 'Ö' => out.push('O'),
            'Ù' | 'Û' | 'Ü' => out.push('U'),
            'Ç' => out.push('C'),
            'Œ' => out.push_str("OE"),
            'Æ' => out.push_str("AE"),
            other => out.push(other),
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
#[path = "option_language_tests.rs"]
mod tests;
