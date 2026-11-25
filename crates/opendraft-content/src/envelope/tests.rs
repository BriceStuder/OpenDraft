use serde_json::json;

use super::header::tests::sample;
use super::*;

fn envelope() -> Envelope {
    Envelope {
        header: Header::check(&sample()).expect("valid"),
        package: json!({ "blocks": [{ "name": "gate", "objects": [] }] }),
    }
}

/// A file with `header` as in `sample` and these top-level changes.
fn file(change: impl FnOnce(&mut serde_json::Map<String, Value>)) -> Vec<u8> {
    let bytes = envelope().to_bytes().expect("writes");
    let Value::Object(mut top) = serde_json::from_slice(&bytes).expect("json") else {
        unreachable!("an envelope is an object")
    };
    change(&mut top);
    serde_json::to_vec(&Value::Object(top)).expect("json")
}

#[test]
fn an_envelope_round_trips_to_the_same_bytes() {
    let bytes = envelope().to_bytes().expect("writes");
    let read = Envelope::read(&bytes).expect("reads");
    assert_eq!(read, envelope());
    assert_eq!(read.to_bytes().expect("writes"), bytes);
    // Readable by eye: indented, the header's name first.
    let text = String::from_utf8(bytes).expect("utf-8");
    assert!(
        text.contains("\n    \"name\": \"acme/piping-valves\""),
        "{text}"
    );
}

#[test]
fn the_same_inputs_make_the_same_bytes() {
    let a = envelope().to_bytes().expect("writes");
    let b = envelope().to_bytes().expect("writes");
    assert_eq!(file_hash(&a), file_hash(&b));
}

