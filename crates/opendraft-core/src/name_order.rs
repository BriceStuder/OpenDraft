//! The order names are listed in: alphabetical, ignoring case, with runs of
//! digits compared as numbers, so "Level 2" comes before "Level 10", as in
//! AutoCAD's layer lists.

use std::cmp::Ordering;
use std::iter::Peekable;
use std::str::Chars;

/// Compare two names for display: case and leading zeros only decide ties.
pub fn compare(a: &str, b: &str) -> Ordering {
    loose(a, b).then_with(|| a.cmp(b))
}

fn loose(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let order = number(digits(&mut a), digits(&mut b));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

fn digits(chars: &mut Peekable<Chars>) -> String {
    let mut run = String::new();
    while let Some(c) = chars.next_if(char::is_ascii_digit) {
        run.push(c);
    }
    run
}

/// Two runs of digits by value, however long they are.
fn number(a: String, b: String) -> Ordering {
    let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::compare;

    fn sorted(names: &[&str]) -> Vec<String> {
        let mut names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        names.sort_by(|a, b| compare(a, b));
        names
    }

    #[test]
    fn names_sort_alphabetically_whatever_their_case() {
        assert_eq!(
            sorted(&["walls", "Doors", "0", "axes", "Defpoints"]),
            ["0", "axes", "Defpoints", "Doors", "walls"]
        );
    }

    #[test]
    fn numbers_in_names_sort_by_value() {
        assert_eq!(
            sorted(&["Level 10", "Level 2", "Level 1", "Level 02b"]),
            ["Level 1", "Level 2", "Level 02b", "Level 10"]
        );
    }

    #[test]
    fn accented_letters_compare_without_their_case() {
        assert_eq!(sorted(&["étage", "Étage"]), ["Étage", "étage"]);
    }

    #[test]
    fn long_numbers_do_not_overflow() {
        let big = "L99999999999999999999999";
        assert_eq!(sorted(&[big, "L1"]), ["L1", big]);
    }
}
