//! The values a parameter may take: lists, ranges and what each does with a
//! value outside it.

use super::*;

#[test]
fn a_list_takes_the_nearest_entry_and_the_lower_on_a_tie() {
    let set = DynamicValueSet::List(vec![2.0, 4.0, 8.0]);
    assert_eq!(set.normalize(5.0), Some(4.0));
    assert_eq!(set.normalize(6.0), Some(4.0), "a tie goes to the lower");
    assert_eq!(set.normalize(100.0), Some(8.0));
    assert_eq!(set.normalize(-3.0), Some(2.0));
}

#[test]
fn a_range_clamps_and_rounds_to_its_increment_from_the_minimum() {
    let set = DynamicValueSet::Range {
        min: 1.0,
        max: 10.0,
        increment: Some(2.0),
    };
    assert_eq!(set.normalize(4.2), Some(5.0));
    assert_eq!(set.normalize(-50.0), Some(1.0));
    assert_eq!(
        set.normalize(50.0),
        Some(9.0),
        "the last step inside the range"
    );
    let plain = DynamicValueSet::Range {
        min: 1.0,
        max: 10.0,
        increment: None,
    };
    assert_eq!(plain.normalize(50.0), Some(10.0));
}

#[test]
fn a_value_that_is_not_finite_or_a_set_that_cannot_hold_one_gives_nothing() {
    assert_eq!(DynamicValueSet::Any.normalize(f64::NAN), None);
    assert_eq!(DynamicValueSet::Any.normalize(f64::INFINITY), None);
    for bad in [
        DynamicValueSet::List(vec![]),
        DynamicValueSet::List(vec![2.0, 2.0]),
        DynamicValueSet::List(vec![f64::NAN]),
        DynamicValueSet::Range {
            min: 3.0,
            max: 1.0,
            increment: None,
        },
        DynamicValueSet::Range {
            min: 0.0,
            max: 1.0,
            increment: Some(0.0),
        },
    ] {
        assert!(!bad.is_valid());
        assert_eq!(bad.normalize(1.0), None);
    }
    let fixed = DynamicValueSet::Range {
        min: 2.0,
        max: 2.0,
        increment: None,
    };
    assert_eq!(fixed.normalize(9.0), Some(2.0));
}
