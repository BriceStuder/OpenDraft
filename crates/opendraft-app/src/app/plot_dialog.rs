//! The Plot dialog (PLOT, PRINT, PREVIEW): the page setup of the layout being
//! worked in, with a picture of exactly what will be written.
//!
//! The picture is the centre of it: a tall preview on the right, and on the
//! left, in the order it is thought of, where it goes, the paper, what is
//! plotted and how big, and the options. A caption and its control share a
//! row, and a control that is not needed is left out rather than greyed.

use eframe::egui;
use opendraft_ui::widgets;

use super::page_setup_dialog::boxes::{options_box, paper_box, plot_box};
use super::OpenDraftApp;
use crate::plot_controls::control_state;
use crate::plot_job::{PlotDialog, Printers, Target};
use crate::plot_zoom::{self, Zoom};

const WIDTH: f32 = 1040.0;
const LEFT: f32 = 340.0;
const GAP: f32 = 14.0;
/// The fixed picture height. Keeping this independent of the controls in the
/// left column prevents plot-area changes (notably Window's coordinate rows)
/// from resizing both the preview and the dialog.
const PREVIEW_HEIGHT: f32 = 345.0;

enum Action {
    PickWindow,
    Apply,
    Plot,
    Cancel,
}

impl OpenDraftApp {
    pub(super) fn plot_dialog_window(&mut self, ctx: &egui::Context) {
        self.state.take_window_pick();
        let Some(mut dialog) = self.state.command_dialogs.plot.take() else {
            // Closed: the next one opens on the whole page.
            self.state.command_dialogs.plot_zoom = Default::default();
            return;
        };
        if dialog.picking.is_some() {
            // Hidden while the drawing asks for the window.
            self.state.command_dialogs.plot = Some(dialog);
            return;
        }
        let theme = &self.theme;
        dialog.printers.poll();
        dialog.settle();
        if matches!(dialog.printers, Printers::Asking(_)) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let model = dialog.draft.space == opendraft_core::plot::PlotSpace::Model;
        let controls = control_state(&dialog.draft.plot, model);
        self.state.command_dialogs.plot = Some(dialog);
        if self.state.plot_dialog_tick(std::time::Instant::now()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
        let Some(mut dialog) = self.state.command_dialogs.plot.take() else {
            return;
        };
        let texture = preview_texture(ctx, &dialog);
        let mut zoom = std::mem::take(&mut self.state.command_dialogs.plot_zoom);
        let speed = self.state.settings.zoom_speed as f32 / 100.0;
        let (mut open, mut action) = (true, None);
        let body_height = (ctx.screen_rect().height() - 190.0).max(240.0);
        widgets::dialog(ctx, theme, "Plot", &mut open, WIDTH, |ui| {
            egui::ScrollArea::vertical()
                .max_height(body_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(LEFT);
                            output_box(ui, theme, &mut dialog);
                            ui.add_space(6.0);
                            paper_box(ui, theme, &mut dialog.draft);
                            ui.add_space(6.0);
                            let mut pick = false;
                            plot_box(ui, theme, &mut dialog.draft, Some(&mut pick));
                            if pick {
                                action = Some(Action::PickWindow);
                            }
                            ui.add_space(6.0);
                            options_box(ui, theme, &mut dialog.draft, false);
                        });
                        ui.add_space(GAP);
                        ui.vertical(|ui| {
                            let width = (WIDTH - LEFT - GAP - 36.0).max(300.0);
                            ui.set_width(width);
                            let blocked = dialog.error.is_some() || controls.reason.is_some();
                            preview_box(
                                ui,
                                theme,
                                &dialog,
                                texture.as_ref(),
                                &mut zoom,
                                speed,
                                blocked,
                            );
                        });
                    });
                });
            ui.add_space(6.0);
            if let Some(why) = dialog.error.as_deref().or(controls.reason) {
                ui.colored_label(theme.colors.accent, why);
            }
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut dialog.save_changes,
                    if model {
                        "Save changes to Model tab"
                    } else {
                        "Save changes to layout"
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                        action = Some(Action::Cancel);
                    }
                    if widgets::secondary_button(
                        ui,
                        theme,
                        if model {
                            "Apply to Model"
                        } else {
                            "Apply to Layout"
                        },
                    )
                    .clicked()
                    {
                        action = Some(Action::Apply);
                    }
                    ui.add_enabled_ui(controls.can_submit, |ui| {
                        if widgets::primary_button(ui, theme, "Plot").clicked() {
                            action = Some(Action::Plot);
                        }
                    });
                });
            });
        });
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = Some(Action::Cancel);
        }
        let sheet = dialog.preview.as_ref().map(|p| &p.sheet);
        if zoom.tick(sheet, std::time::Instant::now()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
        self.state.command_dialogs.plot_zoom = zoom;
        self.state.command_dialogs.plot = Some(dialog);
        match action {
            Some(Action::PickWindow) => self.state.plot_dialog_pick_window(),
            Some(Action::Apply) => self.state.plot_dialog_apply(),
            Some(Action::Plot) => {
                self.state.plot_dialog_run();
            }
            Some(Action::Cancel) => self.state.command_dialogs.plot = None,
            None => {}
        }
    }
}

