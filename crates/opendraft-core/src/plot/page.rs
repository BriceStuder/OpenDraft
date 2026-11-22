//! The sheet as a page to read.
//!
//! A plot turned a quarter on its paper (how AutoCAD keeps a landscape plot
//! on paper that feeds portrait) is drawn on the paper the way it goes
//! through a printer, so the drawing lies on its side. A file or a picture
//! has no printer to feed: its page is turned a quarter too, so it opens the
//! way it is read.

use super::{plot_sheet, PlotError, PlotSheet, PlotSpace};
use crate::plot_plan::validate_plot;
use crate::{Bounds, Document, LayoutId, PlotRotation, Point};

/// Whether the plot is turned a quarter on its paper.
fn quarter(rotation: PlotRotation) -> bool {
    matches!(
        rotation,
        PlotRotation::CounterClockwise90 | PlotRotation::Clockwise90
    )
}

/// The Model tab or a layout as a page to read: [`plot_sheet`], with the page
/// turned a quarter clockwise when the plot is turned a quarter on its paper.
/// A plot turned the other way (landscape upside-down) stays upside-down.
pub fn plot_page(document: &Document, space: PlotSpace) -> Result<PlotSheet, PlotError> {
    let sheet = plot_sheet(document, space)?;
    let rotation = match space {
        PlotSpace::Model => document.model_plot().rotation,
        PlotSpace::Layout(id) => {
            document
                .layout(id)
                .ok_or(PlotError::NoLayout)?
                .plot
                .rotation
        }
    };
    Ok(if quarter(rotation) {
        turned(sheet)
    } else {
        sheet
    })
}

/// The sheet turned a quarter clockwise: its left edge becomes the top.
fn turned(sheet: PlotSheet) -> PlotSheet {
    let width = sheet.width;
    let turn = |points: &mut Vec<Point>| {
        for p in points.iter_mut() {
            *p = Point::new(p.y, width - p.x);
        }
    };
    let mut page = PlotSheet {
        width: sheet.height,
        height: width,
        ..sheet
    };
    for path in &mut page.paths {
        turn(&mut path.points);
    }
    for contour in page.fills.iter_mut().flat_map(|f| f.contours.iter_mut()) {
        turn(contour);
    }
    page
}

/// A paper of `width` by `height` mm with `margins` (left, bottom, right,
/// top) as its page is read under `rotation`: turned a quarter clockwise when
/// the plot is, as [`plot_page`] turns it.
pub fn page_as_read(
    width: f64,
    height: f64,
    margins: [f64; 4],
    rotation: PlotRotation,
) -> (f64, f64, [f64; 4]) {
    if quarter(rotation) {
        let [left, bottom, right, top] = margins;
        (height, width, [bottom, right, top, left])
    } else {
        (width, height, margins)
    }
}

