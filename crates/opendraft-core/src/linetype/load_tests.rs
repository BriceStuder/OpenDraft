use super::load::Loaded;
use super::StrokeElement::{Dash, Gap};
use super::{LinetypeError, LinetypeId, LinetypeTable};

#[test]
fn a_new_name_is_added_with_a_fresh_id() {
    let mut table = LinetypeTable::default();
    let before = table.records().len();
    let got = table
        .load_definition("FENCE", "a fence", vec![Dash(2.0), Gap(1.0)])
        .unwrap();
    let Loaded::Added(id) = got else {
        panic!("{got:?}")
    };
    assert_eq!(table.records().len(), before + 1);
    assert_eq!(table.get(id).unwrap().name, "FENCE");
}

#[test]
fn an_existing_name_is_replaced_in_place_keeping_its_id_and_spelling() {
    let mut table = LinetypeTable::default();
    let id = LinetypeId::DASHED;
    let before = table.records().len();
    let got = table
        .load_definition("dashed", "imperial", vec![Dash(0.5), Gap(0.25)])
        .unwrap();
    assert_eq!(got, Loaded::Replaced(id));
    assert_eq!(table.records().len(), before);
    let record = table.get(id).unwrap();
    assert_eq!(record.name, "DASHED");
    assert_eq!(record.description, "imperial");
    assert_eq!(record.elements, vec![Dash(0.5), Gap(0.25)]);
}

#[test]
fn loading_what_is_there_changes_nothing() {
    let mut table = LinetypeTable::default();
    let record = table.named("DASHED").unwrap().clone();
    let got = table
        .load_definition("DASHED", &record.description, record.elements.clone())
        .unwrap();
    assert_eq!(got, Loaded::Unchanged(record.id));
    assert!(table.is_stock());
}

#[test]
fn the_special_records_are_refused() {
    let mut table = LinetypeTable::default();
    for name in ["Continuous", "BYLAYER", "byblock"] {
        let got = table.load_definition(name, "x", vec![Dash(1.0), Gap(1.0)]);
        assert!(matches!(got, Err(LinetypeError::Special(_))), "{name}");
    }
    assert!(table.is_stock());
}

#[test]
fn would_change_says_whether_a_load_has_anything_to_do() {
    let table = LinetypeTable::default();
    let dashed = table.named("DASHED").unwrap().elements.clone();
    assert!(!table.would_change("dashed", &dashed));
    assert!(table.would_change("DASHED", &[Dash(1.0), Gap(1.0)]));
    assert!(table.would_change("NEW", &[Dash(1.0), Gap(1.0)]));
}

#[test]
fn a_bad_length_changes_nothing() {
    let mut table = LinetypeTable::default();
    let got = table.load_definition("BAD", "x", vec![Dash(f64::NAN), Gap(1.0)]);
    assert_eq!(got, Err(LinetypeError::BadElement));
    assert!(table.is_stock());
}