#[test]
fn the_hash_is_sha256_of_the_exact_bytes() {
    assert_eq!(
        file_hash(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(file_hash(b"").len(), 64);
}

#[test]
fn the_byte_cap_is_checked_before_parsing() {
    let big = vec![b'{'; MAX_BYTES + 1];
    assert_eq!(
        Envelope::read(&big),
        Err(EnvelopeError::TooLarge { max: MAX_BYTES })
    );
}

#[test]
fn the_format_and_its_version_are_checked_first() {
    let read = |change: fn(&mut serde_json::Map<String, Value>)| Envelope::read(&file(change));
    assert_eq!(
        read(|t| {
            t.insert("format".into(), "odraft".into());
        }),
        Err(EnvelopeError::UnknownFormat)
    );
    assert_eq!(
        read(|t| {
            t.remove("format");
        }),
        Err(EnvelopeError::UnknownFormat)
    );
    assert_eq!(
        read(|t| {
            t.insert("format_version".into(), 2.into());
        }),
        Err(EnvelopeError::UnsupportedFormatVersion(Some(2)))
    );
    assert_eq!(
        read(|t| {
            t.insert("format_version".into(), "1".into());
        }),
        Err(EnvelopeError::UnsupportedFormatVersion(None))
    );
    // A newer format is reported as such even when its other fields are new.
    assert_eq!(
        read(|t| {
            t.insert("format_version".into(), 2.into());
            t.insert("header".into(), json!({ "from": "the future" }));
        }),
        Err(EnvelopeError::UnsupportedFormatVersion(Some(2)))
    );
}

#[test]
fn the_top_level_is_strict() {
    let read = |change: fn(&mut serde_json::Map<String, Value>)| Envelope::read(&file(change));
    assert_eq!(
        read(|t| {
            t.insert("signature".into(), "x".into());
        }),
        Err(EnvelopeError::UnknownField("signature".into()))
    );
    assert_eq!(
        read(|t| {
            t.remove("header");
        }),
        Err(EnvelopeError::Missing("header"))
    );
    assert_eq!(
        read(|t| {
            t.remove("package");
        }),
        Err(EnvelopeError::Missing("package"))
    );
    assert_eq!(Envelope::read(b"[]"), Err(EnvelopeError::NotAnObject));
    assert_eq!(Envelope::read(b"3"), Err(EnvelopeError::NotAnObject));
}

#[test]
fn unknown_header_fields_are_ignored_but_a_bad_known_one_is_not() {
    let ok = Envelope::read(&file(|t| {
        if let Some(Value::Object(h)) = t.get_mut("header") {
            h.insert("language".into(), "fr".into());
        }
    }));
    assert_eq!(ok.expect("reads").header, envelope().header);

    let bad = Envelope::read(&file(|t| {
        if let Some(Value::Object(h)) = t.get_mut("header") {
            h.insert("kind_version".into(), "one".into());
        }
    }));
    assert!(matches!(bad, Err(EnvelopeError::Header(_))), "{bad:?}");
}

#[test]
fn a_header_that_breaks_a_rule_is_refused_with_the_field() {
    let bad = Envelope::read(&file(|t| {
        if let Some(Value::Object(h)) = t.get_mut("header") {
            h.insert("license".into(), "GPL-3.0-or-later".into());
        }
    }));
    let Err(EnvelopeError::Header(e)) = bad else {
        unreachable!("refused: {bad:?}")
    };
    assert_eq!(e.field, "license");
}

#[test]
fn an_unknown_kind_reads_and_its_payload_is_left_alone() {
    let read = Envelope::read(&file(|t| {
        if let Some(Value::Object(h)) = t.get_mut("header") {
            h.insert("kind".into(), "opencalc.template".into());
            h.insert("kind_version".into(), 99.into());
        }
        t.insert(
            "package".into(),
            json!({ "anything": [1, 2, { "at": "all" }] }),
        );
    }))
    .expect("reads");
    assert_eq!(read.header.kind.app(), "opencalc");
    assert_eq!(read.package, json!({ "anything": [1, 2, { "at": "all" }] }));
}

#[test]
fn a_duplicate_key_anywhere_is_refused() {
    let text =
        br#"{"format":"odpack","format_version":1,"format":"odpack","header":{},"package":{}}"#;
    assert!(matches!(
        Envelope::read(text),
        Err(EnvelopeError::Json(JsonError::DuplicateKey(_)))
    ));
    let inside = br#"{"format":"odpack","format_version":1,"header":{},"package":{"a":1,"a":2}}"#;
    assert!(matches!(
        Envelope::read(inside),
        Err(EnvelopeError::Json(JsonError::DuplicateKey(_)))
    ));
}

#[test]
fn deep_and_wide_files_are_refused_not_crashed_on() {
    let deep = format!(
        r#"{{"format":"odpack","format_version":1,"header":{{}},"package":{}{}}}"#,
        "[".repeat(MAX_DEPTH + 1),
        "]".repeat(MAX_DEPTH + 1)
    );
    assert!(matches!(
        Envelope::read(deep.as_bytes()),
        Err(EnvelopeError::Json(JsonError::TooDeep { .. }))
    ));
    // Nothing but opening brackets, a megabyte of them.
    assert!(Envelope::read(&vec![b'['; 1 << 20]).is_err());

    // Under the byte cap but over the value cap: two bytes per value.
    let wide = format!(
        r#"{{"format":"odpack","format_version":1,"header":{{}},"package":[{}0]}}"#,
        "0,".repeat(MAX_VALUES)
    );
    assert!(wide.len() < MAX_BYTES);
    assert!(matches!(
        Envelope::read(wide.as_bytes()),
        Err(EnvelopeError::Json(JsonError::TooManyValues { .. }))
    ));
}

#[test]
fn every_truncation_of_a_good_file_is_refused() {
    let bytes = envelope().to_bytes().expect("writes");
    // The final newline is not part of the JSON, so cut at least one more.
    for len in 0..bytes.len() - 1 {
        assert!(Envelope::read(&bytes[..len]).is_err(), "cut at {len}");
    }
}

#[test]
fn a_corrupted_file_never_panics() {
    let bytes = envelope().to_bytes().expect("writes");
    for at in 0..bytes.len() {
        for replacement in [0u8, b'"', b'{', b'}', b'\\', 0xFF, b'9'] {
            let mut copy = bytes.clone();
            copy[at] = replacement;
            let _ = Envelope::read(&copy);
        }
    }
}

#[test]
fn invalid_utf8_is_refused() {
    let mut bytes = envelope().to_bytes().expect("writes");
    let at = bytes
        .windows(5)
        .position(|w| w == b"Acme ")
        .expect("author name present");
    bytes[at] = 0xC0;
    assert!(Envelope::read(&bytes).is_err());
}

#[test]
fn a_file_is_not_written_that_could_not_be_read() {
    let mut huge = envelope();
    huge.package = json!({ "blocks": "x".repeat(MAX_BYTES) });
    assert_eq!(
        huge.to_bytes(),
        Err(EnvelopeError::TooLarge { max: MAX_BYTES })
    );
}
