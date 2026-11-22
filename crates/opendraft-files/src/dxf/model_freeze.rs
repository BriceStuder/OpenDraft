//! OpenDraft's Model-tab freeze in DXF. AutoCAD has no such switch, so a
//! layer frozen in the Model tab carries it as XDATA under OpenDraft's own
//! registered application: `1001 OPENDRAFT`, `1000 FrozenInModel`,
//! `1070 1`. AutoCAD ignores the data (the layer shows everywhere there) and
//! keeps it when it saves. Only R2000 has the APPID table that registers
//! the name, so such a drawing is written as R2000.

use opendraft_core::Layer;

use crate::dxf::write::Out;

/// The registered application name.
pub(super) const APP: &str = "OPENDRAFT";

/// The marker naming the switch inside the application's data.
pub(super) const FROZEN_IN_MODEL: &str = "FrozenInModel";

/// Whether any layer is frozen in the Model tab (and so the APPID is needed).
pub(super) fn any(layers: &[Layer]) -> bool {
    layers.iter().any(|l| l.frozen_in_model)
}

/// The layer's XDATA, last in its LAYER record; nothing when not frozen in
/// the Model tab.
pub(super) fn write(out: &mut Out, layer: &Layer) {
    if layer.frozen_in_model {
        out.pair(1001, APP);
        out.pair(1000, FROZEN_IN_MODEL);
        out.pair(1070, 1);
    }
}

#[cfg(test)]
mod tests {
    use opendraft_core::{Attributes, Document, Entity, Point};

    use super::*;
    use crate::dxf::{read::read, write::write};

    /// Two layers with a line each; "Hatching" frozen in the Model tab when
    /// `frozen`.
    fn drawing(frozen: bool) -> Document {
        let mut doc = Document::default();
        for (name, frozen_in_model) in [("Walls", false), ("Hatching", frozen)] {
            let id = doc.allocate_layer_id();
            doc.push_layer(Layer {
                frozen_in_model,
                ..Layer::new(id, name)
            });
            let line = Entity::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
            };
            doc.add_with(line, Attributes::on_layer(id));
        }
        doc
    }

    fn text(doc: &Document) -> String {
        String::from_utf8_lossy(&write(doc).unwrap().bytes).replace("\r\n", "\n")
    }

    #[test]
    fn a_layer_frozen_in_the_model_tab_comes_back_from_dxf() {
        let text = text(&drawing(true));
        assert!(text.contains("AC1015"), "written as R2000");
        assert!(text.contains("AcDbRegAppTableRecord\n  2\nOPENDRAFT"));
        assert_eq!(text.matches("1001\nOPENDRAFT").count(), 1, "one layer");
        let doc = read(text.as_bytes()).unwrap().document;
        let flag = |name: &str| {
            let layers = doc.layers().iter();
            layers.filter(|l| l.name == name).all(|l| l.frozen_in_model)
        };
        assert!(flag("Hatching") && !flag("Walls"));
    }

    #[test]
    fn nothing_is_written_when_no_layer_is_frozen_in_the_model_tab() {
        let text = text(&drawing(false));
        assert!(!text.contains("OPENDRAFT"));
        assert!(text.contains("AC1009"), "still R12");
    }
}
