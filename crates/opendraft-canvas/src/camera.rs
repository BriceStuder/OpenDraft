use egui::{Pos2, Rect, Vec2};
use opendraft_core::{Bounds, Point};

pub const MIN_ZOOM: f64 = 0.001;
pub const MAX_ZOOM: f64 = 100_000.0;

/// Maps drawing units to screen points. `center` is the world point shown at
/// the middle of the viewport; `zoom` is screen points per drawing unit;
/// `twist` is how far the drawing is turned on the screen, in radians
/// counter-clockwise (AutoCAD's `VIEWTWIST`): with a twist of 30 degrees the
/// world's X axis points 30 degrees up the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub center: Point,
    pub zoom: f64,
    pub twist: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: Point::new(150.0, 100.0),
            zoom: 2.0,
            twist: 0.0,
        }
    }
}

/// The cosine and sine of `twist`, exact when there is none so an unturned
/// view maps exactly as it always has.
fn turn(twist: f64) -> (f64, f64) {
    if twist == 0.0 {
        (1.0, 0.0)
    } else {
        let (s, c) = twist.sin_cos();
        (c, s)
    }
}

impl Camera {
    /// `v`, a world offset, as an offset along the screen's axes (y still up).
    pub fn to_view(&self, v: Point) -> Point {
        let (c, s) = turn(self.twist);
        Point::new(v.x * c - v.y * s, v.x * s + v.y * c)
    }

    /// The reverse of [`to_view`](Self::to_view).
    pub fn from_view(&self, v: Point) -> Point {
        let (c, s) = turn(self.twist);
        Point::new(v.x * c + v.y * s, -v.x * s + v.y * c)
    }

    pub fn world_to_screen(&self, p: Point, viewport: Rect) -> Pos2 {
        let c = viewport.center();
        let v = self.to_view(Point::new(p.x - self.center.x, p.y - self.center.y));
        Pos2::new(
            c.x + (v.x * self.zoom) as f32,
            c.y - (v.y * self.zoom) as f32,
        )
    }

    pub fn screen_to_world(&self, pos: Pos2, viewport: Rect) -> Point {
        let c = viewport.center();
        let v = self.from_view(Point::new(
            f64::from(pos.x - c.x) / self.zoom,
            -f64::from(pos.y - c.y) / self.zoom,
        ));
        Point::new(self.center.x + v.x, self.center.y + v.y)
    }

    /// Drag the drawing along with the pointer.
    pub fn pan_by_pixels(&mut self, delta: Vec2) {
        let v = self.from_view(Point::new(
            f64::from(delta.x) / self.zoom,
            -f64::from(delta.y) / self.zoom,
        ));
        self.center.x -= v.x;
        self.center.y -= v.y;
    }

    /// Turn the drawing on the screen to `twist` about the world point at
    /// the middle of the viewport.
    pub fn set_twist(&mut self, twist: f64) {
        if twist.is_finite() {
            let twist = twist.rem_euclid(std::f64::consts::TAU);
            self.twist = if twist >= std::f64::consts::TAU - 1e-12 {
                0.0
            } else {
                twist
            };
        }
    }

    /// Zoom by `factor`, keeping the world point under `pos` fixed.
    pub fn zoom_at(&mut self, pos: Pos2, viewport: Rect, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let before = self.screen_to_world(pos, viewport);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.screen_to_world(pos, viewport);
        self.center.x += before.x - after.x;
        self.center.y += before.y - after.y;
    }

    /// Frame `bounds` with a margin; an empty drawing resets to the default
    /// view.
    pub fn fit(&mut self, bounds: Option<Bounds>, viewport: Rect) {
        let Some(b) = bounds else {
            *self = Self::default();
            return;
        };
        self.frame(b, viewport, 0.9);
    }

    /// Show the window `b` as large as the viewport allows, with no margin
    /// (ZOOM Window).
    pub fn show_window(&mut self, b: Bounds, viewport: Rect) {
        self.frame(b, viewport, 1.0);
    }

