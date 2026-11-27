//! Tests of the library, on real folders.

use std::fs;

use super::*;
use crate::test_support::{drawing_with, drawing_with_nested};

mod capture;
mod organise;
mod search;
mod trash;

pub(super) const NOW: u64 = 1_000_000;

pub(super) fn open() -> (tempfile::TempDir, Library) {
    let dir = tempfile::tempdir().expect("temp dir");
    let lib = Library::open(&dir.path().join("Library")).expect("opens");
    (dir, lib)
}

pub(super) fn reopen(lib: &Library) -> Library {
    Library::open(lib.root()).expect("reopens")
}

/// Captures a new one-circle block called `name` at the top level.
pub(super) fn add(lib: &mut Library, name: &str, radius: f64) -> BlockUuid {
    let (doc, id) = drawing_with(name, radius);
    lib.capture(&doc, id, &CaptureRequest::default(), NOW)
        .expect("captures")
        .id
}

pub(super) fn names(blocks: &[BlockInfo]) -> Vec<&str> {
    blocks.iter().map(|b| b.name.as_str()).collect()
}

#[test]
fn a_new_library_is_empty_and_makes_its_folders() {
    let (_dir, lib) = open();
    assert!(lib.blocks().is_empty());
    assert!(lib.notices().is_empty());
    assert!(lib.root().join("trash").is_dir());
}

#[test]
fn what_is_captured_is_there_after_closing_and_opening() {
    let (_dir, mut lib) = open();
    let (doc, outer, _) = drawing_with_nested("bolt", "nut");
    let request = CaptureRequest {
        description: Some("An M8 bolt".into()),
        tags: vec!["fixing".into()],
        folder: "Fixings/Metric".into(),
        ..Default::default()
    };
    let id = lib
        .capture(&doc, outer, &request, NOW)
        .expect("captures")
        .id;
    lib.set_favourite(id, true).expect("favourite");

    let again = reopen(&lib);
    let block = again.block(id).expect("kept");
    assert_eq!(block.name, "bolt");
    assert_eq!(block.description, "An M8 bolt");
    assert_eq!(block.tags, ["fixing"]);
    assert_eq!(block.folder, "Fixings/Metric");
    assert_eq!(block.components, ["nut"]);
    assert!(block.favourite);
    assert_eq!(again.blocks().len(), 1);
}

#[test]
fn every_write_keeps_the_one_before_as_the_backup() {
    let (_dir, mut lib) = open();
    add(&mut lib, "one", 1.0);
    assert!(!lib.root().join("personal.bak").exists());
    add(&mut lib, "two", 2.0);
    let backup = store::read_package(&lib.root().join("personal.bak"))
        .expect("reads")
        .expect("exists");
    assert_eq!(backup.0.drawing.blocks().len(), 1);
    let current = store::read_package(&store::personal(lib.root()))
        .expect("reads")
        .expect("exists");
    assert_eq!(current.0.drawing.blocks().len(), 2);
    // The revision is the file's patch number and rises with each write.
    assert_eq!((backup.1, current.1), (1, 2));
}

#[test]
fn no_temporary_file_is_left_behind() {
    let (_dir, mut lib) = open();
    add(&mut lib, "one", 1.0);
    lib.set_favourite(lib.blocks()[0].id, true).expect("saves");
    let leftovers: Vec<_> = fs::read_dir(lib.root())
        .expect("lists")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty());
}

#[test]
fn a_damaged_file_is_replaced_by_the_backup_and_kept_aside() {
    let (_dir, mut lib) = open();
    add(&mut lib, "one", 1.0);
    add(&mut lib, "two", 2.0);
    fs::write(store::personal(lib.root()), b"{ this is not a package").expect("damages");

    let mut again = reopen(&lib);
    assert_eq!(names(&again.blocks()), ["one"]);
    assert_eq!(again.notices().len(), 1);
    assert!(again.notices()[0].contains("backup"));
    assert!(lib.root().join("personal.damaged").exists());
    assert!(!store::personal(lib.root()).exists());

    // Working on from the backup does not lose it.
    add(&mut again, "three", 3.0);
    assert_eq!(names(&reopen(&again).blocks()), ["one", "three"]);
    assert!(reopen(&again).notices().is_empty());
}

