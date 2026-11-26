//! The pieces a list of blocks is drawn from: columns, a header and a row, and
//! the note shown where there is nothing to list.

use eframe::egui;
use opendraft_ui::{icon_set, icons, Theme};

/// A centred note and nothing else, for a place with nothing in it.
pub(super) fn empty_state(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(40.0), egui::Sense::hover());
        icon_set::paint(
            ui.painter(),
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icons::BLOCK_LIBRARY,
            36.0,
            theme.colors.text_disabled,
        );
        ui.add_space(6.0);
        ui.label(egui::RichText::new(text).color(theme.colors.text_weak));
    });
}

const ROW: f32 = 22.0;

/// Widths of the folder and tags columns; the name takes the rest.
pub(super) fn list_widths(ui: &egui::Ui) -> [f32; 2] {
    let total = ui.available_width();
    [
        (total * 0.3).clamp(90.0, 260.0),
        (total * 0.28).clamp(80.0, 260.0),
    ]
}

pub(super) fn list_header(ui: &mut egui::Ui, theme: &Theme, widths: [f32; 2], names: [&str; 3]) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW), egui::Sense::hover());
    for (text, x) in names.iter().zip(columns(rect, widths)) {
        ui.painter().text(
            egui::pos2(x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            egui::FontId::proportional(theme.font_size_small),
            theme.colors.text_weak,
        );
    }
    ui.painter().hline(
        rect.x_range(),
        rect.max.y,
        egui::Stroke::new(1.0_f32, theme.colors.divider),
    );
}

/// Where the three columns start: name, folder, tags.
fn columns(rect: egui::Rect, widths: [f32; 2]) -> [f32; 3] {
    let tags = rect.max.x - widths[1] - 6.0;
    [rect.min.x + 26.0, tags - widths[0], tags]
}

/// One line of a list: name (with an optional icon), folder and tags.
pub(super) fn list_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    widths: [f32; 2],
    selected: bool,
    cells: [&str; 3],
    icon: Option<char>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW),
        egui::Sense::click_and_drag(),
    );
    let c = &theme.colors;
    if selected {
        ui.painter()
            .rect_filled(rect, 3.0, c.accent.gamma_multiply(0.45));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 3.0, c.hover);
    }
    if let Some(icon) = icon {
        icon_set::paint(
            ui.painter(),
            egui::pos2(rect.min.x + 12.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            13.0,
            c.text_weak,
        );
    }
    let xs = columns(rect, widths);
    for (i, (text, x)) in cells.iter().zip(xs).enumerate() {
        let next = xs.get(i + 1).copied().unwrap_or(rect.max.x);
        let mut job = egui::text::LayoutJob::simple_singleline(
            (*text).to_owned(),
            egui::FontId::proportional(theme.font_size_body),
            if i == 0 { c.text } else { c.text_weak },
        );
        job.wrap = egui::text::TextWrapping {
            max_width: (next - x - 8.0).max(10.0),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('\u{2026}'),
        };
        let galley = ui.painter().layout_job(job);
        let at = egui::pos2(x, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(at, galley, c.text);
    }
    response
}
