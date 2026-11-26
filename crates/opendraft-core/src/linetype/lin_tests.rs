use super::lin::{parse, LinNote, MAX_DESCRIPTION_CHARS};
use super::stock;
use super::StrokeElement::{Dash, Dot, Gap};

fn skipped(notes: &[LinNote]) -> Vec<(&str, &str)> {
    notes
        .iter()
        .filter(|n| n.skipped)
        .map(|n| (n.name.as_deref().unwrap_or(""), n.message.as_str()))
        .collect()
}

#[test]
fn a_simple_definition_reads() {
    let file = parse("*DASHED,Dashed __ __ __\nA,.5,-.25\n");
    assert!(file.notes.is_empty());
    assert_eq!(file.entries.len(), 1);
    let e = &file.entries[0];
    assert_eq!(e.name, "DASHED");
    assert_eq!(e.description, "Dashed __ __ __");
    assert_eq!(e.elements, vec![Dash(0.5), Gap(0.25)]);
    assert!(!e.complex);
}

#[test]
fn zero_is_a_dot_and_the_description_is_optional() {
    let file = parse("*DOT\nA,0,-6.35\n");
    assert!(file.notes.is_empty());
    assert_eq!(file.entries[0].description, "");
    assert_eq!(file.entries[0].elements, vec![Dot, Gap(6.35)]);
}

#[test]
fn comments_blanks_crlf_and_a_byte_order_mark_are_ignored() {
    let text =
        "\u{feff};; a header comment\r\n\r\n*HIDDEN,Hidden\r\n  a, 6.35 , -3.175 \r\n; end\r\n";
    let file = parse(text);
    assert!(file.notes.is_empty());
    assert_eq!(file.entries[0].elements, vec![Dash(6.35), Gap(3.175)]);
}

#[test]
fn several_definitions_keep_their_order() {
    let file = parse("*ONE,a\nA,1,-1\n*TWO,b\nA,2,-1\n*THREE,c\nA,3,-1\n");
    let names: Vec<_> = file.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["ONE", "TWO", "THREE"]);
}

#[test]
fn a_bad_entry_is_skipped_and_the_rest_still_read() {
    let text = "*GOOD,ok\nA,1,-1\n*BAD,x\nA,1,abc\n*ALSO,ok\nA,2,-2\n";
    let file = parse(text);
    let names: Vec<_> = file.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["GOOD", "ALSO"]);
    let bad = skipped(&file.notes);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].0, "BAD");
    assert!(bad[0].1.contains("abc"));
    assert_eq!(file.notes[0].line, 3);
}

#[test]
fn the_element_count_and_the_first_element_are_checked() {
    let text = "*SHORT\nA,1\n*GAPFIRST\nA,-1,1\n*LONG\nA,1,-1,1,-1,1,-1,1,-1,1,-1,1,-1,1\n";
    let file = parse(text);
    assert!(file.entries.is_empty());
    let bad = skipped(&file.notes);
    assert_eq!(bad.len(), 3);
    assert!(bad[0].1.contains("2 to 12"));
    assert!(bad[1].1.contains("gap"));
    assert!(bad[2].1.contains("2 to 12"));
    // Twelve is the most.
    let twelve = parse("*TWELVE\nA,1,-1,1,-1,1,-1,1,-1,1,-1,1,-1\n");
    assert_eq!(twelve.entries[0].elements.len(), 12);
}

#[test]
fn only_a_alignment_is_read() {
    let file = parse("*ODD\nB,1,-1\n");
    assert!(file.entries.is_empty());
    assert!(skipped(&file.notes)[0].1.contains("A,"));
    assert_eq!(parse("*LOWER\na,1,-1\n").entries.len(), 1);
}

#[test]
fn a_header_with_no_pattern_line_is_reported() {
    let file = parse("*LOST,never defined\n*FOUND\nA,1,-1\n*LASTLOST\n");
    assert_eq!(file.entries.len(), 1);
    let bad = skipped(&file.notes);
    assert_eq!(bad.len(), 2);
    assert_eq!(bad[0].0, "LOST");
    assert_eq!(bad[1].0, "LASTLOST");
    assert!(bad[0].1.contains("no pattern"));
}

#[test]
fn a_pattern_line_with_no_header_is_reported() {
    let file = parse("A,1,-1\n*OK\nA,1,-1\n");
    assert_eq!(file.entries.len(), 1);
    assert_eq!(file.notes.len(), 1);
    assert_eq!(file.notes[0].line, 1);
    assert_eq!(file.notes[0].name, None);
}

