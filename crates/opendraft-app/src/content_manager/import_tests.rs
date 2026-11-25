use super::actions::Action;
use super::tests::{apply, frame, names, setup};
use super::*;
use crate::library_cmd_tests::{app_with_library, drawing_with_box};
use crate::state::AppState;

/// Saves the drawing of `state` as a `.odraft` file in `dir`.
fn save_drawing(state: &AppState, dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    opendraft_files::save(&path, state.session().document()).expect("saves");
    path
}

fn picking(state: &mut AppState, path: Option<std::path::PathBuf>) {
    use crate::picker::{Script, ScriptedPicker};
    let script = Script::default();
    state.picker = Box::new(ScriptedPicker(script.clone()));
    script.will_open(path);
    state.pick_blocks_to_import();
}

#[test]
fn blocks_of_a_drawing_file_are_ticked_and_imported_into_a_folder_named_for_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let lib_dir = dir.path().join("lib");
    let maker = drawing_with_box(&lib_dir, 5.0);
    let file = save_drawing(&maker, dir.path(), "plant.odraft");

    let mut state = app_with_library(&lib_dir);
    picking(&mut state, Some(file));
    let mut win = std::mem::take(&mut state.command_dialogs.content);
    assert!(win.open && win.import.is_some());
    // The import panel draws.
    assert!(frame(&mut win, &state).is_empty());
    apply(&mut win, &state, vec![Action::RunImport]);
    assert!(win.import.is_none());
    assert!(
        win.status.contains("Imported 1 block into plant"),
        "{}",
        win.status
    );
    assert_eq!(names(&state), ["Box"]);
    assert_eq!(win.place, Place::Mine("plant".into()));
    let folder = state
        .defaults()
        .library
        .with(|l| l.blocks()[0].folder.clone())
        .expect("opens");
    assert_eq!(folder, "plant");
}

#[test]
fn nothing_ticked_or_a_bad_folder_imports_nothing_and_keeps_the_panel() {
    let dir = tempfile::tempdir().expect("temp dir");
    let lib_dir = dir.path().join("lib");
    let maker = drawing_with_box(&lib_dir, 5.0);
    let file = save_drawing(&maker, dir.path(), "plant.odraft");
    let mut state = app_with_library(&lib_dir);
    picking(&mut state, Some(file));
    let mut win = std::mem::take(&mut state.command_dialogs.content);

    if let Some(draft) = &mut win.import {
        draft.untick_all();
    }
    apply(&mut win, &state, vec![Action::RunImport]);
    assert!(win.import.is_some() && names(&state).is_empty());
    apply(&mut win, &state, vec![Action::CancelImport]);
    assert!(win.import.is_none());
}

#[test]
fn a_cancelled_picker_a_file_without_blocks_and_a_bad_file_are_each_said_not_failed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut state = app_with_library(&dir.path().join("lib"));
    picking(&mut state, None);
    assert!(state.command_dialogs.content.import.is_none());
    assert!(!state.command_dialogs.content.open);

    let empty = AppState::new();
    let none = save_drawing(&empty, dir.path(), "empty.odraft");
    picking(&mut state, Some(none));
    assert!(state.command_dialogs.content.status.contains("no blocks"));

    let bad = dir.path().join("broken.odraft");
    std::fs::write(&bad, b"not a drawing").expect("writes");
    picking(&mut state, Some(bad));
    assert!(state
        .command_dialogs
        .content
        .status
        .contains("Could not read"));
    assert!(state.command_dialogs.content.import.is_none());
}

#[test]
fn filters_narrow_the_listing_and_clear() {
    use super::filter::Tri;
    let dir = tempfile::tempdir().expect("temp dir");
    let (state, mut win) = setup(dir.path());
    let snapshot = |win: &Window| {
        let defaults = std::sync::Arc::clone(state.defaults());
        let env = Env {
            library: &defaults.library,
            document: state.session().document(),
            revision: 0,
        };
        snapshot::Snapshot::take(win, &env)
    };
    assert!(!snapshot(&win).searching);
    assert_eq!(snapshot(&win).shown.len(), 1);

    win.filter.dynamic = Tri::Yes;
    let narrowed = snapshot(&win);
    assert!(narrowed.searching && narrowed.shown.is_empty());
    assert!(
        frame(&mut win, &state).is_empty(),
        "the window draws filtered"
    );

    win.filter.dynamic = Tri::No;
    win.filter.favourites = true;
    assert!(snapshot(&win).shown.is_empty(), "no favourites yet");
    let id = state
        .defaults()
        .library
        .with(|l| l.blocks()[0].id)
        .expect("a block");
    apply(&mut win, &state, vec![Action::Favourite(id, true)]);
    assert_eq!(snapshot(&win).shown.len(), 1);

    let units = snapshot(&win).units;
    assert_eq!(units.len(), 1);
    win.filter.unit = Some(opendraft_core::InsertUnits::from_code(4).expect("mm"));
    assert!(snapshot(&win).shown.is_empty(), "the block is unitless");
    win.filter = Default::default();
    assert!(!snapshot(&win).searching);
}

#[test]
fn the_blocks_of_a_dxf_with_attributes_import_with_their_attributes() {
    let dxf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../opendraft-files/tests/fixtures/ezdxf_attributes.dxf");
    let dir = tempfile::tempdir().expect("temp dir");
    let mut state = app_with_library(&dir.path().join("lib"));
    picking(&mut state, Some(dxf));
    let mut win = std::mem::take(&mut state.command_dialogs.content);
    assert!(win.import.is_some(), "{}", win.status);
    apply(&mut win, &state, vec![Action::RunImport]);
    let blocks = state
        .defaults()
        .library
        .with(|l| l.blocks())
        .expect("opens");
    assert!(!blocks.is_empty(), "{}", win.status);
    assert!(blocks.iter().any(|b| b.has_attributes), "{blocks:?}");
    assert!(blocks.iter().all(|b| !b.name.starts_with('*')));
    // Imported blocks can be placed in a drawing again.
    let id = blocks
        .iter()
        .find(|b| b.has_attributes)
        .map(|b| b.id)
        .expect("one");
    state.defaults().library.queue_pick(id);
    state.start_command("LIBINSERT", crate::library_insert_cmd::library_insert);
    state.settle_command();
    state.cmd_submit("0,0");
    state.settle_command();
    // Its attributes ask for their values: accept the defaults.
    for _ in 0..10 {
        if !state.command_running() {
            break;
        }
        state.cmd_submit("");
        state.settle_command();
    }
    assert_eq!(state.session().document().validate(), Ok(()));
    assert_eq!(
        state.session().document().len(),
        1,
        "{:?}",
        state.cmdline.log
    );
}
