//! The sheet as a page to read.
//!
//! A plot turned a quarter on its paper (how AutoCAD keeps a landscape plot
//! on paper that feeds portrait) is drawn on the paper the way it goes
//! through a printer, so the drawing lies on its side. A file or a picture
//! has no printer to feed: its page is turned a quarter too, so it opens the
//! way it is read.

use super::{plot_sheet, PlotError, PlotSheet, PlotSpace};
use crate::{Document, PlotRotation, Point};

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
}
