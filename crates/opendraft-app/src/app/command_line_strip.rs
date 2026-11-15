//! The command line docked above or below the drawing, drawn like AutoCAD's:
//! no tab bar, a narrow grip down its left edge to close or undock it, and,
//! under the drawing, the Model and layout tabs below it.

use eframe::egui;

use super::command_line::MARGIN;
use super::panels::PanelViewer;
use crate::dock::{self, LayoutRequest, PanelTab, Strip};

/// Width of the grip.
const GRIP_WIDTH: f32 = 18.0;

/// Space above and below the Model and layout tabs.
const TABS_MARGIN: f32 = 2.0;

/// Heights of the rows a strip is made of, measured from the fonts and
/// spacing in use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct StripRows {
    /// One line of history.
    pub log: f32,
    /// Space between two rows.
    pub gap: f32,
    /// The prompt and input row.
    pub input: f32,
    /// The Model and layout tabs, when the strip shows them (else 0).
    pub tabs: f32,
}

impl StripRows {
    pub fn measure(ctx: &egui::Context, theme: &opendraft_ui::Theme, strip: Option<Strip>) -> Self {
        let font = egui::FontId::proportional(theme.font_size_small);
        let spacing = ctx.style().spacing.clone();
        Self {
            log: ctx.fonts(|f| f.row_height(&font)).ceil(),
            gap: spacing.item_spacing.y,
            input: spacing.interact_size.y.max(22.0),
            tabs: match strip {
                Some(Strip::Below) => spacing.interact_size.y + 2.0 * TABS_MARGIN,
                _ => 0.0,
            },
        }
    }

    /// Height of a strip showing `lines` of history above its input row.
    pub fn height(&self, lines: usize) -> f32 {
        2.0 * MARGIN + lines as f32 * (self.log + self.gap) + self.input + self.tabs
    }
}

/// The parts of a strip.
struct Parts {
    /// The dock's tab bar, which the strip covers.
    tab_bar: egui::Rect,
    grip: egui::Rect,
    /// The command line itself.
    line: egui::Rect,
    /// The Model and layout tabs, under both.
    tabs: egui::Rect,
}

/// Lay a strip out in a panel whose body (the part under its tab bar) is
/// `body`, keeping `tabs` points along the bottom for the layout tabs.
fn layout(body: egui::Rect, tabs: f32) -> Parts {
    let whole = body.with_min_y(body.top() - dock::TAB_BAR_HEIGHT);
    let (upper, tabs) = whole.split_top_bottom_at_y((whole.bottom() - tabs).max(whole.top()));
    let (grip, line) = upper.split_left_right_at_x(upper.left() + GRIP_WIDTH.min(upper.width()));
    Parts {
        tab_bar: whole.with_max_y(body.top()),
        grip,
        line,
        tabs,
    }
}

