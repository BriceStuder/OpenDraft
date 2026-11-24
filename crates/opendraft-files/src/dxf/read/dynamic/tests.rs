//! A small file shaped like AutoCAD's: one definition `Slide` with a linear
//! parameter and its value list, a grip, a stretch, a lookup parameter and
//! table, and one modified instance `*U2`.

use super::super::{Group, Record};
use super::*;

fn rec(name: &str, groups: &[(i32, &str)]) -> Record {
    Record {
        name: name.to_owned(),
        groups: groups
            .iter()
            .map(|&(code, value)| Group {
                code,
                value: value.to_owned(),
            })
            .collect(),
    }
}

fn node(index: &str, id: &str, handle: &str) -> Vec<(i32, String)> {
    let text = |s: &str| s.to_owned();
    vec![
        (91, text(index)),
        (93, text("32")),
        (95, text(id)),
        (360, text(handle)),
        (92, text("0")),
        (92, text("0")),
        (92, text("-1")),
        (92, text("-1")),
    ]
}

fn edge(number: &str, from: &str, to: &str) -> Vec<(i32, String)> {
    let text = |s: &str| s.to_owned();
    let mut groups = vec![
        (92, text(number)),
        (93, text("0")),
        (94, text("1")),
        (91, text(from)),
        (91, text(to)),
    ];
    groups.extend((0..5).map(|_| (92, text("-1"))));
    groups
}

