//! The single-line text commands: TEXT (typed in place), -TEXT (typed on the
//! command line) and TEXTEDIT.

use opendraft_commands::Command;

use crate::single_line_text_cmd::{draw_text, draw_text_line};
use crate::state::AppState;
use crate::text_edit_cmd::edit_text;

/// As `editor_command!` in `commands.rs`.
macro_rules! editor {
    ($id:literal, $label:literal, $name:literal, $run:path) => {
        Command::new($id, $label, |s: &mut AppState| s.start_command($name, $run))
            .checked(|s: &AppState| s.is_running($name))
            .transparent()
    };
}

/// The commands, for the command table.
pub fn commands() -> Vec<Command<AppState>> {
    vec![
        editor!("annotate.text", "Single Line Text", "TEXT", draw_text),
        editor!("annotate.text_line", "-TEXT", "-TEXT", draw_text_line),
        editor!("annotate.textedit", "Edit Text", "TEXTEDIT", edit_text),
    ]
}
