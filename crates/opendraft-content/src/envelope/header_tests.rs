use super::*;
use crate::envelope::image::fake_png;

pub(crate) fn sample() -> RawHeader {
    RawHeader {
        name: "acme/piping-valves".into(),
        version: "2.1.0".into(),
        title: "Piping valves".into(),
        summary: "Gate, globe and check valves.".into(),
        description: "Valves to ASME.\n\n- gate\n- globe".into(),
        kind: "opendraft.blocks".into(),
        kind_version: 1,
        license: "CC-BY-4.0".into(),
        authors: vec![Author {
            name: "Acme Ltd".into(),
            url: Some("https://acme.example/valves".into()),
        }],
        keywords: vec!["valve".into(), "piping".into()],
        repository: Some("https://github.com/acme/valves".into()),
        copyright: Some("Copyright 2026 Acme Ltd".into()),
        thumbnail: Some(encode_base64(&fake_png(128, 128))),
    }
}

/// The field `change` breaks, as the error names it.
fn refused(change: impl FnOnce(&mut RawHeader)) -> String {
    let mut raw = sample();
    change(&mut raw);
    Header::check(&raw).expect_err("refused").field
}

#[test]
fn a_full_header_passes_and_round_trips() {
    let header = Header::check(&sample()).expect("valid");
    assert_eq!(header.name.author(), "acme");
    assert_eq!(header.name.slug(), "piping-valves");
    assert_eq!(header.kind.app(), "opendraft");
    assert_eq!(header.to_raw(), sample());
}

#[test]
fn only_the_required_fields_are_needed() {
    let json = r#"{"name":"acme/nuts","version":"0.1.0","title":"Nuts","summary":"s",
        "kind":"opendraft.blocks","kind_version":0,"license":"MIT"}"#;
    let raw: RawHeader = serde_json::from_str(json).expect("reads");
    let header = Header::check(&raw).expect("valid");
    assert!(header.authors.is_empty() && header.thumbnail.is_none());
}

#[test]
fn unknown_header_fields_are_ignored() {
    let json = r#"{"name":"acme/nuts","version":"0.1.0","title":"Nuts","summary":"s",
        "kind":"opendraft.blocks","kind_version":0,"license":"MIT","language":"en"}"#;
    assert!(serde_json::from_str::<RawHeader>(json).is_ok());
}

#[test]
fn a_missing_required_field_is_refused() {
    let json = r#"{"name":"acme/nuts","version":"0.1.0","title":"Nuts","summary":"s",
        "kind":"opendraft.blocks","kind_version":0}"#;
    assert!(serde_json::from_str::<RawHeader>(json).is_err());
}

#[test]
fn names_follow_the_pattern() {
    for good in ["abc/def", "acme/piping-valves", "a1b/2c3", "---/---"] {
        assert!(PackageName::parse(good).is_some(), "{good}");
    }
    let long = "x".repeat(41);
    for bad in [
        "ab/cde",
        "abc/de",
        "abc",
        "abc/def/ghi",
        "ABC/def",
        "abc/d_f",
        "abc/déf",
        "/abc",
        &format!("{long}/abc"),
        "abc def/ghi",
    ] {
        assert!(PackageName::parse(bad).is_none(), "{bad}");
    }
    assert!(PackageName::parse(&format!("{}/abc", "x".repeat(40))).is_some());
}

