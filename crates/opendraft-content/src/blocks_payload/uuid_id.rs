//! A block's stable id (specification 11): a random UUID made when the block
//! is first captured, kept in every later release of its package.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockUuid(Uuid);

impl BlockUuid {
    pub fn new() -> Self {
        BlockUuid(Uuid::new_v4())
    }

    /// The canonical spelling only: 36 characters, lowercase, hyphenated, and
    /// a random (version 4) id. One id has one spelling.
    pub fn parse(text: &str) -> Option<BlockUuid> {
        let id = Uuid::try_parse(text).ok()?;
        let canonical = id.hyphenated().to_string() == text;
        (canonical && id.get_version_num() == 4).then_some(BlockUuid(id))
    }
}

impl Default for BlockUuid {
    fn default() -> Self {
        BlockUuid::new()
    }
}

impl fmt::Display for BlockUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl fmt::Debug for BlockUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockUuid({self})")
    }
}

impl Serialize for BlockUuid {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for BlockUuid {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        BlockUuid::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("{text:?} is not a block id")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_random_and_round_trip() {
        let a = BlockUuid::new();
        assert_ne!(a, BlockUuid::new());
        assert_eq!(BlockUuid::parse(&a.to_string()), Some(a));
        let json = serde_json::to_string(&a).expect("serialises");
        assert_eq!(serde_json::from_str::<BlockUuid>(&json).ok(), Some(a));
    }

    #[test]
    fn only_the_canonical_spelling_is_accepted() {
        let id = BlockUuid::new().to_string();
        for bad in [
            id.to_uppercase(),
            id.replace('-', ""),
            format!("{{{id}}}"),
            format!("urn:uuid:{id}"),
            format!(" {id}"),
            String::new(),
            "00000000-0000-0000-0000-000000000000".into(),
            "not an id".into(),
        ] {
            assert_eq!(BlockUuid::parse(&bad), None, "{bad:?}");
        }
        assert!(serde_json::from_str::<BlockUuid>("\"nope\"").is_err());
    }
}
