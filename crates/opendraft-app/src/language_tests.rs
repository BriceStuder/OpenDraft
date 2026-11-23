use std::collections::BTreeSet;
use std::path::Path;

use opendraft_ui::ribbon::{Cell, Column, Item, Ribbon, Split};

use super::*;
use crate::ribbon_def::{ribbon_for, RibbonContext};

#[test]
fn a_system_tag_gives_its_language_when_offered() {
    assert_eq!(from_tag(Some("fr-FR")), "fr");
    assert_eq!(from_tag(Some("fr_CA.UTF-8")), "fr");
    assert_eq!(from_tag(Some("FR")), "fr");
    assert_eq!(from_tag(Some("en-GB")), ENGLISH);
    assert_eq!(from_tag(Some("de-DE")), ENGLISH);
    assert_eq!(from_tag(Some("")), ENGLISH);
    assert_eq!(from_tag(None), ENGLISH);
}

#[test]
fn a_saved_choice_wins_over_the_system() {
    assert_eq!(resolve(Some("fr")), "fr");
    assert_eq!(resolve(Some("en")), ENGLISH);
    assert_eq!(resolve(Some("xx")), ENGLISH);
    assert!(is_offered("fr"));
    assert!(!is_offered("xx"));
    assert_eq!(name("fr"), "Français");
}

#[test]
fn french_translates_and_a_missing_text_stays_english() {
    assert_eq!(rust_i18n::t!("Language", locale = "fr"), "Langue");
    assert_eq!(rust_i18n::t!("Language", locale = "en"), "Language");
    assert_eq!(
        rust_i18n::t!(
            "Same as the system (%{language})",
            locale = "fr",
            language = "Français"
        ),
        "Same as the system (Français)"
    );
}

/// Every key of every translation file is still written somewhere in the
/// code: a text that was reworded or removed leaves a stale translation,
/// which this lists so it can be updated or deleted.
#[test]
fn every_translated_text_is_still_in_the_code() {
    let mut used = texts_in_code();
    used.extend(ribbon_texts().into_iter().map(str::to_owned));
    used.extend(core_labels().into_iter().map(str::to_owned));
    let backend = crate::_rust_i18n_backend();
    let mut stale = Vec::new();
    for locale in backend.available_locales() {
        for (key, _) in backend.messages_for_locale(&locale).unwrap_or_default() {
            if !used.contains(key.as_ref()) {
                stale.push(format!("{locale}: {key:?}"));
            }
        }
    }
    assert!(
        stale.is_empty(),
        "not in any t!(\"…\") or key!(\"…\") of the code: {stale:#?}"
    );
}

