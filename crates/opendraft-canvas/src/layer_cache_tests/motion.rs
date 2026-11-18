//! A pan or zoom: the old picture moved while the new one is made a chunk at
//! a time, and what is still made at once.

use super::highlights::as_set;
use super::*;

/// A view that shows the old picture moved while it makes the new one, one
/// chunk a frame, once the camera has stood still for `settle_ms`.
fn smooth(settle_ms: u32) -> Rig {
    let mut rig = Rig::new(CacheMode::On, Some((1, 0)));
    rig.view.set_cache_config(CacheConfig {
        mode: CacheMode::On,
        motion: Motion::Smooth,
        settle_ms,
        slice_ms: 0,
        ..CacheConfig::default()
    });
    rig.pointer = -2000.0;
    rig
}

fn panned() -> Camera {
    Camera {
        center: Point::new(430.0, 310.0),
        zoom: 1.0,
    }
}

#[test]
fn a_pan_shows_the_old_picture_and_makes_the_new_one_a_chunk_at_a_time() {
    let doc = drawing(400);
    let mut rig = smooth(0);
    let mut direct = Rig::new(CacheMode::Off, Some((1, 0)));
    direct.pointer = -2000.0;
    let before = rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 1);
    rig.view.camera = panned();
    direct.view.camera = panned();
    let mut seen = Vec::new();
    let mut frames = 0;
    while rig.builds() == 1 && frames < 20 {
        let moved = rig.frame(&doc, &none());
        let stats = rig.view.cache_stats();
        assert!(moved.len() >= before.len() / 2, "the old picture is shown");
        seen.push(stats.rebuilding);
        frames += 1;
    }
    let stats = rig.view.cache_stats();
    assert_eq!(stats.builds, 2, "the new picture is made once, in the end");
    assert!(frames > 2, "it took several frames: {frames}");
    assert!(stats.moved >= 2, "the old picture was shown moved");
    let progress: Vec<u8> = seen.iter().flatten().copied().collect();
    assert!(progress.windows(2).all(|w| w[0] <= w[1]), "{progress:?}");
    assert!(progress.iter().any(|&p| p > 0 && p < 100), "{progress:?}");
    // Done, and exact: the same triangles as painting directly.
    let (a, b) = (rig.frame(&doc, &none()), direct.frame(&doc, &none()));
    assert_eq!(rig.view.cache_stats().rebuilding, None);
    assert_eq!(as_set(&a), as_set(&b));
}

#[test]
fn a_view_that_keeps_moving_waits_and_one_that_stops_is_made() {
    let doc = drawing(400);
    let mut rig = smooth(60);
    rig.frame(&doc, &none());
    // Moving every frame: the settle delay never passes, so nothing is made.
    for k in 0..6 {
        rig.view.camera.center.x += 5.0 + f64::from(k);
        rig.frame(&doc, &none());
    }
    let stats = rig.view.cache_stats();
    assert_eq!((stats.builds, stats.rebuilding), (1, None));
    assert!(stats.moved >= 6);
    // Standing still: nothing yet, then after the delay it is made.
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 1);
    std::thread::sleep(std::time::Duration::from_millis(80));
    for _ in 0..20 {
        rig.frame(&doc, &none());
    }
    assert_eq!(rig.builds(), 2);
    assert_eq!(rig.view.cache_stats().rebuilding, None);
}

#[test]
fn moving_again_while_it_is_made_starts_over() {
    let doc = drawing(400);
    let mut rig = smooth(0);
    rig.frame(&doc, &none());
    rig.view.camera = panned();
    rig.frame(&doc, &none());
    rig.frame(&doc, &none());
    let half = rig.view.cache_stats().rebuilding.unwrap_or(0);
    assert!(half > 0 && half < 100);
    rig.view.camera.center.x += 3.0;
    rig.frame(&doc, &none());
    let restarted = rig.view.cache_stats().rebuilding.unwrap_or(100);
    assert!(restarted <= half, "{restarted} after {half}");
    assert_eq!(
        rig.builds(),
        1,
        "nothing was finished for a view left behind"
    );
}

