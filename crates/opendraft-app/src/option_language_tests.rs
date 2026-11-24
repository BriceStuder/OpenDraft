use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use opendraft_commands::Keyword;

use super::*;

fn french(keywords: &[(&str, &str)]) -> Vec<CmdOption> {
    let keywords: Vec<Keyword> = keywords.iter().map(|(l, k)| Keyword::new(*l, *k)).collect();
    options_in(&keywords, "fr")
}

fn picked(options: &[CmdOption], text: &str) -> Option<String> {
    pick(options, text).map(|o| o.value.to_string())
}

#[test]
fn a_french_option_shows_its_shortcut_and_answers_in_english() {
    let options = french(&[("Arc", "A"), ("Length", "L"), ("Undo", "U"), ("Width", "W")]);
    let undo = &options[2];
    assert_eq!((&*undo.label, &*undo.key), ("annUler", "U"));
    assert_eq!(&*undo.value, "Undo");
    for typed in ["u", "U", "ann", "annuler"] {
        assert_eq!(picked(&options, typed).as_deref(), Some("Undo"), "{typed}");
    }
}

#[test]
fn a_default_option_is_shown_translated() {
    let spline = Keyword {
        context: Some("spline"),
        ..Keyword::new("Fit", "F")
    };
    let options = options_in(&[spline, Keyword::new("CV", "C")], "fr");
    assert_eq!(shown_default(&options, "Fit".into()), "Lissage");
    assert_eq!(shown_default(&options, "12.5".into()), "12.5");
}

#[test]
fn english_options_need_an_underscore_in_french() {
    let options = french(&[("Arc", "A"), ("Length", "L"), ("Undo", "U"), ("Width", "W")]);
    assert_eq!(picked(&options, "_U").as_deref(), Some("Undo"));
    assert_eq!(picked(&options, "_undo").as_deref(), Some("Undo"));
    assert_eq!(picked(&options, "_W").as_deref(), Some("Width"));
    assert_eq!(picked(&options, "_Wid").as_deref(), Some("Width"));
    // Without the underscore, only the French names count.
    assert_eq!(picked(&options, "W"), None);
    assert_eq!(picked(&options, "Undo"), None);
    assert_eq!(picked(&options, "_"), None);
}

#[test]
fn english_works_with_or_without_the_underscore_in_english() {
    let keywords = [Keyword::new("Undo", "U"), Keyword::new("eXit", "X")];
    let options = options_in(&keywords, "en");
    assert_eq!((&*options[1].label, &*options[1].key), ("eXit", "X"));
    for typed in ["U", "_U", "undo", "_Undo"] {
        assert_eq!(picked(&options, typed).as_deref(), Some("Undo"), "{typed}");
    }
}

#[test]
fn an_exact_shortcut_wins_over_the_start_of_a_label() {
    // "Longueur" (L) and "LArgeur" (LA): L is Length, LA is Width.
    let options = french(&[("Length", "L"), ("Width", "W")]);
    assert_eq!(picked(&options, "L").as_deref(), Some("Length"));
    assert_eq!(picked(&options, "la").as_deref(), Some("Width"));
    let options = french(&[("Width", "W"), ("Length", "L")]);
    assert_eq!(picked(&options, "L").as_deref(), Some("Length"));
}

#[test]
fn accents_need_not_be_typed() {
    let options = french(&[("Lengthen", "L"), ("Stretch", "S"), ("Knots", "K")]);
    assert_eq!(&*options[1].key, "É");
    assert_eq!(picked(&options, "e").as_deref(), Some("Stretch"));
    assert_eq!(picked(&options, "etirer").as_deref(), Some("Stretch"));
    assert_eq!(picked(&options, "noeuds").as_deref(), Some("Knots"));
}

#[test]
fn a_context_tells_apart_two_options_with_one_english_label() {
    let text = Keyword::new("Fit", "F");
    let spline = Keyword {
        context: Some("spline"),
        ..Keyword::new("Fit", "F")
    };
    let options = options_in(&[text, spline], "fr");
    assert_eq!(&*options[0].label, "Fixé");
    assert_eq!(&*options[1].label, "Lissage");
    assert_eq!(&*options[1].value, "Fit");
}

#[test]
fn an_untranslated_option_stays_as_written() {
    // A block's name is data, not an option to translate.
    let options = french(&[("Door frame", "Door frame")]);
    assert_eq!(&*options[0].label, "Door frame");
    assert_eq!(picked(&options, "door").as_deref(), Some("Door frame"));
}

