use serde_json::json;

use super::*;
use crate::envelope::{Envelope, EnvelopeError};
use crate::test_support::{drawing_with, drawing_with_nested, header, radius};

fn package_of(doc: Document, entries: Vec<Entry>) -> BlocksPackage {
    BlocksPackage {
        drawing: doc,
        entries,
    }
}

fn entry(block: BlockId) -> Entry {
    Entry {
        uuid: BlockUuid::new(),
        block,
        folder: String::new(),
        tags: Vec::new(),
        component: false,
    }
}

fn one() -> BlocksPackage {
    let (doc, id) = drawing_with("valve", 5.0);
    package_of(doc, vec![entry(id)])
}

fn invalid(package: &BlocksPackage) -> String {
    match package.validate() {
        Err(PayloadError::Invalid(m)) => m,
        other => unreachable!("expected an invalid package, got {other:?}"),
    }
}

#[test]
fn a_package_round_trips_through_its_file_bytes() {
    let (doc, outer, inner) = drawing_with_nested("bolt", "nut");
    let mut bolt = entry(outer);
    bolt.folder = "Fixings/Metric".into();
    bolt.tags = vec!["bolt".into(), "m8".into()];
    let mut nut = entry(inner);
    nut.component = true;
    let package = package_of(doc, vec![bolt.clone(), nut.clone()]);

    let bytes = package
        .to_envelope(header("local/personal"))
        .map(|e| e.to_bytes().expect("writes"))
        .expect("builds");
    let read = BlocksPackage::from_envelope(&Envelope::read(&bytes).expect("reads")).expect("ok");
    assert_eq!(read.entries, [bolt, nut]);
    let names: Vec<_> = read
        .drawing
        .blocks()
        .iter()
        .map(|b| b.name.as_str())
        .collect();
    assert_eq!(names, ["nut", "bolt"]);
    let bolt = read.drawing.block(outer).expect("kept id");
    assert_eq!(radius(bolt), 2.0);
    assert_eq!(bolt.nested_blocks().next(), Some(inner));
}

#[test]
fn the_same_package_is_the_same_bytes() {
    let package = one();
    let bytes = |p: &BlocksPackage| {
        p.to_envelope(header("local/personal"))
            .expect("builds")
            .to_bytes()
            .expect("writes")
    };
    assert_eq!(bytes(&package), bytes(&package.clone()));
}

#[test]
fn another_kind_or_version_is_not_read() {
    let mut envelope = one().to_envelope(header("local/personal")).expect("builds");
    envelope.header.kind_version = 1;
    assert_eq!(
        BlocksPackage::from_envelope(&envelope).err(),
        Some(PayloadError::KindVersion {
            found: 1,
            supported: 0
        })
    );
    let mut envelope = one().to_envelope(header("local/personal")).expect("builds");
    envelope.header.kind = crate::envelope::Kind::parse("opencalc.template").expect("kind");
    assert!(matches!(
        BlocksPackage::from_envelope(&envelope),
        Err(PayloadError::WrongKind(_))
    ));
}

fn envelope_with(change: impl FnOnce(&mut Value)) -> Envelope {
    let mut envelope = one().to_envelope(header("local/personal")).expect("builds");
    change(&mut envelope.package);
    envelope
}

#[test]
fn the_payload_is_strict() {
    let extra = envelope_with(|p| p["extra"] = json!(1));
    assert!(matches!(
        BlocksPackage::from_envelope(&extra),
        Err(PayloadError::Malformed(_))
    ));
    let extra_entry = envelope_with(|p| p["blocks"][0]["note"] = json!("hi"));
    assert!(matches!(
        BlocksPackage::from_envelope(&extra_entry),
        Err(PayloadError::Malformed(_))
    ));
    let bad_id = envelope_with(|p| p["blocks"][0]["id"] = json!("123"));
    assert!(matches!(
        BlocksPackage::from_envelope(&bad_id),
        Err(PayloadError::Malformed(_))
    ));
}

