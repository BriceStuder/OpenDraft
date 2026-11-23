//! The language of the interface (issue #138).
//!
//! Texts are written in English in the code and wrapped in `rust_i18n::t!`;
//! `locales/<code>.yml` maps each English text to its translation. A text a
//! file does not translate is shown in English, so a translation can grow a
//! few texts at a time. The choice is a setting: by default the system's
//! language when it is offered here, otherwise English. It takes effect at
//! once, since every frame draws its texts again.

use eframe::egui;
use opendraft_ui::{widgets, Theme};

use crate::settings::Settings;

/// The language the code is written in, used when nothing else applies.
pub const ENGLISH: &str = "en";

/// The languages offered in Settings: code, and name in that language.
pub const LANGUAGES: [(&str, &str); 2] = [(ENGLISH, "English"), ("fr", "Français")];

/// The language to show for a saved choice; `None` follows the system.
pub fn resolve(choice: Option<&str>) -> &'static str {
    match choice {
        Some(code) => offered(code).unwrap_or(ENGLISH),
        None => system(),
    }
}

/// Show the interface in the chosen language from the next frame on.
pub fn apply(choice: Option<&str>) {
    rust_i18n::set_locale(resolve(choice));
}

/// The offered language matching the system's, or English.
pub fn system() -> &'static str {
    from_tag(sys_locale::get_locale().as_deref())
}

/// Whether `code` is one of [`LANGUAGES`]; a saved choice that is not (from a
/// newer version, or edited by hand) follows the system instead.
pub fn is_offered(code: &str) -> bool {
    offered(code).is_some()
}

/// The name of a language in that language ("Français").
pub fn name(code: &str) -> &'static str {
    LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map_or("English", |(_, name)| name)
}

/// The language of a system tag: "fr-FR", "fr_CA.UTF-8" and "fr" are French.
fn from_tag(tag: Option<&str>) -> &'static str {
    let primary = tag
        .and_then(|tag| tag.split(['-', '_', '.', '@']).next())
        .unwrap_or_default();
    offered(primary).unwrap_or(ENGLISH)
}

fn offered(code: &str) -> Option<&'static str> {
    LANGUAGES
        .iter()
        .find(|(c, _)| c.eq_ignore_ascii_case(code))
        .map(|(c, _)| *c)
}

/// The Language group of the Display page.
pub fn settings_group(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    widgets::group_box(ui, theme, &rust_i18n::t!("Language"), |ui| {
        let follow_system = rust_i18n::t!(
            "Same as the system (%{language})",
            language = name(system())
        );
        let current = match draft.language.as_deref() {
            Some(code) => name(code).into(),
            None => follow_system.clone(),
        };
        let width = ui.available_width().min(220.0);
        widgets::dropdown(ui, theme, "language", &current, width, |ui| {
            ui.selectable_value(&mut draft.language, None, follow_system.as_ref());
            for (code, name) in LANGUAGES {
                ui.selectable_value(&mut draft.language, Some(code.to_owned()), name);
            }
        });
    });
}

#[cfg(test)]
#[path = "language_tests.rs"]
mod tests;
