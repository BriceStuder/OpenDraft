//! Custom coordinate systems in DXF, where AutoCAD calls them UCSs: the
//! header's `$UCSNAME`, `$UCSORG`, `$UCSXDIR` and `$UCSYDIR` (the Model tab's
//! system), the UCS table (the systems saved by name) and each VIEWPORT's
//! own system (groups 110, 111 and 112, written by the R2000 viewports).
//!
//! A file's system is three points: an origin and the X and Y directions.
//! Only a flat, right-handed one (Y a quarter turn counter-clockwise from X,
//! both in the XY plane) is a system here; a tilted or mirrored one is left
//! out and counted, so the reader can say so.

use opendraft_core::{Ccs, CcsSpace, Document, NamedCcs};

use super::write::{dxf_name, Out};

/// The system an origin and X and Y directions describe, if it is flat and
/// right-handed (see [`Ccs::from_axes`]).
pub(super) fn from_axes(origin: [f64; 3], x: [f64; 3], y: [f64; 3]) -> Option<Ccs> {
    Ccs::from_axes(origin, x, y)
}

/// The name the Model tab's system goes by: a saved name it matches, or none.
fn name_of(document: &Document, ccs: Ccs) -> String {
    let same = |n: &&NamedCcs| {
        (n.ccs.origin.x - ccs.origin.x).abs() < 1e-9
            && (n.ccs.origin.y - ccs.origin.y).abs() < 1e-9
            && (n.ccs.angle - ccs.angle).abs() < 1e-12
    };
    let named = document.named_ccs().iter().find(same);
    named.map(|n| dxf_name(&n.name)).unwrap_or_default()
}

/// The origin and X and Y directions of `ccs`, as the groups `base`,
/// `base + 1` and `base + 2` (each with its 20s and 30s).
pub(super) fn write_axes(out: &mut Out, base: i32, ccs: Ccs) {
    let (x, y) = (ccs.x_axis(), ccs.y_axis());
    out.point3(base, ccs.origin.x, ccs.origin.y, 0.0);
    out.point3(base + 1, x.x, x.y, 0.0);
    out.point3(base + 2, y.x, y.y, 0.0);
}

/// The header variables of the Model tab's system.
pub(super) fn write_header(out: &mut Out, document: &Document) {
    let ccs = document.ccs_of(CcsSpace::Model).unwrap_or(Ccs::WORLD);
    let (x, y) = (ccs.x_axis(), ccs.y_axis());
    out.pair(9, "$UCSNAME");
    out.pair(2, name_of(document, ccs));
    for (name, p) in [("$UCSORG", ccs.origin), ("$UCSXDIR", x), ("$UCSYDIR", y)] {
        out.pair(9, name);
        out.point3(10, p.x, p.y, 0.0);
    }
}

/// The body of one UCS table record, after its start (`0 UCS` and, in
/// R2000, its handle, owner and subclasses). `r2000` adds the groups R2000
/// has and R12 does not.
pub(super) fn write_record(out: &mut Out, named: &NamedCcs, r2000: bool) {
    out.pair(2, dxf_name(&named.name));
    out.pair(70, 0);
    write_axes(out, 10, named.ccs);
    if r2000 {
        out.pair(79, 0);
        out.pair(146, 0.0);
    }
}

/// R12's UCS table, when there are systems saved by name.
pub(super) fn write_r12_table(out: &mut Out, document: &Document) {
    let named = document.named_ccs();
    if named.is_empty() {
        return;
    }
    out.pair(0, "TABLE");
    out.pair(2, "UCS");
    out.pair(70, named.len());
    for n in named {
        out.pair(0, "UCS");
        write_record(out, n, false);
    }
    out.pair(0, "ENDTAB");
}

