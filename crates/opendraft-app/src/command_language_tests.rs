use std::collections::BTreeMap;

use super::*;
use crate::cmdline::suggestions_in;

/// The languages with a translation file, and English.
fn locales() -> Vec<String> {
    let backend = crate::_rust_i18n_backend();
    let mut locales: Vec<String> = backend
        .available_locales()
        .into_iter()
        .map(|l| l.to_string())
        .collect();
    if !locales.iter().any(|l| l == "en") {
        locales.push("en".to_owned());
    }
    locales
}

#[test]
fn english_names_with_an_underscore_work_in_every_language() {
    for locale in locales() {
        for (typed, id) in [
            ("_LINE", "draw.line"),
            ("_l", "draw.line"),
            ("_-PLOT", "file.plot_line"),
            ("-_PLOT", "file.plot_line"),
            ("'_ZOOM", "view.zoom"),
            ("_CIRCLE TTR", "draw.circle_ttr"),
        ] {
            assert_eq!(command_for(typed, &locale), Some(id), "{locale}: {typed}");
        }
        assert_eq!(command_for("_", &locale), None);
        assert_eq!(command_for("_NOSUCH", &locale), None);
    }
}

#[test]
fn english_names_without_an_underscore_work_in_english() {
    for (typed, id) in [
        ("LINE", "draw.line"),
        ("l", "draw.line"),
        ("'Z", "view.zoom"),
    ] {
        assert_eq!(command_for(typed, "en"), Some(id), "{typed}");
    }
}

#[test]
fn french_names_and_aliases_work_and_english_needs_an_underscore() {
    for (typed, id) in [
        ("LIGNE", "draw.line"),
        ("ligne", "draw.line"),
        ("L", "draw.line"),
        ("DÉPLACER", "modify.move"),
        ("deplacer", "modify.move"),
        ("-TRACEUR", "file.plot_line"),
        ("'Z", "view.zoom"),
    ] {
        assert_eq!(command_for(typed, "fr"), Some(id), "{typed}");
    }
    assert_eq!(command_for("LINE", "fr"), None, "English only after _");
    assert_eq!(command_for("MOVE", "fr"), None, "English only after _");
}

#[test]
fn no_two_commands_share_a_name_in_any_language() {
    let mut sets = vec![("English after _".to_owned(), english_names())];
    sets.extend(locales().into_iter().map(|l| (l.clone(), local_names(&l))));
    for (language, names) in sets {
        let mut seen: BTreeMap<String, &str> = BTreeMap::new();
        let mut clashes = Vec::new();
        for s in &names {
            for typed in [&s.alias, &s.name] {
                let typed = fold(typed).into_owned();
                match seen.get(&typed) {
                    Some(other) if *other != s.command => {
                        clashes.push(format!("{typed}: {other} and {}", s.command));
                    }
                    _ => {
                        seen.insert(typed, s.command);
                    }
                }
            }
        }
        clashes.sort();
        clashes.dedup();
        assert!(clashes.is_empty(), "{language}: {clashes:#?}");
    }
}

#[test]
fn every_command_is_translated() {
    for locale in locales().into_iter().filter(|l| l != "en") {
        let missing: Vec<&str> = commands()
            .into_iter()
            .filter(|(_, name)| translated(name, &locale).is_none())
            .map(|(_, name)| name)
            .collect();
        assert!(missing.is_empty(), "{locale}: {missing:#?}");
    }
}

#[test]
fn every_command_translation_is_a_command() {
    let names: Vec<String> = commands()
        .into_iter()
        .map(|(_, name)| translation_key(name))
        .collect();
    let backend = crate::_rust_i18n_backend();
    let mut stale = Vec::new();
    for locale in backend.available_locales() {
        for (key, _) in backend.messages_for_locale(&locale).unwrap_or_default() {
            if key.starts_with('$') && !names.iter().any(|n| *n == *key) {
                stale.push(format!("{locale}: {key}"));
            }
        }
    }
    assert!(stale.is_empty(), "no such command: {stale:#?}");
}

#[test]
fn commands_are_echoed_and_prompted_in_the_language() {
    assert_eq!(shown_name("draw.line", "LINE", "fr"), "LIGNE");
    assert_eq!(shown_name("draw.line", "LINE", "en"), "LINE");
    assert_eq!(prompt_name("Line", "fr"), "LIGNE");
    assert_eq!(prompt_name("Line", "en"), "LINE");
    assert_eq!(prompt_name("Pick", "fr"), "PICK", "not a command");
}

#[test]
fn suggestions_follow_the_language() {
    let all = |_: &str| true;
    let french = suggestions_in("LIGN", all, "fr");
    assert_eq!(french[0].command, "draw.line");
    assert_eq!(french[0].name, "LIGNE");
    assert!(suggestions_in("LINE", all, "fr")
        .iter()
        .all(|s| s.command != "draw.line"));
    let english = suggestions_in("_LINE", all, "fr");
    assert_eq!(english[0].command, "draw.line");
    assert_eq!(english[0].label(), "_L (_LINE)");
    assert!(suggestions_in("_", all, "fr").is_empty());
}