#[test]
fn an_edit_or_a_resize_is_drawn_exactly_at_once_even_while_panning() {
    let doc = drawing(400);
    let mut rig = smooth(0);
    rig.frame(&doc, &none());
    rig.view.camera = panned();
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 1);
    rig.view.set_document_token(Some((1, 1)));
    rig.frame(&doc, &none());
    assert_eq!(
        rig.builds(),
        2,
        "an edit is never drawn from an old picture"
    );
    assert_eq!(rig.view.cache_stats().rebuilding, None);
    let moved = rig.view.cache_stats().moved;
    rig.screen = Vec2::new(1100.0, 700.0);
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 3, "a resize is made at once");
    assert_eq!(rig.view.cache_stats().moved, moved);
}

#[test]
fn highlights_are_drawn_live_over_the_moved_picture() {
    let doc = drawing(400);
    let one = BTreeSet::from([doc.objects()[10].id]);
    let mut rig = smooth(0);
    rig.frame(&doc, &none());
    rig.view.camera = panned();
    let plain = rig.frame(&doc, &none());
    let selected = rig.frame(&doc, &one);
    assert_eq!(rig.builds(), 1, "still the old picture");
    assert!(selected.len() > plain.len(), "the highlight is there");
}

#[test]
fn without_room_for_the_old_and_the_new_together_the_new_is_made_at_once() {
    let doc = drawing(400);
    let mut probe = Rig::new(CacheMode::On, Some((1, 0)));
    probe.frame(&doc, &none());
    let base = probe.view.cache_stats().bytes;
    let mut rig = smooth(0);
    let limit = base + base / 2;
    rig.view.set_cache_config(CacheConfig {
        mode: CacheMode::On,
        motion: Motion::Smooth,
        settle_ms: 0,
        slice_ms: 0,
        memory_limit: limit,
    });
    rig.frame(&doc, &none());
    rig.view.camera = panned();
    for _ in 0..20 {
        rig.frame(&doc, &none());
    }
    let stats = rig.view.cache_stats();
    assert_eq!(stats.builds, 2);
    assert!(stats.bytes <= limit, "{} over {limit}", stats.bytes);
    assert_eq!(stats.rebuilding, None);
}

#[test]
fn exact_motion_and_a_quick_automatic_one_make_the_new_picture_at_once() {
    let doc = drawing(100);
    let mut exact = Rig::new(CacheMode::On, Some((1, 0)));
    exact.frame(&doc, &none());
    exact.view.camera = panned();
    exact.frame(&doc, &none());
    assert_eq!((exact.builds(), exact.view.cache_stats().moved), (2, 0));
    let mut auto = Rig::new(CacheMode::On, Some((1, 0)));
    auto.view.set_cache_config(CacheConfig {
        mode: CacheMode::On,
        ..CacheConfig::default()
    });
    auto.frame(&doc, &none());
    // Smooth only pays when a picture is slow to make; a small one is not.
    let slow = auto.view.cache_stats().last_build_us > crate::layer_cache::AUTO_SMOOTH_MS * 1000;
    auto.view.camera = panned();
    auto.frame(&doc, &none());
    if !slow {
        assert_eq!((auto.builds(), auto.view.cache_stats().moved), (2, 0));
    }
}

#[test]
fn a_zoom_out_that_leaves_most_of_the_screen_empty_is_made_at_once() {
    let doc = drawing(400);
    let mut rig = smooth(1000);
    let mut direct = Rig::new(CacheMode::Off, Some((1, 0)));
    direct.pointer = -2000.0;
    rig.frame(&doc, &none());
    for view in [&mut rig.view, &mut direct.view] {
        view.camera.zoom *= 0.3;
    }
    let (a, b) = (rig.frame(&doc, &none()), direct.frame(&doc, &none()));
    let stats = rig.view.cache_stats();
    assert_eq!(
        stats.builds, 2,
        "no waiting for a picture of a quarter of it"
    );
    assert_eq!(stats.moved, 0);
    assert_eq!(as_set(&a), as_set(&b));
}
