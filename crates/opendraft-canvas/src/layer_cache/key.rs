//! Everything the cached picture depends on. The cache is reused only while
//! its key equals the one for the frame being drawn, so a new input to the
//! painting of plain entities must be added here (see the audit note in
//! `layer_cache/mod.rs`).
//!
//! The key has two parts. The content is what the picture shows: the drawing
//! and everything that changes how its entities are drawn. The view is where
//! it is looked at from: the camera, and the clip (a viewport's window moves
//! with the sheet). A layer whose content is still right but whose view is not
//! can be drawn moved until a new one is made (see `moved.rs`).

use egui::epaint::TessellationOptions;
use egui::{Color32, Rect};
use opendraft_core::{EntityId, LayerId, LayoutId, PointStyle};

use crate::camera::Camera;

/// The camera and clip as exact bits, so that equal means identical, never
/// "close".
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct View {
    camera: [u64; 4],
    clip: [u32; 4],
}

/// The camera of `view`.
pub(crate) fn camera_of(view: View) -> Camera {
    let [x, y, zoom, twist] = view.camera;
    Camera {
        center: opendraft_core::Point::new(f64::from_bits(x), f64::from_bits(y)),
        zoom: f64::from_bits(zoom),
        twist: f64::from_bits(twist),
    }
}

/// What a layer shows, besides the camera. Floats are kept as their bits.
#[derive(Clone, Debug, PartialEq)]
struct Content {
    /// The drawing: its session and revision (`Session::draw_token`).
    token: (u64, u64),
    space: Option<LayoutId>,
    frozen: Vec<LayerId>,
    canvas: [u32; 4],
    line: Color32,
    point_style: PointStyle,
    hidden: Option<EntityId>,
    pixels_per_point: u32,
    tessellation: TessellationOptions,
}

/// The inputs of one cached layer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayerKey {
    content: Content,
    view: View,
}

fn bits(rect: Rect) -> [u32; 4] {
    [
        rect.min.x.to_bits(),
        rect.min.y.to_bits(),
        rect.max.x.to_bits(),
        rect.max.y.to_bits(),
    ]
}

/// What a layer is made from, besides the drawing itself.
pub(crate) struct Inputs<'a> {
    pub token: (u64, u64),
    pub space: Option<LayoutId>,
    pub frozen: &'a [LayerId],
    pub camera: &'a Camera,
    pub canvas: Rect,
    pub clip: Rect,
    pub line: Color32,
    pub point_style: PointStyle,
    pub hidden: Option<EntityId>,
    pub pixels_per_point: f32,
    pub tessellation: TessellationOptions,
}

impl LayerKey {
    pub fn new(i: Inputs<'_>) -> Self {
        Self {
            content: Content {
                token: i.token,
                space: i.space,
                frozen: i.frozen.to_vec(),
                canvas: bits(i.canvas),
                line: i.line,
                point_style: i.point_style,
                hidden: i.hidden,
                pixels_per_point: i.pixels_per_point.to_bits(),
                tessellation: i.tessellation,
            },
            view: View {
                camera: [
                    i.camera.center.x.to_bits(),
                    i.camera.center.y.to_bits(),
                    i.camera.zoom.to_bits(),
                    i.camera.twist.to_bits(),
                ],
                clip: bits(i.clip),
            },
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    /// Whether `other` shows the same thing, from perhaps another camera.
    pub fn same_content(&self, other: &LayerKey) -> bool {
        self.content == other.content
    }
}