    /// Centre on `b` and fill `usable` of the viewport with it. In a turned
    /// view the box is measured along the screen's axes.
    fn frame(&mut self, b: Bounds, viewport: Rect, usable: f64) {
        let b = self.view_box(b);
        let (w, h) = (b.max.x - b.min.x, b.max.y - b.min.y);
        let (vw, vh) = (f64::from(viewport.width()), f64::from(viewport.height()));
        if vw < 1.0 || vh < 1.0 {
            return;
        }
        let zoom = match (w > 1e-9, h > 1e-9) {
            (true, true) => (vw * usable / w).min(vh * usable / h),
            (true, false) => vw * usable / w,
            (false, true) => vh * usable / h,
            (false, false) => Self::default().zoom,
        };
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.center = self.from_view(Point::new(
            (b.min.x + b.max.x) / 2.0,
            (b.min.y + b.max.y) / 2.0,
        ));
    }

    /// The box round `b` measured along the screen's axes (in turned
    /// coordinates; `b` itself when the view is not turned).
    fn view_box(&self, b: Bounds) -> Bounds {
        if self.twist == 0.0 {
            return b;
        }
        let corners = [
            b.min,
            Point::new(b.max.x, b.min.y),
            b.max,
            Point::new(b.min.x, b.max.y),
        ];
        bounds_of(corners.map(|p| self.to_view(p)))
    }

    /// The world box round what is visible in `viewport` (all of it, and in
    /// a turned view more besides).
    pub fn visible_world(&self, viewport: Rect) -> Bounds {
        if self.twist == 0.0 {
            return Bounds::from_points(
                self.screen_to_world(viewport.left_bottom(), viewport),
                self.screen_to_world(viewport.right_top(), viewport),
            );
        }
        let corners = [
            viewport.left_top(),
            viewport.right_top(),
            viewport.right_bottom(),
            viewport.left_bottom(),
        ];
        bounds_of(corners.map(|c| self.screen_to_world(c, viewport)))
    }
}

