//! -LINETYPE: the command-line form of LINETYPE. `Load` asks which linetypes
//! (names, comma separated, with `*` and `?` wildcards) and which `.lin` file,
//! and loads the matches; a name the drawing already has is loaded again only
//! if the answer is Yes. `?` lists what a `.lin` file holds. `Create` and
//! `Set` are not built.

use opendraft_commands::{Answer, Cancelled, Command, Editor, Options, SetLinetypes};
use opendraft_core::linetype::lin::{self, LinEntry};
use opendraft_ui::key;

use crate::power_select::wildcard_match;
use crate::state::AppState;
use crate::text::plural;

/// The entries whose names match one of the comma-separated `patterns`, in
/// file order and each once, and the patterns that matched nothing.
fn select<'a>(entries: &'a [LinEntry], patterns: &str) -> (Vec<&'a LinEntry>, Vec<String>) {
    let (mut chosen, mut unmatched): (Vec<&LinEntry>, Vec<String>) = (Vec::new(), Vec::new());
    for pattern in patterns.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let mut any = false;
        for entry in entries.iter().filter(|e| wildcard_match(pattern, &e.name)) {
            any = true;
            if !chosen.iter().any(|c| std::ptr::eq(*c, entry)) {
                chosen.push(entry);
            }
        }
        if !any {
            unmatched.push(pattern.to_owned());
        }
    }
    chosen.sort_by_key(|c| entries.iter().position(|e| std::ptr::eq(e, *c)));
    (chosen, unmatched)
}

/// A line of text, or `None` when Enter was pressed with nothing typed.
fn ask_text(
    ed: &Editor,
    message: &str,
    default: Option<&str>,
) -> Result<Option<String>, Cancelled> {
    let mut options = Options::<String>::new(message);
    if let Some(default) = default {
        options = options.default(default.to_owned());
    }
    let text = match ed.ask_string(options)? {
        Answer::Value(t) | Answer::Keyword(t) => t,
        Answer::Enter => default.unwrap_or_default().to_owned(),
    };
    let text = text.trim().trim_matches('"').trim().to_owned();
    Ok((!text.is_empty()).then_some(text))
}

/// Ask for a `.lin` file and read it, saying what could not be read. `None`
/// when nothing was given or the file cannot be opened (said).
fn read_file(ed: &Editor) -> Result<Option<lin::LinFile>, Cancelled> {
    let Some(name) = ask_text(ed, key!("Enter linetype file name"), None)? else {
        return Ok(None);
    };
    let bytes = match std::fs::read(&name) {
        Ok(bytes) => bytes,
        Err(error) => {
            ed.say(format!("Cannot read \"{name}\": {error}"));
            return Ok(None);
        }
    };
    let file = lin::parse_bytes(&bytes);
    for note in file.notes.iter().filter(|n| n.skipped) {
        ed.say(match &note.name {
            Some(entry) => format!(
                "{entry} (line {}) was not read: {}.",
                note.line, note.message
            ),
            None => format!("Line {}: {}.", note.line, note.message),
        });
    }
    Ok(Some(file))
}

/// The command, for the command table.
pub fn commands() -> Vec<Command<AppState>> {
    vec![
        Command::new("format.linetype_line", "-LINETYPE", |s: &mut AppState| {
            s.start_command("-LINETYPE", linetype_prompt)
        })
        .checked(|s| s.is_running("-LINETYPE"))
        .transparent(),
    ]
}

/// -LINETYPE.
fn linetype_prompt(ed: &mut Editor) -> Result<(), Cancelled> {
    let options = Options::<String>::new(key!("Enter an option"))
        .keyword("?", "?")
        .keyword("Create", "C")
        .keyword("Load", "L")
        .keyword("Set", "S");
    match ed.ask_keyword(options)?.as_deref() {
        Some("Load") => load(ed),
        Some("?") => list(ed),
        Some(other) => {
            ed.say(format!("-LINETYPE {other} is not built yet."));
            Ok(())
        }
        None => Ok(()),
    }
}

/// `?`: the names and descriptions of a file's linetypes.
fn list(ed: &Editor) -> Result<(), Cancelled> {
    let Some(patterns) = ask_text(ed, key!("Enter linetype(s) to list"), Some("*"))? else {
        return Ok(());
    };
    let Some(file) = read_file(ed)? else {
        return Ok(());
    };
    let (chosen, unmatched) = select(&file.entries, &patterns);
    for entry in &chosen {
        ed.say(format!("{:<20} {}", entry.name, entry.description));
    }
    for pattern in unmatched {
        ed.say(format!("No linetypes match \"{pattern}\"."));
    }
    ed.say(plural(chosen.len(), "linetype") + " listed.");
    Ok(())
}

/// Load: the matches of the names asked for, as one undo step.
fn load(ed: &Editor) -> Result<(), Cancelled> {
    let Some(patterns) = ask_text(ed, key!("Enter linetype(s) to load"), None)? else {
        return Ok(());
    };
    let Some(file) = read_file(ed)? else {
        return Ok(());
    };
    let (chosen, unmatched) = select(&file.entries, &patterns);
    for pattern in unmatched {
        ed.say(format!("No linetypes match \"{pattern}\"."));
    }
    let (mut table, scale) = ed.read(|doc| (doc.linetypes().clone(), doc.linetype_scale()))?;
    let (mut added, mut replaced) = (0, 0);
    for entry in chosen {
        if let Some(have) = table.named(&entry.name) {
            if have.elements == entry.elements && have.description == entry.description {
                ed.say(format!("Linetype \"{}\" is already loaded.", have.name));
                continue;
            }
            let question = Options::worded(
                key!("Linetype \"%{name}\" is already loaded. Reload it?"),
                &[("name", &have.name)],
            )
            .keyword("Yes", "Y")
            .keyword("No", "N")
            .default("Yes".to_owned());
            if ed.get_keyword(question)? == "No" {
                continue;
            }
        }
        match table.load_definition(&entry.name, &entry.description, entry.elements.clone()) {
            Ok(opendraft_core::linetype::Loaded::Added(_)) => added += 1,
            Ok(opendraft_core::linetype::Loaded::Replaced(_)) => replaced += 1,
            Ok(opendraft_core::linetype::Loaded::Unchanged(_)) => {}
            Err(error) => ed.say(format!("{}: {error}.", entry.name)),
        }
    }
    if added + replaced > 0 {
        ed.transaction("Load linetypes", move |tx| {
            tx.perform(SetLinetypes::new(table, scale));
            Ok(())
        })?;
    }
    let mut line = format!("{} loaded", plural(added, "linetype"));
    if replaced > 0 {
        line.push_str(&format!(", {replaced} reloaded"));
    }
    ed.say(line + ".");
    Ok(())
}

#[cfg(test)]
#[path = "linetype_line_tests.rs"]
mod tests;
