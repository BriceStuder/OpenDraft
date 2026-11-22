//! Reading user coordinate systems (custom coordinate systems) from a DXF
//! written by an independent library (ezdxf, AutoCAD 2018 format). See
//! `fixtures/make_ezdxf_ucs.py` for what is in it.

use std::path::Path;

use opendraft_core::{Ccs, CcsSpace};

fn sample() -> opendraft_files::Loaded {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ezdxf_ucs.dxf");
    opendraft_files::load(&path).expect("the sample reads")
}

fn close(found: Option<Ccs>, origin: (f64, f64), degrees: f64) -> bool {
    found.is_some_and(|c| {
        (c.origin.x - origin.0).abs() < 1e-9
            && (c.origin.y - origin.1).abs() < 1e-9
            && (c.angle - degrees.to_radians()).abs() < 1e-9
    })
}

#[test]
fn the_header_sets_the_model_tabs_system() {
    let document = sample().document;
    assert!(close(document.ccs_of(CcsSpace::Model), (10.0, -5.0), 30.0));
}

#[test]
fn the_ucs_table_gives_the_saved_systems_and_a_tilted_one_is_reported() {
    let loaded = sample();
    let named = loaded.document.named_ccs();
    assert_eq!(named.len(), 1, "Tilted is 3D and left out");
    assert_eq!(named[0].name, "Wing A");
    assert!(close(Some(named[0].ccs), (100.0, 50.0), 90.0));
    assert!(
        loaded.warnings.iter().any(|w| w.contains("read as World")),
        "{:?}",
        loaded.warnings
    );
}

#[test]
fn a_viewport_keeps_its_own_system() {
    let document = sample().document;
    let layout = document
        .layouts()
        .iter()
        .find(|l| l.name == "Layout1")
        .expect("Layout1");
    let vp = layout
        .viewports
        .iter()
        .find(|v| (v.center.x - 100.0).abs() < 50.0)
        .expect("the viewport");
    let space = CcsSpace::Viewport(layout.id, vp.id);
    assert!(close(document.ccs_of(space), (2.0, 3.0), 45.0));
}
