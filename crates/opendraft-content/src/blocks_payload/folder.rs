//! Folder paths (specification 10.4, 11): `Class/600`. Segments are 1 to 64
//! characters without `/ \ : * ? " < > |`, and two spellings that differ only
//! in case are one folder, shown as first written. The empty path is the top
//! level.

use std::fmt;

use crate::envelope::text::check_line;

pub const MAX_SEGMENT: usize = 64;
/// Folders the user makes in the library.
pub const MAX_LIBRARY_DEPTH: usize = 8;
/// Inner folders of a published package.
pub const MAX_PACKAGE_DEPTH: usize = 4;

const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderError {
    /// A segment is empty, too long, has a forbidden character or breaks the
    /// plain-text rules. Holds the segment.
    BadName(String),
    TooDeep {
        max: usize,
    },
}

impl fmt::Display for FolderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FolderError::BadName(name) => write!(
                f,
                "{name:?} is not a folder name: 1 to {MAX_SEGMENT} characters, none of / \\ : * ? \" < > |"
            ),
            FolderError::TooDeep { max } => write!(f, "folders go at most {max} levels deep"),
        }
    }
}

impl std::error::Error for FolderError {}

/// A checked folder name (one segment).
pub fn check_segment(segment: &str) -> Result<(), FolderError> {
    let bad = check_line(segment, MAX_SEGMENT).is_err()
        || segment.contains(FORBIDDEN)
        || segment != segment.trim()
        || segment == "."
        || segment == "..";
    if bad {
        return Err(FolderError::BadName(segment.to_owned()));
    }
    Ok(())
}

/// Checks `path` (segments joined by `/`, no leading or trailing slash) to
/// `max_depth` levels. The empty path is fine.
pub fn check_path(path: &str, max_depth: usize) -> Result<(), FolderError> {
    if path.is_empty() {
        return Ok(());
    }
    let mut depth = 0;
    for segment in path.split('/') {
        check_segment(segment)?;
        depth += 1;
    }
    if depth > max_depth {
        return Err(FolderError::TooDeep { max: max_depth });
    }
    Ok(())
}

/// Whether two paths are the same folder (ignoring case).
pub fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Whether `path` is `ancestor` or inside it.
pub fn is_within(path: &str, ancestor: &str) -> bool {
    let (path, ancestor) = (path.to_lowercase(), ancestor.to_lowercase());
    ancestor.is_empty()
        || path == ancestor
        || path
            .strip_prefix(&ancestor)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The path with its last segment taken off (`""` for a top-level folder).
pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The last segment.
pub fn name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, last)| last)
}

pub fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

/// `path` with its leading `from` replaced by `to` (for renaming and moving a
/// folder with everything in it). `None` when `path` is not within `from`.
pub fn reroot(path: &str, from: &str, to: &str) -> Option<String> {
    if from.is_empty() || !is_within(path, from) {
        return None;
    }
    let skip = from.split('/').count();
    let rest: Vec<&str> = path.split('/').skip(skip).collect();
    Some(if rest.is_empty() {
        to.to_owned()
    } else {
        join(to, &rest.join("/"))
    })
}

/// Every ancestor of `path` and `path` itself, shortest first.
pub fn with_ancestors(path: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut so_far = String::new();
    for segment in path.split('/').filter(|s| !s.is_empty()) {
        so_far = join(&so_far, segment);
        found.push(so_far.clone());
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_and_bad_paths() {
        for ok in ["", "CL600", "Class/600", "a/b/c/d/e/f/g/h", "Café"] {
            assert_eq!(check_path(ok, 8), Ok(()), "{ok:?}");
        }
        for bad in [
            "/a", "a/", "a//b", "a/ b", "a /b", "a\\b", "a:b", "a*", "a?", "a\"b", "a<b", "a|b",
            ".", "a/..", "a\tb", "e\u{301}",
        ] {
            assert!(
                matches!(check_path(bad, 8), Err(FolderError::BadName(_))),
                "{bad:?}"
            );
        }
        assert!(check_path(&"x".repeat(65), 8).is_err());
        assert_eq!(check_path(&"x".repeat(64), 8), Ok(()));
    }

    #[test]
    fn depth_is_limited() {
        assert_eq!(check_path("a/b/c/d", MAX_PACKAGE_DEPTH), Ok(()));
        assert_eq!(
            check_path("a/b/c/d/e", MAX_PACKAGE_DEPTH),
            Err(FolderError::TooDeep { max: 4 })
        );
    }

    #[test]
    fn paths_compare_ignoring_case() {
        assert!(same("Class/600", "class/600"));
        assert!(!same("Class/600", "Class/300"));
        assert!(is_within("Class/600/Elbows", "class"));
        assert!(is_within("Class", "Class"));
        assert!(is_within("anything", ""));
        // A name that merely starts the same is not inside.
        assert!(!is_within("Classic", "Class"));
    }

    #[test]
    fn pieces_of_a_path() {
        assert_eq!(parent("a/b/c"), "a/b");
        assert_eq!(parent("a"), "");
        assert_eq!(name("a/b/c"), "c");
        assert_eq!(name("a"), "a");
        assert_eq!(join("", "a"), "a");
        assert_eq!(join("a", "b"), "a/b");
        assert_eq!(with_ancestors("a/b/c"), ["a", "a/b", "a/b/c"]);
        assert!(with_ancestors("").is_empty());
    }

    #[test]
    fn rerooting_moves_a_folder_and_what_is_in_it() {
        assert_eq!(reroot("a/b/c", "a/b", "x"), Some("x/c".into()));
        assert_eq!(reroot("a/b", "a/b", "x/y"), Some("x/y".into()));
        assert_eq!(reroot("A/B/c", "a/b", "x"), Some("x/c".into()));
        assert_eq!(reroot("ab", "a", "x"), None);
        assert_eq!(reroot("q", "a", "x"), None);
    }
}
