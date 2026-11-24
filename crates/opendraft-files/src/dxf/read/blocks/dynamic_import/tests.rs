//! A DXF's dynamic blocks come in as dynamic blocks: the fixture is the
//! decoder's `Slide` (a linear parameter with four allowed values, its grip, a
//! stretch of one line, a lookup parameter and table) and one modified
//! reference `*U2` that sets the parameter to 10.

use opendraft_core::block::{
    DynamicActionKind, DynamicParameterKind, DynamicValue, DynamicValueSet,
};
use opendraft_core::{Entity, Insert};

use crate::dxf::read::dynamic::tests::file;
use crate::dxf::read::{read, Group, Record};

fn group(code: i32, value: &str) -> Group {
    Group {
        code,
        value: value.to_owned(),
    }
}

/// The fixture as DXF text. `lookup` keeps the lookup parameter and action;
/// the reference is placed at (3, 4).
fn dxf(lookup: bool, mutate: impl Fn(&mut Vec<Record>)) -> Vec<u8> {
    let mut records: Vec<Record> = file()
        .into_iter()
        .filter(|r| {
            lookup
                || !matches!(
                    r.name.as_str(),
                    "BLOCKLOOKUPPARAMETER" | "BLOCKLOOKUPACTION"
                )
        })
        .collect();
    for r in &mut records {
        if r.name == "INSERT" {
            r.groups.extend([group(10, "3"), group(20, "4")]);
        }
        if r.name == "LINE" {
            r.groups.extend([
                group(10, "0"),
                group(20, "0"),
                group(11, "5"),
                group(21, "0"),
            ]);
        }
    }
    // The anonymous block the modified reference inserts.
    let ends = records.iter().rposition(|r| r.name == "ENDBLK").unwrap();
    let picture = [
        Record {
            name: "BLOCK".into(),
            groups: vec![group(5, "B5"), group(2, "*U2"), group(70, "1")],
        },
        Record {
            name: "LINE".into(),
            groups: vec![
                group(5, "E3"),
                group(8, "0"),
                group(10, "0"),
                group(20, "0"),
                group(11, "10"),
                group(21, "0"),
            ],
        },
        Record {
            name: "ENDBLK".into(),
            groups: vec![group(5, "B6")],
        },
    ];
    records.splice(ends + 1..ends + 1, picture);
    mutate(&mut records);
    let mut text = String::new();
    for record in records {
        text.push_str(&format!("0\r\n{}\r\n", record.name));
        for g in &record.groups {
            text.push_str(&format!("{}\r\n{}\r\n", g.code, g.value));
        }
    }
    text.push_str("0\r\nEOF\r\n");
    text.into_bytes()
}

