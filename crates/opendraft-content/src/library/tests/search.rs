use super::*;

fn setup() -> (tempfile::TempDir, Library) {
    let (dir, mut lib) = open();
    let gate = add(&mut lib, "Gate valve", 1.0);
    lib.set_tags(gate, vec!["piping".into(), "water".into()])
        .expect("tags");
    lib.set_description(gate, "Isolating valve, éléphant style")
        .expect("describes");
    lib.move_block(gate, "Piping/Valves").expect("moves");
    let globe = add(&mut lib, "Globe valve", 2.0);
    lib.set_tags(globe, vec!["steam".into()]).expect("tags");
    add(&mut lib, "Valve", 3.0);
    add(&mut lib, "Desk", 4.0);
    (dir, lib)
}

fn found(lib: &Library, text: &str) -> Vec<String> {
    lib.search(&Query {
        text: text.into(),
        ..Default::default()
    })
    .into_iter()
    .map(|b| b.name)
    .collect()
}

#[test]
fn all_the_words_must_match_in_any_order() {
    let (_dir, lib) = setup();
    assert_eq!(found(&lib, "valve gate"), ["Gate valve"]);
    assert_eq!(found(&lib, "valve zzz"), Vec::<String>::new());
}

#[test]
fn case_and_accents_do_not_matter() {
    let (_dir, lib) = setup();
    assert_eq!(found(&lib, "ELEPHANT"), ["Gate valve"]);
    assert_eq!(found(&lib, "éLéPhAnT"), ["Gate valve"]);
}

#[test]
fn tags_and_folders_are_searched_too() {
    let (_dir, lib) = setup();
    assert_eq!(found(&lib, "steam"), ["Globe valve"]);
    assert_eq!(found(&lib, "water"), ["Gate valve"]);
    assert_eq!(found(&lib, "valves piping"), ["Gate valve"]);
}

#[test]
fn the_best_name_comes_first() {
    let (_dir, lib) = setup();
    // "Valve" is the words; "Globe valve" and "Gate valve" merely have them.
    assert_eq!(found(&lib, "valve"), ["Valve", "Gate valve", "Globe valve"]);
    assert_eq!(found(&lib, "gl"), ["Globe valve"]);
}

#[test]
fn no_words_lists_everything_by_name() {
    let (_dir, lib) = setup();
    assert_eq!(
        found(&lib, "  "),
        ["Desk", "Gate valve", "Globe valve", "Valve"]
    );
}

#[test]
fn filters_narrow_the_result() {
    let (_dir, mut lib) = setup();
    let id = lib.blocks()[0].id;
    lib.set_favourite(id, true).expect("favourite");
    let favourites = lib.search(&Query {
        favourites_only: true,
        ..Default::default()
    });
    assert_eq!(favourites.len(), 1);
    assert!(lib
        .search(&Query {
            dynamic: Some(true),
            ..Default::default()
        })
        .is_empty());
    assert_eq!(
        lib.search(&Query {
            dynamic: Some(false),
            has_attributes: Some(false),
            ..Default::default()
        })
        .len(),
        4
    );
}
