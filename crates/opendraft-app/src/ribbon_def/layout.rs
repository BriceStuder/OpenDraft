//! The contextual Layout tab, shown after the usual tabs while a layout is on
//! screen. Its panels follow AutoCAD 2020's Layout tab as far as the commands
//! exist: Layout (New, Page Setup, and Plot, which AutoCAD keeps on its Output
//! tab, a tab this ribbon does not have) and Layout Viewports (Rectangular,
//! Lock, and the viewport scale and layers). Display is OpenDraft's own: the
//! Light Paper toggle (`canvas_style.rs`). It never takes the focus (see the
//! module notes in `ribbon_def.rs`).

use opendraft_ui::icons;
use opendraft_ui::ribbon::{Column, Item, Panel, Tab};

/// The title of the contextual tab shown while a layout is open.
pub const LAYOUT_TAB: &str = "Layout";

pub(super) fn layout() -> Tab {
    Tab {
        title: LAYOUT_TAB,
        panels: vec![
            Panel {
                title: "Layout",
                columns: vec![
                    Column::Large(Item::new("layout.new", "New", icons::LAYOUT_NEW)),
                    Column::Large(Item::new(
                        "layout.page_setup",
                        "Page Setup",
                        icons::LAYOUT_PAGE_SETUP,
                    )),
                    Column::Large(Item::new("file.plot", "Plot", icons::FILE_PLOT)),
                ],
                ..Default::default()
            },
            Panel {
                title: "Layout Viewports",
                columns: vec![
                    Column::Large(Item::new(
                        "layout.mview",
                        "Rectangular",
                        icons::LAYOUT_MVIEW,
                    )),
                    Column::Stack(vec![
                        Item::new("layout.vplock", "Lock", icons::LAYOUT_VPLOCK),
                        Item::new("layout.vpscale", "Viewport Scale", icons::LAYOUT_VPSCALE),
                        Item::new("layout.vplayer", "Viewport Layers", icons::LAYOUT_VPLAYER),
                    ]),
                ],
                ..Default::default()
            },
            Panel {
                title: "Display",
                columns: vec![Column::Large(Item::new(
                    "view.layout_paper",
                    "Light Paper",
                    icons::VIEW_LAYOUT_PAPER,
                ))],
                ..Default::default()
            },
        ],
        contextual: true,
    }
}
