//! A JSON reader that refuses what `serde_json::Value` would quietly accept:
//! a duplicate object key (so the hub's PHP parser and this one can never read
//! one file two ways, specification 5.2), nesting past a depth limit and more
//! values than a node limit.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt;

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

/// Why a file is not acceptable JSON for a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonError {
    /// Not JSON, or not complete JSON (message from the parser).
    Syntax(String),
    TooDeep {
        max: usize,
    },
    TooManyValues {
        max: usize,
    },
    DuplicateKey(String),
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonError::Syntax(m) => write!(f, "not valid JSON: {m}"),
            JsonError::TooDeep { max } => write!(f, "nested more than {max} levels deep"),
            JsonError::TooManyValues { max } => write!(f, "holds more than {max} values"),
            JsonError::DuplicateKey(k) => write!(f, "has the key {k:?} twice in one object"),
        }
    }
}

struct State {
    max_depth: usize,
    max_nodes: usize,
    nodes: Cell<usize>,
    failure: RefCell<Option<JsonError>>,
}

impl State {
    fn fail<E: serde::de::Error>(&self, error: JsonError) -> E {
        let message = error.to_string();
        *self.failure.borrow_mut() = Some(error);
        E::custom(message)
    }

    fn count<E: serde::de::Error>(&self) -> Result<(), E> {
        let n = self.nodes.get() + 1;
        self.nodes.set(n);
        if n > self.max_nodes {
            return Err(self.fail(JsonError::TooManyValues {
                max: self.max_nodes,
            }));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Seed<'a> {
    depth: usize,
    state: &'a State,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "any JSON value")
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
        self.state.count::<E>()?;
        Ok(Value::Bool(v))
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
        self.state.count::<E>()?;
        Ok(Value::Number(v.into()))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
        self.state.count::<E>()?;
        Ok(Value::Number(v.into()))
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        self.state.count::<E>()?;
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("number is not finite"))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> {
        self.state.count::<E>()?;
        Ok(Value::String(v.to_owned()))
    }

    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Value, E> {
        self.state.count::<E>()?;
        Ok(Value::String(v))
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        self.state.count::<E>()?;
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        self.state.count::<A::Error>()?;
        let depth = self.depth + 1;
        if depth > self.state.max_depth {
            return Err(self.state.fail(JsonError::TooDeep {
                max: self.state.max_depth,
            }));
        }
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(Seed {
            depth,
            state: self.state,
        })? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        self.state.count::<A::Error>()?;
        let depth = self.depth + 1;
        if depth > self.state.max_depth {
            return Err(self.state.fail(JsonError::TooDeep {
                max: self.state.max_depth,
            }));
        }
        let mut seen = HashSet::new();
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(self.state.fail(JsonError::DuplicateKey(key)));
            }
            let value = map.next_value_seed(Seed {
                depth,
                state: self.state,
            })?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

/// Parses `bytes` as exactly one JSON value, within the limits.
pub fn parse(bytes: &[u8], max_depth: usize, max_nodes: usize) -> Result<Value, JsonError> {
    let state = State {
        max_depth,
        max_nodes,
        nodes: Cell::new(0),
        failure: RefCell::new(None),
    };
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let result = Seed {
        depth: 0,
        state: &state,
    }
    .deserialize(&mut de)
    .and_then(|value| de.end().map(|()| value));
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(state
            .failure
            .take()
            .unwrap_or_else(|| JsonError::Syntax(error.to_string()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_default(text: &str) -> Result<Value, JsonError> {
        parse(text.as_bytes(), 8, 100)
    }

    #[test]
    fn ordinary_json_reads_like_serde_json() {
        let text = r#"{"a":[1,2.5,-3,"x",null,true],"b":{"c":{}}}"#;
        let expected: Value = serde_json::from_str(text).expect("valid");
        assert_eq!(parse_default(text), Ok(expected));
    }

    #[test]
    fn a_duplicate_key_is_refused_at_any_depth() {
        assert_eq!(
            parse_default(r#"{"a":1,"a":2}"#),
            Err(JsonError::DuplicateKey("a".into()))
        );
        assert_eq!(
            parse_default(r#"{"x":[{"k":1,"k":1}]}"#),
            Err(JsonError::DuplicateKey("k".into()))
        );
        // The same key in different objects is fine.
        assert!(parse_default(r#"{"a":{"a":1},"b":{"a":1}}"#).is_ok());
    }

    #[test]
    fn a_key_written_two_ways_is_still_a_duplicate() {
        assert_eq!(
            parse_default(r#"{"a":1,"a":2}"#),
            Err(JsonError::DuplicateKey("a".into()))
        );
    }

    #[test]
    fn nesting_past_the_limit_is_refused() {
        let deep = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
        assert!(parse_default(&deep(8)).is_ok());
        assert_eq!(parse_default(&deep(9)), Err(JsonError::TooDeep { max: 8 }));
        // Far past serde_json's own limit: still our error, no stack overflow.
        assert_eq!(
            parse_default(&"[".repeat(100_000)),
            Err(JsonError::TooDeep { max: 8 })
        );
    }

    #[test]
    fn too_many_values_are_refused() {
        let items = vec!["1"; 100].join(",");
        // 100 numbers plus the array is 101 values.
        assert_eq!(
            parse_default(&format!("[{items}]")),
            Err(JsonError::TooManyValues { max: 100 })
        );
        let items = vec!["1"; 99].join(",");
        assert!(parse_default(&format!("[{items}]")).is_ok());
    }

    #[test]
    fn syntax_errors_and_trailing_text_are_refused() {
        for bad in ["", "{", "[1,]", "{\"a\":}", "[1] x", "{} {}", "NaN", "'a'"] {
            assert!(
                matches!(parse_default(bad), Err(JsonError::Syntax(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_lone_surrogate_escape_is_refused() {
        assert!(matches!(
            parse_default(r#"["\ud800"]"#),
            Err(JsonError::Syntax(_))
        ));
    }
}
