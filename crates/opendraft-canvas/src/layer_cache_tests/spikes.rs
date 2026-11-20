//! An outline that turns back on itself draws only itself: egui's joins at a
//! reversal can throw a corner across the screen (`view/path_stroke.rs`).

use super::*;

/// The outline that showed a streak (`render_issue_1`): out along an arc, a
/// hair across, straight back along the chord and a hair to the start. Zoomed
/// out, the arc is drawn as its chord, so the path doubles back exactly.
fn sliver() -> Document {
    let p = |x, y| Point::new(x, y);
    Document::from_entities([Entity::Polyline {
        points: vec![
            p(923_399.301_143_586_8, 39_822.657_174_214_8),
            p(923_600.389_018_283, 40_637.671_234_846_54),
            p(923_600.389, 40_637.671),
            p(923_399.301, 39_822.657),
        ],
        closed: true,
        bulges: vec![-0.106_113_379_610_910_26, 0.0, 0.0, 0.0],
        widths: Vec::new(),
    }])
}

#[test]
fn an_outline_that_doubles_back_draws_no_streak() {
    let doc = sliver();
    for mode in [CacheMode::Off, CacheMode::On] {
        let mut rig = Rig::new(mode, Some((1, 0)));
        rig.pointer = -2000.0;
        for zoom in [0.001, 0.003, 0.01, 0.05, 1.0] {
            rig.view.camera = Camera {
                center: Point::new(923_500.0, 40_230.0),
                zoom,
            };
            for v in rig.frame(&doc, &none()) {
                let (x, y) = (f32::from_bits(v[0]), f32::from_bits(v[1]));
                let near = |c: f32, side: f32| c.is_finite() && (-side..2.0 * side).contains(&c);
                assert!(
                    near(x, SCREEN.x) && near(y, SCREEN.y),
                    "a corner at {x}, {y} (zoom {zoom}, {mode:?})"
                );
            }
        }
    }
}
