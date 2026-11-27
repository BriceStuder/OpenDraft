//! `MAJOR.MINOR.PATCH` and nothing else: no pre-release or build suffix, no
//! leading zeros, no signs (specification 5.1).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Version {
            major,
            minor,
            patch,
        }
    }

    /// Parses exactly three dot-separated decimal numbers.
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.split('.');
        let major = number(parts.next()?)?;
        let minor = number(parts.next()?)?;
        let patch = number(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        Some(Version::new(major, minor, patch))
    }
}

fn number(part: &str) -> Option<u32> {
    let digits = !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    if !digits || (part.len() > 1 && part.starts_with('0')) {
        return None;
    }
    part.parse().ok()
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints() {
        let v = Version::parse("2.10.0").expect("valid");
        assert_eq!(v, Version::new(2, 10, 0));
        assert_eq!(v.to_string(), "2.10.0");
    }

    #[test]
    fn refuses_everything_else() {
        for bad in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "1.2.3-beta",
            "1.2.3+build",
            "01.2.3",
            "1.02.3",
            "1.2.-3",
            "+1.2.3",
            "1.2.x",
            " 1.2.3",
            "1..3",
            "4294967296.0.0",
            "v1.2.3",
        ] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn orders_numerically_not_as_text() {
        assert!(Version::new(2, 10, 0) > Version::new(2, 9, 9));
        assert!(Version::parse("0.0.1") < Version::parse("0.1.0"));
    }
}
