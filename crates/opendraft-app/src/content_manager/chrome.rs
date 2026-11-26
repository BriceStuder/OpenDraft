//! Small controls the window draws itself, so that chosen things are lit in
//! neutral tones (accent is for the selected block only) and icons are used.

use eframe::egui;
use opendraft_ui::{icon_set, Theme};

const BUTTON: f32 = 24.0;

/// A square icon button, lit while `on`.
pub fn icon_button(
    ui: &mut egui::Ui,
    theme: &Theme,
    icon: char,
    on: bool,
    enabled: bool,
    tip: &str,
) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(BUTTON), sense);
    let c = &theme.colors;
    if enabled && on {
        ui.painter().rect_filled(rect, 4.0, c.pressed);
    } else if enabled && response.hovered() {
        ui.painter().rect_filled(rect, 4.0, c.hover);
    }
    let colour = match (enabled, on) {
        (false, _) => c.text_disabled,
        (true, true) => c.text,
        (true, false) => c.text_weak,
    };
    icon_set::paint(
        ui.painter(),
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        15.0,
        colour,
    );
    response.on_hover_text(tip)
}

/// A row of the tree: icon, name, and a count at the right. The chosen row is
/// lit in a neutral tone; `indent` is in points.
pub fn nav_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    icon: char,
    label: &str,
    count: Option<usize>,
    selected: bool,
) -> egui::Response {
    let height = ui.spacing().interact_size.y;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    let c = &theme.colors;
    if selected {
        ui.painter().rect_filled(rect, 4.0, c.pressed);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, c.hover);
    }
    let ink = if selected { c.text } else { c.text_weak };
    icon_set::paint(
        ui.painter(),
        egui::pos2(rect.min.x + 12.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        icon,
        14.0,
        ink,
    );
    let font = egui::FontId::proportional(theme.font_size_body);
    let right = match count {
        Some(n) => {
            ui.painter()
                .text(
                    egui::pos2(rect.max.x - 6.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    n.to_string(),
                    egui::FontId::proportional(theme.font_size_small),
                    c.text_weak,
                )
                .min
                .x
        }
        None => rect.max.x,
    };
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), font, c.text);
    job.wrap = egui::text::TextWrapping {
        max_width: (right - rect.min.x - 32.0).max(10.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('\u{2026}'),
    };
    let galley = ui.painter().layout_job(job);
    let at = egui::pos2(rect.min.x + 26.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, c.text);
    response
}

/// A paired toggle drawn as two joined icon buttons.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    theme: &Theme,
    value: &mut T,
    options: &[(T, char, &str)],
) {
    ui.spacing_mut().item_spacing.x = 1.0;
    for (option, icon, tip) in options {
        if icon_button(ui, theme, *icon, *value == *option, true, tip).clicked() {
            *value = *option;
        }
    }
    ui.spacing_mut().item_spacing.x = 4.0;
}

/// Choices as joined text buttons, the chosen one lit in a neutral tone.
pub fn choice<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    theme: &Theme,
    value: &mut T,
    options: &[(T, &str)],
) {
    let c = &theme.colors;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (option, text) in options {
            let on = *value == *option;
            let galley = ui.painter().layout_no_wrap(
                (*text).to_owned(),
                egui::FontId::proportional(theme.font_size_body),
                if on { c.text } else { c.text_weak },
            );
            let size = egui::vec2(galley.size().x + 16.0, 22.0);
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
            if on {
                ui.painter().rect_filled(rect, 4.0, c.pressed);
            } else if response.hovered() {
                ui.painter().rect_filled(rect, 4.0, c.hover);
            }
            ui.painter().galley(
                egui::pos2(rect.min.x + 8.0, rect.center().y - galley.size().y / 2.0),
                galley,
                c.text,
            );
            if response.clicked() {
                *value = *option;
            }
        }
    });
}
