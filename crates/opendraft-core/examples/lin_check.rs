#![allow(clippy::print_stdout, clippy::print_stderr)] // a command-line tool
//! Reads the `.lin` files in `private_material/` (AutoCAD's own, never
//! committed) with `lin::parse` and prints what came out. Skips files it
//! cannot find, so nothing depends on them.
//!
//! `cargo run -p opendraft-core --example lin_check`

use opendraft_core::linetype::lin;

fn main() {
    for name in ["acad.lin", "acadiso.lin"] {
        let path = format!(
            "{}/../../private_material/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        let Ok(bytes) = std::fs::read(&path) else {
            println!("{name}: not found, skipped");
            continue;
        };
        // AutoCAD writes these in a Windows code page, not always UTF-8.
        let text = String::from_utf8_lossy(&bytes);
        let file = lin::parse(&text);
        println!(
            "{name}: {} read, {} notes, utf8 {}",
            file.entries.len(),
            file.notes.len(),
            std::str::from_utf8(&bytes).is_ok()
        );
        for note in &file.notes {
            println!(
                "  line {} {:?}: {} ({})",
                note.line,
                note.name,
                note.message,
                if note.skipped { "skipped" } else { "kept" }
            );
        }
        // Only `acadiso.lin` is in the stock library's units (millimetres).
        if name == "acadiso.lin" {
            let (mut same, mut differ, mut missing, mut words) = (0, 0, 0, 0);
            for e in &file.entries {
                let Some(s) = opendraft_core::linetype::stock::find(&e.name) else {
                    missing += 1;
                    println!("  not in stock: {}", e.name);
                    continue;
                };
                let got: Vec<f64> = e.elements.iter().map(|x| x.to_dxf()).collect();
                let numbers = got.len() == s.pattern.len()
                    && got.iter().zip(s.pattern).all(|(a, b)| (a - b).abs() < 1e-9);
                if !numbers {
                    differ += 1;
                    println!(
                        "  numbers differ: {} file {:?} stock {:?}",
                        e.name, got, s.pattern
                    );
                } else {
                    same += 1;
                    if e.description != s.description {
                        words += 1;
                    }
                }
            }
            println!(
                "  against stock: {same} same numbers ({words} with another description), {differ} differ, {missing} not in stock"
            );
        }
        for e in &file.entries {
            let numbers: Vec<String> = e.elements.iter().map(|x| x.to_dxf().to_string()).collect();
            println!(
                "  {:<12} {}{}",
                e.name,
                numbers.join(","),
                if e.complex { "  [complex]" } else { "" }
            );
        }
    }
}
