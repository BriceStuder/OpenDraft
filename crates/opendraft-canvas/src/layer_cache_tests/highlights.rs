//! Highlights over the kept picture: drawn over nothing, and exactly as
//! painting directly draws them.

use super::*;

/// The triangles of a frame as a set: where in the frame each is drawn does
/// not matter, only what is drawn.
pub(super) fn as_set(flat: &[[u32; 3]]) -> Vec<[[u32; 3]; 3]> {
    let mut triangles: Vec<_> = flat.chunks(3).map(|t| [t[0], t[1], t[2]]).collect();
    triangles.sort_unstable();
    triangles
}

#[test]
fn a_highlight_is_drawn_over_nothing() {
    let doc = drawing(300);
    let some = BTreeSet::from([
        doc.objects()[3].id,
        doc.objects()[150].id,
        doc.objects()[299].id,
    ]);
    let mut kept = Rig::new(CacheMode::On, Some((1, 0)));
    let mut direct = Rig::new(CacheMode::Off, Some((1, 0)));
    // The pointer passes over the drawing in both, so hovering is covered.
    for _ in 0..4 {
        let (a, b) = (kept.frame(&doc, &some), direct.frame(&doc, &some));
        assert_eq!(as_set(&a), as_set(&b));
    }
    assert_eq!(kept.builds(), 1);
}

#[test]
fn selecting_everything_draws_what_painting_directly_does() {
    let doc = drawing(300);
    let all: BTreeSet<EntityId> = doc.objects().iter().map(|o| o.id).collect();
    let mut kept = Rig::new(CacheMode::On, Some((1, 0)));
    let mut direct = Rig::new(CacheMode::Off, Some((1, 0)));
    for _ in 0..3 {
        let (a, b) = (kept.frame(&doc, &all), direct.frame(&doc, &all));
        assert_eq!(as_set(&a), as_set(&b));
    }
}

#[test]
fn only_the_chunks_holding_a_highlight_are_made_again_and_only_once() {
    let doc = drawing(300);
    let (first, last) = (doc.objects()[0].id, doc.objects()[299].id);
    let mut rig = Rig::new(CacheMode::On, Some((1, 0)));
    rig.pointer = -2000.0;
    rig.frame(&doc, &none());
    let base = rig.view.cache_stats().bytes;
    let both = BTreeSet::from([first, last]);
    for _ in 0..3 {
        rig.frame(&doc, &both);
    }
    let stats = rig.view.cache_stats();
    assert_eq!(stats.builds, 1, "the layer is not rebuilt");
    assert_eq!(stats.variant_builds, 2, "two chunks, each once");
    assert!(stats.bytes > base);
    // Another entity in a chunk that already has a variant remakes only it.
    let more = BTreeSet::from([first, doc.objects()[1].id, last]);
    rig.frame(&doc, &more);
    assert_eq!(rig.view.cache_stats().variant_builds, 3);
    // With nothing highlighted the variants are let go.
    rig.frame(&doc, &none());
    let stats = rig.view.cache_stats();
    assert_eq!(stats.bytes, base);
    assert_eq!(stats.builds, 1);
}

#[test]
fn a_variant_that_does_not_fit_the_limit_is_not_kept() {
    let doc = drawing(300);
    let mut probe = Rig::new(CacheMode::On, Some((1, 0)));
    probe.frame(&doc, &none());
    let base = probe.view.cache_stats().bytes;
    let mut rig = Rig::new(CacheMode::On, Some((1, 0)));
    rig.view.set_cache_config(CacheConfig {
        memory_limit: base + 10,
        ..exact(CacheMode::On)
    });
    let one = BTreeSet::from([doc.objects()[0].id]);
    for _ in 0..3 {
        assert!(!rig.frame(&doc, &one).is_empty());
    }
    let stats = rig.view.cache_stats();
    assert_eq!(stats.bytes, base, "memory stays within the limit");
    assert_eq!(stats.builds, 1);
}

#[test]
fn highlights_at_the_edges_of_chunks_are_drawn_over_nothing() {
    // Chunks are 64 entities in tests, so these sit on both sides of edges.
    let doc = drawing(300);
    let at = |n: usize| doc.objects()[n].id;
    let edges: BTreeSet<EntityId> = [0, 63, 64, 127, 128, 255, 256, 299].map(at).into();
    let mut kept = Rig::new(CacheMode::On, Some((1, 0)));
    let mut direct = Rig::new(CacheMode::Off, Some((1, 0)));
    for _ in 0..3 {
        let (a, b) = (kept.frame(&doc, &edges), direct.frame(&doc, &edges));
        assert_eq!(as_set(&a), as_set(&b));
    }
}

#[test]
fn highlights_off_screen_or_before_the_first_visible_entity_are_harmless() {
    let doc = drawing(300);
    let (first, last) = (doc.objects()[0].id, doc.objects()[299].id);
    let both = BTreeSet::from([first, last]);
    let mut kept = Rig::new(CacheMode::On, Some((1, 0)));
    let mut direct = Rig::new(CacheMode::Off, Some((1, 0)));
    // Zoomed in on the middle: neither entity is on screen, and the first
    // visible one is far down the drawing order.
    for rig in [&mut kept, &mut direct] {
        rig.view.camera = Camera {
            center: Point::new(400.0, 150.0),
            zoom: 5.0,
        };
    }
    for _ in 0..3 {
        let (a, b) = (kept.frame(&doc, &both), direct.frame(&doc, &both));
        assert!(!a.is_empty());
        assert_eq!(as_set(&a), as_set(&b));
    }
}
