//! Drawing a layer moved: while a new picture is made, the old one is shown
//! as if panned and zoomed to the new view. A pan is exact; a zoom scales the
//! lines too, which is wrong for the moment until the new picture replaces it.
//! A turn cannot be shown this way (egui moves shapes only by shifting and
//! scaling), so a picture made at another twist is not shown moved.

use std::sync::Arc;

use egui::emath::TSTransform;
use egui::epaint::Mesh;
use egui::layers::ShapeIdx;
use egui::{Painter, Rect, Shape};

use crate::camera::Camera;

/// The move that takes what `from` shows on `rect` to what `to` shows; none
/// when the two are turned differently.
pub(crate) fn move_between(from: &Camera, to: &Camera, rect: Rect) -> Option<TSTransform> {
    if from.twist.to_bits() != to.twist.to_bits() {
        return None;
    }
    let scale = (to.zoom / from.zoom) as f32;
    let was = from.world_to_screen(from.center, rect);
    let now = to.world_to_screen(from.center, rect);
    Some(TSTransform::new(
        now.to_vec2() - was.to_vec2() * scale,
        scale,
    ))
}

/// The part of the screen `rect` (0 to 1) that the old picture, of `picture`,
/// covers once moved by `by`: the picture holds only what was in view (and its
/// margin) when it was made, so after a zoom out most of the screen is empty.
pub(crate) fn covered(by: TSTransform, picture: Rect, rect: Rect) -> f32 {
    let old = Rect::from_min_max(by * picture.min, by * picture.max);
    let shared = old.intersect(rect);
    if shared.is_negative() || rect.area() <= 0.0 {
        return 0.0;
    }
    shared.area() / rect.area()
}

/// Draw `meshes` through `transform`, clipped as `painter` is: egui moves the
/// clip rectangle of a shape along with it, which would move the edge of the
/// canvas too.
pub(crate) fn draw_moved(painter: &Painter, meshes: &[Arc<Mesh>], transform: TSTransform) {
    let clip = painter.clip_rect();
    let mut span: Option<(usize, usize)> = None;
    for mesh in meshes {
        let at = painter.add(Shape::Mesh(mesh.clone())).0;
        span = Some(span.map_or((at, at + 1), |(first, _)| (first, at + 1)));
    }
    let Some((first, end)) = span else {
        return;
    };
    painter.ctx().graphics_mut(|g| {
        let list = g.entry(painter.layer_id());
        list.transform_range(ShapeIdx(first), ShapeIdx(end), transform);
        for at in first..end {
            list.mutate_shape(ShapeIdx(at), |shape| shape.clip_rect = clip);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendraft_core::Point;

    #[test]
    fn a_zoom_out_leaves_most_of_the_screen_uncovered() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let still = TSTransform::IDENTITY;
        assert!((covered(still, rect, rect) - 1.0).abs() < 1e-6);
        let centre = rect.center().to_vec2();
        let out = |s: f32| TSTransform::new(centre * (1.0 - s), s);
        assert!((covered(out(0.5), rect, rect) - 0.25).abs() < 1e-4);
        assert!(
            (covered(out(2.0), rect, rect) - 1.0).abs() < 1e-6,
            "zooming in keeps it all"
        );
        let away = TSTransform::new(egui::vec2(5000.0, 0.0), 1.0);
        assert_eq!(covered(away, rect, rect), 0.0);
        // A picture with a margin of half the screen on each side still
        // covers it all after zooming out to a half.
        let wide = rect.expand2(rect.size() / 2.0);
        assert!((covered(out(0.5), wide, rect) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn the_move_takes_a_point_to_where_the_new_camera_shows_it() {
        let rect = Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(800.0, 600.0));
        let from = Camera {
            center: Point::new(100.0, 50.0),
            zoom: 2.0,
            twist: 0.0,
        };
        for to in [
            from,
            Camera {
                center: Point::new(130.0, 40.0),
                zoom: 2.0,
                twist: 0.0,
            },
            Camera {
                center: Point::new(100.0, 50.0),
                zoom: 5.0,
                twist: 0.0,
            },
            Camera {
                center: Point::new(-20.0, 90.0),
                zoom: 0.5,
                twist: 0.0,
            },
        ] {
            let t = move_between(&from, &to, rect).unwrap();
            for world in [
                Point::new(100.0, 50.0),
                Point::new(130.0, 20.0),
                Point::new(-10.0, 80.0),
            ] {
                let moved = t * from.world_to_screen(world, rect);
                let want = to.world_to_screen(world, rect);
                assert!((moved - want).length() < 0.01, "{moved:?} vs {want:?}");
            }
        }
    }

    #[test]
    fn a_picture_at_another_twist_is_not_moved() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0));
        let from = Camera::default();
        let to = Camera { twist: 0.3, ..from };
        assert!(move_between(&from, &to, rect).is_none());
    }
}