/// The records of the file, in section order.
pub(in crate::dxf::read) fn file() -> Vec<Record> {
    let mut graph = vec![
        (5, "G1".to_owned()),
        (100, "AcDbEvalGraph".to_owned()),
        (96, "16".to_owned()),
        (97, "16".to_owned()),
    ];
    for (index, id, handle) in [
        ("0", "6", "P1"),
        ("1", "7", "GR"),
        ("2", "8", "LC"),
        ("3", "10", "ST"),
        ("4", "12", "LP"),
        ("5", "16", "LA"),
    ] {
        graph.extend(node(index, id, handle));
    }
    graph.extend(edge("0", "0", "3"));
    graph.extend(edge("1", "1", "0"));
    graph.extend(edge("2", "4", "5"));
    let graph: Vec<(i32, &str)> = graph.iter().map(|(c, v)| (*c, v.as_str())).collect();
    vec![
        rec("SECTION", &[(2, "TABLES")]),
        rec("TABLE", &[(2, "BLOCK_RECORD")]),
        rec(
            "BLOCK_RECORD",
            &[
                (5, "A1"),
                (2, "Slide"),
                (102, "{ACAD_XDICTIONARY"),
                (360, "D1"),
                (102, "}"),
                (1001, "AcDbDynamicBlockTrueName"),
                (1000, "Slide"),
                (1001, "AcDbDynamicBlockGUID"),
                (1000, "{GUID}"),
            ],
        ),
        rec(
            "BLOCK_RECORD",
            &[(5, "A2"), (2, "*U2"), (1001, "AcDbBlockRepBTag")],
        ),
        rec("ENDTAB", &[]),
        rec("ENDSEC", &[]),
        rec("SECTION", &[(2, "BLOCKS")]),
        rec("BLOCK", &[(5, "B1"), (2, "Slide")]),
        rec("LINE", &[(5, "E1"), (8, "0")]),
        rec("ENDBLK", &[(5, "B2")]),
        rec("BLOCK", &[(5, "B3"), (2, "Other")]),
        rec("LINE", &[(5, "E2"), (8, "0")]),
        rec("ENDBLK", &[(5, "B4")]),
        rec("ENDSEC", &[]),
        rec("SECTION", &[(2, "ENTITIES")]),
        rec(
            "INSERT",
            &[
                (5, "I1"),
                (102, "{ACAD_XDICTIONARY"),
                (360, "X1"),
                (102, "}"),
                (2, "*U2"),
            ],
        ),
        rec("ENDSEC", &[]),
        rec("SECTION", &[(2, "OBJECTS")]),
        rec(
            "DICTIONARY",
            &[(5, "D1"), (3, "ACAD_ENHANCEDBLOCK"), (360, "G1")],
        ),
        rec("ACAD_EVALUATION_GRAPH", &graph),
        rec(
            "BLOCKLINEARPARAMETER",
            &[
                (5, "P1"),
                (90, "6"),
                (300, "Linear"),
                (1010, "0"),
                (1020, "0"),
                (1011, "5"),
                (1021, "0"),
                (305, "Distance1"),
                (175, "4"),
                (144, "2.5"),
                (144, "5"),
                (144, "7.5"),
                (144, "10"),
                (1010, "5.2"),
                (1020, "2.3"),
            ],
        ),
        rec(
            "BLOCKLINEARGRIP",
            &[
                (5, "GR"),
                (90, "7"),
                (300, "End Grip"),
                (1010, "5"),
                (1020, "0"),
            ],
        ),
        rec("BLOCKGRIPLOCATIONCOMPONENT", &[(5, "LC"), (90, "8")]),
        rec(
            "BLOCKSTRETCHACTION",
            &[
                (5, "ST"),
                (330, "OWNER"),
                (90, "10"),
                (300, "Stretch"),
                (71, "1"),
                (330, "E1"),
                (72, "2"),
                (1011, "1"),
                (1021, "-1"),
                (1011, "6"),
                (1021, "2"),
                (73, "1"),
                (331, "E1"),
                (140, "1"),
                (141, "0"),
            ],
        ),
        rec(
            "BLOCKLOOKUPPARAMETER",
            &[(5, "LP"), (90, "12"), (300, "Lookup"), (303, "Lookup1")],
        ),
        rec(
            "BLOCKLOOKUPACTION",
            &[
                (5, "LA"),
                (90, "16"),
                (300, "Lookup2"),
                (92, "2"),
                (93, "2"),
                (302, "2.5"),
                (302, "2.5mm"),
                (302, "5"),
                (302, "5.0mm"),
                (281, "0"),
                (304, "UpdatedDistance"),
                (281, "1"),
                (304, "lookupString"),
            ],
        ),
        rec(
            "DICTIONARY",
            &[(5, "X1"), (3, "AcDbBlockRepresentation"), (360, "R1")],
        ),
        rec(
            "DICTIONARY",
            &[
                (5, "R1"),
                (3, "AcDbRepData"),
                (360, "RD"),
                (3, "AppDataCache"),
                (360, "C1"),
            ],
        ),
        rec(
            "ACDB_BLOCKREPRESENTATION_DATA",
            &[(5, "RD"), (70, "1"), (340, "A1")],
        ),
        rec(
            "DICTIONARY",
            &[(5, "C1"), (3, "ACAD_ENHANCEDBLOCKDATA"), (360, "C2")],
        ),
        rec(
            "DICTIONARY",
            &[
                (5, "C2"),
                (3, "6"),
                (360, "XR6"),
                (3, "12"),
                (360, "XR12"),
                (3, "10"),
                (360, "XR10"),
            ],
        ),
        rec(
            "XRECORD",
            &[
                (5, "XR6"),
                (1071, "18597260"),
                (70, "25"),
                (10, "0"),
                (20, "0"),
                (30, "0"),
                (10, "10"),
                (20, "0"),
                (30, "0"),
                (10, "0"),
                (20, "0"),
                (30, "-1"),
            ],
        ),
        rec(
            "XRECORD",
            &[
                (5, "XR12"),
                (70, "25"),
                (10, "-1.7"),
                (20, "0"),
                (1, "10mm"),
            ],
        ),
        rec("XRECORD", &[(5, "XR10"), (70, "25"), (40, "0")]),
        rec("ENDSEC", &[]),
    ]
}

fn decoded() -> DynamicBlocks {
    decode(&file())
}

