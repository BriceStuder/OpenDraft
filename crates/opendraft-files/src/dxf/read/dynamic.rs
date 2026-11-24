//! Dynamic blocks in a DXF file, read into plain values. AutoCAD keeps a
//! dynamic block as a named definition whose BLOCK_RECORD carries the XDATA
//! `AcDbDynamicBlockTrueName`, an `ACAD_EVALUATION_GRAPH` of parameter, grip
//! and action objects behind its `ACAD_ENHANCEDBLOCK` dictionary entry, and,
//! for each modified reference, an anonymous `*U` block with a record per
//! parameter under the INSERT's extension dictionary.
//!
//! Autodesk does not publish these objects: this is read from AutoCAD's own
//! files (see `docs/ARCHITECTURE.md`, "Dynamic blocks in DXF") and not yet
//! checked against AutoCAD. The result describes the file and knows nothing
//! of the drawing; `blocks/dynamic_import` maps it onto dynamic blocks. Only what was read is decoded: linear, polar and
//! lookup and rotation parameters, stretch, polar stretch, move, scale, rotate and lookup actions
//! and grips. Anything else is kept by its object name in [`NodeKind::Other`].

use std::collections::{HashMap, HashSet};

use opendraft_core::Point;

use super::{Group, Record};
use crate::FileError;

mod nodes;

/// Most objects read from one definition's graph.
const MAX_NODES: usize = 4096;
/// Longest list read from one object (value sets, frame points, tables).
const MAX_LIST: usize = 1 << 16;

/// What the file holds of dynamic blocks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DynamicBlocks {
    pub definitions: Vec<DynamicDefinition>,
    pub instances: Vec<DynamicInstance>,
}

/// A named block with parameters and actions.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicDefinition {
    pub name: String,
    pub nodes: Vec<Node>,
    /// Edges of the evaluation graph as pairs of node ids (from, to). A
    /// parameter has an edge to each action that uses it.
    pub links: Vec<(u32, u32)>,
}

/// One object of the evaluation graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// The id the file gives it, which an instance's records are keyed by.
    pub id: u32,
    /// Its element name (`Linear`, `Stretch`, `End Grip`...).
    pub label: String,
    pub kind: NodeKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Linear(Parameter),
    Polar(Parameter),
    /// An XY parameter: `start` is its base and `end` its corner; `name` is
    /// the X distance's and `angle_name` the Y distance's.
    Xy(Parameter),
    /// A point parameter: `start` and `end` are its location.
    Point(Parameter),
    /// A rotation parameter: `start` is its base point and `end` where its
    /// grip starts.
    Rotation(Parameter),
    /// A flip parameter: `start` and `end` are its mirror line.
    Flip(Parameter),
    Lookup {
        name: String,
        position: Point,
    },
    Visibility(Visibility),
    Stretch(Action),
    PolarStretch(Action),
    Move(Action),
    Scale(Action),
    /// A rotate action: `frame` holds the point it turns about.
    Rotate(Action),
    FlipAction(Action),
    LookupTable(Table),
    Grip(Point),
    /// An object not decoded, by its DXF name.
    Other(String),
}

/// A linear or polar parameter in the definition.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub start: Point,
    /// The default end; the default value is its distance from `start`.
    pub end: Point,
    /// A list of allowed distances; empty when there is none.
    pub values: Vec<f64>,
    /// A rotation whose angle is not free (it has a list or an increment), or
    /// a polar parameter whose distances or angles are a list.
    pub restricted: bool,
    /// A polar parameter's angle: its name, and its minimum, maximum and
    /// increment (radians) when it has an increment.
    pub angle_name: String,
    pub angle_range: Option<(f64, f64, f64)>,
}

/// A visibility parameter: the objects it governs and, for each named state,
/// the ones that state shows. An object it governs that a state does not
/// list is hidden in that state; one it does not govern is always shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Visibility {
    pub name: String,
    pub position: Point,
    /// Handles of the objects it governs.
    pub governed: Vec<String>,
    /// Each state's name and the handles of the objects it shows, in the
    /// file's order (the first is the default).
    pub states: Vec<(String, Vec<String>)>,
}

/// An action and what it acts on.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    /// Handles of the block's objects it moves, stretches or scales.
    pub selection: Vec<String>,
    /// The stretch frame's corners, in the block's coordinates.
    pub frame: Vec<Point>,
    /// Distance multiplier and angle offset (stretch and move only).
    pub multiplier: Option<f64>,
    pub angle_offset: Option<f64>,
}

/// A lookup action's table: rows of cells, column by column named for the
/// property each holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub property: String,
    /// An output column is looked up from the input columns.
    pub output: bool,
}

