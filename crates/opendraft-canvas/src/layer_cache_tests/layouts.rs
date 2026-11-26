//! A sheet through the kept pictures: each viewport and the sheet itself have
//! one, and they draw what painting directly draws.

use super::highlights::as_set;
use super::*;
use crate::Space;
use opendraft_core::{Attributes, Layer, LayoutId};

/// A drawing with a sheet whose one viewport shows the model. Lines on the
/// layer called "Hidden" are frozen in that viewport; so are lines drawn on the
/// sheet itself.
pub(super) fn sheet_drawing() -> (Document, LayoutId) {
    let mut doc = drawing(400);
    let hidden = doc.allocate_layer_id();
    doc.push_layer(Layer::new(hidden, "Hidden"));
    for i in 0..30 {
        let at = f64::from(i) * 25.0;
        let line = Entity::Line {
            start: Point::new(at, 305.0),
            end: Point::new(at + 10.0, 290.0),
        };
        doc.add_with(line, Attributes::on_layer(hidden));
    }
    let mut layout = doc.new_layout("Sheet", 297.0, 210.0);
    layout.viewports[0].set_frozen(&[hidden], true);
    let id = layout.id;
    doc.set_layouts(vec![layout]);
    doc.set_active_layout(Some(id));
    for i in 0..40 {
        let at = f64::from(i) * 6.0;
        doc.add(Entity::Line {
            start: Point::new(at, 10.0),
            end: Point::new(at + 3.0, 25.0),
        });
    }
    doc.set_active_layout(None);
    (doc, id)
}

pub(super) fn on_sheet(mode: CacheMode, layout: LayoutId) -> Rig {
    let mut rig = Rig::new(mode, Some((1, 0)));
    rig.view.set_space(Space::Paper(layout));
    rig.pointer = -2000.0;
    rig
}

#[test]
fn the_viewport_and_the_sheet_each_keep_a_picture_and_pointer_frames_do_not_rebuild() {
    let (doc, layout) = sheet_drawing();
    let mut rig = on_sheet(CacheMode::On, layout);
    for _ in 0..5 {
        rig.frame(&doc, &none());
    }
    let stats = rig.view.cache_stats();
    assert_eq!(stats.builds, 2, "one for the viewport, one for the sheet");
    assert_eq!(stats.hits, 10);
    assert!(stats.bytes > 0);
}

#[test]
fn a_sheet_through_the_kept_pictures_is_the_sheet_painted_directly() {
    let (doc, layout) = sheet_drawing();
    let mut kept = on_sheet(CacheMode::On, layout);
    let mut direct = on_sheet(CacheMode::Off, layout);
    for _ in 0..3 {
        let (a, b) = (kept.frame(&doc, &none()), direct.frame(&doc, &none()));
        assert!(!a.is_empty());
        assert_eq!(as_set(&a), as_set(&b));
    }
    assert!(kept.view.cache_stats().hits >= 4);
}

#[test]
fn a_layer_frozen_in_the_viewport_stays_out_of_its_picture() {
    let (doc, layout) = sheet_drawing();
    let mut kept = on_sheet(CacheMode::On, layout);
    let mut direct = on_sheet(CacheMode::Off, layout);
    // Highlights in the viewport are drawn live; one on the frozen layer is not.
    let frozen = doc.objects()[404].id;
    let on_the_sheet = doc.objects().last().map(|o| o.id);
    let selection = BTreeSet::from([frozen, on_the_sheet.unwrap_or(frozen)]);
    for _ in 0..3 {
        let (a, b) = (kept.frame(&doc, &selection), direct.frame(&doc, &selection));
        assert_eq!(as_set(&a), as_set(&b));
    }
}

#[test]
fn zooming_the_sheet_moves_the_pictures_and_makes_new_ones() {
    let (doc, layout) = sheet_drawing();
    let mut rig = on_sheet(CacheMode::On, layout);
    rig.view.set_cache_config(CacheConfig {
        render: Render::Triangles,
        mode: CacheMode::On,
        motion: Motion::Smooth,
        settle_ms: 0,
        slice_ms: 0,
        ..CacheConfig::default()
    });
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 2);
    // Zooming the sheet moves every viewport on it, and the sheet's own picture.
    rig.view
        .request_zoom_window(Point::new(0.0, 0.0), Point::new(100.0, 70.0));
    let mut frames = 0;
    while rig.view.cache_stats().rebuilding.is_some() || frames < 2 {
        rig.frame(&doc, &none());
        frames += 1;
        assert!(frames < 40, "it finishes");
    }
    let stats = rig.view.cache_stats();
    assert!(stats.moved > 0, "the old pictures were shown moved");
    assert_eq!(stats.builds, 4, "each slot made a new one");
}

#[test]
fn the_slots_share_the_memory_limit() {
    let (doc, layout) = sheet_drawing();
    let mut probe = on_sheet(CacheMode::On, layout);
    probe.frame(&doc, &none());
    let both = probe.view.cache_stats().bytes;
    assert!(both > 0);
    let mut tight = on_sheet(CacheMode::On, layout);
    tight.view.set_cache_config(CacheConfig {
        memory_limit: both / 2,
        ..exact(CacheMode::On)
    });
    for _ in 0..3 {
        assert!(!tight.frame(&doc, &none()).is_empty());
    }
    let stats = tight.view.cache_stats();
    assert!(stats.bytes <= both / 2, "{} over {}", stats.bytes, both / 2);
}
