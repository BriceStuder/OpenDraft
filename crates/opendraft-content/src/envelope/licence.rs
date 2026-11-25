//! The allowed licences (specification 8.4): permissive only at launch.
//! Share-alike licences are excluded until there is an informed answer.

/// One SPDX identifier from the allowed list, or the personal package's own
/// marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Licence {
    Cc0,
    CcBy4,
    Mit,
    Apache2,
    Bsd2,
    Bsd3,
    /// The user's own blocks (`local/personal`): never published, so no
    /// licence is granted. Valid only in a package by the author `local`.
    Personal,
}

impl Licence {
    /// The licences a published package may use.
    pub const PUBLISHABLE: [Licence; 6] = [
        Licence::Cc0,
        Licence::CcBy4,
        Licence::Mit,
        Licence::Apache2,
        Licence::Bsd2,
        Licence::Bsd3,
    ];

    pub fn spdx(self) -> &'static str {
        match self {
            Licence::Cc0 => "CC0-1.0",
            Licence::CcBy4 => "CC-BY-4.0",
            Licence::Mit => "MIT",
            Licence::Apache2 => "Apache-2.0",
            Licence::Bsd2 => "BSD-2-Clause",
            Licence::Bsd3 => "BSD-3-Clause",
            Licence::Personal => "LicenseRef-OpenDraft-Personal",
        }
    }

    pub fn from_spdx(id: &str) -> Option<Licence> {
        Licence::PUBLISHABLE
            .into_iter()
            .chain([Licence::Personal])
            .find(|l| l.spdx() == id)
    }

    /// MIT, BSD and Apache-2.0 need the copyright notice to travel with
    /// copies (specification 8.4); the others ask for attribution or nothing.
    pub fn needs_notice(self) -> bool {
        matches!(
            self,
            Licence::Mit | Licence::Apache2 | Licence::Bsd2 | Licence::Bsd3
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_licence_round_trips() {
        for l in Licence::PUBLISHABLE.into_iter().chain([Licence::Personal]) {
            assert_eq!(Licence::from_spdx(l.spdx()), Some(l));
        }
    }

    #[test]
    fn share_alike_and_free_text_are_refused() {
        for bad in [
            "GPL-3.0-or-later",
            "CC-BY-SA-4.0",
            "mit",
            "MIT OR Apache-2.0",
            "",
        ] {
            assert_eq!(Licence::from_spdx(bad), None, "{bad:?}");
        }
    }
}
