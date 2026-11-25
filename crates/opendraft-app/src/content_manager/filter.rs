//! The Filter menu: narrow the blocks shown by unit, favourites, dynamic and
//! attributes (specification 10.4). The search itself is `Library::search`.

use eframe::egui;
use opendraft_content::library::Query;
use opendraft_core::InsertUnits;

/// A yes/no question that may be left unasked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tri {
    #[default]
    Any,
    Yes,
    No,
}

impl Tri {
    fn answer(self) -> Option<bool> {
        match self {
            Tri::Any => None,
            Tri::Yes => Some(true),
            Tri::No => Some(false),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub favourites: bool,
    pub unit: Option<InsertUnits>,
    pub dynamic: Tri,
    pub attributes: Tri,
}

impl Filter {
    /// Whether anything is narrowed.
    pub fn active(&self) -> bool {
        *self != Filter::default()
    }

    /// The search with these filters and these words.
    pub fn query(&self, text: &str) -> Query {
        Query {
            text: text.to_owned(),
            unit: self.unit,
            favourites_only: self.favourites,
            dynamic: self.dynamic.answer(),
            has_attributes: self.attributes.answer(),
        }
    }

    /// The menu. `units` are the units the library's blocks are in.
    pub fn menu(&mut self, ui: &mut egui::Ui, units: &[InsertUnits]) {
        let label = if self.active() {
            "Filter \u{2022}"
        } else {
            "Filter"
        };
        ui.menu_button(label, |ui| {
            ui.checkbox(&mut self.favourites, "Favourites only");
            tri(ui, "Dynamic", &mut self.dynamic);
            tri(ui, "Has attributes", &mut self.attributes);
            ui.horizontal(|ui| {
                ui.label("Unit");
                let shown = self.unit.map_or("Any", InsertUnits::label);
                egui::ComboBox::from_id_salt("cm_filter_unit")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.unit, None, "Any");
                        for unit in units {
                            ui.selectable_value(&mut self.unit, Some(*unit), unit.label());
                        }
                    });
            });
            ui.separator();
            if ui
                .add_enabled(self.active(), egui::Button::new("Clear filters"))
                .clicked()
            {
                *self = Filter::default();
            }
        });
    }
}

fn tri(ui: &mut egui::Ui, label: &str, value: &mut Tri) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.selectable_value(value, Tri::Any, "Any");
        ui.selectable_value(value, Tri::Yes, "Yes");
        ui.selectable_value(value, Tri::No, "No");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_filtered_until_something_is_chosen() {
        let mut f = Filter::default();
        assert!(!f.active());
        f.dynamic = Tri::No;
        assert!(f.active());
        let q = f.query("valve");
        assert_eq!(
            (q.dynamic, q.has_attributes, q.favourites_only),
            (Some(false), None, false)
        );
        assert_eq!(q.text, "valve");
    }
}