#[test]
fn names_are_unique_ignoring_case_and_the_first_is_kept() {
    let file = parse("*Dashed,first\nA,1,-1\n*DASHED,second\nA,2,-2\n");
    assert_eq!(file.entries.len(), 1);
    assert_eq!(file.entries[0].description, "first");
    assert!(skipped(&file.notes)[0].1.contains("twice"));
}

#[test]
fn the_built_in_names_cannot_be_redefined() {
    let file = parse("*continuous\nA,1,-1\n*ByLayer\nA,1,-1\n*BYBLOCK\nA,1,-1\n");
    assert!(file.entries.is_empty());
    assert_eq!(skipped(&file.notes).len(), 3);
}

#[test]
fn an_empty_name_is_refused() {
    let file = parse("*,nothing\nA,1,-1\n");
    assert!(file.entries.is_empty());
    assert!(skipped(&file.notes)[0].1.contains("name"));
}

#[test]
fn a_long_description_is_cut_quietly_as_autocad_does() {
    let long = "x".repeat(60);
    let file = parse(&format!("*LONG,{long}\nA,1,-1\n"));
    assert_eq!(file.entries[0].description.len(), MAX_DESCRIPTION_CHARS);
    assert!(file.notes.is_empty());
}

#[test]
fn a_description_may_hold_commas() {
    let file = parse("*C,one, two, three\nA,1,-1\n");
    assert_eq!(file.entries[0].description, "one, two, three");
}

#[test]
fn text_and_shape_elements_are_dropped_and_reported() {
    let text = "*GAS_LINE,Gas line ----GAS----GAS----\n\
                A,.5,-.2,[\"GAS\",STANDARD,S=.1,U=0.0,X=-0.1,Y=-.05],-.25\n\
                *FENCE,Fence ----0-----0----\n\
                A,.15,[CIRC1,ltypeshp.shx,x=-.1,s=.1],-.1,1\n";
    let file = parse(text);
    assert_eq!(file.entries.len(), 2);
    assert_eq!(
        file.entries[0].elements,
        vec![Dash(0.5), Gap(0.2), Gap(0.25)]
    );
    assert!(file.entries.iter().all(|e| e.complex));
    assert_eq!(file.notes.len(), 2);
    assert!(file.notes.iter().all(|n| !n.skipped));
}

#[test]
fn brackets_and_commas_inside_a_quoted_string_do_not_end_the_group() {
    let file = parse("*TXT\nA,1,[\"a],b\",STANDARD,S=.1],-1\n");
    assert_eq!(file.entries[0].elements, vec![Dash(1.0), Gap(1.0)]);
}

#[test]
fn numbers_that_are_not_lengths_are_refused() {
    for bad in ["nan", "inf", "1,,x", "--1"] {
        let file = parse(&format!("*N\nA,1,{bad}\n"));
        assert!(file.entries.is_empty(), "{bad} should be refused");
    }
}

#[test]
fn an_empty_file_is_empty() {
    assert!(parse("").entries.is_empty());
    assert!(parse(";; only comments\n\n").notes.is_empty());
}

#[test]
fn every_stock_pattern_survives_being_written_as_lin_text() {
    let mut text = String::new();
    for s in stock::LIBRARY {
        let numbers: Vec<String> = s.pattern.iter().map(|v| format!("{v}")).collect();
        text.push_str(&format!(
            "*{},{}\nA,{}\n",
            s.name,
            s.description,
            numbers.join(",")
        ));
    }
    let file = parse(&text);
    assert_eq!(file.entries.len(), stock::LIBRARY.len(), "{:?}", file.notes);
    for (entry, s) in file.entries.iter().zip(stock::LIBRARY) {
        let want: Vec<f64> = s.pattern.to_vec();
        let got: Vec<f64> = entry.elements.iter().map(|e| e.to_dxf()).collect();
        assert_eq!(got, want, "{}", s.name);
    }
}

#[test]
fn bytes_that_are_not_utf8_still_read() {
    // Shift-JIS's full-width space, as in acadiso.lin's JIS descriptions.
    let mut bytes = b"*JIS_09_29,2SASEN29 ____".to_vec();
    bytes.extend_from_slice(&[0x81, 0x40]);
    bytes.extend_from_slice(b"__\r\nA,12,-3\r\n");
    let file = super::lin::parse_bytes(&bytes);
    assert_eq!(file.entries.len(), 1);
    assert!(file.entries[0].description.starts_with("2SASEN29 ____"));
    assert!(file.notes.is_empty());
}