#[test]
fn the_shortcut_is_the_first_run_of_capitals() {
    assert_eq!(shortcut("annUler"), "U");
    assert_eq!(shortcut("LArgeur"), "LA");
    assert_eq!(shortcut("2P"), "2P");
    assert_eq!(shortcut("?"), "?");
    assert_eq!(shortcut("point de Base"), "B");
    assert_eq!(shortcut("détection des Îlots"), "Î");
    assert_eq!(shortcut("tout"), "t");
}

/// Options offered together that the source scan cannot see as one prompt:
/// a table of options with more added around it.
const JOINED: &[&[&str]] = &[
    &["COUnt", "Spacing", "COLumns", "Rows", "eXit"],
    &[
        "Items",
        "Angle between",
        "Fill angle",
        "ROTate items",
        "eXit",
    ],
    &["Method", "Items", "Align items", "eXit"],
    &[
        "Blocks",
        "Dimstyles",
        "LAyers",
        "LTypes",
        "MUltileaderstyles",
        "textSTyles",
        "Zero-length geometry",
        "Empty text objects",
        "All",
        "Groups",
        "MAterials",
        "PLotstyles",
        "SHapes",
        "Mlinestyles",
        "Tablestyles",
        "Visualstyles",
        "Regapps",
        "detail View styles",
        "orphaned Data",
    ],
    &["Invisible", "Constant", "Verify", "Preset", "Lock position"],
    &[
        "Align", "Fit", "Left", "Center", "Middle", "Right", "TL", "TC", "TR", "ML", "MC", "MR",
        "BL", "BC", "BR",
    ],
    &["Text", "Angle", "Horizontal", "Vertical", "Rotated"],
    &[
        "Close",
        "Open",
        "Join",
        "Width",
        "Edit vertex",
        "Decurve",
        "Ltype gen",
        "Reverse",
        "Undo",
    ],
    &["Parameter", "Included angle"],
    &["Angle", "Included angle"],
    &["Move", "Stretch", "Scale", "Rotate", "Flip"],
];

/// In every language, no two options of one prompt share a shortcut.
#[test]
fn shortcuts_are_unique_in_each_prompt() {
    let mut prompts = scanned_prompts();
    prompts.extend(JOINED.iter().map(|labels| {
        let keywords = labels.iter().map(|l| Keyword::new(*l, "")).collect();
        ("JOINED".to_owned(), keywords)
    }));
    let properties = opendraft_core::block::ObjectProperty::ALL;
    let keywords = properties
        .iter()
        .map(|p| Keyword::new(p.name(), ""))
        .collect();
    prompts.push(("field properties".to_owned(), keywords));
    let backend = crate::_rust_i18n_backend();
    let mut clashes = Vec::new();
    for locale in backend.available_locales() {
        for (file, keywords) in &prompts {
            let mut seen: BTreeMap<String, &str> = BTreeMap::new();
            for (keyword, option) in keywords.iter().zip(options_in(keywords, &locale)) {
                if translated(keyword, &locale).is_none() {
                    continue; // English: the shortcuts written in the code
                }
                let key = fold(&option.key).into_owned();
                match seen.get(&key) {
                    Some(other) if *other != keyword.label => clashes.push(format!(
                        "{locale} {file}: {other} and {} are both {key}",
                        keyword.label
                    )),
                    _ => {
                        seen.insert(key, &keyword.label);
                    }
                }
            }
        }
    }
    assert!(clashes.is_empty(), "{clashes:#?}");
}

/// Every option the code offers is translated into French.
#[test]
fn every_option_is_translated_into_french() {
    let mut missing: BTreeSet<String> = scanned_prompts()
        .iter()
        .flat_map(|(_, keywords)| keywords.iter())
        .filter(|k| translated(k, "fr").is_none())
        .map(translation_key)
        .collect();
    missing.retain(|k| !NOT_OPTIONS.contains(&k.as_str()));
    assert!(missing.is_empty(), "no French: {missing:#?}");
}

/// Pairs the scan reads as options that are something else.
const NOT_OPTIONS: &[&str] = &[];

/// Every option translation is for an option the code still offers.
#[test]
fn every_option_translation_is_still_offered() {
    let mut offered: BTreeSet<String> = scanned_prompts()
        .iter()
        .flat_map(|(_, keywords)| keywords.iter().map(translation_key))
        .collect();
    let properties = opendraft_core::block::ObjectProperty::ALL;
    offered.extend(properties.iter().map(|p| format!("[{}]", p.name())));
    offered.extend(
        JOINED
            .iter()
            .flat_map(|labels| labels.iter().map(|l| format!("[{l}]"))),
    );
    // Offered through a helper the scan cannot follow (`plot_line::text_or`).
    offered.extend(["[plot: Fit]".to_owned(), "[plot: Center]".to_owned()]);
    let backend = crate::_rust_i18n_backend();
    let mut stale = Vec::new();
    for locale in backend.available_locales() {
        for (key, _) in backend.messages_for_locale(&locale).unwrap_or_default() {
            if key.starts_with('[') && !offered.contains(key.as_ref()) {
                stale.push(format!("{locale}: {key:?}"));
            }
        }
    }
    assert!(stale.is_empty(), "no longer offered: {stale:#?}");
}