impl PanelViewer<'_> {
    /// Cover the panel's tab bar and draw the grip: a close button above a
    /// handle that undocks the panel when dragged or double-clicked. Returns
    /// the `Ui` for the command line itself, right of the grip.
    pub(super) fn command_line_grip(
        &mut self,
        ui: &mut egui::Ui,
        side: Strip,
        rows: &StripRows,
    ) -> egui::Ui {
        let theme = self.theme;
        let colors = &theme.colors;
        let Parts {
            tab_bar,
            grip,
            line,
            tabs,
        } = layout(ui.max_rect(), rows.tabs);
        let whole = tab_bar.union(tabs).union(line);
        // The dock always draws a tab bar and clips the panel under it, so
        // draw over it from a `Ui` of our own.
        let mut over = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(whole)
                .id_salt("command_line_strip"),
        );
        over.set_clip_rect(whole);
        let id = over.id();
        // Keep the pointer from the tab hidden underneath.
        over.interact(tab_bar, id.with("cover"), egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Default);
        let painter = over.painter().clone();
        painter.rect_filled(whole, 0.0, colors.sidebar_bg);
        painter.rect_filled(grip, 0.0, colors.window_bg);

        let (close_rect, handle_rect) = grip.split_top_bottom_at_y(grip.top() + GRIP_WIDTH);
        let handle = over
            .interact(
                handle_rect,
                id.with("handle"),
                egui::Sense::click_and_drag(),
            )
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text("Drag to undock the command line");
        let close = over
            .interact(close_rect, id.with("close"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Close (Ctrl+9 shows it again)");

        let weak_or_text = |hovered| match hovered {
            true => colors.text,
            false => colors.text_weak,
        };
        let cross = egui::Stroke::new(1.2_f32, weak_or_text(close.hovered()));
        let c = close_rect.center();
        for dy in [-3.5_f32, 3.5] {
            painter.line_segment([c + egui::vec2(-3.5, dy), c + egui::vec2(3.5, -dy)], cross);
        }
        // Two columns of dots, as on a toolbar handle.
        let dots = weak_or_text(handle.hovered() || handle.dragged());
        let mut y = handle_rect.top() + 4.0;
        while y < handle_rect.bottom() - 3.0 {
            for dx in [-2.0_f32, 2.0] {
                painter.circle_filled(egui::pos2(handle_rect.center().x + dx, y), 0.9, dots);
            }
            y += 4.0;
        }

        if close.clicked() {
            self.state.layout_request = Some(LayoutRequest::Toggle(PanelTab::CommandLine));
        } else if handle.drag_started() || handle.double_clicked() {
            self.state.layout_request = Some(LayoutRequest::Float(PanelTab::CommandLine));
        }

        if side == Strip::Below && tabs.height() > 0.0 {
            // The drawing leaves its Model and layout tabs to the strip, so
            // they come under the command line.
            let mut tabs_ui = over.new_child(
                egui::UiBuilder::new()
                    .max_rect(tabs.shrink2(egui::vec2(6.0, TABS_MARGIN)))
                    .id_salt("layout_tabs"),
            );
            let doc = &self.state.docs[self.state.active];
            let (document, space) = (doc.session.document(), doc.view.space());
            let action = crate::layout_tabs::show(&mut tabs_ui, theme, document, space);
            if let Some(action) = action {
                crate::layout_tabs::apply(self.state, action);
            }
        }

        let mut inner = over.new_child(egui::UiBuilder::new().max_rect(line).id_salt("line"));
        inner.set_clip_rect(line);
        inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS: StripRows = StripRows {
        log: 14.0,
        gap: 4.0,
        input: 26.0,
        tabs: 0.0,
    };

    #[test]
    fn the_strip_takes_the_tab_bar_and_keeps_a_grip_on_the_left() {
        let body = egui::Rect::from_min_max(egui::pos2(10.0, 524.0), egui::pos2(810.0, 574.0));
        let Parts {
            tab_bar,
            grip,
            line,
            tabs,
        } = layout(body, 0.0);
        assert_eq!(tab_bar.top(), 500.0);
        assert_eq!(tab_bar.bottom(), body.top());
        // Grip and command line share the whole height, tab bar included.
        assert_eq!((grip.top(), grip.bottom()), (500.0, 574.0));
        assert_eq!((line.top(), line.bottom()), (500.0, 574.0));
        assert_eq!((grip.left(), grip.right()), (10.0, 10.0 + GRIP_WIDTH));
        assert_eq!((line.left(), line.right()), (10.0 + GRIP_WIDTH, 810.0));
        assert_eq!(tabs.height(), 0.0);
    }

    #[test]
    fn the_layout_tabs_go_under_the_grip_and_the_command_line() {
        let body = egui::Rect::from_min_max(egui::pos2(10.0, 524.0), egui::pos2(810.0, 604.0));
        let parts = layout(body, 30.0);
        assert_eq!((parts.tabs.top(), parts.tabs.bottom()), (574.0, 604.0));
        assert_eq!((parts.tabs.left(), parts.tabs.right()), (10.0, 810.0));
        assert_eq!(parts.grip.bottom(), 574.0);
        assert_eq!(parts.line.bottom(), 574.0);
    }

    #[test]
    fn the_least_strip_is_two_lines_of_history_and_the_input_row() {
        // Margins, two lines each with the space under it, and the row.
        assert_eq!(ROWS.height(2), 4.0 + 18.0 + 18.0 + 26.0 + 4.0);
        assert_eq!(ROWS.height(0), 4.0 + 26.0 + 4.0);
        let with_tabs = StripRows { tabs: 30.0, ..ROWS };
        assert_eq!(with_tabs.height(2), ROWS.height(2) + 30.0);
    }
}
