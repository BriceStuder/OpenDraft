//! Custom coordinate systems: the Model tab's and the ones saved by name
//! (each viewport's is in its own record, `layout.rs`). Left out of the
//! file when they are the world's and there are none saved.

use std::collections::HashSet;

use opendraft_core::ccs::name_refusal;
use opendraft_core::{Ccs, CcsSpace, Document, NamedCcs, Point};
use serde::{Deserialize, Serialize};

use super::entity::invalid;
use super::limits;
use crate::FileError;

/// One system: its origin, and its X axis in radians counter-clockwise from
/// world east.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub(super) struct CcsDto {
    origin: [f64; 2],
    angle: f64,
}

impl CcsDto {
    pub(super) fn of(ccs: Ccs) -> Self {
        Self {
            origin: [ccs.origin.x, ccs.origin.y],
            angle: ccs.angle,
        }
    }

    /// The system, or the file refused: a number that is not finite is
    /// damage, not a system.
    pub(super) fn read(self) -> Result<Ccs, FileError> {
        let [x, y] = self.origin;
        if ![x, y, self.angle].iter().all(|v| v.is_finite()) {
            return invalid("a coordinate system has a number that is not finite");
        }
        Ok(Ccs::new(Point::new(x, y), self.angle))
    }

    /// The field of a viewport or the Model tab: none for the world's.
    pub(super) fn unless_world(ccs: Ccs) -> Option<Self> {
        (!ccs.is_world()).then(|| Self::of(ccs))
    }
}

#[derive(Serialize, Deserialize)]
struct NamedCcsDto {
    name: String,
    #[serde(flatten)]
    ccs: CcsDto,
}

/// The document's own fields.
#[derive(Default, Serialize, Deserialize)]
pub(super) struct TableDto {
    /// The Model tab's system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_ccs: Option<CcsDto>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "limits::table_rows"
    )]
    named_ccs: Vec<NamedCcsDto>,
}

impl TableDto {
    pub(super) fn of(document: &Document) -> Self {
        Self {
            model_ccs: document
                .ccs_of(CcsSpace::Model)
                .and_then(CcsDto::unless_world),
            named_ccs: document
                .named_ccs()
                .iter()
                .map(|n| NamedCcsDto {
                    name: n.name.clone(),
                    ccs: CcsDto::of(n.ccs),
                })
                .collect(),
        }
    }

    pub(super) fn load(self, document: &mut Document) -> Result<(), FileError> {
        if let Some(model) = self.model_ccs {
            document.set_ccs_of(CcsSpace::Model, model.read()?);
        }
        let mut names = HashSet::new();
        let mut named = Vec::new();
        for n in self.named_ccs {
            if let Some(why) = name_refusal(&n.name) {
                return invalid(format!("coordinate system \"{}\": {why}", n.name));
            }
            if !names.insert(n.name.to_lowercase()) {
                return invalid(format!("coordinate system \"{}\" appears twice", n.name));
            }
            named.push(NamedCcs {
                name: n.name,
                ccs: n.ccs.read()?,
            });
        }
        document.set_named_ccs(named);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::tests::roundtrip;
    use crate::native::NativeFormat;
    use crate::FileFormat;

    fn turned() -> Ccs {
        Ccs::new(Point::new(10.0, -2.5), 0.75)
    }

    #[test]
    fn world_systems_are_not_written() {
        let text =
            String::from_utf8(NativeFormat.write(&Document::default()).unwrap().bytes).unwrap();
        assert!(!text.contains("ccs"));
    }

    #[test]
    fn model_and_named_systems_round_trip() {
        let mut document = Document::default();
        document.set_ccs_of(CcsSpace::Model, turned());
        document.set_named_ccs(vec![NamedCcs {
            name: "Wing A".into(),
            ccs: turned(),
        }]);
        let loaded = roundtrip(&document);
        assert_eq!(loaded.ccs_of(CcsSpace::Model), Some(turned()));
        assert_eq!(loaded.named_ccs(), document.named_ccs());
    }

    #[test]
    fn a_viewports_own_system_round_trips() {
        let mut document = Document::default();
        let mut layout = document.new_layout("Sheet", 297.0, 210.0);
        layout.viewports[0].ccs = turned();
        let space = CcsSpace::Viewport(layout.id, layout.viewports[0].id);
        document.set_layouts(vec![layout]);
        let loaded = roundtrip(&document);
        assert_eq!(loaded.ccs_of(space), Some(turned()));
        assert!(loaded.ccs_of(CcsSpace::Model).is_some_and(|c| c.is_world()));
    }

    #[test]
    fn numbers_that_are_not_finite_are_refused() {
        let dto = CcsDto {
            origin: [0.0, f64::NAN],
            angle: 0.0,
        };
        assert!(dto.read().is_err());
    }

    #[test]
    fn bad_names_are_refused() {
        let bad = |table: serde_json::Value| {
            let dto: TableDto = serde_json::from_value(table).unwrap();
            dto.load(&mut Document::default()).is_err()
        };
        let one =
            |name: &str| serde_json::json!({"name": name, "origin": [0.0, 0.0], "angle": 0.0});
        assert!(bad(serde_json::json!({"named_ccs": [one("A"), one("a")]})));
        assert!(bad(serde_json::json!({"named_ccs": [one("a/b")]})));
        assert!(!bad(serde_json::json!({"named_ccs": [one("A")]})));
    }

    #[test]
    fn an_ordinate_dimension_keeps_the_system_it_measures_in() {
        let mut document = Document::default();
        let style = document.dim_style().clone();
        let frame = turned();
        let (feature, end) = (
            frame.to_world(Point::new(4.0, 7.0)),
            frame.to_world(Point::new(4.0, 20.0)),
        );
        let d = opendraft_core::Dimension::ordinate_in(frame, feature, end, true, &style);
        document.add(opendraft_core::Entity::Dimension(Box::new(d)));
        let back = roundtrip(&document);
        let found = back.objects().iter().find_map(|o| match &o.entity {
            opendraft_core::Entity::Dimension(d) => Some(d.clone()),
            _ => None,
        });
        let d = found.expect("the dimension");
        assert!((d.measurement().unwrap_or_default() - 4.0).abs() < 1e-6);
        let opendraft_core::DimKind::Ordinate { frame: kept, .. } = d.kind else {
            panic!("an ordinate dimension");
        };
        assert!((kept.angle - frame.angle).abs() < 1e-9);
    }
}
