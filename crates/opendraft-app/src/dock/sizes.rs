//! How small a dragged separator may make a panel. The dock has one limit
//! for every separator, set low enough for the command line (see
//! [`super::style`]); these put the usual limit back on every other panel.

use egui_dock::{DockState, NodeIndex};

use super::{PanelTab, PANEL_MIN};

/// The share of every split of the main area, taken before the dock is
/// shown for [`keep_minimum_sizes`] to see which separators moved.
pub fn split_shares(dock: &DockState<PanelTab>) -> Vec<Option<f32>> {
    let share = |node: &egui_dock::Node<PanelTab>| match node {
        egui_dock::Node::Vertical { fraction, .. }
        | egui_dock::Node::Horizontal { fraction, .. } => Some(*fraction),
        _ => None,
    };
    dock.main_surface().iter().map(share).collect()
}

/// `share` of a split `range` points long, held so that its two halves keep
/// at least `least` points each. Left alone when both cannot fit.
fn held_share(share: f32, range: f32, least: [f32; 2]) -> f32 {
    if range <= 0.0 || least[0] + least[1] > range {
        return share;
    }
    share.clamp(least[0] / range, 1.0 - least[1] / range)
}

/// The dock's own limit on separators is set low enough for the command
/// line (see [`super::style`]); this puts the usual limit back on every other
/// panel and keeps `line_least` points for the command line. Only
/// separators moved since `before` are touched.
pub fn keep_minimum_sizes(dock: &mut DockState<PanelTab>, before: &[Option<f32>], line_least: f32) {
    let tree = dock.main_surface_mut();
    if tree.len() != before.len() {
        return;
    }
    for (i, was) in before.iter().enumerate() {
        let index = NodeIndex(i);
        let vertical = matches!(tree[index], egui_dock::Node::Vertical { .. });
        let least = [index.left(), index.right()].map(|child| {
            let command_line = (child.0 < tree.len())
                .then(|| tree[child].tabs())
                .flatten()
                .is_some_and(|tabs| tabs.contains(&PanelTab::CommandLine));
            match vertical && command_line {
                true => line_least,
                false => PANEL_MIN,
            }
        });
        match &mut tree[index] {
            egui_dock::Node::Vertical { rect, fraction, .. } if Some(*fraction) != *was => {
                *fraction = held_share(*fraction, rect.height(), least);
            }
            egui_dock::Node::Horizontal { rect, fraction, .. } if Some(*fraction) != *was => {
                *fraction = held_share(*fraction, rect.width(), least);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_share_is_held_between_the_least_sizes_when_they_fit() {
        assert_eq!(held_share(0.5, 1000.0, [175.0, 74.0]), 0.5);
        assert_eq!(held_share(0.0, 1000.0, [175.0, 74.0]), 0.175);
        assert_eq!(held_share(1.0, 1000.0, [175.0, 74.0]), 1.0 - 0.074);
        assert_eq!(held_share(0.9, 200.0, [175.0, 74.0]), 0.9);
        assert_eq!(held_share(0.9, 0.0, [175.0, 74.0]), 0.9);
    }
}
