//! The navigation gizmo in the top right corner of the drawing: a home button
//! (PLAN, World), a ring of eight sectors that turn the view to the four
//! directions and the four between, and an arrow outside the ring showing where
//! the drawing's up lies, which can be dragged round to turn the view freely.
//! The geometry is `opendraft_canvas::nav_gizmo`; this paints it and acts on
//! the pointer. It changes the view only, so a running command carries on.

use eframe::egui::{self, Color32, Id, Mesh, Pos2, Sense, Shape, Stroke};
use opendraft_canvas::nav_gizmo::{sector_of_twist, sector_twist, Gizmo, Part, RADIUS, SECTORS};
use opendraft_canvas::{Space, View};
use opendraft_core::Document;
use opendraft_ui::{icon_set, icons, Theme};

use crate::state::AppState;

/// Gap between the gizmo and the canvas edge.
const MARGIN: f32 = 10.0;
/// What the arrow's drag is held to with Shift, in degrees.
const SHIFT_STEP: f64 = 15.0;
/// The gap left between two sectors, in degrees.
const GAP: f64 = 5.0;
/// The size of the house on the home button.
const HOME_ICON: f32 = 18.0;

/// Show the gizmo in the active drawing's canvas, if the setting is on and
/// its space can turn. Returns `canvas` less the strip the gizmo takes along
/// its right edge, for what else is drawn in the top right corner.
pub fn show_in(
    ctx: &egui::Context,
    state: &mut AppState,
    canvas: egui::Rect,
    theme: &Theme,
) -> egui::Rect {
    if !state.settings.show_nav_gizmo {
        return canvas;
    }
    let doc = &mut state.docs[state.active];
    if !turns(doc.session.document(), doc.view.space()) {
        return canvas;
    }
    show(ctx, canvas, theme, &mut doc.view);
    canvas.with_max_x(canvas.right() - 2.0 * (RADIUS + MARGIN))
}

/// Whether the space being worked in can turn: not a layout's sheet, nor a
/// locked viewport.
pub fn turns(document: &Document, space: Space) -> bool {
    match space {
        Space::Model => true,
        Space::Paper(_) => false,
        Space::Viewport(layout, viewport) => document
            .layout(layout)
            .and_then(|l| l.viewport(viewport))
            .is_some_and(|v| !v.locked),
    }
}

/// A dragged angle held to `step` degrees.
fn held_to(twist: f64, step: f64) -> f64 {
    let step = step.to_radians();
    (twist / step).round() * step
}

/// The shade of a sector or the home button.
fn fill(theme: &Theme, hovered: bool, current: bool) -> Color32 {
    let colors = &theme.colors;
    if hovered {
        colors.hover
    } else if current {
        colors.border_strong
    } else {
        colors.panel_raised.gamma_multiply(0.8)
    }
}

/// Paint the gizmo, and turn the view as the pointer asks. `canvas` is the
/// drawing's rectangle.
pub fn show(ctx: &egui::Context, canvas: egui::Rect, theme: &Theme, view: &mut View) {
    let gizmo = Gizmo::in_corner(canvas, MARGIN);
    let id = Id::new("nav_gizmo");
    let twist = view.twist();
    egui::Area::new(id)
        .order(egui::Order::Middle)
        .fixed_pos(gizmo.centre - egui::Vec2::splat(RADIUS))
        .constrain(false)
        .show(ctx, |ui| {
            let size = egui::Vec2::splat(2.0 * RADIUS);
            let (_, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            // Whether the arrow was pressed, kept from the press to the release.
            let mut grabbed: bool = ui.data(|d| d.get_temp(id)).unwrap_or(false);
            if response.drag_started() {
                let pressed = ui.input(|i| i.pointer.press_origin());
                grabbed = pressed.and_then(|p| gizmo.part_at(p, twist)) == Some(Part::Arrow);
                // The first frame of a drag is a step of its own for ZOOM Previous.
                if grabbed {
                    view.request_twist(twist);
                }
            }
            if grabbed && response.dragged() {
                if let Some(at) = ui.input(|i| i.pointer.latest_pos()) {
                    let mut turn = gizmo.twist_at(at);
                    if ui.input(|i| i.modifiers.shift) {
                        turn = held_to(turn, SHIFT_STEP);
                    }
                    view.request_twist_live(turn);
                }
            }
            if response.drag_stopped() {
                grabbed = false;
            }
            ui.data_mut(|d| d.insert_temp(id, grabbed));
            if response.clicked() {
                let at = ui.input(|i| i.pointer.interact_pos());
                match at.and_then(|p| gizmo.part_at(p, twist)) {
                    Some(Part::Home) => view.request_plan(0.0),
                    Some(Part::Sector(k)) => view.request_twist(sector_twist(k)),
                    _ => {}
                }
            }
            // What is under the pointer; the arrow while it is dragged.
            let hot = if grabbed {
                Some(Part::Arrow)
            } else {
                response.hover_pos().and_then(|p| gizmo.part_at(p, twist))
            };
            match hot {
                Some(Part::Arrow) if grabbed => {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing)
                }
                Some(Part::Arrow) => ui.ctx().set_cursor_icon(egui::CursorIcon::Grab),
                Some(_) => ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand),
                None => {}
            }
            paint(ui, theme, &gizmo, twist, hot);
        });
}

