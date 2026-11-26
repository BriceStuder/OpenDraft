//! New pictures made in the background (`view/builder.rs`): the old one is
//! shown moved meanwhile, the new one is kept when it comes back, and it is the
//! picture this thread would have made (every frame here that draws a kept
//! image checks it against a fresh one, see `image_paint.rs`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;

/// A view that keeps an image and makes new ones in the background.
fn background_rig(doc: &Arc<Document>) -> Rig {
    let mut rig = Rig::new(CacheMode::On, Some((1, 0)));
    rig.view.set_cache_config(CacheConfig {
        render: Render::Image,
        mode: CacheMode::On,
        motion: Motion::Smooth,
        ..CacheConfig::default()
    });
    rig.view.set_shared_document(Arc::downgrade(doc));
    rig.pointer = -2000.0;
    rig
}

/// Frames until one is drawn from a kept picture, not one moved; the frames
/// it took.
fn until_sharp(rig: &mut Rig, doc: &Document) -> usize {
    let started = Instant::now();
    for frames in 1.. {
        let moved = rig.view.cache_stats().moved;
        assert!(!rig.frame(doc, &none()).is_empty());
        if rig.view.cache_stats().moved == moved {
            return frames;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "it comes back");
        std::thread::sleep(Duration::from_millis(2));
    }
    0
}

#[test]
fn a_pan_shows_the_old_image_moved_until_the_background_makes_the_new_one() {
    let doc = Arc::new(drawing(300));
    let mut rig = background_rig(&doc);
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 1, "the first picture is made here");
    for step in 1..=3 {
        rig.view.camera.center.x += 40.0 * f64::from(step);
        assert!(until_sharp(&mut rig, &doc) > 1, "shown moved first");
        // One more frame: drawn from the kept image, and checked.
        rig.frame(&doc, &none());
    }
    let stats = rig.view.cache_stats();
    assert!(stats.moved >= 3);
    assert_eq!(stats.builds, 4, "one here, three in the background");
    assert_eq!(stats.rebuilding, None, "nothing sliced here");
}

#[test]
fn without_the_drawing_shared_a_new_picture_is_made_here() {
    let doc = Arc::new(drawing(300));
    let mut rig = background_rig(&doc);
    // Another drawing than the one painted: never used.
    let other = Arc::new(drawing(10));
    rig.view.set_shared_document(Arc::downgrade(&other));
    rig.view.set_cache_config(CacheConfig {
        render: Render::Image,
        mode: CacheMode::On,
        motion: Motion::Smooth,
        settle_ms: 0,
        slice_ms: 0,
        ..CacheConfig::default()
    });
    rig.frame(&doc, &none());
    rig.view.camera.center.x += 60.0;
    let mut frames = 0;
    while rig.view.cache_stats().rebuilding.is_some() || frames < 2 {
        rig.frame(&doc, &none());
        frames += 1;
        assert!(frames < 40, "it finishes");
    }
    assert_eq!(rig.builds(), 2);
}

#[test]
fn panning_a_sheet_remakes_each_viewport_and_the_sheet_in_the_background() {
    let (doc, layout) = super::layouts::sheet_drawing();
    let doc = Arc::new(doc);
    let mut rig = background_rig(&doc);
    rig.view.set_space(crate::Space::Paper(layout));
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 2, "one for the viewport, one for the sheet");
    rig.view.request_zoom_window(
        opendraft_core::Point::new(0.0, 0.0),
        opendraft_core::Point::new(100.0, 70.0),
    );
    until_sharp(&mut rig, &doc);
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 4, "both again, in the background");
}