fn inserts(doc: &opendraft_core::Document) -> Vec<Insert> {
    doc.objects()
        .iter()
        .filter_map(|o| match &o.entity {
            Entity::Insert(i) => Some((**i).clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_definition_comes_in_with_its_parameter_and_action() {
    let loaded = read(&dxf(false, |_| {})).unwrap();
    loaded.document.validate().unwrap();
    let slide = loaded.document.block_named("Slide").unwrap();
    let schema = slide.dynamic.as_deref().expect("dynamic");
    assert_eq!(schema.parameters.len(), 1);
    let p = &schema.parameters[0];
    assert_eq!((p.name.as_str(), p.grips), ("Distance1", 1));
    let DynamicParameterKind::Linear { value_set, .. } = &p.kind;
    assert_eq!(*value_set, DynamicValueSet::List(vec![2.5, 5.0, 7.5, 10.0]));
    let DynamicActionKind::Stretch {
        objects,
        frame,
        multiplier,
        ..
    } = &schema.actions[0].kind
    else {
        panic!("a stretch");
    };
    assert_eq!(objects.len(), 1);
    assert!(slide.entities.iter().any(|e| e.id == objects[0]));
    assert_eq!((frame.min.x, frame.max.x, *multiplier), (1.0, 6.0, 1.0));
}

#[test]
fn a_modified_reference_becomes_a_reference_to_the_definition_with_its_value() {
    let loaded = read(&dxf(false, |_| {})).unwrap();
    let doc = &loaded.document;
    assert!(
        doc.blocks().iter().all(|b| !b.name.contains('U')),
        "the anonymous block is not kept"
    );
    let refs = inserts(doc);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].block.name, "Slide");
    let state = refs[0].dynamic.as_deref().expect("a state");
    assert_eq!(state.values[0].value, DynamicValue::Distance(10.0));
    // The picture follows the value: the stretched line ends at 5 + (10 - 5).
    assert!(
        (refs[0].bounds().max.x - 13.0).abs() < 1e-9,
        "{:?}",
        refs[0].bounds()
    );
    assert_eq!(
        loaded.warnings,
        ["Imported 1 dynamic block with their linear parameters and stretch and move actions: Slide; 1 modified reference rebuilt from recorded values."]
    );
}

#[test]
fn a_lookup_table_is_named_but_does_not_stop_a_reference_being_rebuilt() {
    let loaded = read(&dxf(true, |_| {})).unwrap();
    let doc = &loaded.document;
    assert!(doc.block_named("Slide").is_some_and(|b| b.is_dynamic()));
    let refs = inserts(doc);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].block.name, "Slide");
    assert!(refs[0].dynamic.is_some());
    assert!(
        loaded
            .warnings
            .iter()
            .any(|w| w.contains("lookup action, lookup parameter")),
        "{:?}",
        loaded.warnings
    );
}

#[test]
fn a_parameter_the_app_cannot_represent_leaves_the_reference_its_picture() {
    let loaded = read(&dxf(true, |records| {
        for r in records
            .iter_mut()
            .filter(|r| r.name == "BLOCKLOOKUPPARAMETER")
        {
            r.name = "BLOCKFLIPPARAMETER".into();
        }
    }))
    .unwrap();
    let doc = &loaded.document;
    assert!(doc.block_named("Slide").is_some_and(|b| b.is_dynamic()));
    let refs = inserts(doc);
    assert_eq!(refs.len(), 1);
    assert!(refs[0].dynamic.is_none());
    assert!(!refs[0].block.is_dynamic(), "the anonymous block's picture");
    assert!(
        loaded.warnings.iter().any(|w| w.contains("flip parameter")),
        "{:?}",
        loaded.warnings
    );
}

#[test]
fn a_definition_whose_objects_are_not_there_is_kept_as_a_picture() {
    let loaded = read(&dxf(false, |records| {
        for r in records.iter_mut().filter(|r| r.name == "LINE") {
            r.groups[0] = group(5, "ELSEWHERE");
        }
    }))
    .unwrap();
    assert!(loaded
        .document
        .block_named("Slide")
        .is_some_and(|b| !b.is_dynamic()));
    assert!(
        loaded
            .warnings
            .iter()
            .any(|w| w.starts_with("Kept the default picture of 1 dynamic block: Slide.")),
        "{:?}",
        loaded.warnings
    );
}

#[test]
fn a_value_the_parameter_does_not_allow_is_brought_into_its_set() {
    let loaded = read(&dxf(false, |records| {
        for r in records.iter_mut().filter(|r| r.name == "XRECORD") {
            if let Some(g) = r
                .groups
                .iter_mut()
                .find(|g| g.code == 10 && g.value == "10")
            {
                g.value = "9".into();
            }
        }
    }))
    .unwrap();
    let refs = inserts(&loaded.document);
    let state = refs[0].dynamic.as_deref().expect("a state");
    assert_eq!(
        state.values[0].value,
        DynamicValue::Distance(10.0),
        "nearest of 7.5 and 10"
    );
}

#[test]
fn a_file_with_no_dynamic_block_has_no_note() {
    let loaded = read(b"0\r\nSECTION\r\n2\r\nENTITIES\r\n0\r\nENDSEC\r\n0\r\nEOF\r\n").unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
}