#[test]
fn the_scan_finds_chains_and_tables() {
    let source = r#"
        const NAMED: &str = "Named";
        let o = Options::new("a").keyword("Undo", "U").keyword(NAMED, "N");
        let t = [("Arc", "A"), ("Close", "C")];
        let s = o.keyword_in("spline", "Fit", "F");
    "#;
    let found: Vec<Vec<String>> = prompts_in(source)
        .into_iter()
        .map(|k| k.iter().map(translation_key).collect())
        .collect();
    assert_eq!(
        found,
        [
            vec!["[Undo]", "[Named]"],
            vec!["[Arc]", "[Close]"],
            vec!["[spline: Fit]"],
        ]
    );
}

/// The options of the app's prompts, read from its sources: the
/// `.keyword(…)` calls of each statement, and each table of
/// `("Label", "KEY")` pairs.
fn scanned_prompts() -> Vec<(String, Vec<Keyword>)> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut folders = vec![folder];
    let mut prompts = Vec::new();
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                folders.push(path);
            } else if name.ends_with(".rs") && !name.contains("tests") && name != "cmd_aliases.rs" {
                let source = std::fs::read_to_string(&path).unwrap();
                let source = source.split("#[cfg(test)]").next().unwrap_or_default();
                prompts.extend(prompts_in(source).into_iter().map(|k| (name.clone(), k)));
            }
        }
    }
    prompts
}

fn prompts_in(source: &str) -> Vec<Vec<Keyword>> {
    let constants = constants(source);
    let mut prompts = Vec::new();
    for statement in source.split(';') {
        let mut chain = Vec::new();
        for (at, _) in statement.match_indices(".keyword") {
            let rest = &statement[at + ".keyword".len()..];
            let (context, rest) = match rest.strip_prefix("_in(") {
                Some(rest) => match first_literal(rest) {
                    Some((context, rest)) => (Some(context), rest.trim_start_matches([',', ' '])),
                    None => continue,
                },
                None => match rest.strip_prefix('(') {
                    Some(rest) => (None, rest),
                    None => continue,
                },
            };
            let label = first_literal(rest).map(|(l, _)| l).or_else(|| {
                let name: String = rest
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                constants.get(&name).cloned()
            });
            if let Some(label) = label {
                let context = context.map(|c| &*Box::leak(c.into_boxed_str()));
                chain.push(Keyword {
                    context,
                    ..Keyword::new(label, "")
                });
            }
        }
        if !chain.is_empty() {
            prompts.push(chain);
        }
        for table in statement.split('[').skip(1) {
            let table = table.split(']').next().unwrap_or_default();
            let pairs: Vec<Keyword> = table
                .split("(\"")
                .skip(1)
                .filter_map(|pair| {
                    let (label, rest) = pair.split_once('"')?;
                    let key = rest.trim_start().strip_prefix(',')?.trim_start();
                    let key = key.strip_prefix('"')?.split('"').next()?;
                    // Command ids ("draw.line") are not options.
                    let short = key.len() <= 4
                        && key.chars().all(|c| c.is_alphanumeric() || c == '?')
                        && !label.contains('.');
                    short.then(|| Keyword::new(label, key))
                })
                .collect();
            if !pairs.is_empty() {
                prompts.push(pairs);
            }
        }
    }
    prompts
}

/// `const NAME: &str = "text";` (or `key!("text")`) in `source`.
fn constants(source: &str) -> BTreeMap<String, String> {
    source
        .split("const ")
        .skip(1)
        .filter_map(|rest| {
            let (name, rest) = rest.split_once(": &str = ")?;
            let rest = rest.strip_prefix("key!(").unwrap_or(rest);
            Some((name.to_owned(), first_literal(rest)?.0))
        })
        .collect()
}

/// The string literal at the start of `rest` (after `key!(` if marked), and
/// what follows it. Options are plain literals, without escapes.
fn first_literal(rest: &str) -> Option<(String, &str)> {
    let rest = rest.trim_start();
    let rest = rest.strip_prefix("key!(").unwrap_or(rest);
    let rest = rest.strip_prefix('"')?;
    let (text, after) = rest.split_once('"')?;
    Some((text.to_owned(), after.trim_start_matches(')')))
}
