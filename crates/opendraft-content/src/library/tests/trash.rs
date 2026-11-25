use super::*;
use crate::test_support::drawing_with_nested;

const DAY: u64 = 24 * 60 * 60;

fn uuid_of(lib: &Library, name: &str) -> BlockUuid {
    let id = lib
        .package()
        .drawing
        .block_named(name)
        .map(|b| b.id)
        .expect("block");
    lib.package()
        .entry_for_block(id)
        .map(|e| e.uuid)
        .expect("entry")
}

#[test]
fn a_deleted_block_is_in_the_trash_and_can_come_back_as_it_was() {
    let (_dir, mut lib) = open();
    let id = add(&mut lib, "valve", 1.0);
    lib.set_tags(id, vec!["water".into()]).expect("tags");
    lib.set_description(id, "Gate").expect("describes");
    lib.move_block(id, "Piping/Valves").expect("moves");
    lib.set_favourite(id, true).expect("favourite");

    lib.delete_block(id, NOW).expect("deletes");
    assert!(lib.blocks().is_empty());
    let items = lib.trash_items();
    assert_eq!(items.len(), 1);
    assert_eq!(
        (items[0].name.as_str(), items[0].folder.as_str()),
        ("valve", "Piping/Valves")
    );
    assert!(store::trash_file(lib.root(), items[0].id).exists());

    let restored = lib
        .restore(items[0].id, NameTaken::Cancel, NOW)
        .expect("restores");
    assert_eq!(restored.id, id);
    let block = lib.block(id).expect("back");
    assert_eq!(block.name, "valve");
    assert_eq!(block.description, "Gate");
    assert_eq!(block.tags, ["water"]);
    assert_eq!(block.folder, "Piping/Valves");
    assert!(lib.trash_items().is_empty());
    assert!(!store::trash_file(lib.root(), items[0].id).exists());
    // Favourites are choices, not content: they are not kept through the Trash.
    assert!(!block.favourite);
}

#[test]
fn the_trash_survives_closing_the_library() {
    let (_dir, mut lib) = open();
    let id = add(&mut lib, "valve", 1.0);
    lib.delete_block(id, NOW).expect("deletes");
    let mut again = reopen(&lib);
    assert_eq!(again.trash_items().len(), 1);
    let item = again.trash_items()[0].id;
    again
        .restore(item, NameTaken::Cancel, NOW)
        .expect("restores");
    assert_eq!(names(&again.blocks()), ["valve"]);
}

#[test]
fn restoring_onto_a_taken_name_asks_as_capture_does() {
    let (_dir, mut lib) = open();
    let id = add(&mut lib, "valve", 1.0);
    lib.delete_block(id, NOW).expect("deletes");
    add(&mut lib, "valve", 9.0);
    let item = lib.trash_items()[0].id;

    assert_eq!(
        lib.restore(item, NameTaken::Cancel, NOW)
            .expect_err("taken"),
        LibraryError::NameTaken("valve".into())
    );
    assert_eq!(
        lib.trash_items().len(),
        1,
        "still in the Trash after a refusal"
    );

    let restored = lib
        .restore(item, NameTaken::KeepBoth, NOW)
        .expect("restores");
    assert_eq!(restored.name, "valve_2");
    assert_eq!(restored.id, id);
    assert_eq!(names(&lib.blocks()), ["valve", "valve_2"]);
}

#[test]
fn a_block_goes_with_the_parts_only_it_uses() {
    let (_dir, mut lib) = open();
    let (doc, outer, _) = drawing_with_nested("bolt", "nut");
    let bolt = lib
        .capture(&doc, outer, &CaptureRequest::default(), NOW)
        .expect("captures")
        .id;
    let nut = uuid_of(&lib, "nut");

    lib.delete_block(bolt, NOW).expect("deletes");
    assert!(lib.package().entries.is_empty());
    assert!(lib.package().drawing.blocks().is_empty());

    let item = lib.trash_items()[0].id;
    lib.restore(item, NameTaken::Cancel, NOW).expect("restores");
    assert_eq!(lib.package().entries.len(), 2);
    // Both come back with the ids they had.
    assert_eq!(uuid_of(&lib, "nut"), nut);
    assert_eq!(
        lib.block(bolt).map(|b| b.components),
        Some(vec!["nut".into()])
    );
    assert!(lib.package().drawing.validate().is_ok());
}

#[test]
fn a_part_that_another_block_uses_stays() {
    let (_dir, mut lib) = open();
    let (a, bolt, _) = drawing_with_nested("bolt", "nut");
    let (b, screw, _) = drawing_with_nested("screw", "nut");
    let bolt = lib
        .capture(&a, bolt, &CaptureRequest::default(), NOW)
        .expect("captures")
        .id;
    lib.capture(&b, screw, &CaptureRequest::default(), NOW)
        .expect("captures");

    lib.delete_block(bolt, NOW).expect("deletes");
    assert_eq!(names(&lib.blocks()), ["screw"]);
    assert!(lib.package().drawing.block_named("nut").is_some());
    assert!(lib.package().drawing.validate().is_ok());

    // Restoring the bolt reuses the nut that is there.
    let item = lib.trash_items()[0].id;
    lib.restore(item, NameTaken::Cancel, NOW).expect("restores");
    assert_eq!(lib.package().entries.len(), 3);
    assert_eq!(names(&lib.blocks()), ["bolt", "screw"]);
}

