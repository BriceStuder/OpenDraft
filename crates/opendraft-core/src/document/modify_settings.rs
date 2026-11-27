//! What the modify commands remember, kept in the drawing.

use super::Document;
use crate::ModifySettings;

impl Document {
    pub fn modify_settings(&self) -> ModifySettings {
        self.modify_settings
    }

    /// Replace the settings and return the old ones. A value that cannot be
    /// used (a negative radius, a zero offset distance...) is put back to its
    /// default rather than refused, so a damaged file still opens.
    pub fn set_modify_settings(&mut self, settings: ModifySettings) -> ModifySettings {
        std::mem::replace(&mut self.modify_settings, settings.sanitized())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_replaced_and_the_old_ones_come_back() {
        let mut doc = Document::default();
        let wanted = ModifySettings {
            fillet_radius: 2.5,
            ..ModifySettings::default()
        };
        assert_eq!(doc.set_modify_settings(wanted), ModifySettings::default());
        assert_eq!(doc.modify_settings(), wanted);
        doc.set_modify_settings(ModifySettings {
            fillet_radius: f64::NAN,
            ..wanted
        });
        assert_eq!(doc.modify_settings().fillet_radius, 10.0);
    }
}
