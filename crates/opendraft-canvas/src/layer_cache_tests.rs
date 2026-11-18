//! The kept picture: used when it should be, rebuilt when anything it
//! depends on changes, and the same picture as painting directly.
//!
//! In debug builds every frame drawn from a kept layer is also checked against
//! a fresh one (`entity_paint.rs`), so any frame run here that reuses a stale
//! layer panics.

use std::collections::BTreeSet;

use egui::epaint::Primitive;
use egui::{Color32, Context, Event, Pos2, RawInput, Rect, Vec2};
use opendraft_core::{Document, Entity, EntityId, MText, Point, PointStyle, TextStyleId};

use crate::layer_cache::AUTO_MIN_OBJECTS;
use crate::{CacheConfig, CacheMode, Camera, CanvasStyle, GridSettings, SnapModes, View};

const SCREEN: Vec2 = Vec2::new(1000.0, 700.0);

fn style() -> CanvasStyle {
    CanvasStyle {
        background: Color32::from_rgb(0x25, 0x25, 0x25),
        grid_minor: Color32::from_white_alpha(18),
        grid_major: Color32::from_white_alpha(36),
        axis_x: Color32::GRAY,
        axis_y: Color32::GRAY,
        entity: Color32::WHITE,
        hover: Color32::from_rgba_unmultiplied(20, 115, 230, 60),
        selected: Color32::from_rgb(120, 200, 255),
        selected_glow: Color32::from_rgba_unmultiplied(20, 115, 230, 120),
        snap: Color32::YELLOW,
        window: Color32::LIGHT_BLUE,
        crossing: Color32::LIGHT_GREEN,
    }
}

/// A grid of short lines and a few texts, all on screen at `camera()`.
fn drawing(lines: usize) -> Document {
    let mut entities: Vec<Entity> = (0..lines)
        .map(|i| {
            let (x, y) = ((i % 40) as f64 * 20.0, (i / 40) as f64 * 20.0);
            Entity::Line {
                start: Point::new(x, y),
                end: Point::new(x + 12.0, y + 7.0),
            }
        })
        .collect();
    for i in 0..4 {
        entities.push(Entity::MText(Box::new(MText::new(
            Point::new(10.0 + 90.0 * f64::from(i), 300.0),
            3.0,
            format!("note {i}"),
            TextStyleId::STANDARD,
        ))));
    }
    Document::from_entities(entities)
}

fn camera() -> Camera {
    Camera {
        center: Point::new(400.0, 300.0),
        zoom: 1.0,
    }
}

struct Rig {
    ctx: Context,
    view: View,
    style: CanvasStyle,
    screen: Vec2,
    pointer: f32,
}

impl Rig {
    /// A view that keeps its picture (`mode`), told the drawing is
    /// `(1, 0)`; or one that was never told, if `token` is none.
    fn new(mode: CacheMode, token: Option<(u64, u64)>) -> Self {
        let mut view = View::default();
        view.camera = camera();
        view.set_cache_config(CacheConfig {
            mode,
            ..CacheConfig::default()
        });
        view.set_document_token(token);
        Self {
            ctx: Context::default(),
            view,
            style: style(),
            screen: SCREEN,
            pointer: 100.0,
        }
    }

    /// One frame; the pointer moves a little each time. Returns the triangles
    /// as the exact bits of corners and colours, in order.
    fn frame(&mut self, doc: &Document, selection: &BTreeSet<EntityId>) -> Vec<[u32; 3]> {
        self.pointer += 3.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            events: vec![Event::PointerMoved(Pos2::new(self.pointer, 200.0))],
            ..RawInput::default()
        };
        let (view, style) = (&mut self.view, &self.style);
        let out = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::new())
                .show(ctx, |ui| {
                    view.show(
                        ui,
                        style,
                        &GridSettings {
                            visible: true,
                            spacing: 10.0,
                        },
                        doc,
                        selection,
                        &SnapModes::default(),
                    );
                });
        });
        let mut triangles = Vec::new();
        for m in self.ctx.tessellate(out.shapes, 1.0) {
            let Primitive::Mesh(mesh) = m.primitive else {
                continue;
            };
            for t in mesh.indices.chunks(3) {
                for &i in t {
                    let v = mesh.vertices[i as usize];
                    let colour = u32::from_le_bytes(v.color.to_array());
                    triangles.push([v.pos.x.to_bits(), v.pos.y.to_bits(), colour]);
                }
            }
        }
        triangles
    }

    fn builds(&self) -> u64 {
        self.view.cache_stats().builds
    }
}

fn none() -> BTreeSet<EntityId> {
    BTreeSet::new()
}

#[test]
fn frames_where_only_the_pointer_moves_do_not_rebuild() {
    let doc = drawing(200);
    let mut rig = Rig::new(CacheMode::On, Some((1, 0)));
    for _ in 0..6 {
        rig.frame(&doc, &none());
    }
    let stats = rig.view.cache_stats();
    assert_eq!(stats.builds, 1);
    assert_eq!(stats.hits, 6);
    assert!(stats.bytes > 0);
}

#[test]
fn a_view_that_was_not_told_the_drawing_keeps_nothing() {
    let doc = drawing(200);
    let mut rig = Rig::new(CacheMode::On, None);
    for _ in 0..3 {
        rig.frame(&doc, &none());
    }
    assert_eq!(rig.view.cache_stats().builds, 0);
}

