//! The command line as a strip above or below the drawing: when it is one,
//! where it may sit, and how small a dragged separator may leave it.

use egui_dock::{DockState, NodeIndex};

use super::{PanelTab, BELOW_SPLIT, PANEL_MIN};

/// The command line docked above or below the drawing with no other tab
/// beside it. It is then drawn like AutoCAD's: no tab bar, and a narrow grip
/// down its left edge instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strip {
    Above,
    Below,
}

/// The other half of the split `node` is in.
pub(super) fn sibling(node: NodeIndex) -> Option<NodeIndex> {
    let parent = node.parent()?;
    Some(if node == parent.left() {
        parent.right()
    } else {
        parent.left()
    })
}

/// Whether the command line shows as a strip, and on which side of the
/// drawing.
pub fn command_line_strip(dock: &DockState<PanelTab>) -> Option<Strip> {
    let (surface, node, _) = dock.find_tab(&PanelTab::CommandLine)?;
    let (_, canvas, _) = dock.find_tab(&PanelTab::Canvas)?;
    let (parent, other) = (node.parent()?, sibling(node)?);
    let tree = dock.main_surface();
    if surface != egui_dock::SurfaceIndex::main()
        || tree[node].tabs_count() != 1
        || !matches!(tree[parent], egui_dock::Node::Vertical { .. })
    {
        return None;
    }
    // The other half of the split holds the drawing.
    std::iter::successors(Some(canvas), |n| n.parent())
        .any(|n| n == other)
        .then_some(match node == parent.left() {
            true => Strip::Above,
            false => Strip::Below,
        })
}

/// A strip dropped along the whole window moves to the drawing's own column,
/// so the panels beside the drawing keep their full height, as in AutoCAD.
pub fn tuck_command_line(dock: &mut DockState<PanelTab>) {
    let Some(strip) = command_line_strip(dock) else {
        return;
    };
    let (Some(found), Some((_, canvas, _))) = (
        dock.find_tab(&PanelTab::CommandLine),
        dock.find_tab(&PanelTab::Canvas),
    ) else {
        return;
    };
    if sibling(found.1) == Some(canvas) {
        return;
    }
    let tree = dock.main_surface();
    let height = |node: NodeIndex| tree[node].rect().map_or(0.0, |r| r.height());
    // The drawing gets back the height the strip leaves, then shares it.
    let (own, drawing) = (height(found.1), height(canvas));
    let share = match own > 0.0 && drawing > 0.0 {
        true => own / (own + drawing),
        false => 1.0 - BELOW_SPLIT,
    };
    dock.remove_tab(found);
    let Some((_, canvas, _)) = dock.find_tab(&PanelTab::Canvas) else {
        return;
    };
    // The fraction is the top half's share.
    let (tree, tabs) = (dock.main_surface_mut(), vec![PanelTab::CommandLine]);
    match strip {
        Strip::Above => tree.split_above(canvas, share, tabs),
        Strip::Below => tree.split_below(canvas, 1.0 - share, tabs),
    };
}

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
pub(super) fn held_share(share: f32, range: f32, least: [f32; 2]) -> f32 {
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