/// Draw the sectors, the home button and the arrow.
fn paint(ui: &egui::Ui, theme: &Theme, gizmo: &Gizmo, twist: f64, hot: Option<Part>) {
    let painter = ui.painter();
    let colors = &theme.colors;
    let outline = Stroke::new(1.0_f32, colors.border);
    let current = sector_of_twist(twist);
    for sector in 0..SECTORS {
        let color = fill(
            theme,
            hot == Some(Part::Sector(sector)),
            current == Some(sector),
        );
        let points = gizmo.sector_outline(sector, GAP);
        painter.add(strip(&points, color));
        painter.add(Shape::closed_line(points, outline));
    }
    let home = fill(theme, hot == Some(Part::Home), false);
    painter.circle(gizmo.centre, gizmo.home_radius(), home, outline);
    icon_set::paint(
        painter,
        gizmo.centre,
        egui::Align2::CENTER_CENTER,
        icons::NAV_GIZMO_HOME,
        HOME_ICON,
        colors.text,
    );
    let arrow = if hot == Some(Part::Arrow) {
        colors.text
    } else {
        colors.text_weak
    };
    painter.add(Shape::convex_polygon(
        gizmo.arrow_corners(twist).to_vec(),
        arrow,
        Stroke::NONE,
    ));
}

/// A sector outline (the outer arc, then the inner one back) filled as a strip
/// of quads: a ring's sector is not convex, which a polygon fill needs.
fn strip(points: &[Pos2], color: Color32) -> Shape {
    let mut mesh = Mesh::default();
    let n = points.len() / 2;
    for p in points {
        mesh.colored_vertex(*p, color);
    }
    for i in 0..n.saturating_sub(1) {
        let (o0, o1) = (i as u32, i as u32 + 1);
        let (i0, i1) = ((2 * n - 1 - i) as u32, (2 * n - 2 - i) as u32);
        mesh.add_triangle(o0, o1, i1);
        mesh.add_triangle(o0, i1, i0);
    }
    Shape::mesh(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_turn_goes_to_the_nearest_step() {
        let held = held_to(20.0_f64.to_radians(), SHIFT_STEP);
        assert!((held - 15.0_f64.to_radians()).abs() < 1e-12);
    }

    #[test]
    fn the_model_tab_turns() {
        assert!(turns(&Document::default(), Space::Model));
    }

    #[test]
    fn a_strip_covers_every_quad_of_the_sector() {
        let gizmo = Gizmo {
            centre: egui::pos2(50.0, 50.0),
        };
        let points = gizmo.sector_outline(0, GAP);
        let Shape::Mesh(mesh) = strip(&points, Color32::WHITE) else {
            panic!("a strip is a mesh");
        };
        assert_eq!(mesh.vertices.len(), points.len());
        assert_eq!(mesh.indices.len(), 6 * (points.len() / 2 - 1));
        assert!(mesh.is_valid());
    }
}