#[test]
fn a_block_inside_another_cannot_be_deleted() {
    let (_dir, mut lib) = open();
    let (doc, outer, _) = drawing_with_nested("bolt", "nut");
    lib.capture(&doc, outer, &CaptureRequest::default(), NOW)
        .expect("captures");
    let nut = uuid_of(&lib, "nut");
    assert_eq!(
        lib.delete_block(nut, NOW),
        Err(LibraryError::InUse(vec!["bolt".into()]))
    );
    assert_eq!(lib.package().entries.len(), 2);
    assert!(lib.trash_items().is_empty());
    assert_eq!(
        lib.delete_block(BlockUuid::new(), NOW),
        Err(LibraryError::NotFound)
    );
}

#[test]
fn deleting_a_folder_trashes_its_blocks_and_restoring_remakes_the_folder() {
    let (_dir, mut lib) = open();
    let a = add(&mut lib, "a", 1.0);
    let b = add(&mut lib, "b", 2.0);
    let keep = add(&mut lib, "keep", 3.0);
    lib.move_block(a, "Old").expect("moves");
    lib.move_block(b, "Old/Inner").expect("moves");
    lib.create_folder("Old/Empty").expect("creates");

    lib.delete_folder("old", NOW).expect("deletes");
    assert_eq!(names(&lib.blocks()), ["keep"]);
    assert!(lib.folders().is_empty());
    assert_eq!(lib.trash_items().len(), 2);

    for item in lib.trash_items() {
        lib.restore(item.id, NameTaken::Cancel, NOW)
            .expect("restores");
    }
    assert_eq!(lib.block(a).map(|x| x.folder), Some("Old".into()));
    assert_eq!(lib.block(b).map(|x| x.folder), Some("Old/Inner".into()));
    assert!(lib.block(keep).is_some());
    assert_eq!(lib.delete_folder("Nope", NOW), Err(LibraryError::NotFound));
    assert_eq!(lib.delete_folder("", NOW), Err(LibraryError::NotFound));
}

#[test]
fn a_folder_with_a_block_something_outside_holds_is_not_deleted() {
    let (_dir, mut lib) = open();
    let (doc, outer, _) = drawing_with_nested("bolt", "nut");
    let bolt = lib
        .capture(&doc, outer, &CaptureRequest::default(), NOW)
        .expect("captures")
        .id;
    // A top-level nut that the bolt also holds, in a folder of its own.
    let (d2, nut2) = crate::test_support::drawing_with("nut", 1.0);
    let request = CaptureRequest {
        folder: "Parts".into(),
        if_name_taken: NameTaken::KeepBoth,
        ..Default::default()
    };
    lib.capture(&d2, nut2, &request, NOW).expect("captures");
    let before = lib.package().entries.len();
    lib.delete_folder("Parts", NOW)
        .expect("a separate block, so it goes");
    assert_eq!(lib.package().entries.len(), before - 1);
    assert!(lib.block(bolt).is_some());
}

#[test]
fn the_trash_is_emptied_after_thirty_days_or_by_hand() {
    let (_dir, mut lib) = open();
    let a = add(&mut lib, "a", 1.0);
    let b = add(&mut lib, "b", 2.0);
    lib.delete_block(a, NOW).expect("deletes");
    lib.delete_block(b, NOW + 20 * DAY).expect("deletes");
    let files: Vec<_> = lib
        .trash_items()
        .iter()
        .map(|t| store::trash_file(lib.root(), t.id))
        .collect();

    assert_eq!(lib.purge_trash(NOW + 30 * DAY).expect("purges"), 0);
    assert_eq!(lib.purge_trash(NOW + 30 * DAY + 1).expect("purges"), 1);
    assert_eq!(lib.trash_items().len(), 1);
    assert_eq!(lib.trash_items()[0].name, "b");
    assert_eq!(files.iter().filter(|f| f.exists()).count(), 1);

    assert_eq!(lib.empty_trash().expect("empties"), 1);
    assert!(lib.trash_items().is_empty());
    assert!(files.iter().all(|f| !f.exists()));
    assert_eq!(lib.empty_trash().expect("nothing to do"), 0);
}

#[test]
fn a_restore_that_fails_leaves_the_trash_alone() {
    let (_dir, mut lib) = open();
    let id = add(&mut lib, "valve", 1.0);
    lib.delete_block(id, NOW).expect("deletes");
    let item = lib.trash_items()[0].id;
    std::fs::write(store::trash_file(lib.root(), item), b"damaged").expect("damages");
    let error = lib
        .restore(item, NameTaken::Cancel, NOW)
        .expect_err("damaged");
    assert!(matches!(error, LibraryError::Corrupt(_)), "{error:?}");
    assert_eq!(lib.trash_items().len(), 1);
    assert_eq!(
        lib.restore(999, NameTaken::Cancel, NOW),
        Err(LibraryError::NotFound)
    );
}