/// The box round four points.
fn bounds_of(points: [Point; 4]) -> Bounds {
    let mut b = Bounds::from_points(points[0], points[1]);
    for p in &points[2..] {
        b = Bounds::from_points(
            Point::new(b.min.x.min(p.x), b.min.y.min(p.y)),
            Point::new(b.max.x.max(p.x), b.max.y.max(p.y)),
        );
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(800.0, 600.0))
    }

    #[test]
    fn round_trips_between_world_and_screen() {
        let cam = Camera {
            center: Point::new(12.0, -7.0),
            zoom: 3.5,
            twist: 0.0,
        };
        let p = Point::new(40.0, 25.0);
        let back = cam.screen_to_world(cam.world_to_screen(p, viewport()), viewport());
        assert!((back.x - p.x).abs() < 1e-3 && (back.y - p.y).abs() < 1e-3);
    }

    #[test]
    fn y_axis_points_up() {
        let cam = Camera::default();
        let low = cam.world_to_screen(Point::new(0.0, 0.0), viewport());
        let high = cam.world_to_screen(Point::new(0.0, 10.0), viewport());
        assert!(high.y < low.y);
    }

    #[test]
    fn viewport_center_shows_camera_center() {
        let cam = Camera::default();
        let p = cam.world_to_screen(cam.center, viewport());
        assert_eq!(p, viewport().center());
    }

    #[test]
    fn zoom_keeps_point_under_cursor_fixed() {
        let mut cam = Camera::default();
        let cursor = Pos2::new(300.0, 200.0);
        let before = cam.screen_to_world(cursor, viewport());
        cam.zoom_at(cursor, viewport(), 1.7);
        let after = cam.screen_to_world(cursor, viewport());
        assert!((before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3);
        assert!((cam.zoom - 3.4).abs() < 1e-9);
    }

    #[test]
    fn zoom_is_clamped_and_ignores_bad_factors() {
        let mut cam = Camera::default();
        cam.zoom_at(Pos2::ZERO, viewport(), 1e12);
        assert_eq!(cam.zoom, MAX_ZOOM);
        cam.zoom_at(Pos2::ZERO, viewport(), 1e-12);
        assert_eq!(cam.zoom, MIN_ZOOM);
        let snapshot = cam;
        cam.zoom_at(Pos2::ZERO, viewport(), f64::NAN);
        cam.zoom_at(Pos2::ZERO, viewport(), -2.0);
        assert_eq!(cam, snapshot);
    }

    #[test]
    fn panning_moves_content_with_the_pointer() {
        let mut cam = Camera::default();
        let p = Point::new(10.0, 10.0);
        let before = cam.world_to_screen(p, viewport());
        cam.pan_by_pixels(Vec2::new(25.0, -10.0));
        let after = cam.world_to_screen(p, viewport());
        assert!((after.x - before.x - 25.0).abs() < 1e-3);
        assert!((after.y - before.y + 10.0).abs() < 1e-3);
    }

    #[test]
    fn fit_frames_bounds_inside_the_viewport() {
        let mut cam = Camera::default();
        let b = Bounds::from_points(Point::new(0.0, 0.0), Point::new(1000.0, 500.0));
        cam.fit(Some(b), viewport());
        let min = cam.world_to_screen(b.min, viewport());
        let max = cam.world_to_screen(b.max, viewport());
        assert!(min.x >= viewport().left() && max.x <= viewport().right());
        assert!(min.y <= viewport().bottom() && max.y >= viewport().top());
        assert_eq!(cam.center, Point::new(500.0, 250.0));
    }

    #[test]
    fn fit_handles_a_single_point_and_empty_drawing() {
        let mut cam = Camera {
            center: Point::new(9.0, 9.0),
            zoom: 77.0,
            twist: 0.0,
        };
        let pt = Bounds::from_points(Point::new(5.0, 5.0), Point::new(5.0, 5.0));
        cam.fit(Some(pt), viewport());
        assert_eq!(cam.center, Point::new(5.0, 5.0));
        assert!(cam.zoom.is_finite());
        cam.fit(None, viewport());
        assert_eq!(cam, Camera::default());
    }

    #[test]
    fn a_window_fills_the_viewport_with_no_margin() {
        let mut cam = Camera::default();
        // Wider than the viewport's shape: its width fills the viewport.
        let b = Bounds::from_points(Point::new(10.0, 10.0), Point::new(410.0, 20.0));
        cam.show_window(b, viewport());
        let min = cam.world_to_screen(b.min, viewport());
        let max = cam.world_to_screen(b.max, viewport());
        assert!((min.x - viewport().left()).abs() < 1e-3);
        assert!((max.x - viewport().right()).abs() < 1e-3);
        assert_eq!(cam.center, Point::new(210.0, 15.0));
    }

    #[test]
    fn a_turned_view_maps_both_ways_and_turns_the_drawing_counter_clockwise() {
        let cam = Camera {
            center: Point::new(10.0, 5.0),
            zoom: 2.0,
            twist: std::f64::consts::FRAC_PI_2,
        };
        // The world's X axis points up the screen.
        let o = cam.world_to_screen(cam.center, viewport());
        let x = cam.world_to_screen(Point::new(20.0, 5.0), viewport());
        assert!((x.x - o.x).abs() < 1e-3 && (o.y - x.y - 20.0).abs() < 1e-3);
        let p = Point::new(-3.0, 40.0);
        let back = cam.screen_to_world(cam.world_to_screen(p, viewport()), viewport());
        assert!((back.x - p.x).abs() < 1e-3 && (back.y - p.y).abs() < 1e-3);
    }

    #[test]
    fn a_turned_view_pans_with_the_pointer_and_frames_along_the_screen() {
        let mut cam = Camera {
            twist: 0.7,
            ..Camera::default()
        };
        let p = Point::new(10.0, 10.0);
        let before = cam.world_to_screen(p, viewport());
        cam.pan_by_pixels(Vec2::new(25.0, -10.0));
        let after = cam.world_to_screen(p, viewport());
        assert!(
            (after.x - before.x - 25.0).abs() < 1e-3 && (after.y - before.y + 10.0).abs() < 1e-3
        );
        let b = Bounds::from_points(Point::new(0.0, 0.0), Point::new(1000.0, 500.0));
        cam.fit(Some(b), viewport());
        for corner in [
            b.min,
            b.max,
            Point::new(0.0, 500.0),
            Point::new(1000.0, 0.0),
        ] {
            assert!(viewport().contains(cam.world_to_screen(corner, viewport())));
        }
        let seen = cam.visible_world(viewport());
        assert!(seen.min.x <= 0.0 && seen.max.x >= 1000.0);
    }

    #[test]
    fn twist_is_kept_in_one_turn() {
        let mut cam = Camera::default();
        cam.set_twist(-std::f64::consts::FRAC_PI_2);
        assert!((cam.twist - 1.5 * std::f64::consts::PI).abs() < 1e-12);
        cam.set_twist(f64::NAN);
        assert!((cam.twist - 1.5 * std::f64::consts::PI).abs() < 1e-12);
        cam.set_twist(std::f64::consts::TAU);
        assert_eq!(cam.twist, 0.0);
    }
}