/// A modified reference: the anonymous block it inserts, the definition it
/// came from and the values of its parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicInstance {
    /// The anonymous block's name in the file (`*U12`).
    pub block: String,
    pub definition: String,
    /// The state of each parameter the instance changed, by node id.
    pub states: Vec<(u32, NodeState)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeState {
    /// A linear, polar or rotation parameter's current end point.
    Parameter { start: Point, end: Point },
    /// The text a lookup parameter shows.
    Lookup(String),
    /// The name of the state a visibility parameter shows.
    Visibility(String),
}

/// Reads the dynamic blocks of a DXF file.
pub fn dynamic_blocks(bytes: &[u8]) -> Result<DynamicBlocks, FileError> {
    Ok(decode(&super::records(bytes)?))
}

/// Everything of the file looked up by handle.
struct File<'a> {
    by_handle: HashMap<String, &'a Record>,
    /// Handles of each block's objects, by lower-cased block name.
    block_objects: HashMap<String, HashSet<String>>,
}

pub(super) fn decode(records: &[Record]) -> DynamicBlocks {
    let records_of_definitions: Vec<&Record> = records
        .iter()
        .filter(|r| r.name == "BLOCK_RECORD" && xdata_text(r, "AcDbDynamicBlockTrueName").is_some())
        .collect();
    if records_of_definitions.is_empty() {
        return DynamicBlocks::default();
    }
    let file = File::index(records);
    let mut found = DynamicBlocks::default();
    let mut by_record: HashMap<String, usize> = HashMap::new();
    for record in records_of_definitions {
        let name = super::decode(record.text(2).unwrap_or(""));
        by_record.insert(upper(record.text(5).unwrap_or("")), found.definitions.len());
        found.definitions.push(file.definition(record, name));
    }
    let mut seen = HashSet::new();
    for insert in records.iter().filter(|r| r.name == "INSERT") {
        let block = insert.text(2).unwrap_or("");
        let anonymous = block.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("*U"));
        if anonymous && seen.insert(block.to_owned()) {
            if let Some(instance) = file.instance(insert, &found.definitions, &by_record) {
                found.instances.push(instance);
            }
        }
    }
    found
}

impl<'a> File<'a> {
    fn index(records: &'a [Record]) -> Self {
        let mut by_handle = HashMap::new();
        let mut block_objects: HashMap<String, HashSet<String>> = HashMap::new();
        let mut block: Option<String> = None;
        for record in records {
            match record.name.as_str() {
                "BLOCK" => block = Some(super::decode(record.text(2).unwrap_or("")).to_lowercase()),
                "ENDBLK" => block = None,
                _ => {
                    if let (Some(block), Some(handle)) = (&block, record.text(5)) {
                        block_objects
                            .entry(block.clone())
                            .or_default()
                            .insert(upper(handle));
                    }
                }
            }
            if let Some(handle) = record.text(5) {
                by_handle.insert(upper(handle), record);
            }
        }
        File {
            by_handle,
            block_objects,
        }
    }

    fn get(&self, handle: &str) -> Option<&'a Record> {
        self.by_handle.get(&upper(handle)).copied()
    }

    /// The record an entry of `dictionary` names.
    fn entry(&self, dictionary: &Record, name: &str) -> Option<&'a Record> {
        let (_, handle) = entries(dictionary)
            .into_iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))?;
        self.get(&handle)
    }

    /// The extension dictionary of `record`.
    fn extension(&self, record: &Record) -> Option<&'a Record> {
        self.get(&extension_handle(record)?)
    }

    fn definition(&self, record: &Record, name: String) -> DynamicDefinition {
        let objects = self.block_objects.get(&name.to_lowercase());
        let graph = self
            .extension(record)
            .and_then(|d| self.entry(d, "ACAD_ENHANCEDBLOCK"));
        let (members, edges) = graph.map(graph_parts).unwrap_or_default();
        let mut found = Vec::new();
        let mut ids: HashMap<u32, u32> = HashMap::new();
        for (index, handle) in members.into_iter().take(MAX_NODES) {
            let Some(object) = self.get(&handle) else {
                continue;
            };
            let Some(id) = object.int(90).and_then(|v| u32::try_from(v).ok()) else {
                continue;
            };
            if let Some(kind) = nodes::kind(object, objects) {
                ids.insert(index, id);
                found.push(Node {
                    id,
                    label: object.text(300).unwrap_or("").to_owned(),
                    kind,
                });
            }
        }
        let links = edges
            .into_iter()
            .filter_map(|(from, to)| Some((*ids.get(&from)?, *ids.get(&to)?)))
            .collect();
        DynamicDefinition {
            name,
            nodes: found,
            links,
        }
    }

    /// A modified reference of a definition read from this file.
    fn instance(
        &self,
        insert: &Record,
        definitions: &[DynamicDefinition],
        by_record: &HashMap<String, usize>,
    ) -> Option<DynamicInstance> {
        let rep = self.entry(self.extension(insert)?, "AcDbBlockRepresentation")?;
        let data = self.entry(rep, "AcDbRepData")?;
        let definition = definitions.get(*by_record.get(&upper(data.text(340)?))?)?;
        let mut states = Vec::new();
        let cache = self
            .entry(rep, "AppDataCache")
            .and_then(|c| self.entry(c, "ACAD_ENHANCEDBLOCKDATA"));
        for (key, handle) in cache.map(entries).unwrap_or_default() {
            let Ok(id) = key.trim().parse::<u32>() else {
                continue;
            };
            let node = definition.nodes.iter().find(|n| n.id == id);
            let state = node
                .zip(self.get(&handle))
                .and_then(|(n, r)| nodes::state(&n.kind, r));
            states.extend(state.map(|s| (id, s)));
        }
        Some(DynamicInstance {
            block: insert.text(2)?.to_owned(),
            definition: definition.name.clone(),
            states,
        })
    }
}

