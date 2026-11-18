//! The layer kept as an image: one rectangle in a frame whatever the drawing
//! holds, made once, moved while a new one is made, and checked against a fresh
//! one in every frame of these tests (see `image_paint.rs`).

use super::highlights::as_set;
use super::*;

/// A view that keeps its layer as an image.
fn image_rig() -> Rig {
    let mut rig = Rig::new(CacheMode::On, Some((1, 0)));
    rig.view.set_cache_config(CacheConfig {
        render: Render::Image,
        ..exact(CacheMode::On)
    });
    rig.pointer = -2000.0;
    rig
}

#[test]
fn an_image_is_made_once_and_costs_few_triangles_to_draw() {
    let doc = drawing(300);
    let mut image = image_rig();
    let mut triangles = Rig::new(CacheMode::On, Some((1, 0)));
    triangles.pointer = -2000.0;
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..4 {
        a = image.frame(&doc, &none());
        b = triangles.frame(&doc, &none());
    }
    let stats = image.view.cache_stats();
    assert_eq!(stats.builds, 1);
    assert_eq!(stats.hits, 4);
    let pixels = (SCREEN.x * SCREEN.y) as usize * 4;
    assert_eq!(stats.bytes, pixels, "the image is what is kept");
    assert!(
        a.len() * 4 < b.len(),
        "{} vertices to draw the image, {} the triangles",
        a.len(),
        b.len()
    );
}

#[test]
fn selecting_does_not_remake_the_image() {
    let doc = drawing(300);
    let ids: Vec<EntityId> = doc.objects().iter().map(|o| o.id).collect();
    let few: BTreeSet<EntityId> = ids[..3].iter().copied().collect();
    let many: BTreeSet<EntityId> = ids[..100].iter().copied().collect();
    let mut rig = image_rig();
    rig.frame(&doc, &none());
    for selection in [&few, &many, &few, &none()] {
        for _ in 0..2 {
            rig.frame(&doc, selection);
        }
    }
    let stats = rig.view.cache_stats();
    assert_eq!(stats.builds, 1);
    assert_eq!(
        stats.variant_builds, 0,
        "no chunk is made without a highlight"
    );
    assert_eq!(stats.selected_builds, 1, "the large selection, once");
}

#[test]
fn a_zoom_shows_the_old_image_moved_and_ends_on_a_new_one() {
    let doc = drawing(300);
    let mut rig = image_rig();
    rig.view.set_cache_config(CacheConfig {
        motion: Motion::Smooth,
        settle_ms: 0,
        slice_ms: 0,
        render: Render::Image,
        ..exact(CacheMode::On)
    });
    rig.frame(&doc, &none());
    rig.view.request_zoom_step(true);
    let mut frames = 0;
    while rig.view.cache_stats().rebuilding.is_some() || frames < 2 {
        let shown = rig.frame(&doc, &none());
        assert!(!shown.is_empty());
        frames += 1;
        assert!(frames < 40, "it finishes");
    }
    let stats = rig.view.cache_stats();
    assert!(stats.moved > 0, "the old image was shown moved");
    assert_eq!(stats.builds, 2);
    assert_eq!(stats.bytes, (SCREEN.x * SCREEN.y) as usize * 4);
}

#[test]
fn changing_how_it_is_drawn_remakes_the_layer_and_the_triangles_are_exact_again() {
    let doc = drawing(300);
    let mut rig = image_rig();
    rig.frame(&doc, &none());
    rig.view.set_cache_config(exact(CacheMode::On));
    let mut direct = Rig::new(CacheMode::Off, Some((1, 0)));
    direct.pointer = -2000.0;
    for _ in 0..2 {
        let (a, b) = (rig.frame(&doc, &none()), direct.frame(&doc, &none()));
        assert_eq!(as_set(&a), as_set(&b));
    }
    assert_eq!(rig.view.cache_stats().builds, 2);
}

#[test]
fn a_sheet_keeps_an_image_for_each_viewport_and_for_the_sheet() {
    let (doc, layout) = super::layouts::sheet_drawing();
    let mut rig = super::layouts::on_sheet(CacheMode::On, layout);
    rig.view.set_cache_config(CacheConfig {
        render: Render::Image,
        ..exact(CacheMode::On)
    });
    for _ in 0..4 {
        assert!(!rig.frame(&doc, &none()).is_empty());
    }
    let stats = rig.view.cache_stats();
    assert_eq!(stats.builds, 2, "one for the viewport, one for the sheet");
    assert_eq!(stats.hits, 8);
    let pixels = (SCREEN.x * SCREEN.y) as usize * 4;
    assert_eq!(stats.bytes, 2 * pixels, "two images");
}
