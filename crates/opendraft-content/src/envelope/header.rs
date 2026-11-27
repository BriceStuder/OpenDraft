//! The generic header (specification 5.1): the same fields for every kind of
//! content and every Open app. `RawHeader` is the file's shape (unknown
//! optional fields are ignored); `Header` is what passed the rules.
//!
//! The numeric limits not written in the specification (title, author name,
//! URL lengths, counts of authors) are starting figures chosen here, and the
//! hub's contract (docs/openhub-api.md, slice 3) has to agree with them.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::image::{decode_thumbnail, encode_base64};
use super::licence::Licence;
use super::semver::Version;
use super::text::{check_line, check_paragraphs};

pub const MAX_TITLE: usize = 80;
pub const MAX_SUMMARY: usize = 140;
pub const MAX_COPYRIGHT: usize = 140;
/// Bytes, as the specification gives it ("8 KB").
pub const MAX_DESCRIPTION_BYTES: usize = 8 * 1024;
pub const MAX_AUTHORS: usize = 10;
pub const MAX_AUTHOR_NAME: usize = 80;
pub const MAX_KEYWORDS: usize = 10;
pub const MIN_KEYWORD: usize = 2;
pub const MAX_KEYWORD: usize = 24;
pub const MAX_URL: usize = 200;
/// The author of the personal package, reserved (specification 5.1).
pub const LOCAL_AUTHOR: &str = "local";
pub const PERSONAL_SLUG: &str = "personal";

/// A header field that broke a rule, named as it is in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderError {
    pub field: String,
    pub reason: String,
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "header `{}` {}", self.field, self.reason)
    }
}

fn bad(field: &str, reason: impl fmt::Display) -> HeaderError {
    HeaderError {
        field: field.to_owned(),
        reason: reason.to_string(),
    }
}

/// `author/slug`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageName {
    author: String,
    slug: String,
}

impl PackageName {
    pub fn parse(text: &str) -> Option<PackageName> {
        let (author, slug) = text.split_once('/')?;
        let ok = |part: &str| {
            (3..=40).contains(&part.len())
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        };
        (ok(author) && ok(slug)).then(|| PackageName {
            author: author.to_owned(),
            slug: slug.to_owned(),
        })
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }

    /// `local/personal`: the user's own blocks.
    pub fn is_personal(&self) -> bool {
        self.author == LOCAL_AUTHOR && self.slug == PERSONAL_SLUG
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.author, self.slug)
    }
}

/// `app.name`, such as `opendraft.blocks`: what the payload is, and so which
/// app reads it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Kind {
    app: String,
    name: String,
}

impl Kind {
    pub fn parse(text: &str) -> Option<Kind> {
        let (app, name) = text.split_once('.')?;
        let ok = |part: &str| {
            (2..=32).contains(&part.len())
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        };
        (ok(app) && ok(name)).then(|| Kind {
            app: app.to_owned(),
            name: name.to_owned(),
        })
    }

