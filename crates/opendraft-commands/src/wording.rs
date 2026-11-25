//! A text with values in it ("Enter new value for DIMSCALE", "3 corners
//! done."), kept as its English wording with `%{name}` fields and the values
//! that fill them. The command asks or says the filled English text, which
//! outside callers read; the interface translates the wording and fills it
//! again, so the text shows in the language of the interface (issue #138).

use std::borrow::Cow;

use crate::prompt::Options;

/// English text with `%{name}` fields, and the value of each field. A part
/// is a field filled with another wording, translated with it ("3 lines").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wording {
    pub text: String,
    pub values: Vec<(String, String)>,
    pub parts: Vec<(String, Wording)>,
}

impl Wording {
    pub fn new(text: &str, values: &[(&str, &str)]) -> Self {
        Self {
            text: text.to_owned(),
            values: values
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            parts: Vec::new(),
        }
    }

    /// A text without values.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// `one` for a count of 1, otherwise `many`, with the count in the
    /// field `%{count}`: "1 line", "3 lines".
    pub fn count(count: usize, one: &str, many: &str) -> Self {
        let text = if count == 1 { one } else { many };
        Self::new(text, &[("count", &count.to_string())])
    }

    /// Fills the field `name` with `part`, translated with this wording.
    pub fn part(mut self, name: &str, part: Wording) -> Self {
        self.parts.push((name.to_owned(), part));
        self
    }

    /// The filled English text.
    pub fn english(&self) -> String {
        self.shown(&|text| Cow::Borrowed(text))
    }

    /// The filled text, each text (this one and its parts) given by
    /// `translate`.
    pub fn shown(&self, translate: &dyn Fn(&str) -> Cow<'_, str>) -> String {
        if self.parts.is_empty() {
            return self.fill(&translate(&self.text));
        }
        let mut all = self.clone();
        for (name, part) in &self.parts {
            all.values.push((name.clone(), part.shown(translate)));
        }
        all.fill(&translate(&self.text))
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

impl From<String> for Wording {
    fn from(text: String) -> Self {
        Self::plain(text)
    }
}

impl From<&str> for Wording {
    fn from(text: &str) -> Self {
        Self::plain(text)
    }
}

impl From<Cow<'_, str>> for Wording {
    fn from(text: Cow<'_, str>) -> Self {
        Self::plain(text.into_owned())
    }
}

impl From<&String> for Wording {
    fn from(text: &String) -> Self {
        Self::plain(text.as_str())
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
    fn a_count_picks_its_wording_and_parts_are_translated_with_it() {
        let said = Wording::new("Joined %{what} into %{into}.", &[])
            .part("what", Wording::count(3, "%{count} line", "%{count} lines"))
            .part("into", Wording::count(1, "%{count} line", "%{count} lines"));
        assert_eq!(said.english(), "Joined 3 lines into 1 line.");
        let french = said.shown(&|text| {
            Cow::Borrowed(match text {
                "Joined %{what} into %{into}." => "%{what} réunies en %{into}.",
                "%{count} line" => "%{count} ligne",
                "%{count} lines" => "%{count} lignes",
                other => other,
            })
        });
        assert_eq!(french, "3 lignes réunies en 1 ligne.");
    }

    #[test]
    fn a_value_is_not_read_as_a_field() {
        let wording = Wording::new("Name %{a} and %{b}", &[("a", "%{b}"), ("b", "x")]);
        assert_eq!(wording.fill("Name %{a} and %{b}"), "Name %{b} and x");
        assert_eq!(wording.fill("100%{ and %{c}"), "100%{ and %{c}");
    }
}
