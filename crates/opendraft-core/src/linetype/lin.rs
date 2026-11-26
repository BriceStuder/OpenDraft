//! Reading an AutoCAD `.lin` linetype file: pure text in, definitions and
//! per-entry problems out. Nothing here touches a drawing or a file; the
//! caller decides which entries to load.
//!
//! The grammar is in `private_material/autocad_linetype_reference_guide.md`,
//! section 4: `*NAME[,description]` then `A,len,len,...`, `;` comment lines,
//! 2 to 12 elements, the first not a gap. A bad entry is skipped and
//! reported, and the rest of the file still reads.

use super::{StrokeElement, MAX_NAME_BYTES};

/// The longest description AutoCAD keeps, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 47;

/// One linetype read from the file.
#[derive(Clone, Debug, PartialEq)]
pub struct LinEntry {
    pub name: String,
    pub description: String,
    pub elements: Vec<StrokeElement>,
    /// The pattern had text or shape elements (`["GAS",STANDARD,...]`). They
    /// are dropped: `elements` holds only the dashes, as a DXF import does.
    pub complex: bool,
}

/// Something wrong with the file or one of its entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinNote {
    /// The 1-based line it was found on.
    pub line: usize,
    /// The linetype it is about, when there was a header to name one.
    pub name: Option<String>,
    pub message: String,
    /// True when the entry was left out, false when it was kept with a
    /// change (text or shape elements dropped).
    pub skipped: bool,
}

/// What a `.lin` file holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinFile {
    pub entries: Vec<LinEntry>,
    pub notes: Vec<LinNote>,
}

/// Why a pattern could not be stored in a `.lin` file, if it could:
/// AutoCAD allows 2 to 12 elements and a first one that is not a gap.
pub fn pattern_problem(elements: &[StrokeElement]) -> Option<&'static str> {
    if !(2..=12).contains(&elements.len()) {
        Some("a .lin pattern has 2 to 12 elements")
    } else if matches!(elements[0], StrokeElement::Gap(_)) {
        Some("a .lin pattern cannot start with a gap")
    } else {
        None
    }
}

/// Reads the bytes of a `.lin` file. They are not always UTF-8 (AutoCAD's
/// `acadiso.lin` holds Shift-JIS spaces in its JIS descriptions), so what
/// does not decode becomes a replacement character in a description rather
/// than the file being refused.
pub fn parse_bytes(bytes: &[u8]) -> LinFile {
    parse(&String::from_utf8_lossy(bytes))
}

/// A header waiting for its pattern line.
struct Header {
    line: usize,
    name: String,
    description: String,
    /// False when the header was refused (and said so): its pattern line is
    /// passed over without a second report.
    usable: bool,
}

/// Reads the text of a `.lin` file.
pub fn parse(text: &str) -> LinFile {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut file = LinFile::default();
    let mut waiting: Option<Header> = None;
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let content = raw.trim();
        if content.is_empty() || content.starts_with(';') {
            continue;
        }
        if let Some(header) = content.strip_prefix('*') {
            if let Some(lost) = waiting.take().filter(|h| h.usable) {
                file.skip(&lost, "has no pattern line");
            }
            waiting = Some(file.header(line, header));
        } else if let Some(header) = waiting.take() {
            if header.usable {
                file.pattern(&header, content);
            }
        } else {
            file.notes.push(LinNote {
                line,
                name: None,
                message: "a line outside any linetype definition was ignored".into(),
                skipped: true,
            });
        }
    }
    if let Some(lost) = waiting.filter(|h| h.usable) {
        file.skip(&lost, "has no pattern line");
    }
    file
}

impl LinFile {
    fn skip(&mut self, header: &Header, message: &str) {
        self.notes.push(LinNote {
            line: header.line,
            name: Some(header.name.clone()),
            message: message.into(),
            skipped: true,
        });
    }

    /// A `*NAME,description` line (the star already taken off). When the
    /// name cannot be used, says why and returns an unusable header.
    fn header(&mut self, line: usize, text: &str) -> Header {
        let (name, description) = text.split_once(',').unwrap_or((text, ""));
        let name = name.trim().to_owned();
        let mut description = description.trim().to_owned();
        let header = Header {
            line,
            name,
            description: String::new(),
            usable: false,
        };
        let refusal = if header.name.is_empty() {
            Some("a linetype needs a name")
        } else if header.name.len() > MAX_NAME_BYTES {
            Some("the name is too long")
        } else if ["CONTINUOUS", "BYLAYER", "BYBLOCK"]
            .iter()
            .any(|n| n.eq_ignore_ascii_case(&header.name))
        {
            Some("is built in and cannot be redefined")
        } else if self
            .entries
            .iter()
            .any(|e| e.name.eq_ignore_ascii_case(&header.name))
        {
            Some("appears twice; the first one is kept")
        } else {
            None
        };
        if let Some(message) = refusal {
            self.skip(&header, message);
            return header;
        }
        // AutoCAD's own files have longer ones and it keeps the first 47
        // characters (its DXF of `DOT` does), so no note is made of it.
        description = description.chars().take(MAX_DESCRIPTION_CHARS).collect();
        Header {
            description,
            usable: true,
            ..header
        }
    }

    /// The `A,len,len,...` line of the entry `header` began.
    fn pattern(&mut self, header: &Header, content: &str) {
        let (alignment, rest) = content.split_once(',').unwrap_or((content, ""));
        if !alignment.trim().eq_ignore_ascii_case("A") {
            return self.skip(header, "its pattern line must start with A,");
        }
        let (plain, complex) = without_decorations(rest);
        let mut elements = Vec::new();
        for field in plain.split(',').map(str::trim).filter(|f| !f.is_empty()) {
            match field.parse::<f64>().ok().and_then(StrokeElement::from_dxf) {
                Some(element) => elements.push(element),
                None => return self.skip(header, &format!("{field:?} is not a length")),
            }
        }
        if let Some(problem) = pattern_problem(&elements) {
            return self.skip(header, problem);
        }
        self.entries.push(LinEntry {
            name: header.name.clone(),
            description: header.description.clone(),
            elements,
            complex,
        });
        if complex {
            self.notes.push(LinNote {
                line: header.line,
                name: Some(header.name.clone()),
                message: "its text or shape elements were dropped, leaving the dashes".into(),
                skipped: false,
            });
        }
    }
}

/// `rest` with every `[...]` group taken out (text and shape elements; a
/// quoted string inside may hold brackets and commas), and whether there
/// were any.
fn without_decorations(rest: &str) -> (String, bool) {
    let mut plain = String::new();
    let mut depth = 0usize;
    let mut quoted = false;
    let mut found = false;
    for c in rest.chars() {
        match c {
            '"' if depth > 0 => quoted = !quoted,
            '[' if !quoted => {
                depth += 1;
                found = true;
            }
            ']' if !quoted && depth > 0 => depth -= 1,
            _ if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    (plain, found)
}
