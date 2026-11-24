//! A prompt with a value in it ("Enter new value for DIMSCALE"), kept as its
//! English wording with `%{name}` fields and the values that fill them.
//! The command asks with the filled English text, which outside callers
//! read; the interface translates the wording and fills it again, so the
//! prompt shows in the language of the interface (issue #138).

use crate::prompt::Options;

/// English text with `%{name}` fields, and the value of each field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wording {
    pub text: String,
    pub values: Vec<(String, String)>,
}

impl Wording {
    pub fn new(text: &str, values: &[(&str, &str)]) -> Self {
        Self {
            text: text.to_owned(),
            values: values
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }

    /// `text` (this wording or its translation) with each `%{name}` replaced
    /// by its value, in one pass: a value is never read as a field.
    pub fn fill(&self, text: &str) -> String {
        let mut filled = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find("%{") {
            filled.push_str(&rest[..at]);
            let after = &rest[at + 2..];
            let value = after.find('}').and_then(|end| {
                let name = &after[..end];
                let (_, value) = self.values.iter().find(|(n, _)| n == name)?;
                Some((value, end))
            });
            match value {
                Some((value, end)) => {
                    filled.push_str(value);
                    rest = &after[end + 1..];
                }
                None => {
                    filled.push_str("%{");
                    rest = after;
                }
            }
        }
        filled.push_str(rest);
        filled
    }
}

impl<T> Options<T> {
    /// A prompt with values in it: `text` has a `%{name}` field for each of
    /// `values`. The message is the filled English text.
    pub fn worded(text: &str, values: &[(&str, &str)]) -> Self {
        let wording = Wording::new(text, values);
        let mut options = Self::new(wording.fill(text));
        options.wording = Some(wording);
        options
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_is_the_filled_english_and_the_wording_is_kept() {
        let options = Options::<String>::worded(
            "Enter new value for %{name} <%{current}>",
            &[("name", "DIMSCALE"), ("current", "1")],
        );
        assert_eq!(options.message, "Enter new value for DIMSCALE <1>");
        let wording = options.wording.expect("kept");
        assert_eq!(
            wording.fill("Nouvelle valeur de %{name} <%{current}>"),
            "Nouvelle valeur de DIMSCALE <1>"
        );
    }

    #[test]
    fn a_value_is_not_read_as_a_field() {
        let wording = Wording::new("Name %{a} and %{b}", &[("a", "%{b}"), ("b", "x")]);
        assert_eq!(wording.fill("Name %{a} and %{b}"), "Name %{b} and x");
        assert_eq!(wording.fill("100%{ and %{c}"), "100%{ and %{c}");
    }
}
