//! Search (specification 10.4): words are matched together (all must match)
//! against name, description, tags and folder, ignoring case and accents.

use opendraft_core::InsertUnits;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use super::{BlockInfo, Library};

#[derive(Debug, Clone, Default)]
pub struct Query {
    /// Words; empty matches everything.
    pub text: String,
    pub unit: Option<InsertUnits>,
    pub favourites_only: bool,
    pub dynamic: Option<bool>,
    pub has_attributes: Option<bool>,
}

/// Lowercase, accents taken off.
pub fn fold(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

impl Library {
    /// The blocks that match, best first: a name that is the words, then one
    /// that starts with them, then one that has them, then the rest by name.
    pub fn search(&self, query: &Query) -> Vec<BlockInfo> {
        let words: Vec<String> = fold(&query.text)
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let whole = words.join(" ");
        let mut found: Vec<(u8, BlockInfo)> = self
            .blocks()
            .into_iter()
            .filter(|b| query.unit.is_none_or(|u| b.unit == u))
            .filter(|b| !query.favourites_only || b.favourite)
            .filter(|b| query.dynamic.is_none_or(|d| b.dynamic == d))
            .filter(|b| query.has_attributes.is_none_or(|a| b.has_attributes == a))
            .filter_map(|b| {
                let name = fold(&b.name);
                let haystack = format!(
                    "{name} {} {} {}",
                    fold(&b.description),
                    fold(&b.tags.join(" ")),
                    fold(&b.folder)
                );
                if !words.iter().all(|w| haystack.contains(w.as_str())) {
                    return None;
                }
                let rank = if words.is_empty() || name == whole {
                    0
                } else if name.starts_with(&whole) {
                    1
                } else if words.iter().all(|w| name.contains(w.as_str())) {
                    2
                } else {
                    3
                };
                Some((rank, b))
            })
            .collect();
        found.sort_by_cached_key(|(rank, b)| (*rank, b.name.to_lowercase()));
        found.into_iter().map(|(_, b)| b).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_ignores_case_and_accents() {
        assert_eq!(fold("Éléphant ÀB"), "elephant ab");
        assert_eq!(fold("Straße"), "straße");
    }
}