#[test]
fn a_definition_is_found_by_its_xdata_and_its_nodes_read() {
    let found = decoded();
    assert_eq!(found.definitions.len(), 1);
    let slide = &found.definitions[0];
    assert_eq!(slide.name, "Slide");
    let kinds: Vec<(&str, u32)> = slide
        .nodes
        .iter()
        .map(|n| (n.label.as_str(), n.id))
        .collect();
    // The grip's location component is plumbing and is left out.
    assert_eq!(
        kinds,
        [
            ("Linear", 6),
            ("End Grip", 7),
            ("Stretch", 10),
            ("Lookup", 12),
            ("Lookup2", 16)
        ]
    );
    assert_eq!(slide.links, [(6, 10), (7, 6), (12, 16)]);
    let NodeKind::Linear(distance) = &slide.nodes[0].kind else {
        panic!("a linear parameter");
    };
    assert_eq!(distance.name, "Distance1");
    assert_eq!(
        (distance.start, distance.end),
        (Point::new(0.0, 0.0), Point::new(5.0, 0.0))
    );
    assert_eq!(distance.values, [2.5, 5.0, 7.5, 10.0]);
    assert_eq!(slide.nodes[1].kind, NodeKind::Grip(Point::new(5.0, 0.0)));
    assert_eq!(
        slide.nodes[3].kind,
        NodeKind::Lookup {
            name: "Lookup1".into(),
            position: Point::new(0.0, 0.0)
        }
    );
}

#[test]
fn a_stretch_keeps_its_frame_factors_and_only_objects_of_its_block() {
    let found = decoded();
    let NodeKind::Stretch(stretch) = &found.definitions[0].nodes[2].kind else {
        panic!("a stretch");
    };
    // The owner (330 before the count) and the repeated handle do not count.
    assert_eq!(stretch.selection, ["E1"]);
    assert_eq!(stretch.frame, [Point::new(1.0, -1.0), Point::new(6.0, 2.0)]);
    assert_eq!(
        (stretch.multiplier, stretch.angle_offset),
        (Some(1.0), Some(0.0))
    );
}

#[test]
fn a_lookup_table_reads_its_columns_and_rows() {
    let found = decoded();
    let NodeKind::LookupTable(table) = &found.definitions[0].nodes[4].kind else {
        panic!("a lookup action");
    };
    assert_eq!(table.columns[0].property, "UpdatedDistance");
    assert!(!table.columns[0].output && table.columns[1].output);
    assert_eq!(table.rows, [["2.5", "2.5mm"], ["5", "5.0mm"]]);
}

#[test]
fn a_modified_reference_gives_the_values_of_its_parameters() {
    let found = decoded();
    assert_eq!(found.instances.len(), 1);
    let instance = &found.instances[0];
    assert_eq!(
        (instance.block.as_str(), instance.definition.as_str()),
        ("*U2", "Slide")
    );
    // The stretch's record (id 10) holds nothing a reader needs.
    assert_eq!(
        instance.states,
        [
            (
                6,
                NodeState::Parameter {
                    start: Point::new(0.0, 0.0),
                    end: Point::new(10.0, 0.0)
                }
            ),
            (12, NodeState::Lookup("10mm".into()))
        ]
    );
}

#[test]
fn a_file_without_dynamic_blocks_has_none_and_no_warning() {
    let records = [rec("SECTION", &[(2, "ENTITIES")]), rec("LINE", &[(5, "1")])];
    let found = decode(&records);
    assert_eq!(found, DynamicBlocks::default());
}

#[test]
fn damaged_objects_are_read_as_far_as_they_go_and_never_panic() {
    let mut records = file();
    for r in &mut records {
        // A graph cut short.
        if r.name == "ACAD_EVALUATION_GRAPH" {
            r.groups.truncate(5);
        }
    }
    let found = decode(&records);
    assert_eq!(found.definitions.len(), 1);
    // The cut graph names no node; the definition is still listed.
    assert!(found.definitions[0].nodes.is_empty());
    // With no parameter node there is nothing to read the instance's records by.
    assert!(found.instances[0].states.is_empty());
}

#[test]
fn a_table_that_does_not_add_up_is_kept_as_an_unread_object() {
    let mut records = file();
    for r in &mut records {
        if r.name == "BLOCKLOOKUPACTION" {
            r.groups.retain(|g| g.code != 302);
        }
        if r.name == "BLOCKLINEARPARAMETER" {
            r.groups.retain(|g| g.code != 144);
            for g in &mut r.groups {
                if g.code == 175 {
                    g.value = "99999999999".into();
                }
            }
        }
    }
    let found = decode(&records);
    let slide = &found.definitions[0];
    assert_eq!(
        slide.nodes[4].kind,
        NodeKind::Other("BLOCKLOOKUPACTION".into())
    );
    let NodeKind::Linear(distance) = &slide.nodes[0].kind else {
        panic!("a linear parameter");
    };
    assert!(distance.values.is_empty());
}