#[test]
fn kinds_are_app_dot_name() {
    let kind = Kind::parse("opencalc.template").expect("valid");
    assert_eq!((kind.app(), kind.name()), ("opencalc", "template"));
    for bad in [
        "blocks",
        "a.b",
        ".blocks",
        "Open.blocks",
        "a.b.c",
        "open draft.x",
    ] {
        assert!(Kind::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn each_rule_names_the_field_it_broke() {
    assert_eq!(refused(|h| h.name = "nope".into()), "name");
    assert_eq!(refused(|h| h.version = "1.0.0-beta".into()), "version");
    assert_eq!(refused(|h| h.kind = "blocks".into()), "kind");
    assert_eq!(
        refused(|h| h.license = "GPL-3.0-or-later".into()),
        "license"
    );
    assert_eq!(refused(|h| h.title = String::new()), "title");
    assert_eq!(refused(|h| h.title = "x".repeat(81)), "title");
    assert_eq!(refused(|h| h.summary = "x".repeat(141)), "summary");
    assert_eq!(refused(|h| h.summary = "two\nlines".into()), "summary");
    assert_eq!(
        refused(|h| h.copyright = Some("x".repeat(141))),
        "copyright"
    );
    assert_eq!(refused(|h| h.title = "e\u{301}".into()), "title");
    assert_eq!(refused(|h| h.title = "a\u{202E}b".into()), "title");
}

#[test]
fn the_description_is_capped_in_bytes_and_may_have_paragraphs() {
    let mut raw = sample();
    raw.description = "é".repeat(4096);
    assert!(Header::check(&raw).is_ok());
    raw.description = "é".repeat(4097);
    assert_eq!(Header::check(&raw).expect_err("long").field, "description");
    raw.description = "a\tb".into();
    assert_eq!(Header::check(&raw).expect_err("tab").field, "description");
}

#[test]
fn keywords_follow_the_rules() {
    assert_eq!(refused(|h| h.keywords = vec!["Valve".into()]), "keywords");
    assert_eq!(refused(|h| h.keywords = vec!["v".into()]), "keywords");
    assert_eq!(refused(|h| h.keywords = vec!["x".repeat(25)]), "keywords");
    assert_eq!(
        refused(|h| h.keywords = vec!["two words".into()]),
        "keywords"
    );
    assert_eq!(
        refused(|h| h.keywords = (0..11).map(|i| format!("key{i}")).collect()),
        "keywords"
    );
    let mut raw = sample();
    raw.keywords = (0..10).map(|i| format!("key{i}")).collect();
    assert!(Header::check(&raw).is_ok());
}

#[test]
fn authors_are_counted_and_their_urls_checked() {
    let author = |url: Option<&str>| Author {
        name: "A".into(),
        url: url.map(str::to_owned),
    };
    assert_eq!(
        refused(|h| h.authors = (0..11).map(|_| author(None)).collect()),
        "authors"
    );
    assert_eq!(
        refused(|h| h.authors = vec![author(Some("http://a.example"))]),
        "authors.url"
    );
    assert_eq!(
        refused(|h| h.authors = vec![Author {
            name: String::new(),
            url: None
        }]),
        "authors.name"
    );
}

#[test]
fn urls_must_be_plain_https_with_a_real_host() {
    for good in [
        "https://example.com",
        "https://example.com/a/b?x=1#y",
        "https://a-b.example:8443/x",
        "https://127.0.0.1/",
    ] {
        assert!(https_url("repository", good).is_ok(), "{good}");
    }
    let long = format!("https://example.com/{}", "a".repeat(200));
    for bad in [
        "http://example.com",
        "HTTPS://example.com",
        "ftp://example.com",
        "//example.com",
        "https://",
        "https://user@example.com",
        "https://example.com@evil.example",
        "https://exa mple.com",
        "https://exämple.com",
        "https://example.com\\evil",
        "https://-example.com",
        "https://.com",
        "https://example.com\n",
        "javascript:alert(1)",
        &long,
    ] {
        assert!(https_url("repository", bad).is_err(), "{bad:?}");
    }
}

#[test]
fn the_thumbnail_is_checked_in_the_header() {
    assert_eq!(
        refused(|h| h.thumbnail = Some("not base64!".into())),
        "thumbnail"
    );
    assert_eq!(
        refused(|h| h.thumbnail = Some(encode_base64(&fake_png(300, 10)))),
        "thumbnail"
    );
}

#[test]
fn the_local_author_is_for_the_personal_package_only() {
    let mut raw = sample();
    raw.name = "local/personal".into();
    raw.license = Licence::Personal.spdx().into();
    raw.version = "0.0.7".into();
    assert!(Header::check(&raw).expect("personal").name.is_personal());

    // A published licence on the personal package, or the personal marker on
    // anyone else's, or another package by `local`: all refused.
    raw.license = "MIT".into();
    assert!(Header::check(&raw).is_err());
    raw.license = Licence::Personal.spdx().into();
    raw.name = "acme/piping-valves".into();
    assert!(Header::check(&raw).is_err());
    raw.name = "local/valves".into();
    assert!(Header::check(&raw).is_err());
}
