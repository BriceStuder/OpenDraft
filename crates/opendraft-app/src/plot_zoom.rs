//! Zooming and panning the Plot dialog's preview, with the drawing's own
//! gestures: the wheel zooms about the pointer, a drag (left or middle
//! button) moves the page, a double-click shows the whole page again.
//!
//! The picture of the whole page is shown larger at once. Once the view has
//! been still a moment, the part in view is drawn again on another thread,
//! sharp at the screen's own resolution, from the same sheet.

use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use opendraft_core::plot::PlotSheet;

/// The closest the preview goes: 64 times the whole page.
const MAX_ZOOM: f32 = 64.0;
/// How long the view stays still before the part in view is drawn sharp.
const SETTLE: Duration = Duration::from_millis(200);
/// The longest side of a sharp picture, in pixels.
const MAX_PIXELS: f32 = 4096.0;
/// How much the wheel zooms per point of scroll, as in the drawing.
const WHEEL_RATE: f32 = 0.003;

/// Which part of the page the preview shows: zoom 1 is the whole page, and
/// `centre` is the point of the page (in fractions of its width and height,
/// from the top left) in the middle of the preview.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub zoom: f32,
    pub centre: egui::Vec2,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            centre: egui::vec2(0.5, 0.5),
        }
    }
}

impl View {
    pub fn is_whole(&self) -> bool {
        self.zoom <= 1.0
    }

    /// Where the page lies on screen: `fit` is its size when whole, and
    /// `middle` the middle of the preview.
    pub fn page(&self, middle: egui::Pos2, fit: egui::Vec2) -> egui::Rect {
        let size = fit * self.zoom;
        egui::Rect::from_min_size(middle - self.centre * size, size)
    }

    /// Zoom by `factor`, keeping the point of the page under `pointer` where
    /// it is.
    pub fn zoom_at(
        &mut self,
        factor: f32,
        pointer: egui::Pos2,
        middle: egui::Pos2,
        fit: egui::Vec2,
    ) {
        let page = self.page(middle, fit);
        let under = (pointer - page.min) / page.size();
        self.zoom = (self.zoom * factor).clamp(1.0, MAX_ZOOM);
        let size = fit * self.zoom;
        self.centre = (middle - (pointer - under * size)) / size;
        self.keep_page_in_view();
    }

    /// Move the page with the pointer by `delta` screen points.
    pub fn pan(&mut self, delta: egui::Vec2, fit: egui::Vec2) {
        self.centre -= delta / (fit * self.zoom);
        self.keep_page_in_view();
    }

    /// Whole, the page is in the middle; zoomed in, the middle of the
    /// preview never leaves the page.
    fn keep_page_in_view(&mut self) {
        if self.is_whole() {
            *self = Self::default();
        } else {
            self.centre = self.centre.clamp(egui::Vec2::ZERO, egui::Vec2::splat(1.0));
        }
    }

    /// The part of the page seen in `canvas`, in fractions of the page from
    /// its top left, or `None` when none of it is.
    pub fn part(&self, canvas: egui::Rect, fit: egui::Vec2) -> Option<egui::Rect> {
        let page = self.page(canvas.center(), fit);
        let seen = canvas.intersect(page);
        if !seen.is_positive() {
            return None;
        }
        let fraction = |p: egui::Pos2| ((p - page.min) / page.size()).to_pos2();
        Some(egui::Rect::from_min_max(
            fraction(seen.min),
            fraction(seen.max),
        ))
    }
}

/// A sharp picture of part of a sheet.
pub struct Detail {
    /// The sheet it is of: the preview's.
    pub sheet: Arc<PlotSheet>,
    /// The part, in fractions of the page from its top left.
    pub part: egui::Rect,
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
}

/// The preview's view, and the sharp picture of the part in it.
#[derive(Default)]
pub struct Zoom {
    pub view: View,
    pub detail: Option<Detail>,
    /// Bumped whenever the sharp picture changes, so the window uploads it.
    pub serial: u64,
    drawing: Option<Receiver<Option<Detail>>>,
    /// The part and pixel size the preview last asked for, and since when.
    wanted: Option<(egui::Rect, [u32; 2], Instant)>,
}

