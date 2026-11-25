//! Messages in the language of the interface (issue #138). A command says
//! its messages in English, as outside callers read them (the status line
//! goes back to automation); they are translated where they are shown.

use opendraft_commands::Wording;

use crate::state::AppState;

/// A message in the language of the interface.
pub fn shown(wording: &Wording) -> String {
    wording.shown(&|text| rust_i18n::t!(text))
}

/// A line said in English, in the language of the interface: by its
/// wording when it has one that still gives this line.
pub fn shown_line(line: &str, wording: Option<&Wording>) -> String {
    match wording {
        Some(wording) if wording.english() == line => shown(wording),
        _ => rust_i18n::t!(line).into_owned(),
    }
}

/// The status bar's message in the language of the interface.
pub fn shown_message(state: &AppState) -> String {
    shown_line(&state.message, state.message_wording.as_ref())
}

impl AppState {
    /// Says `message` in the command line and the status bar.
    pub fn tell(&mut self, message: impl Into<Wording>) {
        let wording = message.into();
        self.set_message(wording.clone());
        self.cmdline.say(wording);
    }

    /// Shows `message` in the status bar.
    pub fn set_message(&mut self, message: impl Into<Wording>) {
        let wording = message.into();
        self.message = wording.english();
        let worded = !(wording.values.is_empty() && wording.parts.is_empty());
        self.message_wording = worded.then_some(wording);
    }
}