#[test]
fn a_damaged_drawing_is_reported_as_such() {
    let broken = envelope_with(|p| p["drawing"]["format"] = json!("something else"));
    assert!(matches!(
        BlocksPackage::from_envelope(&broken),
        Err(PayloadError::Drawing(_))
    ));
    let broken = envelope_with(|p| p["drawing"] = json!([]));
    assert!(matches!(
        BlocksPackage::from_envelope(&broken),
        Err(PayloadError::Drawing(_))
    ));
}

#[test]
fn entries_and_blocks_must_match_one_to_one() {
    let mut p = one();
    p.entries.clear();
    assert!(invalid(&p).contains("has no entry"));

    let mut p = one();
    p.entries[0].block = BlockId(999);
    assert!(invalid(&p).contains("not there"));

    let mut p = one();
    let again = p.entries[0].clone();
    p.entries.push(again);
    assert!(invalid(&p).contains("used twice"));

    let mut p = one();
    let mut second = entry(p.entries[0].block);
    second.uuid = BlockUuid::new();
    p.entries.push(second);
    assert!(invalid(&p).contains("two entries"));
}

#[test]
fn folders_tags_and_components_follow_the_rules() {
    let mut p = one();
    p.entries[0].folder = "a/../b".into();
    assert!(invalid(&p).contains("not a folder name"));

    let mut p = one();
    p.entries[0].folder = "a/b/c/d/e/f/g/h/i".into();
    assert!(invalid(&p).contains("levels"));

    let mut p = one();
    p.entries[0].tags = vec![" padded".into()];
    assert!(invalid(&p).contains("tag"));
    p.entries[0].tags = (0..33).map(|i| format!("t{i}")).collect();
    assert!(invalid(&p).contains("more than"));

    let mut p = one();
    p.entries[0].component = true;
    p.entries[0].folder = "x".into();
    assert!(invalid(&p).contains("has a folder"));
}

#[test]
fn block_text_follows_the_plain_text_rules() {
    let mut p = one();
    let mut def = (**p.drawing.block(p.entries[0].block).expect("block")).clone();
    def.description = "two\nparagraphs are fine".into();
    p.drawing.set_blocks(vec![std::sync::Arc::new(def.clone())]);
    assert!(p.validate().is_ok());
    def.description = "bad\u{202E}text".into();
    p.drawing.set_blocks(vec![std::sync::Arc::new(def.clone())]);
    assert!(invalid(&p).contains("description"));
    def.description = String::new();
    def.name = "e\u{301}".into();
    p.drawing.set_blocks(vec![std::sync::Arc::new(def)]);
    assert!(!invalid(&p).is_empty());
}

#[test]
fn a_font_that_names_a_file_location_is_refused() {
    use opendraft_core::text_style::{FontDescriptor, TextStyle, TextStyleId};
    let mut p = one();
    let id = p.drawing.allocate_text_style_id();
    let mut style = TextStyle::new(id, "Plans");
    style.font = FontDescriptor::Shx {
        file_name: r"\\server\share\x.shx".into(),
    };
    p.drawing.push_text_style(style.clone());
    assert!(invalid(&p).contains("font"));

    p.drawing = one().drawing;
    let id = p.drawing.allocate_text_style_id();
    style.id = id;
    style.name = "Plans2".into();
    style.font = FontDescriptor::TrueType {
        family: "C:/Windows/Fonts/arial.ttf".into(),
        bold: false,
        italic: false,
    };
    p.drawing.push_text_style(style);
    assert!(invalid(&p).contains("font"));
    let _ = TextStyleId::STANDARD;
}

#[test]
fn a_full_file_with_a_bad_envelope_is_refused_before_the_payload() {
    assert!(matches!(
        Envelope::read(b"{}"),
        Err(EnvelopeError::UnknownFormat)
    ));
}