fn upper(handle: &str) -> String {
    handle.trim().to_ascii_uppercase()
}

/// The text of the 1000 group after the 1001 group naming `application`.
fn xdata_text<'a>(record: &'a Record, application: &str) -> Option<&'a str> {
    let at = record
        .groups
        .iter()
        .position(|g| g.code == 1001 && g.value == application)?;
    let next = record.groups.get(at + 1)?;
    (next.code == 1000).then_some(next.value.as_str())
}

/// The handle of the record's extension dictionary (`102 {ACAD_XDICTIONARY`).
fn extension_handle(record: &Record) -> Option<String> {
    let mut inside = false;
    for g in &record.groups {
        match g.code {
            102 => inside = g.value.starts_with("{ACAD_XDICTIONARY"),
            360 if inside => return Some(g.value.clone()),
            _ => {}
        }
    }
    None
}

/// A dictionary's (name, handle) entries.
fn entries(dictionary: &Record) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut name: Option<&str> = None;
    for g in &dictionary.groups {
        match g.code {
            3 => name = Some(g.value.as_str()),
            350 | 360 => {
                if let Some(n) = name.take() {
                    found.push((n.to_owned(), g.value.clone()));
                }
            }
            _ => {}
        }
        if found.len() >= MAX_LIST {
            break;
        }
    }
    found
}

type Edges = Vec<(u32, u32)>;

/// A graph's objects as (index in the graph, handle) and its edges as
/// (index, index). A node is `91 93 95 360` and four `92`; an edge is `92 93
/// 94 91 91` and five `92`.
fn graph_parts(graph: &Record) -> (Vec<(u32, String)>, Edges) {
    let g = &graph.groups;
    let code = |i: usize| g.get(i).map(|x| x.code);
    let number = |i: usize| g.get(i).and_then(|x| x.value.trim().parse::<u32>().ok());
    let (mut members, mut edges) = (Vec::new(), Vec::new());
    let mut i = 0;
    while i < g.len() {
        let node = [code(i), code(i + 1), code(i + 2), code(i + 3)];
        let edge = [code(i), code(i + 1), code(i + 2), code(i + 3), code(i + 4)];
        if node == [Some(91), Some(93), Some(95), Some(360)] {
            if let Some(index) = number(i) {
                members.push((index, g[i + 3].value.clone()));
            }
            i += 8;
        } else if edge == [Some(92), Some(93), Some(94), Some(91), Some(91)] {
            if let (Some(from), Some(to)) = (number(i + 3), number(i + 4)) {
                edges.push((from, to));
            }
            i += 10;
        } else {
            i += 1;
        }
        if members.len() >= MAX_NODES || edges.len() >= MAX_LIST {
            break;
        }
    }
    (members, edges)
}

/// Every (x, y) pair of groups with these codes, an x code followed by its y.
fn points(record: &Record, x: i32, y: i32) -> Vec<Point> {
    let num = |g: &Group| g.value.trim().parse::<f64>().ok().filter(|v| v.is_finite());
    let mut found = Vec::new();
    for (i, g) in record.groups.iter().enumerate() {
        let next = record.groups.get(i + 1);
        if let (true, Some(n)) = (g.code == x, next.filter(|n| n.code == y)) {
            if let (Some(px), Some(py)) = (num(g), num(n)) {
                found.push(Point::new(px, py));
            }
        }
        if found.len() >= MAX_LIST {
            break;
        }
    }
    found
}

#[cfg(test)]
pub(in crate::dxf::read) mod tests;
