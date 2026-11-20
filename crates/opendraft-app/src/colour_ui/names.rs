//! Colours as AutoCAD names them: "Red" for the named index colours, the
//! number for the other index colours ("34") and "R,G,B" for true colours;
//! and the same forms (or "#RRGGBB") read back from what the user types.

use opendraft_core::aci::{aci_rgb, nearest_aci};
use opendraft_core::Color;

use super::Pick;

/// The index colours AutoCAD calls by a name (7 is the foreground).
const NAMED: [(u8, &str); 6] = [
    (1, "Red"),
    (2, "Yellow"),
    (3, "Green"),
    (4, "Cyan"),
    (5, "Blue"),
    (6, "Magenta"),
];

/// The index colour `i` (1..=255); 7 is the foreground.
pub fn from_index(i: u8) -> Color {
    if i == 7 {
        Color::Foreground
    } else {
        let (r, g, b) = aci_rgb(i);
        Color::Rgb(r, g, b)
    }
}

/// The index whose colour `color` is exactly, if any (the lowest when two
/// indices share a colour).
pub fn index_of(color: Color) -> Option<u8> {
    match color {
        Color::Foreground => Some(7),
        Color::Rgb(r, g, b) => {
            let i = nearest_aci((r, g, b));
            (aci_rgb(i) == (r, g, b)).then_some(i)
        }
    }
}

/// Human name: "Foreground", "Red", "34" or "255,128,0".
pub fn name(color: Color) -> String {
    match (index_of(color), color) {
        (Some(7), _) => "Foreground".to_owned(),
        (Some(i), _) => match NAMED.iter().find(|(n, _)| *n == i) {
            Some((_, name)) => (*name).to_owned(),
            None => i.to_string(),
        },
        (None, Color::Rgb(r, g, b)) => format!("{r},{g},{b}"),
        (None, Color::Foreground) => "Foreground".to_owned(),
    }
}

/// What the user typed: a colour name, an index number (1 to 255), "R,G,B",
/// "#RRGGBB", or ByLayer and ByBlock when `by_layer` allows them.
pub fn parse(text: &str, by_layer: bool) -> Option<Pick> {
    let text = text.trim().to_ascii_lowercase();
    match text.as_str() {
        "bylayer" if by_layer => return Some(Pick::ByLayer),
        "byblock" if by_layer => return Some(Pick::ByBlock),
        "foreground" | "white" => return Some(Pick::Color(Color::Foreground)),
        _ => {}
    }
    if let Some((i, _)) = NAMED.iter().find(|(_, n)| n.eq_ignore_ascii_case(&text)) {
        return Some(Pick::Color(from_index(*i)));
    }
    if let Ok(i) = text.parse::<u8>() {
        return (i >= 1).then(|| Pick::Color(from_index(i)));
    }
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    if let [r, g, b] = parts.as_slice() {
        return Some(Pick::Color(Color::Rgb(
            r.parse().ok()?,
            g.parse().ok()?,
            b.parse().ok()?,
        )));
    }
    parse_hex(&text).map(Pick::Color)
}

/// Parse "#RRGGBB" or "RRGGBB".
fn parse_hex(text: &str) -> Option<Color> {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(Color::Rgb(byte(0)?, byte(2)?, byte(4)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_with_or_without_hash() {
        assert_eq!(parse_hex("#FF8000"), Some(Color::Rgb(255, 128, 0)));
        assert_eq!(parse_hex(" 00ff00 "), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(parse_hex("#FFF"), None);
        assert_eq!(parse_hex("GG0000"), None);
        assert_eq!(parse_hex("\u{e9}\u{e9}\u{e9}"), None);
    }

    #[test]
    fn names_follow_autocad() {
        assert_eq!(name(Color::Foreground), "Foreground");
        assert_eq!(name(Color::Rgb(255, 0, 0)), "Red");
        assert_eq!(name(from_index(34)), "34");
        assert_eq!(name(Color::Rgb(255, 255, 255)), "255");
        assert_eq!(name(Color::Rgb(1, 2, 3)), "1,2,3");
    }

    #[test]
    fn every_index_reads_back_as_itself() {
        for i in 1..=255u8 {
            let color = from_index(i);
            assert_eq!(aci_rgb(index_of(color).unwrap_or(0)), aci_rgb(i), "{i}");
            assert_eq!(parse(&name(color), false), Some(Pick::Color(color)), "{i}");
        }
    }

    #[test]
    fn typed_colours() {
        let pick = |t: &str| parse(t, true);
        assert_eq!(pick("red"), Some(Pick::Color(Color::Rgb(255, 0, 0))));
        assert_eq!(pick(" 7 "), Some(Pick::Color(Color::Foreground)));
        assert_eq!(pick("White"), Some(Pick::Color(Color::Foreground)));
        assert_eq!(pick("34"), Some(Pick::Color(from_index(34))));
        assert_eq!(
            pick("255,128,0"),
            Some(Pick::Color(Color::Rgb(255, 128, 0)))
        );
        assert_eq!(pick("#FF8000"), Some(Pick::Color(Color::Rgb(255, 128, 0))));
        assert_eq!(pick("ByLayer"), Some(Pick::ByLayer));
        assert_eq!(parse("ByLayer", false), None);
        assert_eq!(pick("0"), None);
        assert_eq!(pick("256"), None);
        assert_eq!(pick("300,0,0"), None);
        assert_eq!(pick("mauve"), None);
    }
}
