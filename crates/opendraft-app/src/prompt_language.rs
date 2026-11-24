//! A command's prompt in the language of the interface (issue #138). The
//! command asks in English, as outside callers read it; the prompt is
//! translated where it is shown.

use opendraft_commands::Prompt;
use opendraft_ui::i18n::tr;

/// What the command asks, in the language of the interface: a message with
/// a value in it is translated by its wording, then filled.
pub fn shown(ask: &Prompt) -> String {
    match &ask.wording {
        Some(wording) => wording.fill(&tr(&wording.text)),
        None => tr(&ask.message).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use opendraft_commands::Options;

    #[test]
    fn a_worded_prompt_asks_in_english_and_shows_translated() {
        let options = Options::<String>::worded(
            "Enter new value for dimension variable %{name}",
            &[("name", "DIMSCALE")],
        );
        assert_eq!(
            options.message,
            "Enter new value for dimension variable DIMSCALE"
        );
        let wording = options.wording.expect("kept");
        let french = rust_i18n::t!(&wording.text, locale = "fr");
        assert_eq!(
            wording.fill(&french),
            "Entrez la nouvelle valeur de la variable de cote DIMSCALE"
        );
    }
}