/// A translation keeps the `%{name}` placeholders of its English text.
#[test]
fn translations_keep_their_placeholders() {
    let backend = crate::_rust_i18n_backend();
    let mut wrong = Vec::new();
    for locale in backend.available_locales() {
        for (key, value) in backend.messages_for_locale(&locale).unwrap_or_default() {
            if placeholders(&key) != placeholders(&value) {
                wrong.push(format!("{locale}: {key:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "placeholders differ: {wrong:#?}");
}

#[test]
fn the_code_scanner_reads_escapes() {
    let mut found = BTreeSet::new();
    literals_after_t(
        r#"t!("A \"b\"\n") + t!( "c" , x = 1) + not!("d") + key!("e") + monkey!("f")"#,
        &mut found,
    );
    assert_eq!(
        found,
        BTreeSet::from(["A \"b\"\n".to_owned(), "c".to_owned(), "e".to_owned()])
    );
}

/// Labels from `opendraft-core`, which has no translations: the app
/// translates them where it draws them (Properties).
fn core_labels() -> BTreeSet<&'static str> {
    let mut labels: BTreeSet<&'static str> = opendraft_core::EntityKind::ALL
        .iter()
        .map(|k| k.title())
        .collect();
    labels.extend(
        (0..3).filter_map(|code| {
            opendraft_core::hatch::HatchStyle::from_code(code).map(|s| s.label())
        }),
    );
    labels.extend(
        opendraft_core::mleader::MLeaderLineType::ALL
            .iter()
            .map(|t| t.label()),
    );
    labels.extend(
        opendraft_core::dimension::Arrow::ALL
            .iter()
            .map(|a| a.label()),
    );
    labels
}

fn placeholders(text: &str) -> BTreeSet<&str> {
    text.split("%{")
        .skip(1)
        .filter_map(|rest| rest.split_once('}').map(|(name, _)| name))
        .collect()
}

/// The ribbon translates its texts where it draws them, so its labels are
/// keys without being written in a `t!`.
#[test]
fn the_ribbon_texts_include_every_tab_panel_and_button() {
    let texts = ribbon_texts();
    for text in ["Home", "Draw", "Line", "Layers", "Block Editor", "Layout"] {
        assert!(texts.contains(text), "{text}");
    }
    assert!(!texts.contains(""));
}

/// Every text the ribbon draws, in each of its contexts: tab, panel and
/// button labels (a button's tooltip is its label).
fn ribbon_texts() -> BTreeSet<&'static str> {
    let mut texts = BTreeSet::new();
    let contexts = [
        RibbonContext::Normal,
        RibbonContext::Layout,
        RibbonContext::BlockEditor,
    ];
    for context in contexts {
        add_ribbon(&ribbon_for(context), &mut texts);
    }
    texts
}

fn add_ribbon(ribbon: &Ribbon, texts: &mut BTreeSet<&'static str>) {
    let item = |i: &Item, texts: &mut BTreeSet<&'static str>| {
        texts.insert(i.label);
    };
    let split = |s: &Split, texts: &mut BTreeSet<&'static str>| {
        texts.extend(s.label);
        s.items.iter().for_each(|i| item(i, texts));
    };
    for tab in &ribbon.tabs {
        texts.insert(tab.title);
        for panel in &tab.panels {
            texts.insert(panel.title);
            panel.extra.iter().for_each(|i| item(i, texts));
            for column in &panel.columns {
                match column {
                    Column::Large(i) => item(i, texts),
                    Column::LargeSplit(s) => split(s, texts),
                    Column::Stack(items) => items.iter().for_each(|i| item(i, texts)),
                    Column::Rows(rows) => {
                        for cell in rows.iter().flatten() {
                            match cell {
                                Cell::Button(i) | Cell::Icon(i) => item(i, texts),
                                Cell::Split(s) | Cell::IconSplit(s) => split(s, texts),
                                Cell::Slot { .. } => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The first argument of every `t!("…")` and `key!("…")` in the program's
/// sources.
fn texts_in_code() -> BTreeSet<String> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = BTreeSet::new();
    let mut folders = vec![crates];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                literals_after_t(&std::fs::read_to_string(&path).unwrap(), &mut found);
            }
        }
    }
    found
}

/// Adds the string literal that opens each `t!(` and `key!(` call of
/// `source` (`key!` marks a text translated where it is drawn).
fn literals_after_t(source: &str, found: &mut BTreeSet<String>) {
    for call in ["t!(", "key!("] {
        for (at, _) in source.match_indices(call) {
            let before = source[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue; // `not!(`, `print!(`, `monkey!(`…
            }
            let rest = source[at + call.len()..].trim_start();
            if let Some(text) = rest.strip_prefix('"').and_then(unescape) {
                found.insert(text);
            }
        }
    }
}

/// The text of a string literal whose opening quote is already read.
fn unescape(rest: &str) -> Option<String> {
    let mut text = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(text),
            '\\' => match chars.next()? {
                'n' => text.push('\n'),
                't' => text.push('\t'),
                '\n' => {
                    // A line continuation drops the next line's indent.
                    let rest = chars.as_str().trim_start();
                    chars = rest.chars();
                }
                other => text.push(other),
            },
            c => text.push(c),
        }
    }
    None
}