impl Zoom {
    /// The wheel, a drag and a double-click on the preview, `response`,
    /// whose page is `fit` points when whole; `speed` is the drawing's wheel
    /// zoom speed (1 is normal).
    pub fn follow_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        fit: egui::Vec2,
        speed: f32,
    ) {
        let middle = response.rect.center();
        if response.double_clicked() || response.double_clicked_by(egui::PointerButton::Middle) {
            self.view = View::default();
            return;
        }
        if response.dragged_by(egui::PointerButton::Primary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            self.view.pan(response.drag_delta(), fit);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if response.hovered() {
            let (scroll, pinch, pointer) =
                ui.input(|i| (i.raw_scroll_delta.y, i.zoom_delta(), i.pointer.hover_pos()));
            if let Some(pointer) = pointer.filter(|_| scroll != 0.0 || pinch != 1.0) {
                let factor = (scroll * WHEEL_RATE * speed).exp() * pinch;
                self.view.zoom_at(factor, pointer, middle, fit);
            }
            // The wheel zooms the picture; it does not scroll the dialog.
            ui.ctx().input_mut(|i| {
                i.smooth_scroll_delta = egui::Vec2::ZERO;
                i.raw_scroll_delta = egui::Vec2::ZERO;
            });
            if !self.view.is_whole() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
        }
    }

    /// Ask for the part of the page in view, `pixels` across and down. It is
    /// drawn sharp once it has stayed the same for a moment.
    pub fn want(&mut self, part: egui::Rect, pixels: [u32; 2], now: Instant) {
        if self.wanted.map(|(p, s, _)| (p, s)) != Some((part, pixels)) {
            self.wanted = Some((part, pixels, now));
        }
    }

    /// The sharp picture, if it is of `sheet`.
    pub fn detail_of(&self, sheet: &Arc<PlotSheet>) -> Option<&Detail> {
        self.detail
            .as_ref()
            .filter(|d| Arc::ptr_eq(&d.sheet, sheet))
    }

    /// One step: take a finished picture, and draw the part wanted once it
    /// has stayed the same for a moment, from `sheet` (the preview's).
    /// Returns whether the window should look again soon.
    pub fn tick(&mut self, sheet: Option<&Arc<PlotSheet>>, now: Instant) -> bool {
        if let Some(drawing) = &self.drawing {
            match drawing.try_recv() {
                Ok(detail) => {
                    self.drawing = None;
                    if detail.is_none() {
                        // Not drawn: do not ask again for the same part.
                        self.wanted = None;
                    }
                    if let Some(detail) = detail {
                        self.detail = Some(detail);
                        self.serial += 1;
                    }
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => self.drawing = None,
            }
        }
        let (Some(sheet), Some((part, pixels, since))) = (sheet, self.wanted) else {
            return false;
        };
        let size = [pixels[0] as usize, pixels[1] as usize];
        let current = self
            .detail_of(sheet)
            .is_some_and(|d| d.part == part && d.size == size);
        if self.view.is_whole() || current {
            return false;
        }
        if now.duration_since(since) < SETTLE {
            return true;
        }
        let (send, receive) = channel();
        let sheet = Arc::clone(sheet);
        let spawned = std::thread::Builder::new()
            .name("opendraft-plot-zoom".into())
            .spawn(move || {
                let _ = send.send(draw_part(sheet, part, pixels));
            });
        match spawned {
            Ok(_) => self.drawing = Some(receive),
            Err(_) => self.wanted = None,
        }
        true
    }
}

/// The pixel size of a sharp picture of `seen` screen points at
/// `pixels_per_point`, never more than [`MAX_PIXELS`] on a side.
pub fn pixels_for(seen: egui::Vec2, pixels_per_point: f32) -> [u32; 2] {
    let size = seen * pixels_per_point;
    let shrink = (MAX_PIXELS / size.x.max(size.y)).min(1.0);
    let side = |v: f32| (v * shrink).round().max(1.0) as u32;
    [side(size.x), side(size.y)]
}

/// `part` of `sheet` (fractions of the page from its top left) as `pixels`.
fn draw_part(sheet: Arc<PlotSheet>, part: egui::Rect, pixels: [u32; 2]) -> Option<Detail> {
    let (w, h) = (sheet.width, sheet.height);
    let millimetres = [
        f64::from(part.min.x) * w,
        (1.0 - f64::from(part.max.y)) * h,
        f64::from(part.width()) * w,
        f64::from(part.height()) * h,
    ];
    let rgba = opendraft_files::rgba_part(&sheet, millimetres, pixels[0], pixels[1]).ok()?;
    Some(Detail {
        sheet,
        part,
        size: [pixels[0] as usize, pixels[1] as usize],
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendraft_core::plot::PlotPath;
    use opendraft_core::Point;

    const FIT: egui::Vec2 = egui::vec2(200.0, 100.0);
    const MIDDLE: egui::Pos2 = egui::pos2(300.0, 200.0);

    fn close(a: egui::Vec2, b: egui::Vec2) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn the_wheel_zooms_about_the_pointer_and_never_below_the_whole_page() {
        let mut view = View::default();
        assert_eq!(
            view.page(MIDDLE, FIT),
            egui::Rect::from_center_size(MIDDLE, FIT)
        );
        // The page's top left corner stays under the pointer.
        let corner = view.page(MIDDLE, FIT).min;
        view.zoom_at(2.0, corner, MIDDLE, FIT);
        assert_eq!(view.zoom, 2.0);
        assert!(close(
            view.page(MIDDLE, FIT).min.to_vec2(),
            corner.to_vec2()
        ));
        assert!(close(view.centre, egui::vec2(0.25, 0.25)));
        // Out again, past the whole page: the whole page, in the middle.
        view.zoom_at(0.1, corner, MIDDLE, FIT);
        assert_eq!(view, View::default());
        view.zoom_at(1e6, MIDDLE, MIDDLE, FIT);
        assert_eq!(view.zoom, MAX_ZOOM);
    }

    #[test]
    fn a_drag_moves_the_page_but_the_middle_stays_on_it() {
        let mut view = View::default();
        view.pan(egui::vec2(50.0, 0.0), FIT);
        assert_eq!(view, View::default(), "the whole page does not move");
        view.zoom_at(2.0, MIDDLE, MIDDLE, FIT);
        // 40 points right at 400 points across is a tenth of the page.
        view.pan(egui::vec2(40.0, 0.0), FIT);
        assert!(close(view.centre, egui::vec2(0.4, 0.5)));
        view.pan(egui::vec2(10_000.0, -10_000.0), FIT);
        assert!(
            close(view.centre, egui::vec2(0.0, 1.0)),
            "{:?}",
            view.centre
        );
    }

    #[test]
    fn the_part_in_view_is_in_fractions_of_the_page() {
        let canvas = egui::Rect::from_center_size(MIDDLE, egui::vec2(220.0, 110.0));
        let whole = View::default().part(canvas, FIT).unwrap();
        assert_eq!(
            whole,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
        );
        let mut view = View::default();
        view.zoom_at(4.0, MIDDLE, MIDDLE, FIT);
        // 800 x 400 points of page behind 220 x 110: the middle 27.5%.
        let part = view.part(canvas, FIT).unwrap();
        assert!(
            close(part.min.to_vec2(), egui::vec2(0.3625, 0.3625)),
            "{part:?}"
        );
        assert!(close(part.size(), egui::vec2(0.275, 0.275)));
    }

    #[test]
    fn a_sharp_picture_is_never_too_big() {
        assert_eq!(pixels_for(egui::vec2(400.0, 200.0), 1.5), [600, 300]);
        assert_eq!(pixels_for(egui::vec2(8000.0, 2000.0), 1.0), [4096, 1024]);
        assert_eq!(pixels_for(egui::vec2(0.1, 0.1), 1.0), [1, 1]);
    }

    #[test]
    fn the_part_in_view_is_drawn_sharp_once_the_view_is_still() {
        let sheet = Arc::new(PlotSheet {
            width: 100.0,
            height: 50.0,
            paths: vec![PlotPath {
                points: vec![Point::new(0.0, 25.0), Point::new(100.0, 25.0)],
                closed: false,
                rgb: [0, 0, 0],
                width: 1.0,
            }],
            fills: Vec::new(),
            notes: Vec::new(),
        });
        let mut zoom = Zoom::default();
        let t0 = Instant::now();
        let middle = egui::Rect::from_min_max(egui::pos2(0.25, 0.25), egui::pos2(0.75, 0.75));
        zoom.want(middle, [100, 50], t0);
        assert!(
            !zoom.tick(Some(&sheet), t0),
            "nothing to sharpen when whole"
        );
        zoom.view.zoom = 2.0;
        assert!(zoom.tick(Some(&sheet), t0), "waits for the view to settle");
        assert!(zoom.detail.is_none());
        let later = t0 + SETTLE;
        for _ in 0..500 {
            if !zoom.tick(Some(&sheet), later) {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let detail = zoom.detail_of(&sheet).expect("drawn");
        assert_eq!((detail.part, detail.size), (middle, [100, 50]));
        // The line across the middle of the page is across the middle of
        // the picture.
        let dark = |x: usize, y: usize| detail.rgba[(y * 100 + x) * 4] < 128;
        assert!(dark(50, 25) && dark(5, 25) && !dark(50, 5));
        assert_eq!(zoom.serial, 1);
        // Another sheet (the settings changed): this picture is not of it.
        let other = Arc::new(PlotSheet::clone(&sheet));
        assert!(zoom.detail_of(&other).is_none());
    }
}