/// The preview as a texture, uploaded again only when the picture changed.
fn preview_texture(ctx: &egui::Context, dialog: &PlotDialog) -> Option<egui::TextureHandle> {
    let preview = dialog.preview.as_ref()?;
    let id = egui::Id::new("plot_preview_texture");
    let kept: Option<(u64, egui::TextureHandle)> = ctx.data(|d| d.get_temp(id));
    if let Some((serial, texture)) = kept {
        if serial == dialog.preview_serial {
            return Some(texture);
        }
    }
    let image = egui::ColorImage::from_rgba_unmultiplied(preview.size, &preview.rgba);
    let texture = ctx.load_texture("plot_preview", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (dialog.preview_serial, texture.clone())));
    Some(texture)
}

/// The sharp picture of the part in view as a texture, uploaded again only
/// when it changed; none unless it is of the preview's sheet.
fn detail_texture(
    ctx: &egui::Context,
    zoom: &Zoom,
    preview: &crate::plot_job::PlotPreview,
) -> Option<(egui::Rect, egui::TextureHandle)> {
    let detail = zoom.detail_of(&preview.sheet)?;
    let id = egui::Id::new("plot_preview_detail_texture");
    let kept: Option<(u64, egui::TextureHandle)> = ctx.data(|d| d.get_temp(id));
    if let Some((_, texture)) = kept.filter(|(serial, _)| *serial == zoom.serial) {
        return Some((detail.part, texture));
    }
    let image = egui::ColorImage::from_rgba_unmultiplied(detail.size, &detail.rgba);
    let texture = ctx.load_texture("plot_preview_detail", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (zoom.serial, texture.clone())));
    Some((detail.part, texture))
}

