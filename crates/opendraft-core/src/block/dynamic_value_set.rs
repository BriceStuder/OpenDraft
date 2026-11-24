//! The values a parameter may take: whether a set is usable, and the member
//! nearest a value.

use crate::block::DynamicValueSet;

impl DynamicValueSet {
    /// Whether the set can hold a value: a list is non-empty, finite and
    /// strictly increasing; a range is finite with `min <= max` and a
    /// positive increment, if it has one.
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Any => true,
            Self::List(xs) => {
                !xs.is_empty()
                    && xs.iter().all(|x| x.is_finite())
                    && xs.windows(2).all(|w| w[0] < w[1])
            }
            Self::Range {
                min,
                max,
                increment,
            } => {
                min.is_finite()
                    && max.is_finite()
                    && min <= max
                    && increment.is_none_or(|i| i.is_finite() && i > 0.0)
            }
        }
    }

    /// The value of the set nearest `value`, or `None` when `value` is not
    /// finite or the set is invalid. A list takes the nearest entry (the
    /// lower on a tie); a range clamps, then rounds to its increment counted
    /// from `min` (never past `max`).
    pub fn normalize(&self, value: f64) -> Option<f64> {
        if !value.is_finite() || !self.is_valid() {
            return None;
        }
        Some(match self {
            Self::Any => value,
            Self::List(xs) => {
                let mut best = xs[0];
                for &x in &xs[1..] {
                    if (x - value).abs() < (best - value).abs() {
                        best = x;
                    }
                }
                best
            }
            Self::Range {
                min,
                max,
                increment,
            } => {
                let v = value.clamp(*min, *max);
                match increment {
                    Some(i) => {
                        let mut steps = ((v - min) / i).round();
                        // Rounding up past the end lands on no step: take
                        // the last one inside the range.
                        if min + steps * i > *max {
                            steps -= 1.0;
                        }
                        (min + steps * i).clamp(*min, *max)
                    }
                    None => v,
                }
            }
        })
    }
}
