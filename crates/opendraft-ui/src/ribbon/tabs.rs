//! The tab strip above the ribbon body, as in AutoCAD: the active tab takes
//! the body's colour so it merges into the body below it.
//!
//! A contextual tab (one there only because of where the user is, such as the
//! Block Editor's or a layout's) is marked with the accent: its label in
//! `accent_text` and a thin line of it along the tab's top. The chrome is
//! otherwise kept to neutral tones; a tab that has just appeared is one of the
//! few things worth drawing the eye to.

use egui::{vec2, Align2, Color32, CornerRadius, FontId, Frame, Margin, Rect, Sense, Ui};

use super::buttons::text_width;
use super::Tab;
use crate::theme::Colors;
use crate::Theme;

const TAB_H: f32 = 25.0;
/// Thickness of the accent line along a contextual tab's top.
const CONTEXT_LINE: f32 = 2.0;
const TOP_ROUNDED: CornerRadius = CornerRadius {
    nw: 4,
    ne: 4,
    sw: 0,
    se: 0,
};

/// How one tab is painted.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TabLook {
    fill: Option<Color32>,
    label: Color32,
    /// The line along the top of a contextual tab.
    line: Option<Color32>,
}

impl TabLook {
    fn of(c: &Colors, selected: bool, hovered: bool, contextual: bool) -> Self {
        let fill = if selected {
            Some(c.ribbon_bg)
        } else if hovered {
            Some(c.surface)
        } else {
            None
        };
        let label = if contextual {
            c.accent_text
        } else if selected || hovered {
            c.text
        } else {
            c.text_weak
        };
        Self {
            fill,
            label,
            line: contextual.then_some(c.accent_text),
        }
    }
}

/// Draw the tabs and make the clicked one `selected`.
pub(super) fn show(ui: &mut Ui, theme: &Theme, selected: &mut usize, tabs: &[Tab]) {
    let c = &theme.colors;
    Frame::new()
        .fill(c.tab_strip())
        .inner_margin(Margin {
            left: 8,
            right: 8,
            top: 3,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, tab) in tabs.iter().enumerate() {
                    let width = text_width(ui, tab.title, theme.font_size_tab) + 24.0;
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(width, TAB_H), Sense::click());
                    let look = TabLook::of(c, *selected == i, response.hovered(), tab.contextual);
                    let painter = ui.painter();
                    if let Some(fill) = look.fill {
                        painter.rect_filled(rect, TOP_ROUNDED, fill);
                    }
                    if let Some(line) = look.line {
                        let top = Rect::from_min_size(rect.min, vec2(rect.width(), CONTEXT_LINE));
                        painter.rect_filled(top, TOP_ROUNDED, line);
                    }
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        tab.title,
                        FontId::proportional(theme.font_size_tab),
                        look.label,
                    );
                    crate::widgets::focus_ring(ui, theme, &response, rect);
                    if response.clicked() {
                        *selected = i;
                    }
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme_contrast_tests::contrast;

    fn palettes() -> [(&'static str, Colors); 2] {
        [("neutral", Colors::neutral()), ("slate", Colors::slate())]
    }

    #[test]
    fn a_usual_tab_stays_neutral() {
        let c = Colors::neutral();
        let idle = TabLook::of(&c, false, false, false);
        assert_eq!(
            (idle.fill, idle.label, idle.line),
            (None, c.text_weak, None)
        );
        let active = TabLook::of(&c, true, false, false);
        assert_eq!((active.fill, active.label), (Some(c.ribbon_bg), c.text));
        assert_eq!(TabLook::of(&c, false, true, false).fill, Some(c.surface));
    }

    #[test]
    fn a_contextual_tab_carries_the_accent_however_it_is_shown() {
        let c = Colors::neutral();
        for (selected, hovered) in [(false, false), (false, true), (true, false)] {
            let look = TabLook::of(&c, selected, hovered, true);
            assert_eq!(look.label, c.accent_text);
            assert_eq!(look.line, Some(c.accent_text));
            // Its fill is the usual one, so a selected tab still merges
            // into the ribbon body.
            assert_eq!(look.fill, TabLook::of(&c, selected, hovered, false).fill);
        }
    }

    #[test]
    fn a_contextual_label_is_readable_on_every_fill_it_sits_on() {
        for (name, c) in palettes() {
            for (selected, hovered) in [(false, false), (false, true), (true, false)] {
                let look = TabLook::of(&c, selected, hovered, true);
                let behind = look.fill.unwrap_or(c.tab_strip());
                let ratio = contrast(look.label, behind);
                assert!(
                    ratio >= 4.5,
                    "{name}: selected {selected}, hovered {hovered}: {ratio:.2}"
                );
            }
        }
    }
}
