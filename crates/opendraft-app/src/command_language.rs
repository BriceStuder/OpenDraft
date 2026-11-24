//! Command names in the language of the interface (issue #138, step 6).
//!
//! The commands keep their English names and aliases ([`ALIASES`]), and the
//! app runs them by id whatever the language. A translation gives each
//! command its own name and aliases, as AutoCAD does in French: `LIGNE` or
//! `L` runs LINE. Typed without `_`, only the names of the interface's
//! language work; the English ones, with `_` in front (`_LINE`, `_L`,
//! `_-PLOT` or `-_PLOT`), always work, so scripts, macros and English habits
//! do not depend on the language. A leading `'` (AutoCAD's transparent
//! `'ZOOM`) is accepted and changes nothing: a command that may run inside
//! another one does so anyway.
//!
//! A command's translation is under `$` and its English name ("$LINE"):
//! its name, then its aliases, separated by commas ("LIGNE, L"). A command
//! with no translation keeps its English name and aliases.

use std::borrow::Cow;

use crate::cmdline::{Suggestion, ALIASES};
use crate::option_language::fold;

/// The key a command's names are translated by in `locales/`: `$` and its
/// English name ("$LINE").
pub fn translation_key(name: &str) -> String {
    format!("${name}")
}

/// Each command once, in the order of [`ALIASES`], with the English name it
/// is echoed by.
pub fn commands() -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for &(_, name, command) in ALIASES {
        if !out.iter().any(|(id, _)| *id == command) {
            out.push((command, name));
        }
    }
    out
}

/// A command's name and aliases in `locale`, or `None` when it has none.
pub fn translated(name: &str, locale: &str) -> Option<Vec<String>> {
    let key = translation_key(name);
    let shown = rust_i18n::t!(&key, locale = locale);
    if shown == key {
        return None;
    }
    let names: Vec<String> = shown
        .split(',')
        .map(|n| n.trim().to_uppercase())
        .filter(|n| !n.is_empty())
        .collect();
    (!names.is_empty()).then_some(names)
}

/// Every name and alias that runs a command in English (typed after `_`).
pub fn english_names() -> Vec<Suggestion> {
    ALIASES
        .iter()
        .map(|&(alias, name, command)| Suggestion {
            alias: Cow::Borrowed(alias),
            name: Cow::Borrowed(name),
            command,
        })
        .collect()
}

/// Every name and alias that runs a command typed without `_` in `locale`:
/// the translated ones, and the English ones of a command not translated.
pub fn local_names(locale: &str) -> Vec<Suggestion> {
    let mut out = Vec::new();
    for (command, english) in commands() {
        match translated(english, locale) {
            Some(names) => {
                let name: Cow<'static, str> = Cow::Owned(names[0].clone());
                for typed in &names {
                    out.push(Suggestion {
                        alias: Cow::Owned(typed.clone()),
                        name: name.clone(),
                        command,
                    });
                }
            }
            None => out.extend(english_names().into_iter().filter(|s| s.command == command)),
        }
    }
    out
}

/// What was typed, read as a command name: whether it is English (`_` in
/// front), and the name itself, upper case and without accents. `'` in
/// front is dropped, and `-_PLOT` reads as `_-PLOT`.
pub fn read(text: &str) -> (bool, String) {
    let text = text.trim();
    let text = text.strip_prefix('\'').unwrap_or(text);
    let (english, name) = if let Some(rest) = text.strip_prefix('_') {
        (true, Cow::Borrowed(rest))
    } else if let Some(rest) = text.strip_prefix("-_") {
        (true, Cow::Owned(format!("-{rest}")))
    } else {
        (false, Cow::Borrowed(text))
    };
    (english, fold(&name).into_owned())
}

/// The names `text` is matched against in `locale`, with the name read
/// from it.
pub fn names_for(text: &str, locale: &str) -> (Vec<Suggestion>, String, bool) {
    let (english, typed) = read(text);
    let names = if english {
        english_names()
    } else {
        local_names(locale)
    };
    (names, typed, english)
}

/// The command a fully typed name or alias runs in `locale`.
pub fn command_for(text: &str, locale: &str) -> Option<&'static str> {
    let (names, typed, _) = names_for(text, locale);
    if typed.is_empty() {
        return None;
    }
    names
        .iter()
        .find(|s| fold(&s.alias) == typed || fold(&s.name) == typed)
        .map(|s| s.command)
}

/// The name a command is echoed by in `locale`: its translated name, else
/// its English one (`english`, from [`crate::cmdline::echo_name`]).
pub fn shown_name<'a>(command: &str, english: &'a str, locale: &str) -> Cow<'a, str> {
    let name = commands()
        .into_iter()
        .find(|(id, _)| *id == command)
        .map(|(_, name)| name);
    match name.and_then(|n| translated(n, locale)) {
        Some(mut names) => Cow::Owned(names.swap_remove(0)),
        None => Cow::Borrowed(english),
    }
}

/// The running command's name as shown before its prompts ("LIGNE"). The
/// command is named in English by the code that starts it ("Line",
/// "QSELECT"); a name that is no command's is shown as it is.
pub fn prompt_name(runner: &str, locale: &str) -> String {
    let upper = runner.to_uppercase();
    let command = ALIASES
        .iter()
        .find(|(alias, name, _)| *name == upper || *alias == upper)
        .map(|(_, _, command)| *command);
    match command {
        Some(command) => shown_name(command, &upper, locale).into_owned(),
        None => upper,
    }
}

#[cfg(test)]
#[path = "command_language_tests.rs"]
mod tests;