/// Where a layout's paper lies on its sheet, in sheet millimetres: the
/// inverse of the placement the plot makes (area, scale, offset, centring),
/// so the canvas can show the paper where the plot will put the drawing on
/// it. Extents plots the layout's objects to find them; the other areas need
/// nothing drawn.
pub fn paper_on_sheet(document: &Document, layout: LayoutId) -> Result<Bounds, PlotError> {
    let layout = document.layout(layout).ok_or(PlotError::NoLayout)?;
    validate_plot(&layout.plot).map_err(PlotError::Invalid)?;
    let (width, height) = layout.page_size();
    let mut sheet = PlotSheet {
        width,
        height,
        paths: Vec::new(),
        fills: Vec::new(),
        notes: Vec::new(),
    };
    if layout.plot.area == crate::PlotArea::Extents {
        sheet = super::layout_sheet(document, layout)?;
    }
    let source = super::settings::source_bounds(&sheet, &layout.plot)?;
    let (at, factor) = super::settings::placement(&layout.plot, &source);
    let paper = super::settings::upright_paper(&layout.plot);
    let on_sheet = |p: Point| {
        Point::new(
            source.min.x + (p.x - at.x) / factor,
            source.min.y + (p.y - at.y) / factor,
        )
    };
    Ok(Bounds::from_points(
        on_sheet(paper.min),
        on_sheet(paper.max),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Entity, PlotSettings};

    /// A horizontal line, left to right, plotted from the Model tab on A4
    /// paper that feeds portrait, under `rotation`, with `margins`.
    fn drawing(rotation: PlotRotation, margins: [f64; 4]) -> Document {
        let mut doc = Document::default();
        doc.add(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(4000.0, 0.0),
        });
        doc.add(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(0.0, 1000.0),
        });
        let mut settings: PlotSettings = doc.model_plot().clone();
        settings.paper_width_mm = 210.0;
        settings.paper_height_mm = 297.0;
        settings.rotation = rotation;
        settings.margins_mm = margins;
        doc.set_model_plot(settings);
        doc
    }

    fn page(rotation: PlotRotation) -> PlotSheet {
        plot_page(&drawing(rotation, [0.0; 4]), PlotSpace::Model).unwrap()
    }

    #[test]
    fn a_plot_turned_a_quarter_comes_on_a_page_turned_with_it() {
        let doc = drawing(PlotRotation::CounterClockwise90, [0.0; 4]);
        let sheet = plot_sheet(&doc, PlotSpace::Model).unwrap();
        assert_eq!((sheet.width, sheet.height), (210.0, 297.0), "the paper");
        let flat = &page(PlotRotation::CounterClockwise90);
        assert_eq!((flat.width, flat.height), (297.0, 210.0), "the page");
        // The line reads left to right, level, and the drawing fills the page.
        let [a, b] = [flat.paths[0].points[0], flat.paths[0].points[1]];
        assert!((a.y - b.y).abs() < 1e-9 && b.x > a.x, "{a:?} {b:?}");
        assert!(a.x.abs() < 1e-6 && (b.x - 297.0).abs() < 1e-6);
        // The upright line goes up the page.
        let [c, d] = [flat.paths[1].points[0], flat.paths[1].points[1]];
        assert!((c.x - d.x).abs() < 1e-9 && d.y > c.y, "{c:?} {d:?}");
    }

    #[test]
    fn landscape_upside_down_stays_upside_down_and_other_plots_are_as_they_were() {
        let flipped = page(PlotRotation::Clockwise90);
        assert_eq!((flipped.width, flipped.height), (297.0, 210.0));
        let [a, b] = [flipped.paths[0].points[0], flipped.paths[0].points[1]];
        assert!((a.y - b.y).abs() < 1e-9 && b.x < a.x, "read right to left");
        for rotation in [PlotRotation::Degrees0, PlotRotation::Degrees180] {
            let doc = drawing(rotation, [0.0; 4]);
            assert_eq!(
                plot_page(&doc, PlotSpace::Model),
                plot_sheet(&doc, PlotSpace::Model)
            );
        }
        assert_eq!(
            plot_page(&Document::default(), PlotSpace::Layout(crate::LayoutId(9))),
            Err(PlotError::NoLayout)
        );
    }

    #[test]
    fn the_margins_turn_with_the_page_and_the_plot_stays_inside_them() {
        let margins = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(
            page_as_read(210.0, 297.0, margins, PlotRotation::Degrees0),
            (210.0, 297.0, margins)
        );
        let (w, h, [left, bottom, right, top]) =
            page_as_read(210.0, 297.0, margins, PlotRotation::CounterClockwise90);
        assert_eq!(
            (w, h, [left, bottom, right, top]),
            (297.0, 210.0, [2.0, 3.0, 4.0, 1.0])
        );
        let doc = drawing(PlotRotation::CounterClockwise90, margins);
        let sheet = plot_page(&doc, PlotSpace::Model).unwrap();
        let points = sheet.paths.iter().flat_map(|p| p.points.iter());
        let near = |v: f64, edge: f64| (v - edge).abs() < 1e-6;
        let (mut touches_left, mut touches_right) = (false, false);
        for p in points {
            assert!(p.x >= left - 1e-6 && p.x <= w - right + 1e-6, "{p:?}");
            assert!(p.y >= bottom - 1e-6 && p.y <= h - top + 1e-6, "{p:?}");
            touches_left |= near(p.x, left);
            touches_right |= near(p.x, w - right);
        }
        assert!(touches_left && touches_right, "fitted across the page");
    }

    /// A landscape A4 layout with no viewport, 5 mm margins and one line on
    /// its sheet from (100, 100) to (200, 150).
    fn sheet_with_a_line(area: crate::PlotArea) -> (Document, LayoutId) {
        let mut doc = Document::default();
        let mut layout = doc.new_layout("A4", 297.0, 210.0);
        layout.viewports.clear();
        layout.plot.margins_mm = [5.0; 4];
        layout.plot.area = area;
        layout.plot.flags |= crate::plot_settings::flags::PLOT_CENTERED;
        let id = layout.id;
        doc.set_layouts(vec![layout]);
        doc.set_active_layout(Some(id));
        doc.add(Entity::Line {
            start: Point::new(100.0, 100.0),
            end: Point::new(200.0, 150.0),
        });
        (doc, id)
    }

    fn near_point(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
    }

    #[test]
    fn a_layouts_origin_is_the_printable_corner_so_its_paper_starts_below_it() {
        let (doc, id) = sheet_with_a_line(crate::PlotArea::Layout);
        let sheet = plot_sheet(&doc, PlotSpace::Layout(id)).unwrap();
        assert!(near_point(
            sheet.paths[0].points[0],
            Point::new(105.0, 105.0)
        ));
        let paper = paper_on_sheet(&doc, id).unwrap();
        assert!(near_point(paper.min, Point::new(-5.0, -5.0)), "{paper:?}");
        assert!(near_point(paper.max, Point::new(292.0, 205.0)), "{paper:?}");
    }

    #[test]
    fn the_paper_lies_on_the_sheet_where_the_plot_puts_the_drawing_on_it() {
        // Extents, centred: the line's 100 × 50 box goes in the middle of the
        // printable area, which is the middle of the paper here.
        let (mut doc, id) = sheet_with_a_line(crate::PlotArea::Extents);
        let paper = paper_on_sheet(&doc, id).unwrap();
        assert!(near_point(paper.min, Point::new(1.5, 20.0)), "{paper:?}");
        assert!(near_point(paper.max, Point::new(298.5, 230.0)), "{paper:?}");
        let sheet = plot_sheet(&doc, PlotSpace::Layout(id)).unwrap();
        let start = sheet.paths[0].points[0];
        assert!(near_point(start, Point::new(100.0 - 1.5, 100.0 - 20.0)));

        // Not centred, offset (2, 3) from the printable corner.
        let mut layouts = doc.layouts().to_vec();
        layouts[0].plot.flags &= !crate::plot_settings::flags::PLOT_CENTERED;
        layouts[0].plot.origin_mm = Point::new(2.0, 3.0);
        doc.set_layouts(layouts);
        let paper = paper_on_sheet(&doc, id).unwrap();
        assert!(near_point(paper.min, Point::new(93.0, 92.0)), "{paper:?}");

        // At 1:2 the paper covers twice as much of the sheet.
        let mut layouts = doc.layouts().to_vec();
        layouts[0].plot.scale = crate::PlotScale::Custom {
            paper_units: 1.0,
            drawing_units: 2.0,
        };
        doc.set_layouts(layouts);
        let paper = paper_on_sheet(&doc, id).unwrap();
        assert!(
            (paper.max.x - paper.min.x - 594.0).abs() < 1e-6,
            "{paper:?}"
        );
    }

    #[test]
    fn a_turned_paper_lies_on_the_sheet_the_way_it_is_read() {
        let (mut doc, id) = sheet_with_a_line(crate::PlotArea::Extents);
        let mut layouts = doc.layouts().to_vec();
        let layout = &mut layouts[0];
        (layout.width, layout.height) = (210.0, 297.0);
        (layout.plot.paper_width_mm, layout.plot.paper_height_mm) = (210.0, 297.0);
        layout.plot.rotation = PlotRotation::CounterClockwise90;
        doc.set_layouts(layouts);
        let paper = paper_on_sheet(&doc, id).unwrap();
        assert!(near_point(paper.min, Point::new(1.5, 20.0)), "{paper:?}");
        assert!(near_point(paper.max, Point::new(298.5, 230.0)), "{paper:?}");
    }
}