#[test]
fn off_keeps_nothing_and_automatic_waits_for_a_big_drawing() {
    let doc = drawing(200);
    let mut off = Rig::new(CacheMode::Off, Some((1, 0)));
    let mut auto = Rig::new(CacheMode::Automatic, Some((1, 0)));
    for _ in 0..3 {
        off.frame(&doc, &none());
        auto.frame(&doc, &none());
    }
    assert_eq!(off.builds(), 0);
    assert_eq!(auto.builds(), 0, "a small drawing is painted directly");
    let big = drawing(AUTO_MIN_OBJECTS);
    auto.frame(&big, &none());
    assert_eq!(auto.builds(), 1);
}

#[test]
fn the_kept_picture_is_the_picture_painted_directly() {
    let doc = drawing(300);
    let mut kept = Rig::new(CacheMode::On, Some((1, 0)));
    let mut direct = Rig::new(CacheMode::Off, Some((1, 0)));
    for _ in 0..3 {
        let (a, b) = (kept.frame(&doc, &none()), direct.frame(&doc, &none()));
        assert!(!a.is_empty());
        assert_eq!(a, b);
    }
    assert!(kept.view.cache_stats().hits >= 2);
}

#[test]
fn anything_the_picture_depends_on_rebuilds_it() {
    let doc = drawing(200);
    let mut rig = Rig::new(CacheMode::On, Some((1, 0)));
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 1);
    // Each change below must add exactly one build, then settle.
    let mut count = rig.builds();
    macro_rules! changes {
        ($what:expr, $change:expr) => {{
            $change;
            rig.frame(&doc, &none());
            rig.frame(&doc, &none());
            count += 1;
            assert_eq!(rig.builds(), count, "{} rebuilds the picture once", $what);
        }};
    }
    changes!(
        "an edit (the revision)",
        rig.view.set_document_token(Some((1, 1)))
    );
    changes!("another drawing", rig.view.set_document_token(Some((2, 1))));
    changes!("panning", rig.view.camera.center = Point::new(410.0, 300.0));
    changes!("zooming", rig.view.camera.zoom = 1.25);
    changes!("a bigger window", rig.screen = Vec2::new(1100.0, 700.0));
    changes!("the line colour", rig.style.entity = Color32::YELLOW);
    changes!(
        "a text being edited",
        rig.view.set_hidden_entity(Some(EntityId(0)))
    );
}

#[test]
fn the_point_style_is_part_of_the_key() {
    let mut doc = drawing(100);
    doc.add(Entity::Point {
        at: Point::new(5.0, 5.0),
    });
    let mut rig = Rig::new(CacheMode::On, Some((1, 0)));
    rig.frame(&doc, &none());
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 1);
    doc.set_point_style(PointStyle {
        mode: 35,
        size: 6.0,
    });
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 2);
}

#[test]
fn a_selected_or_hovered_entity_is_drawn_over_the_kept_picture_without_a_rebuild() {
    let doc = drawing(200);
    let first = doc.objects()[0].id;
    let mut rig = Rig::new(CacheMode::On, Some((1, 0)));
    // Off the canvas, so that nothing is hovered.
    rig.pointer = -2000.0;
    let plain = rig.frame(&doc, &none());
    let selected = rig.frame(&doc, &BTreeSet::from([first]));
    assert_eq!(rig.builds(), 1, "selecting does not rebuild");
    assert!(selected.len() > plain.len(), "the highlight is drawn");
    let back = rig.frame(&doc, &none());
    assert_eq!(back.len(), plain.len());
    assert_eq!(rig.builds(), 1);
}

#[test]
fn a_layer_over_the_memory_limit_is_not_kept_and_is_still_drawn() {
    let doc = drawing(300);
    let mut small = Rig::new(CacheMode::On, Some((1, 0)));
    small.view.set_cache_config(CacheConfig {
        mode: CacheMode::On,
        memory_limit: 1_000,
    });
    let mut direct = Rig::new(CacheMode::Off, Some((1, 0)));
    for _ in 0..3 {
        let (a, b) = (small.frame(&doc, &none()), direct.frame(&doc, &none()));
        assert_eq!(a, b);
    }
    let stats = small.view.cache_stats();
    assert_eq!((stats.builds, stats.hits, stats.bytes), (1, 0, 0));
}

#[test]
fn clearing_the_cache_builds_it_again() {
    let doc = drawing(100);
    let mut rig = Rig::new(CacheMode::On, Some((1, 0)));
    rig.frame(&doc, &none());
    rig.view.clear_layer_cache();
    rig.frame(&doc, &none());
    assert_eq!(rig.builds(), 2);
}

/// The triangles of a frame as a set: where in the frame each is drawn does
/// not matter, only what is drawn.
fn as_set(flat: &[[u32; 3]]) -> Vec<[[u32; 3]; 3]> {
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
        mode: CacheMode::On,
        memory_limit: base + 10,
    });
    let one = BTreeSet::from([doc.objects()[0].id]);
    for _ in 0..3 {
        assert!(!rig.frame(&doc, &one).is_empty());
    }
    let stats = rig.view.cache_stats();
    assert_eq!(stats.bytes, base, "memory stays within the limit");
    assert_eq!(stats.builds, 1);
}
