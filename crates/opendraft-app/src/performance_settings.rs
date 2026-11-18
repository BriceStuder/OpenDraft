//! The Performance page of Settings: how the drawing's kept picture behaves
//! (see `opendraft_canvas::layer_cache`) and an optional readout of what a
//! frame costs. These describe the machine, so they are the user's settings
//! and never part of a drawing.

use eframe::egui;
use opendraft_canvas::layer_cache::{AUTO_MIN_OBJECTS, DEFAULT_MEMORY_LIMIT};
use opendraft_canvas::{CacheConfig, CacheMode};
use opendraft_ui::{widgets, Theme};
use serde::{Deserialize, Serialize};

use crate::settings::Settings;

/// The least and most, in megabytes, the kept picture of one drawing may use.
pub const MEMORY_LIMIT_MB_RANGE: (u32, u32) = (64, 2048);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceSettings {
    /// Whether the picture of the drawing is kept between frames.
    pub drawing_cache: CacheMode,
    /// The most the kept picture of one drawing may take.
    pub memory_limit_mb: u32,
    /// Show the frame time and cache use over the canvas.
    pub show_frame_time: bool,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            drawing_cache: CacheMode::default(),
            memory_limit_mb: (DEFAULT_MEMORY_LIMIT >> 20) as u32,
            show_frame_time: false,
        }
    }
}

impl PerformanceSettings {
    /// What the canvas is told.
    pub fn cache_config(&self) -> CacheConfig {
        CacheConfig {
            mode: self.drawing_cache,
            memory_limit: (self.memory_limit_mb as usize) << 20,
        }
    }

    pub fn sanitized(mut self) -> Self {
        let (min, max) = MEMORY_LIMIT_MB_RANGE;
        self.memory_limit_mb = self.memory_limit_mb.clamp(min, max);
        self
    }
}

fn hint(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(theme.font_size_small)
            .color(theme.colors.text_weak),
    );
}

pub fn page(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    let p = &mut draft.performance;
    widgets::group_box(ui, theme, "Drawing cache", |ui| {
        hint(
            ui,
            theme,
            "Keeps the picture of your drawing between frames, so moving the pointer, \
             hovering and selecting stay quick in large drawings. It uses memory.",
        );
        ui.add_space(4.0);
        let automatic = format!("Automatic: for drawings of {AUTO_MIN_OBJECTS} objects or more");
        ui.radio_value(&mut p.drawing_cache, CacheMode::Automatic, automatic);
        ui.radio_value(&mut p.drawing_cache, CacheMode::On, "Always on");
        ui.radio_value(
            &mut p.drawing_cache,
            CacheMode::Off,
            "Off: paint the drawing afresh every frame",
        );
        hint(
            ui,
            theme,
            "Turn it off if a drawing ever looks wrong, and tell us what you saw.",
        );
        ui.add_space(6.0);
        let (min, max) = MEMORY_LIMIT_MB_RANGE;
        ui.horizontal(|ui| {
            ui.label("Memory limit");
            ui.add(
                egui::Slider::new(&mut p.memory_limit_mb, min..=max)
                    .logarithmic(true)
                    .suffix(" MB"),
            );
        });
        hint(
            ui,
            theme,
            "The most the kept picture of one drawing may take. A drawing that needs more \
             is painted without it.",
        );
    });
    ui.add_space(6.0);
    widgets::group_box(ui, theme, "Diagnostics", |ui| {
        ui.checkbox(
            &mut p.show_frame_time,
            "Show frame time and cache use on the canvas",
        );
        hint(
            ui,
            theme,
            "Off by default; nothing is measured while it is off.",
        );
    });
    ui.add_space(6.0);
    if widgets::secondary_button(ui, theme, "Reset to defaults").clicked() {
        *p = PerformanceSettings::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_canvas_defaults() {
        assert_eq!(
            PerformanceSettings::default().cache_config(),
            CacheConfig::default()
        );
        assert!(!PerformanceSettings::default().show_frame_time);
    }

    #[test]
    fn the_memory_limit_is_kept_in_range() {
        let (min, max) = MEMORY_LIMIT_MB_RANGE;
        for (given, want) in [(0, min), (1, min), (512, 512), (999_999, max)] {
            let p = PerformanceSettings {
                memory_limit_mb: given,
                ..Default::default()
            };
            assert_eq!(p.sanitized().memory_limit_mb, want);
        }
        let p = PerformanceSettings {
            memory_limit_mb: 100,
            ..Default::default()
        };
        assert_eq!(p.cache_config().memory_limit, 100 << 20);
    }

    #[test]
    fn saved_settings_without_the_page_load_its_defaults() {
        let old = r#"{"show_grid": false, "zoom_speed": 150}"#;
        let loaded = Settings::from_json(old);
        assert_eq!(loaded.performance, PerformanceSettings::default());
        assert!(!loaded.show_grid);
    }

    #[test]
    fn the_page_round_trips_and_a_bad_value_costs_only_itself() {
        let settings = Settings {
            performance: PerformanceSettings {
                drawing_cache: CacheMode::Off,
                memory_limit_mb: 1024,
                show_frame_time: true,
            },
            ..Settings::default()
        };
        let text = serde_json::to_string(&settings).unwrap();
        assert_eq!(Settings::from_json(&text).performance, settings.performance);
        let bad = r#"{"show_grid": false, "performance": {"drawing_cache": "sideways"}}"#;
        let loaded = Settings::from_json(bad);
        assert_eq!(loaded.performance, PerformanceSettings::default());
        assert!(!loaded.show_grid);
    }

    #[test]
    fn the_settings_dialog_lists_the_page_before_automation() {
        let tabs = crate::settings_ui::TABS;
        let at = |name: &str| tabs.iter().position(|t| *t == name);
        assert_eq!(at("Performance"), Some(3));
        assert_eq!(at("Automation"), Some(4));
        assert_eq!(at("Updates"), Some(5));
    }
}
