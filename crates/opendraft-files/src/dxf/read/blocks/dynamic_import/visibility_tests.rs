//! A DXF's visibility parameter coming in as one: the objects of every
//! state are kept (AutoCAD writes the hidden ones as invisible), each state
//! hides what it does not show, and a modified reference keeps its state.

use opendraft_core::block::{BlockObjectId, DynamicParameterKind, DynamicValue};

use super::tests::{dxf, group, inserts};
use crate::dxf::read::{read, Record};

fn record(name: &str, groups: &[(i32, &str)]) -> Record {
    Record {
        name: name.into(),
        groups: groups.iter().map(|(c, v)| group(*c, v)).collect(),
    }
}

/// The fixture turned into a visibility parameter over two lines: `E1`
/// shown by "Plain" and `E9` (written invisible) by "Alt"; the reference
/// shows "Alt".
fn alternatives(mutate: impl Fn(&mut Vec<Record>)) -> opendraft_core::Document {
    let loaded = read(&dxf(false, |records| {
        records.retain(|r| r.name != "BLOCKSTRETCHACTION");
        let line = records.iter().position(|r| r.name == "LINE").unwrap();
        records.insert(
            line + 1,
            record(
                "LINE",
                &[
                    (5, "E9"),
                    (8, "0"),
                    (60, "1"),
                    (10, "0"),
                    (20, "1"),
                    (11, "5"),
                    (21, "1"),
                ],
            ),
        );
        for r in records.iter_mut() {
            if r.name == "BLOCKLINEARPARAMETER" {
                *r = record(
                    "BLOCKVISIBILITYPARAMETER",
                    &[
                        (5, "P1"),
                        (90, "6"),
                        (300, "Visibility State"),
                        (1010, "2.5"),
                        (1020, "0"),
                        (301, "Visibility1"),
                        (93, "2"),
                        (331, "E1"),
                        (331, "E9"),
                        (92, "2"),
                        (303, "Plain"),
                        (94, "1"),
                        (332, "E1"),
                        (95, "0"),
                        (303, "Alt"),
                        (94, "1"),
                        (332, "E9"),
                        (95, "0"),
                    ],
                );
            }
            if r.name == "XRECORD" && r.groups.iter().any(|g| g.code == 5 && g.value == "XR6") {
                *r = record(
                    "XRECORD",
                    &[(5, "XR6"), (70, "25"), (10, "0"), (20, "0"), (1, "Alt")],
                );
            }
        }
        mutate(records);
    }))
    .unwrap();
    loaded.document
}

#[test]
fn each_state_hides_the_objects_it_does_not_show() {
    let doc = alternatives(|_| {});
    doc.validate().unwrap();
    let slide = doc.block_named("Slide").unwrap();
    assert_eq!(slide.entities.len(), 2, "the hidden state's object is kept");
    let schema = slide.dynamic.as_deref().expect("dynamic");
    let p = &schema.parameters[0];
    assert_eq!(p.name, "Visibility1");
    let DynamicParameterKind::Visibility { states, .. } = &p.kind else {
        panic!("a visibility parameter");
    };
    assert_eq!(states.len(), 2);
    assert_eq!(
        (states[0].name.as_str(), &states[0].hidden),
        ("Plain", &vec![BlockObjectId(1)])
    );
    assert_eq!(
        (states[1].name.as_str(), &states[1].hidden),
        ("Alt", &vec![BlockObjectId(0)])
    );
}

#[test]
fn a_modified_reference_shows_the_state_it_recorded() {
    let doc = alternatives(|_| {});
    let refs = inserts(&doc);
    assert_eq!(refs.len(), 1);
    let state = refs[0].dynamic.as_deref().expect("a state");
    assert_eq!(state.values[0].value, DynamicValue::Text("Alt".into()));
    let shown: Vec<u64> = refs[0].evaluated().iter().map(|e| e.id.0).collect();
    assert_eq!(shown, [1]);
}

#[test]
fn a_state_the_block_does_not_have_leaves_the_reference_as_a_picture() {
    let doc = alternatives(|records| {
        for r in records.iter_mut() {
            if r.name == "XRECORD" && r.groups.iter().any(|g| g.code == 5 && g.value == "XR6") {
                for g in &mut r.groups {
                    if g.code == 1 {
                        g.value = "Nowhere".into();
                    }
                }
            }
        }
    });
    assert!(inserts(&doc)
        .iter()
        .all(|i| i.block.dynamic.is_none() || i.dynamic.is_none()));
}

#[test]
fn an_object_marked_invisible_that_no_state_governs_is_left_out() {
    let doc = alternatives(|records| {
        for r in records.iter_mut() {
            if r.name == "BLOCKVISIBILITYPARAMETER" {
                // It governs only E1, so E9 stays hidden for good.
                r.groups
                    .retain(|g| !(g.code == 331 && g.value == "E9") && g.value != "E9");
            }
        }
    });
    let slide = doc.block_named("Slide").unwrap();
    assert_eq!(slide.entities.len(), 1);
}
