//! The questions BPARAMETER asks that do not depend on the kind of parameter:
//! its name and label, whether it shows, how many grips, and its value set.
//! Distances are in the block's units and angles are typed in degrees and
//! kept in radians.

use opendraft_commands::{Answer, Cancelled, Editor, NumberOptions, Options};
use opendraft_core::block::{DynamicBlock, DynamicValueSet};

pub(super) const EPS: f64 = 1e-9;

/// What a parameter's value measures.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Quantity {
    Distance,
    Angle,
}

impl Quantity {
    fn word(self) -> &'static str {
        match self {
            Quantity::Distance => "distance",
            Quantity::Angle => "angle",
        }
    }

    /// A number as typed, in the block's unit.
    fn stored(self, typed: f64) -> f64 {
        match self {
            Quantity::Distance => typed,
            Quantity::Angle => typed.to_radians(),
        }
    }
}

/// The first `{prefix}N` (N from 1) that `taken` does not hold, ignoring case.
pub(super) fn free_name(prefix: &str, taken: &[&str]) -> String {
    (1..)
        .map(|n| format!("{prefix}{n}"))
        .find(|name| !taken.iter().any(|t| t.eq_ignore_ascii_case(name)))
        .unwrap_or_default()
}

/// A line where Enter takes `default`.
pub(super) fn ask_line(ed: &Editor, message: &str, default: &str) -> Result<String, Cancelled> {
    let shown = if default.is_empty() {
        format!("{message} <none>")
    } else {
        format!("{message} <{default}>")
    };
    match ed.ask_string(Options::<String>::new(shown))? {
        Answer::Value(text) => Ok(text.trim().to_owned()),
        Answer::Enter | Answer::Keyword(_) => Ok(default.to_owned()),
    }
}

/// A name nothing else in `taken` has, asked again until it is one.
pub(super) fn ask_name(
    ed: &Editor,
    message: &str,
    default: &str,
    taken: &[&str],
) -> Result<String, Cancelled> {
    loop {
        let name = ask_line(ed, message, default)?;
        if name.is_empty() || name.contains(char::is_whitespace) {
            ed.say("A name cannot be empty or contain spaces.");
        } else if taken.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
            ed.say(format!("The name \"{name}\" is already used."));
        } else {
            return Ok(name);
        }
    }
}

pub(super) fn ask_yes_no(ed: &Editor, message: &str, default_yes: bool) -> Result<bool, Cancelled> {
    let default = if default_yes { "Yes" } else { "No" };
    let options = Options::<String>::new(format!("{message} [Yes/No] <{default}>"))
        .keyword("Yes", "Y")
        .keyword("No", "N")
        .default(default.to_owned());
    Ok(ed.get_keyword(options)? == "Yes")
}

/// The numbers of a typed list, sorted and without repeats; why not, if any
/// is not a finite number.
pub(super) fn parse_list(text: &str) -> Result<Vec<f64>, String> {
    let mut values = Vec::new();
    for part in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match part.parse::<f64>() {
            Ok(v) if v.is_finite() => values.push(v),
            _ => return Err(format!("\"{part}\" is not a number.")),
        }
    }
    values.sort_by(f64::total_cmp);
    values.dedup();
    if values.is_empty() {
        Err("Enter at least one value.".to_owned())
    } else {
        Ok(values)
    }
}

/// The value set a parameter takes, asked as `None`, `List` or `Increment`.
/// A list always holds `default`, the value of the block as drawn.
pub(super) fn ask_value_set(
    ed: &Editor,
    quantity: Quantity,
    default: f64,
) -> Result<DynamicValueSet, Cancelled> {
    let word = quantity.word();
    let options = Options::<String>::new(format!(
        "Enter {word} value set type [None/List/Increment] <None>"
    ))
    .keyword("None", "N")
    .keyword("List", "L")
    .keyword("Increment", "I")
    .default("None".to_owned());
    match ed.get_keyword(options)?.as_str() {
        "List" => loop {
            let text = ed.get_string(Options::<String>::new(format!(
                "Enter list of {word} values (separated by commas)"
            )))?;
            match parse_list(&text) {
                Ok(typed) => {
                    let mut values: Vec<f64> =
                        typed.into_iter().map(|v| quantity.stored(v)).collect();
                    if !values.iter().any(|v| (v - default).abs() <= EPS) {
                        values.push(default);
                        values.sort_by(f64::total_cmp);
                    }
                    return Ok(DynamicValueSet::List(values));
                }
                Err(why) => ed.say(why),
            }
        },
        "Increment" => loop {
            let min = ed.get_number(NumberOptions::new(format!("Enter minimum {word}")))?;
            let max = ed.get_number(NumberOptions::new(format!("Enter maximum {word}")))?;
            let step = ed.get_number(NumberOptions::new(format!("Enter {word} increment")))?;
            if min.is_finite() && max.is_finite() && step.is_finite() && min <= max && step > 0.0 {
                return Ok(DynamicValueSet::Range {
                    min: quantity.stored(min),
                    max: quantity.stored(max),
                    increment: Some(quantity.stored(step)),
                });
            }
            ed.say(
                "The minimum cannot be above the maximum, and the increment must be above zero.",
            );
        },
        _ => Ok(DynamicValueSet::Any),
    }
}

/// How many grips the parameter shows: 0 or 1.
pub(super) fn ask_grips(ed: &Editor) -> Result<u8, Cancelled> {
    loop {
        let n = ed
            .get_number(NumberOptions::new("Enter number of grips [0/1] <1>").default(1.0))?
            .round();
        if n == 0.0 || n == 1.0 {
            return Ok(n as u8);
        }
        ed.say("Two grips are not available yet; use 0 or 1.");
    }
}

/// What every kind of parameter is asked after its geometry.
pub(super) struct Naming {
    pub name: String,
    pub label: String,
    pub description: String,
    pub show: bool,
}

/// The name (default `{prefix}N`), label, description and visibility.
pub(super) fn ask_naming(
    ed: &Editor,
    schema: &DynamicBlock,
    prefix: &str,
    label_word: &str,
) -> Result<Naming, Cancelled> {
    let names: Vec<&str> = schema.parameters.iter().map(|p| p.name.as_str()).collect();
    let name = ask_name(
        ed,
        "Enter parameter name",
        &free_name(prefix, &names),
        &names,
    )?;
    let label = ask_line(ed, &format!("Enter {label_word} label"), &name)?;
    let description = ask_line(ed, "Enter parameter description", "")?;
    let show = ask_yes_no(ed, "Show the parameter in the Properties palette?", true)?;
    Ok(Naming {
        name,
        label,
        description,
        show,
    })
}
