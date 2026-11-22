//! Where the paper of a layout lies on its sheet.
//!
//! The canvas shows the paper where the plot will put the drawing on it:
//! centred, offset or scaled as the page setup says, so the screen, the
//! preview and the plotted file agree. With the Extents area this plots the
//! layout's objects to find them, so the answer is kept until the drawing
//! changes (`Session::draw_token`).

use opendraft_core::{plot::paper_on_sheet, Bounds, Document, Layout, LayoutId};

use super::View;

/// The paper found for one layout of the drawing as it was.
pub(super) struct KeptPaper {
    token: (u64, u64),
    layout: LayoutId,
    paper: Bounds,
}

impl View {
    /// Where `layout`'s paper lies on its sheet, in sheet millimetres; the
    /// sheet as it is read when its page setup cannot plot.
    pub(super) fn paper_bounds(&self, document: &Document, layout: &Layout) -> Bounds {
        if let Some(kept) = self.paper.borrow().as_ref() {
            if Some(kept.token) == self.document_token && kept.layout == layout.id {
                return kept.paper;
            }
        }
        let paper = paper_on_sheet(document, layout.id).unwrap_or_else(|_| layout.sheet_bounds());
        *self.paper.borrow_mut() = self.document_token.map(|token| KeptPaper {
            token,
            layout: layout.id,
            paper,
        });
        paper
    }
}
