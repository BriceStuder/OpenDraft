use std::path::PathBuf;

use super::*;
use crate::state::AppState;

const FILE: &str = "*FENCE,A fence\nA,2,-1\n\
                    *FENCE2,A longer fence\nA,4,-2\n\
                    *DASHED,Dashed, imperial\nA,.5,-.25\n\
                    *GAS_LINE,Gas\nA,.5,-.2,[\"GAS\",STANDARD,S=.1],-.25\n\
                    *BAD,No good\nA,x,-1\n";

fn file(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("opendraft-{name}-{}.lin", std::process::id()));
    std::fs::write(&path, FILE).unwrap();
    path
}

fn said(state: &AppState, text: &str) -> bool {
    state.cmdline.log.iter().any(|l| l.contains(text))
}

fn linetypes(state: &AppState) -> Vec<String> {
    let doc = state.session().document();
    doc.linetypes()
        .records()
        .iter()
        .map(|r| r.name.clone())
        .collect()
}

/// Start -LINETYPE and answer each prompt in turn.
fn run(state: &mut AppState, answers: &[&str]) {
    state.start_command("-LINETYPE", linetype_prompt);
    state.settle_command();
    for a in answers {
        state.cmd_submit(a);
        state.settle_command();
    }
}

fn entries() -> Vec<LinEntry> {
    lin::parse(FILE).entries
}

#[test]
fn names_select_with_wildcards_in_file_order_each_once() {
    let all = entries();
    let names = |patterns: &str| -> (Vec<String>, Vec<String>) {
        let (chosen, unmatched) = select(&all, patterns);
        (chosen.iter().map(|e| e.name.clone()).collect(), unmatched)
    };
    assert_eq!(names("*").0, ["FENCE", "FENCE2", "DASHED", "GAS_LINE"]);
    assert_eq!(names("fence*").0, ["FENCE", "FENCE2"]);
    assert_eq!(names("GAS_LINE, fence").0, ["FENCE", "GAS_LINE"]);
    assert_eq!(names("fence?, fence*").0, ["FENCE", "FENCE2"]);
    let (chosen, unmatched) = names("nothing, fence");
    assert_eq!(chosen, ["FENCE"]);
    assert_eq!(unmatched, ["nothing"]);
}

#[test]
fn load_adds_the_named_linetypes_as_one_undo_step() {
    let path = file("load");
    let mut state = AppState::new();
    let before = linetypes(&state);
    run(&mut state, &["Load", "FENCE*", &path.to_string_lossy()]);
    let _ = std::fs::remove_file(&path);
    let now = linetypes(&state);
    assert_eq!(now.len(), before.len() + 2);
    assert!(now.contains(&"FENCE".to_owned()) && now.contains(&"FENCE2".to_owned()));
    assert!(said(&state, "2 linetypes loaded."));
    assert!(said(&state, "BAD (line 9) was not read"));
    state.undo();
    assert_eq!(linetypes(&state), before);
    assert!(!state.command_running());
}

#[test]
fn a_name_the_drawing_has_is_reloaded_only_when_the_answer_is_yes() {
    let path = file("reload");
    let name = path.to_string_lossy().into_owned();
    let mut state = AppState::new();
    let dashed = state
        .session()
        .document()
        .linetypes()
        .named("DASHED")
        .unwrap()
        .id;
    // The drawing's DASHED has other numbers than the file's.
    run(&mut state, &["Load", "DASHED", &name, "No"]);
    assert!(said(&state, "0 linetypes loaded."));
    let have = state
        .session()
        .document()
        .linetypes()
        .get(dashed)
        .unwrap()
        .clone();
    assert_ne!(have.description, "Dashed, imperial");
    run(&mut state, &["Load", "DASHED", &name, "Yes"]);
    let _ = std::fs::remove_file(&path);
    let table = state.session().document().linetypes().clone();
    assert_eq!(table.get(dashed).unwrap().description, "Dashed, imperial");
    assert!(said(&state, "0 linetypes loaded, 1 reloaded."));
}

#[test]
fn loading_what_is_already_there_says_so_and_records_nothing() {
    let path = file("same");
    let name = path.to_string_lossy().into_owned();
    let mut state = AppState::new();
    run(&mut state, &["Load", "FENCE", &name]);
    let steps = state.session().history().can_undo();
    run(&mut state, &["Load", "FENCE", &name]);
    let _ = std::fs::remove_file(&path);
    assert!(steps);
    assert!(said(&state, "Linetype \"FENCE\" is already loaded."));
    assert!(said(&state, "0 linetypes loaded."));
    assert_eq!(
        linetypes(&state).iter().filter(|n| *n == "FENCE").count(),
        1
    );
}

#[test]
fn a_missing_file_is_said_and_nothing_changes() {
    let mut state = AppState::new();
    let before = linetypes(&state);
    let missing = std::env::temp_dir().join("opendraft-no-such-file.lin");
    run(&mut state, &["Load", "*", &missing.to_string_lossy()]);
    assert!(said(&state, "Cannot read"));
    assert_eq!(linetypes(&state), before);
    assert!(!state.command_running());
}

#[test]
fn a_name_nothing_matches_is_said() {
    let path = file("nomatch");
    let mut state = AppState::new();
    run(&mut state, &["Load", "nothing", &path.to_string_lossy()]);
    let _ = std::fs::remove_file(&path);
    assert!(said(&state, "No linetypes match \"nothing\"."));
    assert!(said(&state, "0 linetypes loaded."));
}

#[test]
fn a_question_mark_lists_the_file_without_changing_the_drawing() {
    let path = file("list");
    let mut state = AppState::new();
    let before = linetypes(&state);
    run(&mut state, &["?", "fence*", &path.to_string_lossy()]);
    let _ = std::fs::remove_file(&path);
    assert!(said(&state, "FENCE2"));
    assert!(said(&state, "A longer fence"));
    assert!(said(&state, "2 linetypes listed."));
    assert_eq!(linetypes(&state), before);
}

#[test]
fn quotes_round_a_path_are_ignored() {
    let path = file("quoted");
    let mut state = AppState::new();
    run(
        &mut state,
        &["Load", "FENCE", &format!("\"{}\"", path.to_string_lossy())],
    );
    let _ = std::fs::remove_file(&path);
    assert!(linetypes(&state).contains(&"FENCE".to_owned()));
}

#[test]
fn create_and_set_say_they_are_not_built() {
    for option in ["Create", "Set"] {
        let mut state = AppState::new();
        run(&mut state, &[option]);
        assert!(said(&state, "is not built yet"), "{option}");
        assert!(!state.command_running());
    }
}

#[test]
fn enter_alone_ends_the_command_at_each_question() {
    let mut state = AppState::new();
    run(&mut state, &["Load", ""]);
    assert!(!state.command_running());
    assert!(linetypes(&state).len() >= 7);
}