#[test]
fn when_nothing_can_be_read_nothing_is_touched() {
    let (_dir, mut lib) = open();
    add(&mut lib, "one", 1.0);
    add(&mut lib, "two", 2.0);
    let main = store::personal(lib.root());
    let backup = lib.root().join("personal.bak");
    fs::write(&main, b"damaged").expect("damages");
    fs::write(&backup, b"damaged too").expect("damages");

    let error = Library::open(lib.root()).expect_err("refused");
    assert!(matches!(error, LibraryError::Corrupt(_)), "{error:?}");
    assert_eq!(fs::read(&main).expect("kept"), b"damaged");
    assert_eq!(fs::read(&backup).expect("kept"), b"damaged too");
}

#[test]
fn a_missing_file_with_a_backup_is_restored() {
    let (_dir, mut lib) = open();
    add(&mut lib, "one", 1.0);
    add(&mut lib, "two", 2.0);
    fs::remove_file(store::personal(lib.root())).expect("removes");
    let again = reopen(&lib);
    assert_eq!(names(&again.blocks()), ["one"]);
    assert!(again.notices()[0].contains("missing"));
}

#[test]
fn an_unreadable_state_file_is_reset_and_kept_aside() {
    let (_dir, mut lib) = open();
    let id = add(&mut lib, "one", 1.0);
    lib.set_favourite(id, true).expect("favourite");
    fs::write(lib.root().join("library.json"), b"][").expect("damages");
    let again = reopen(&lib);
    // The blocks are untouched; only the user's choices are reset.
    assert_eq!(again.blocks().len(), 1);
    assert!(again.favourites().is_empty());
    assert!(again.notices()[0].contains("library.json"));
    assert!(lib.root().join("library.json.bad").exists());
}

#[test]
fn a_lost_state_file_costs_only_the_choices() {
    let (_dir, mut lib) = open();
    let id = add(&mut lib, "one", 1.0);
    lib.move_block(id, "Mine/Sub").expect("moves");
    lib.set_favourite(id, true).expect("favourite");
    fs::remove_file(lib.root().join("library.json")).expect("removes");
    let again = reopen(&lib);
    assert_eq!(again.blocks().len(), 1);
    assert!(again.favourites().is_empty());
    // The folder lives with the block, so it is still there.
    assert_eq!(again.block(id).map(|b| b.folder), Some("Mine/Sub".into()));
}

#[test]
fn a_failed_write_changes_nothing_in_memory_or_on_disk() {
    let (_dir, mut lib) = open();
    add(&mut lib, "one", 1.0);
    let before = fs::read(store::personal(lib.root())).expect("reads");
    // A folder where the temporary file goes makes the write fail.
    fs::create_dir(lib.root().join("personal.odpack.tmp")).expect("blocks");

    let (doc, id) = drawing_with("two", 2.0);
    let error = lib
        .capture(&doc, id, &CaptureRequest::default(), NOW)
        .expect_err("cannot write");
    assert!(matches!(error, LibraryError::Io(_)), "{error:?}");
    assert_eq!(names(&lib.blocks()), ["one"]);
    assert_eq!(
        fs::read(store::personal(lib.root())).expect("reads"),
        before
    );

    fs::remove_dir(lib.root().join("personal.odpack.tmp")).expect("clears");
    add(&mut lib, "two", 2.0);
    assert_eq!(lib.blocks().len(), 2);
}

#[test]
fn what_refers_to_nothing_is_dropped_on_opening() {
    let (_dir, mut lib) = open();
    let id = add(&mut lib, "one", 1.0);
    lib.set_favourite(id, true).expect("favourite");
    lib.touch_recent(id).expect("recent");
    lib.delete_block(id, NOW).expect("deletes");
    let again = reopen(&lib);
    assert!(again.favourites().is_empty() && again.recents().is_empty());
    assert_eq!(again.trash_items().len(), 1);
}