    /// The prefix, used to say "needs another app".
    pub fn app(&self) -> &str {
        &self.app
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.app, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// The header as written in the file. Unknown fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawHeader {
    pub name: String,
    pub version: String,
    pub title: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub kind: String,
    pub kind_version: u32,
    pub license: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<Author>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

/// A header that passed every rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub name: PackageName,
    pub version: Version,
    pub title: String,
    pub summary: String,
    pub description: String,
    pub kind: Kind,
    pub kind_version: u32,
    pub license: Licence,
    pub authors: Vec<Author>,
    pub keywords: Vec<String>,
    pub repository: Option<String>,
    pub copyright: Option<String>,
    /// The author's thumbnail as PNG bytes (shape checked, never decoded).
    pub thumbnail: Option<Vec<u8>>,
}

impl Header {
    pub fn check(raw: &RawHeader) -> Result<Header, HeaderError> {
        let name = PackageName::parse(&raw.name).ok_or_else(|| {
            bad(
                "name",
                "must be author/slug, each 3 to 40 of a-z, 0-9 and -",
            )
        })?;
        let version = Version::parse(&raw.version)
            .ok_or_else(|| bad("version", "must be MAJOR.MINOR.PATCH, with nothing after"))?;
        let license = Licence::from_spdx(&raw.license)
            .ok_or_else(|| bad("license", "is not on the allowed list"))?;
        // The author `local` is reserved for the personal package, which is
        // never published and so grants no licence.
        let personal = name.author() == LOCAL_AUTHOR;
        if personal != (license == Licence::Personal) || (personal && !name.is_personal()) {
            return Err(bad(
                "name",
                "uses the reserved author `local` only for local/personal",
            ));
        }
        let kind = Kind::parse(&raw.kind)
            .ok_or_else(|| bad("kind", "must be app.name, such as opendraft.blocks"))?;

        line("title", &raw.title, MAX_TITLE)?;
        line("summary", &raw.summary, MAX_SUMMARY)?;
        if !raw.description.is_empty() {
            if raw.description.len() > MAX_DESCRIPTION_BYTES {
                return Err(bad("description", "is longer than 8 KB"));
            }
            check_paragraphs(&raw.description, MAX_DESCRIPTION_BYTES)
                .map_err(|e| bad("description", e))?;
        }
        if let Some(copyright) = &raw.copyright {
            line("copyright", copyright, MAX_COPYRIGHT)?;
        }
        if let Some(url) = &raw.repository {
            https_url("repository", url)?;
        }

        if raw.authors.len() > MAX_AUTHORS {
            return Err(bad("authors", format!("has more than {MAX_AUTHORS}")));
        }
        for author in &raw.authors {
            line("authors.name", &author.name, MAX_AUTHOR_NAME)?;
            if let Some(url) = &author.url {
                https_url("authors.url", url)?;
            }
        }

        if raw.keywords.len() > MAX_KEYWORDS {
            return Err(bad("keywords", format!("has more than {MAX_KEYWORDS}")));
        }
        for keyword in &raw.keywords {
            line("keywords", keyword, MAX_KEYWORD)?;
            let short = keyword.chars().count() < MIN_KEYWORD;
            if short
                || keyword.chars().any(char::is_whitespace)
                || *keyword != keyword.to_lowercase()
            {
                return Err(bad(
                    "keywords",
                    format!("{keyword:?} must be lowercase, with no spaces, 2 to 24 characters"),
                ));
            }
        }

        let thumbnail = match &raw.thumbnail {
            Some(text) => Some(decode_thumbnail(text).map_err(|e| bad("thumbnail", e))?),
            None => None,
        };

        Ok(Header {
            name,
            version,
            title: raw.title.clone(),
            summary: raw.summary.clone(),
            description: raw.description.clone(),
            kind,
            kind_version: raw.kind_version,
            license,
            authors: raw.authors.clone(),
            keywords: raw.keywords.clone(),
            repository: raw.repository.clone(),
            copyright: raw.copyright.clone(),
            thumbnail,
        })
    }

    pub fn to_raw(&self) -> RawHeader {
        RawHeader {
            name: self.name.to_string(),
            version: self.version.to_string(),
            title: self.title.clone(),
            summary: self.summary.clone(),
            description: self.description.clone(),
            kind: self.kind.to_string(),
            kind_version: self.kind_version,
            license: self.license.spdx().to_owned(),
            authors: self.authors.clone(),
            keywords: self.keywords.clone(),
            repository: self.repository.clone(),
            copyright: self.copyright.clone(),
            thumbnail: self.thumbnail.as_deref().map(encode_base64),
        }
    }
}

fn line(field: &str, text: &str, max: usize) -> Result<(), HeaderError> {
    check_line(text, max).map_err(|e| bad(field, e))
}

/// An `https` address, shown as text and opened only after a click, so it is
/// plain ASCII (a non-ASCII host is a look-alike risk; punycode is written
/// out), has no credentials and no whitespace.
fn https_url(field: &str, url: &str) -> Result<(), HeaderError> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| bad(field, "must be an https address"))?;
    if url.len() > MAX_URL {
        return Err(bad(field, format!("is longer than {MAX_URL} characters")));
    }
    if !url.bytes().all(|b| b.is_ascii_graphic()) || url.contains('\\') {
        return Err(bad(field, "must be plain ASCII with no spaces"));
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit_once(':').map_or(authority, |(h, _)| h);
    let host_ok = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-']);
    if !host_ok || authority.contains('@') {
        return Err(bad(field, "has no valid host"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "header_tests.rs"]
pub(crate) mod tests;
