//! The `.odpack` envelope (specification 5): one UTF-8 JSON file with a generic
//! `header` and a kind-specific `package`. This module reads and writes the
//! envelope and never looks inside the payload, so it holds no OpenDraft types
//! and depends only on `serde`, `serde_json`, `sha2` and Unicode tables. It is
//! written to move unchanged into a shared crate (5.3).

use std::fmt;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub mod header;
pub mod image;
pub mod licence;
pub mod semver;
pub mod strict_json;
pub mod text;

pub use header::{Author, Header, HeaderError, Kind, PackageName, RawHeader};
pub use licence::Licence;
pub use semver::Version;
pub use strict_json::JsonError;

/// The only `format` there is.
pub const FORMAT: &str = "odpack";
/// The only `format_version` this reader knows.
pub const FORMAT_VERSION: u64 = 1;

/// A package file is at most this big, checked before parsing (5.2).
pub const MAX_BYTES: usize = 5 * 1024 * 1024;
/// Starting figures, to be set from measurement (specification 13.5).
pub const MAX_DEPTH: usize = 32;
pub const MAX_VALUES: usize = 1_000_000;

/// Why a file is not a readable `.odpack`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    TooLarge {
        max: usize,
    },
    Json(JsonError),
    NotAnObject,
    /// `format` is missing or is not `odpack`.
    UnknownFormat,
    /// `format_version` is missing or newer than this reader knows.
    UnsupportedFormatVersion(Option<u64>),
    /// A top-level field that is not `format`, `format_version`, `header` or
    /// `package`. (Header fields are the ones that may be unknown.)
    UnknownField(String),
    Missing(&'static str),
    Header(HeaderError),
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvelopeError::TooLarge { max } => write!(f, "is larger than {} MB", max >> 20),
            EnvelopeError::Json(e) => write!(f, "{e}"),
            EnvelopeError::NotAnObject => write!(f, "is not a JSON object"),
            EnvelopeError::UnknownFormat => write!(f, "is not an odpack file"),
            EnvelopeError::UnsupportedFormatVersion(Some(v)) => {
                write!(
                    f,
                    "is odpack format version {v}; this app reads {FORMAT_VERSION}"
                )
            }
            EnvelopeError::UnsupportedFormatVersion(None) => {
                write!(f, "has no readable format_version")
            }
            EnvelopeError::UnknownField(name) => write!(f, "has an unknown field {name:?}"),
            EnvelopeError::Missing(name) => write!(f, "has no `{name}`"),
            EnvelopeError::Header(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EnvelopeError {}

impl From<HeaderError> for EnvelopeError {
    fn from(e: HeaderError) -> Self {
        EnvelopeError::Header(e)
    }
}

/// A read envelope. The payload is whatever the header's `kind` says it is,
/// held as parsed JSON and not interpreted here: an unknown kind's payload is
/// never read any further.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    pub header: Header,
    pub package: Value,
}

impl Envelope {
    /// Reads a file's bytes, in the order of the reading rules (5.2): the byte
    /// cap first, then the strict parse, then the format, then the header.
    pub fn read(bytes: &[u8]) -> Result<Envelope, EnvelopeError> {
        if bytes.len() > MAX_BYTES {
            return Err(EnvelopeError::TooLarge { max: MAX_BYTES });
        }
        let value =
            strict_json::parse(bytes, MAX_DEPTH, MAX_VALUES).map_err(EnvelopeError::Json)?;
        let Value::Object(mut top) = value else {
            return Err(EnvelopeError::NotAnObject);
        };

        if top.remove("format").as_ref().and_then(Value::as_str) != Some(FORMAT) {
            return Err(EnvelopeError::UnknownFormat);
        }
        let version = top.remove("format_version").and_then(|v| v.as_u64());
        if version != Some(FORMAT_VERSION) {
            return Err(EnvelopeError::UnsupportedFormatVersion(version));
        }
        let header = top
            .remove("header")
            .ok_or(EnvelopeError::Missing("header"))?;
        let package = top
            .remove("package")
            .ok_or(EnvelopeError::Missing("package"))?;
        if let Some(name) = top.keys().next() {
            return Err(EnvelopeError::UnknownField(name.clone()));
        }

        let raw: RawHeader = serde_json::from_value(header).map_err(|e| HeaderError {
            field: "header".into(),
            reason: e.to_string(),
        })?;
        Ok(Envelope {
            header: Header::check(&raw)?,
            package,
        })
    }

    /// The file's bytes: indented so it can be read and compared by eye, with
    /// the fields in a fixed order, so the same envelope is the same bytes.
    /// What is written is read back first, so this never produces a file that
    /// `read` would refuse.
    pub fn to_bytes(&self) -> Result<Vec<u8>, EnvelopeError> {
        #[derive(Serialize)]
        struct File<'a> {
            format: &'static str,
            format_version: u64,
            header: RawHeader,
            package: &'a Value,
        }
        let file = File {
            format: FORMAT,
            format_version: FORMAT_VERSION,
            header: self.header.to_raw(),
            package: &self.package,
        };
        let mut bytes = serde_json::to_vec_pretty(&file)
            .map_err(|e| EnvelopeError::Json(JsonError::Syntax(e.to_string())))?;
        bytes.push(b'\n');
        Envelope::read(&bytes)?;
        Ok(bytes)
    }
}

/// The SHA-256 of a file's exact bytes, as 64 lowercase hex digits. A
/// published release never changes, so no canonical form is needed (5.1).
pub fn file_hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests;
