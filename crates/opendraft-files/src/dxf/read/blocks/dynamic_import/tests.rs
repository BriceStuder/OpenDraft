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
    let DynamicParameterKind::Linear { value_set, .. } = &p.kind else {
        panic!("a linear parameter");
    };
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
        ["Imported 1 dynamic block with their linear, rotation and flip parameters and stretch, move, scale, rotate and flip actions: Slide; 1 modified reference rebuilt from recorded values."]
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

#[test]
fn an_imported_drawing_survives_being_saved_and_opened_again() {
    use crate::{FileFormat, NativeFormat};
    let doc = read(&dxf(false, |_| {})).unwrap().document;
    let bytes = NativeFormat.write(&doc).unwrap().bytes;
    let back = NativeFormat.read(&bytes).unwrap().document;
    assert_eq!(back.blocks(), doc.blocks());
    assert_eq!(back.objects(), doc.objects());
    back.validate().unwrap();
    let refs = inserts(&back);
    assert_eq!(refs.len(), 1);
    assert_eq!(
        refs[0].dynamic.as_deref().unwrap().values[0].value,
        DynamicValue::Distance(10.0)
    );
    assert!((refs[0].bounds().max.x - 13.0).abs() < 1e-9);
}

/// The fixture turned into a rotation: the linear parameter becomes a
/// rotation about (0, 0), the stretch a rotate, and the modified reference
/// has its grip straight up.
fn turned(mutate: impl Fn(&mut Record)) -> opendraft_core::Document {
    let loaded = read(&dxf(false, |records| {
        for r in records.iter_mut() {
            match r.name.as_str() {
                "BLOCKLINEARPARAMETER" => {
                    r.name = "BLOCKROTATIONPARAMETER".into();
                    r.groups.retain(|g| g.code != 144);
                    for g in &mut r.groups {
                        if g.code == 175 {
                            g.value = "0".into();
                        }
                    }
                }
                "BLOCKSTRETCHACTION" => {
                    r.name = "BLOCKROTATEACTION".into();
                    for g in &mut r.groups {
                        match (g.code, g.value.as_str()) {
                            (1011, "1") => g.value = "0".into(),
                            (1021, "-1") => g.value = "0".into(),
                            _ => {}
                        }
                    }
                }
                "XRECORD" if r.groups.iter().any(|g| g.code == 5 && g.value == "XR6") => {
                    // The second point is where the grip is: straight up.
                    let at = |r: &Record, code| {
                        r.groups
                            .iter()
                            .enumerate()
                            .filter(|(_, g)| g.code == code)
                            .map(|(i, _)| i)
                            .nth(1)
                    };
                    if let (Some(x), Some(y)) = (at(r, 10), at(r, 20)) {
                        r.groups[x].value = "0".into();
                        r.groups[y].value = "10".into();
                    }
                }
                _ => {}
            }
            mutate(r);
        }
    }))
    .unwrap();
    loaded.document
}

#[test]
fn a_rotation_comes_in_with_its_rotate_action_and_a_turned_reference_keeps_its_angle() {
    let doc = turned(|_| {});
    let slide = doc.block_named("Slide").unwrap();
    let schema = slide.dynamic.as_deref().expect("dynamic");
    assert!(matches!(
        schema.parameters[0].kind,
        DynamicParameterKind::Rotation { radius, .. } if (radius - 5.0).abs() < 1e-9
    ));
    assert!(matches!(
        schema.actions[0].kind,
        DynamicActionKind::Rotate { .. }
    ));
    let refs = inserts(&doc);
    assert_eq!(refs.len(), 1);
    let state = refs[0].dynamic.as_deref().expect("a state");
    let DynamicValue::Angle(angle) = state.values[0].value else {
        panic!("an angle");
    };
    assert!(
        (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "{angle}"
    );
}

#[test]
fn a_rotation_with_a_list_of_angles_is_left_as_a_picture() {
    let loaded = read(&dxf(false, |records| {
        for r in records.iter_mut() {
            if r.name == "BLOCKLINEARPARAMETER" {
                r.name = "BLOCKROTATIONPARAMETER".into();
            }
        }
    }))
    .unwrap();
    assert!(loaded
        .warnings
        .iter()
        .any(|w| w.contains("rotation parameter with a list or increment of angles")));
}

#[test]
fn a_flip_comes_in_but_its_modified_references_keep_their_pictures() {
    let loaded = read(&dxf(false, |records| {
        for r in records.iter_mut() {
            match r.name.as_str() {
                "BLOCKLINEARPARAMETER" => {
                    r.name = "BLOCKFLIPPARAMETER".into();
                    r.groups.retain(|g| g.code != 144);
                }
                "BLOCKSTRETCHACTION" => r.name = "BLOCKFLIPACTION".into(),
                _ => {}
            }
        }
    }))
    .unwrap();
    let slide = loaded.document.block_named("Slide").unwrap();
    let schema = slide.dynamic.as_deref().expect("dynamic");
    assert!(matches!(
        schema.parameters[0].kind,
        DynamicParameterKind::Flip { .. }
    ));
    assert!(matches!(
        schema.actions[0].kind,
        DynamicActionKind::Flip { .. }
    ));
    // Whether a reference is flipped is not read from the file yet.
    let refs = inserts(&loaded.document);
    assert_eq!(refs.len(), 1);
    assert!(refs[0].dynamic.is_none());
    assert!(!refs[0].block.is_dynamic(), "the anonymous block's picture");
}

#[test]
fn a_scale_comes_in_and_a_reference_to_it_is_rebuilt_with_its_value() {
    let loaded = read(&dxf(false, |records| {
        for r in records.iter_mut() {
            if r.name == "BLOCKSTRETCHACTION" {
                r.name = "BLOCKSCALEACTION".into();
                // It enlarges about the parameter's start, (0, 0).
                for g in &mut r.groups {
                    match (g.code, g.value.as_str()) {
                        (1011, "1") => g.value = "0".into(),
                        (1021, "-1") => g.value = "0".into(),
                        _ => {}
                    }
                }
            }
        }
    }))
    .unwrap();
    let slide = loaded.document.block_named("Slide").unwrap();
    let schema = slide.dynamic.as_deref().expect("dynamic");
    assert!(matches!(
        schema.actions[0].kind,
        DynamicActionKind::Scale { .. }
    ));
    let refs = inserts(&loaded.document);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].block.name, "Slide");
    // The parameter went from 5 to 10: the line, 5 long, is now 10 long.
    let b = refs[0].bounds();
    assert!((b.max.x - b.min.x - 10.0).abs() < 1e-9, "{b:?}");
}

#[test]
fn a_scale_about_a_point_of_its_own_is_left_out() {
    let loaded = read(&dxf(false, |records| {
        for r in records.iter_mut() {
            if r.name == "BLOCKSTRETCHACTION" {
                r.name = "BLOCKSCALEACTION".into();
            }
        }
    }))
    .unwrap();
    assert!(loaded
        .warnings
        .iter()
        .any(|w| w.contains("scale action about a point of its own")));
}
