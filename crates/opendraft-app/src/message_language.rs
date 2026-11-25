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

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use opendraft_commands::Wording;

    use crate::cmdline::CommandLine;
    use crate::text::objects;

    /// The files whose messages are marked for translation; the others
    /// follow in later steps of issue #138.
    const MARKED: &[&str] = &[
        "arc_cmd",
        "circle",
        "ellipse_cmd",
        "spline_cmd",
        "polyline_cmd",
        "polyline_arc",
        "rectangle",
        "rect_variants",
        "polygon_cmd",
        "donut_cmd",
        "xline_cmd",
        "point_cmd",
        "draw",
        "divide_cmd",
        "boundary_cmd",
        "modify",
        "scale_rotate",
        "stretch",
        "offset",
        "corners",
        "lengthen",
        "edges",
        "edges_sweep",
        "break_cmd",
        "join",
        "explode",
        "array_cmd",
        "array_path",
        "pedit_cmd",
        "pedit_vertex",
        "pedit_join",
        "erase",
        "match_props",
        "grips",
        "selection",
        "once_input",
        "zoom_cmd",
        "view_twist_cmd",
        "ccs_cmd",
        "measure_cmd",
    ];

    fn french(wording: &Wording) -> String {
        wording.shown(&|text| Cow::Owned(rust_i18n::t!(text, locale = "fr").into_owned()))
    }

    #[test]
    fn a_message_is_said_in_english_and_shown_translated() {
        let said = Wording::new("Moved %{objects}", &[]).part("objects", objects(3));
        assert_eq!(said.english(), "Moved 3 objects");
        assert_eq!(french(&said), "3 objets déplacés");
        let one = Wording::new("Moved %{objects}", &[]).part("objects", objects(1));
        assert_eq!(one.english(), "Moved 1 object");
    }

    #[test]
    fn the_log_keeps_english_and_shows_each_line_by_its_wording() {
        let mut cmdline = CommandLine::default();
        cmdline.say("No previous view saved.");
        cmdline.say(Wording::new("%{count} selected", &[("count", "4")]));
        assert_eq!(cmdline.log, ["No previous view saved.", "4 selected"]);
        cmdline.log.push("typed by hand".to_owned());
        let shown: Vec<String> = cmdline.shown_log().collect();
        assert_eq!(
            shown,
            ["No previous view saved.", "4 selected", "typed by hand"]
        );
    }

    #[test]
    fn the_marked_files_say_nothing_unmarked() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut unmarked = Vec::new();
        for name in MARKED {
            let text = std::fs::read_to_string(src.join(format!("{name}.rs"))).unwrap();
            let production = text.split("#[cfg(test)]").next().unwrap_or_default();
            for (no, line) in production.lines().enumerate() {
                let squeezed: String = line.split_whitespace().collect();
                if squeezed.contains(".say(\"") || squeezed.contains(".say(format!") {
                    unmarked.push(format!("{name}.rs:{}", no + 1));
                }
            }
        }
        assert!(
            unmarked.is_empty(),
            "say(key!(..)) or a Wording: {unmarked:#?}"
        );
    }
}
