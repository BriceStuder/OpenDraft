//! The drawing view alone in its place has no tab bar: its one tab,
//! "Drawing", could not be closed, floated or switched away from, and the
//! open drawings already have their own tabs above. The drawing takes the
//! bar's height instead. With another panel tabbed beside it, the bar stays
//! so the user can switch between them.

use eframe::egui;

use super::panels::PanelViewer;
use crate::dock;

/// The whole of a panel whose body (the part under its tab bar) is `body`.
fn without_tab_bar(body: egui::Rect) -> egui::Rect {
    body.with_min_y(body.top() - dock::TAB_BAR_HEIGHT)
}

impl PanelViewer<'_> {
    /// The drawing view, over its tab bar when it is alone in its place.
    pub(super) fn drawing(&mut self, ui: &mut egui::Ui) {
        if !self.drawing_alone {
            self.canvas(ui);
            return;
        }
        let whole = without_tab_bar(ui.max_rect());
        // The dock always draws a tab bar and clips the panel under it, so
        // draw over it from a `Ui` of our own. The drawing view then takes
        // the pointer there too, away from the hidden tab.
        let mut over = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(whole)
                .id_salt("drawing_without_tab_bar"),
        );
        over.set_clip_rect(whole);
        self.canvas(&mut over);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drawing_takes_the_height_of_the_tab_bar() {
        let body = egui::Rect::from_min_max(egui::pos2(10.0, 124.0), egui::pos2(810.0, 600.0));
        let whole = without_tab_bar(body);
        assert_eq!(whole.top(), 124.0 - dock::TAB_BAR_HEIGHT);
        assert_eq!(whole.bottom(), 600.0);
        assert_eq!((whole.left(), whole.right()), (10.0, 810.0));
    }
}
