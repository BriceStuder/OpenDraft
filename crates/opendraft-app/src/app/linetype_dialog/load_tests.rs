use std::path::PathBuf;

use opendraft_core::linetype::stock;
use opendraft_core::{LinetypeTable, StrokeElement};

use super::*;
use crate::picker::{Script, ScriptedPicker};

fn draft() -> Draft {
    Draft {
        table: LinetypeTable::default(),
        scale: 1.0,
        selected: None,
        load: None,
    }
}

const FILE: &str = "*FENCE,A fence\nA,2,-1\n\
                    *DASHED,Dashed, imperial\nA,.5,-.25\n\
                    *GAS_LINE,Gas\nA,.5,-.2,[\"GAS\",STANDARD,S=.1],-.25\n\
                    *BAD,No good\nA,x,-1\n";

/// A `.lin` file in the temporary folder, and the state with a scripted
/// picker that will pick it.
fn state_picking(name: &str, bytes: &[u8]) -> (AppState, Script, PathBuf) {
    let path = std::env::temp_dir().join(format!("opendraft-{name}-{}.lin", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let script = Script::default();
    script.will_open_file(Some(path.clone()));
    let mut state = AppState::new();
    state.picker = Box::new(ScriptedPicker(script.clone()));
    (state, script, path)
}

fn tick_all(load: &mut LoadState) {
    load.ticked.fill(true);
}

#[test]
fn the_stock_library_is_offered_unticked() {
    let load = LoadState::stock();
    assert_eq!(load.offers.len(), stock::LIBRARY.len());
    assert!(load.ticked.iter().all(|t| !t));
    assert_eq!(load.source, "Stock library");
}

#[test]
fn a_file_offers_what_it_holds_and_says_what_it_could_not_read() {
    let load = LoadState::from_file("mine.lin", FILE.as_bytes());
    let names: Vec<_> = load.offers.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["FENCE", "DASHED", "GAS_LINE"]);
    assert_eq!(load.source, "mine.lin");
    assert_eq!(load.unreadable.len(), 1);
    assert!(
        load.unreadable[0].starts_with("BAD (line 7)"),
        "{:?}",
        load.unreadable
    );
    assert_eq!(load.dashes_only, 1);
}

#[test]
fn choosing_a_file_asks_for_a_lin_file_and_offers_it() {
    let (mut state, script, path) = state_picking("choose", FILE.as_bytes());
    let mut draft = draft();
    choose_lin_file(&mut state, &mut draft);
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        script.file_dialogs(),
        [(
            "Select Linetype File".to_owned(),
            "Linetype files".to_owned()
        )]
    );
    let load = draft.load.expect("the file's definitions are on offer");
    assert_eq!(load.offers.len(), 3);
    assert!(load.source.starts_with("opendraft-choose-"));
}

#[test]
fn cancelling_the_file_dialog_keeps_what_was_offered() {
    let script = Script::default();
    script.will_open_file(None);
    let mut state = AppState::new();
    state.picker = Box::new(ScriptedPicker(script));
    let mut draft = draft();
    draft.load = Some(LoadState::stock());
    choose_lin_file(&mut state, &mut draft);
    assert_eq!(draft.load.unwrap().source, "Stock library");
}

#[test]
fn a_file_that_cannot_be_read_says_so_and_changes_nothing() {
    let script = Script::default();
    script.will_open_file(Some(
        std::env::temp_dir().join("opendraft-no-such-file.lin"),
    ));
    let mut state = AppState::new();
    state.picker = Box::new(ScriptedPicker(script));
    let mut draft = draft();
    draft.load = Some(LoadState::stock());
    choose_lin_file(&mut state, &mut draft);
    assert!(state
        .message
        .starts_with("Could not read opendraft-no-such-file.lin"));
    assert_eq!(draft.load.unwrap().source, "Stock library");
}

#[test]
fn a_file_that_is_not_utf8_still_loads() {
    let mut bytes = b"*JIS_09_29,2SASEN29 ____".to_vec();
    bytes.extend_from_slice(&[0x81, 0x40]);
    bytes.extend_from_slice(b"__\r\nA,12,-3\r\n");
    let (mut state, _, path) = state_picking("sjis", &bytes);
    let mut draft = draft();
    choose_lin_file(&mut state, &mut draft);
    let _ = std::fs::remove_file(&path);
    assert_eq!(draft.load.unwrap().offers[0].name, "JIS_09_29");
}

#[test]
fn only_ticked_definitions_load_and_a_same_name_is_replaced_in_place() {
    let mut draft = draft();
    let dashed = draft.table.named("DASHED").unwrap().id;
    let mut load = LoadState::from_file("mine.lin", FILE.as_bytes());
    load.ticked = vec![true, true, false];
    let before = draft.table.records().len();
    let line = load.apply(&mut draft);
    assert_eq!(line, "Loaded 1 linetype, replaced 1");
    assert_eq!(draft.table.records().len(), before + 1);
    assert!(draft.table.named("FENCE").is_some());
    assert!(draft.table.named("GAS_LINE").is_none(), "left unticked");
    let record = draft.table.get(dashed).unwrap();
    assert_eq!(
        record.elements,
        vec![StrokeElement::Dash(0.5), StrokeElement::Gap(0.25)]
    );
    assert_eq!(record.description, "Dashed, imperial");
}

#[test]
fn loading_nothing_says_so_and_loading_again_changes_nothing() {
    let mut draft = draft();
    let line = LoadState::from_file("mine.lin", FILE.as_bytes()).apply(&mut draft);
    assert_eq!(line, "Loaded 0 linetypes");
    let mut load = LoadState::from_file("mine.lin", FILE.as_bytes());
    tick_all(&mut load);
    load.apply(&mut draft);
    let after_first = draft.table.clone();
    let mut again = LoadState::from_file("mine.lin", FILE.as_bytes());
    tick_all(&mut again);
    assert_eq!(again.apply(&mut draft), "Loaded 0 linetypes");
    assert_eq!(draft.table, after_first);
}
