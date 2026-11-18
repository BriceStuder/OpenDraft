//! Everything the cached picture depends on. The cache is reused only while
//! its key equals the one for the frame being drawn, so a new input to the
//! painting of plain entities must be added here (see the audit note in
//! `layer_cache/mod.rs`).

use egui::epaint::TessellationOptions;
use egui::{Color32, Rect};
use opendraft_core::{EntityId, LayerId, LayoutId, PointStyle};

use crate::camera::Camera;

/// The inputs of one cached layer. Floats are kept as their bits so that
/// equal means identical, never "close".
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayerKey {
    /// The drawing: its session and revision (`Session::draw_token`).
    token: (u64, u64),
    space: Option<LayoutId>,
    frozen: Vec<LayerId>,
    camera: [u64; 3],
    canvas: [u32; 4],
    clip: [u32; 4],
    line: Color32,
    point_style: PointStyle,
    hidden: Option<EntityId>,
    pixels_per_point: u32,
    tessellation: TessellationOptions,
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
            token: i.token,
            space: i.space,
            frozen: i.frozen.to_vec(),
            camera: [
                i.camera.center.x.to_bits(),
                i.camera.center.y.to_bits(),
                i.camera.zoom.to_bits(),
            ],
            canvas: bits(i.canvas),
            clip: bits(i.clip),
            line: i.line,
            point_style: i.point_style,
            hidden: i.hidden,
            pixels_per_point: i.pixels_per_point.to_bits(),
            tessellation: i.tessellation,
        }
    }
}