/// Set the Model tab's system from a header's groups (code, value). Returns
/// whether the header had a system that could not be read as one (tilted or
/// mirrored); a missing one leaves the world.
pub(super) fn apply_header<'a>(
    groups: impl Iterator<Item = (i32, &'a str)>,
    document: &mut Document,
) -> bool {
    let (mut origin, mut x, mut y) = ([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let mut seen = false;
    let mut variable = "";
    for (code, value) in groups {
        if code == 9 {
            variable = value;
            continue;
        }
        let slot = match variable {
            "$UCSORG" => &mut origin,
            "$UCSXDIR" => &mut x,
            "$UCSYDIR" => &mut y,
            _ => continue,
        };
        let axis = match code {
            10 => 0,
            20 => 1,
            30 => 2,
            _ => continue,
        };
        if let Ok(v) = value.trim().parse::<f64>() {
            slot[axis] = v;
            seen = true;
        }
    }
    if !seen {
        return false;
    }
    match from_axes(origin, x, y) {
        Some(ccs) => {
            document.set_ccs_of(CcsSpace::Model, ccs);
            false
        }
        None => true,
    }
}

/// What a reader says when it left tilted or mirrored systems out.
pub(super) fn warning(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 coordinate system (UCS) was 3D or mirrored and was read as World.".into()),
        n => Some(format!(
            "{n} coordinate systems (UCSs) were 3D or mirrored and were read as World."
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_6;

    use opendraft_core::Point;

    use super::*;

    fn turned() -> Ccs {
        Ccs::new(Point::new(10.0, -5.0), FRAC_PI_6)
    }

    #[test]
    fn the_header_round_trips() {
        let mut document = Document::default();
        document.set_ccs_of(CcsSpace::Model, turned());
        let mut out = Out::new();
        write_header(&mut out, &document);
        let lines: Vec<&str> = out.text.lines().map(str::trim).collect();
        let groups: Vec<(i32, &str)> = lines
            .chunks(2)
            .map(|p| (p[0].parse().unwrap_or(-1), p[1]))
            .collect();
        let mut back = Document::default();
        assert!(!apply_header(groups.into_iter(), &mut back));
        let c = back.ccs();
        assert!((c.origin.x - 10.0).abs() < 1e-9 && (c.angle - FRAC_PI_6).abs() < 1e-9);
    }

    #[test]
    fn a_missing_header_is_world_and_a_tilted_one_is_reported() {
        let mut doc = Document::default();
        assert!(!apply_header(std::iter::empty(), &mut doc));
        assert!(doc.ccs().is_world());
        let tilted = [
            (9, "$UCSXDIR"),
            (10, "1"),
            (20, "0"),
            (30, "0"),
            (9, "$UCSYDIR"),
            (10, "0"),
            (20, "0"),
            (30, "1"),
        ];
        assert!(apply_header(tilted.into_iter(), &mut doc));
        assert!(doc.ccs().is_world());
        assert!(warning(1).is_some_and(|w| w.contains("read as World")));
    }

    fn round_trip(document: &Document) -> Document {
        use crate::FileFormat;
        let bytes = crate::DxfFormat.write(document).unwrap().bytes;
        crate::DxfFormat.read(&bytes).unwrap().document
    }

    fn named() -> Vec<NamedCcs> {
        vec![NamedCcs {
            name: "Wing A".into(),
            ccs: turned(),
        }]
    }

    fn close(a: Option<Ccs>, b: Ccs) -> bool {
        a.is_some_and(|a| {
            (a.origin.x - b.origin.x).abs() < 1e-9
                && (a.origin.y - b.origin.y).abs() < 1e-9
                && (a.angle - b.angle).abs() < 1e-9
        })
    }

    #[test]
    fn r12_keeps_the_model_system_and_the_saved_ones() {
        let mut document = Document::default();
        document.set_ccs_of(CcsSpace::Model, turned());
        document.set_named_ccs(named());
        let back = round_trip(&document);
        assert!(close(back.ccs_of(CcsSpace::Model), turned()));
        assert_eq!(back.named_ccs().len(), 1);
        assert_eq!(back.named_ccs()[0].name, "Wing A");
        assert!(close(Some(back.named_ccs()[0].ccs), turned()));
    }

    #[test]
    fn r2000_keeps_a_viewports_own_system_too() {
        let mut document = Document::default();
        let mut layout = document.new_layout("Sheet", 297.0, 210.0);
        layout.viewports[0].ccs = turned();
        document.set_layouts(vec![layout]);
        document.set_named_ccs(named());
        let back = round_trip(&document);
        let layout = &back.layouts()[0];
        let space = CcsSpace::Viewport(layout.id, layout.viewports[0].id);
        assert!(close(back.ccs_of(space), turned()));
        assert!(back.ccs_of(CcsSpace::Model).is_some_and(|c| c.is_world()));
        assert_eq!(back.named_ccs().len(), 1);
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
        let back = round_trip(&document);
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