/// The wide picture of the plot, the printable area marked on the paper, and
/// a reminder when the settings have moved on from what it shows. The wheel,
/// a drag and a double-click zoom and move it (`zoom`, at the drawing's wheel
/// `speed`). `blocked` is set when there is a reason (shown under the
/// dialog) no picture comes.
fn preview_box(
    ui: &mut egui::Ui,
    theme: &opendraft_ui::Theme,
    dialog: &PlotDialog,
    texture: Option<&egui::TextureHandle>,
    zoom: &mut Zoom,
    speed: f32,
    blocked: bool,
) {
    widgets::group_box(ui, theme, "Preview", |ui| {
        let size = egui::vec2(ui.available_width(), PREVIEW_HEIGHT);
        let (canvas, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let painter = ui.painter_at(canvas);
        painter.rect_filled(canvas, 2.0, ui.visuals().extreme_bg_color);
        match (texture, dialog.preview.as_ref()) {
            (Some(texture), Some(preview)) => {
                let shape = egui::vec2(preview.size[0] as f32, preview.size[1] as f32);
                let room = canvas.shrink(10.0);
                let fit = shape * (room.width() / shape.x).min(room.height() / shape.y);
                zoom.follow_input(ui, &response, fit, speed);
                let paper = zoom.view.page(canvas.center(), fit);
                let whole = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(texture.id(), paper, whole, egui::Color32::WHITE);
                if let Some((part, sharp)) = detail_texture(ui.ctx(), zoom, preview) {
                    let on_screen = |p: egui::Pos2| paper.min + p.to_vec2() * paper.size();
                    let rect = egui::Rect::from_min_max(on_screen(part.min), on_screen(part.max));
                    painter.image(sharp.id(), rect, whole, egui::Color32::WHITE);
                }
                if let Some(part) = zoom.view.part(canvas, fit) {
                    let seen = canvas.intersect(paper).size();
                    let pixels = plot_zoom::pixels_for(seen, ui.ctx().pixels_per_point());
                    zoom.want(part, pixels, std::time::Instant::now());
                }
                painter.rect_stroke(
                    paper,
                    0.0,
                    egui::Stroke::new(1.0_f32, egui::Color32::from_gray(110)),
                    egui::StrokeKind::Outside,
                );
                printable_outline(&painter, paper, dialog);
                zoom_caption(ui, &painter, canvas, zoom);
            }
            _ => {
                painter.text(
                    canvas.center(),
                    egui::Align2::CENTER_CENTER,
                    if blocked {
                        "No preview for these settings"
                    } else {
                        "Drawing the preview..."
                    },
                    egui::FontId::proportional(13.0),
                    ui.visuals().weak_text_color(),
                );
            }
        }
        let stale = dialog.previewed.as_ref() != Some(&dialog.draft);
        if dialog.preview.is_some() && stale {
            ui.small("Updating the preview...");
        } else if let Some(preview) = &dialog.preview {
            for note in &preview.notes {
                ui.small(note);
            }
        }
    });
}

/// How to zoom the preview, or how far it is zoomed, in its bottom corner.
fn zoom_caption(ui: &egui::Ui, painter: &egui::Painter, canvas: egui::Rect, zoom: &Zoom) {
    let text = if zoom.view.is_whole() {
        "Scroll to zoom".to_owned()
    } else {
        format!(
            "{:.0}% · drag to move · double-click for the whole page",
            zoom.view.zoom * 100.0
        )
    };
    let colour = ui.visuals().weak_text_color();
    let galley = painter.layout_no_wrap(text, egui::FontId::proportional(11.0), colour);
    let at = canvas.left_bottom() + egui::vec2(6.0, -6.0 - galley.size().y);
    let back = egui::Rect::from_min_size(at, galley.size()).expand(3.0);
    painter.rect_filled(back, 3.0, ui.visuals().extreme_bg_color);
    painter.galley(at, galley, colour);
}

/// The edge of the area the printer can reach, on the paper in the picture.
fn printable_outline(painter: &egui::Painter, paper: egui::Rect, dialog: &PlotDialog) {
    let [left, bottom, right, top] = dialog.draft.plot.margins_mm;
    if [left, bottom, right, top].iter().all(|m| *m == 0.0) {
        return;
    }
    let (w, h) = (dialog.draft.width as f32, dialog.draft.height as f32);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (kx, ky) = (paper.width() / w, paper.height() / h);
    let printable = egui::Rect::from_min_max(
        paper.min + egui::vec2(left as f32 * kx, top as f32 * ky),
        paper.max - egui::vec2(right as f32 * kx, bottom as f32 * ky),
    );
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(80, 150, 170));
    painter.add(egui::Shape::dashed_line(
        &[
            printable.left_top(),
            printable.right_top(),
            printable.right_bottom(),
            printable.left_bottom(),
            printable.left_top(),
        ],
        stroke,
        5.0,
        3.0,
    ));
}

/// The device the plot goes to: OpenDraft's own PDF and SVG, or one of the
/// machine's printers. Copies and resolution appear only for a printer.
fn output_box(ui: &mut egui::Ui, theme: &opendraft_ui::Theme, dialog: &mut PlotDialog) {
    widgets::group_box(ui, theme, "Printer/plotter", |ui| {
        let shown = dialog.target.device_name();
        let width = ui.available_width();
        widgets::dropdown(ui, theme, "plot_device", &shown, width, |ui| {
            for target in [Target::Pdf, Target::Svg] {
                let name = target.device_name();
                if ui
                    .selectable_label(dialog.target == target, &name)
                    .clicked()
                {
                    dialog.target = target;
                }
            }
            if let Printers::Ready(list) = &dialog.printers {
                if !list.is_empty() {
                    ui.separator();
                    ui.small("System printers");
                }
                for p in list {
                    let chosen = dialog.target == Target::Printer(p.id.clone());
                    if ui.selectable_label(chosen, &p.id.0).clicked() {
                        dialog.target = Target::Printer(p.id.clone());
                    }
                }
            }
        });
        match &dialog.printers {
            Printers::Asking(_) => {
                ui.small("Looking for system printers...");
            }
            Printers::Failed(why) => {
                ui.small(format!("System printers are not available: {why}"));
            }
            Printers::Ready(_) => {}
        }
        if matches!(dialog.target, Target::Printer(_)) {
            ui.horizontal(|ui| {
                widgets::field_label(ui, theme, "Copies");
                ui.add(egui::DragValue::new(&mut dialog.copies).range(1..=999));
                ui.add_space(8.0);
                widgets::field_label(ui, theme, "Resolution");
                ui.add(
                    egui::DragValue::new(&mut dialog.dpi)
                        .range(72..=1200)
                        .suffix(" dpi"),
                );
            });
        }
    });
}
