//! A DXF's rotation, flip, scale and polar parameters and actions coming in as
//! dynamic blocks, on the same fixture as the linear ones.

use opendraft_core::block::{
    DynamicActionKind, DynamicParameterKind, DynamicValue, DynamicValueSet,
};

use super::tests::{dxf, group, inserts};
use crate::dxf::read::{read, Record};

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

/// The fixture turned into a polar parameter: its linear record becomes a
/// polar one with no listed values, and the stretch follows it.
fn polar(mutate: impl Fn(&mut Record)) -> opendraft_core::Document {
    read(&dxf(false, |records| {
        for r in records.iter_mut() {
            if r.name == "BLOCKLINEARPARAMETER" {
                r.name = "BLOCKPOLARPARAMETER".into();
                r.groups.retain(|g| g.code != 144);
                for g in &mut r.groups {
                    if g.code == 175 {
                        g.value = "0".into();
                    }
                }
            }
            mutate(r);
        }
    }))
    .unwrap()
    .document
}

#[test]
fn a_polar_parameter_comes_in_and_a_reference_to_it_keeps_where_its_end_was() {
    let doc = polar(|_| {});
    let slide = doc.block_named("Slide").unwrap();
    let schema = slide.dynamic.as_deref().expect("dynamic");
    assert!(matches!(
        schema.parameters[0].kind,
        DynamicParameterKind::Polar { .. }
    ));
    let refs = inserts(&doc);
    assert_eq!(refs.len(), 1);
    let state = refs[0].dynamic.as_deref().expect("a state");
    assert_eq!(
        state.values[0].value,
        DynamicValue::Point(opendraft_core::Point::new(10.0, 0.0))
    );
    // The end went from 5 to 10: the line it holds is 10 long.
    let b = refs[0].bounds();
    assert!((b.max.x - b.min.x - 10.0).abs() < 1e-9, "{b:?}");
}

#[test]
fn a_polar_angle_increment_comes_in_and_a_list_of_angles_is_left_out() {
    let doc = polar(|r| {
        if r.name == "BLOCKPOLARPARAMETER" {
            r.groups.extend([
                group(145, "0"),
                group(146, "6.283185307179586"),
                group(147, "0.0872664625997165"),
            ]);
        }
    });
    let slide = doc.block_named("Slide").unwrap();
    let DynamicParameterKind::Polar { angle_set, .. } =
        &slide.dynamic.as_deref().unwrap().parameters[0].kind
    else {
        panic!("a polar parameter");
    };
    assert!(matches!(
        angle_set,
        DynamicValueSet::Range { increment: Some(i), .. } if (i - 0.0872664625997165).abs() < 1e-12
    ));
    let loaded = read(&dxf(false, |records| {
        for r in records.iter_mut() {
            if r.name == "BLOCKLINEARPARAMETER" {
                r.name = "BLOCKPOLARPARAMETER".into();
            }
        }
    }))
    .unwrap();
    assert!(loaded
        .warnings
        .iter()
        .any(|w| w.contains("polar parameter with a list of distances or angles")));
}
