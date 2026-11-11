//! Phobos `OnlyUseLandSequences`: an infantry type that plays its land
//! sequences even while it is in water.
//!
//! ```ini
//! ; art section of the InfantryType
//! OnlyUseLandSequences=   ; Nullable<bool>, unset   Ext/InfantryType/Body.cpp:32
//!
//! ; [General]
//! OnlyUseLandSequences=   ; Valueable<bool>, no     Ext/Rules/Body.cpp:622
//! ```
//!
//! Declarations `Ext/InfantryType/Body.h:19` and `Ext/Rules/Body.h:520`,
//! constructors `InfantryType/Body.h:44` and `Rules/Body.h:1017`.
//!
//! The type key is read from the **art** INI (`exArtINI, pArtSection`), not
//! the rules INI — the same split VERA's art projection already models for
//! the other `art`-only keys.
//!
//! `InfantryClass_DoAction_Water` (`0x51D7E0`,
//! `Ext/Techno/Hooks.Misc.cpp:883-901`):
//!
//! ```cpp
//! if (pTypeExt->OnlyUseLandSequences.Get(RulesExt::Global()->OnlyUseLandSequences))
//!     return SkipWaterSequences;      // 0x51D842
//! if (sequence == Sequence::Walk || sequence == Sequence::Crawl)
//!     return UseSwim;                 // 0x51D83D
//! else if ((sequence == SecondaryFire || sequence == SecondaryProne)
//!          && pTypeExt->SecondaryFireSequenceLandOnly.Get(Global))
//!     return UseWetAttack;            // 0x51D82F
//! return Continue;                    // 0x51D7EC
//! ```
//!
//! The `SecondaryFireSequenceLandOnly` branch is a separate key and is not
//! owned here.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

/// `OnlyUseLandSequences` on an `InfantryType`'s art section, plus the
/// `[General]` global it falls back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhobosLandSequences {
    /// `OnlyUseLandSequences=`, `Nullable`: `None` keeps `[General]`.
    pub only_use_land_sequences: Option<bool>,
}

impl PhobosLandSequences {
    pub fn from_section(section: &IniSection) -> Self {
        Self {
            only_use_land_sequences: section
                .read_phobos_nullable_bool("OnlyUseLandSequences", None),
        }
    }

    /// `OnlyUseLandSequences.Get(RulesExt::Global()->OnlyUseLandSequences)`
    /// (`Ext/Techno/Hooks.Misc.cpp:890`).
    pub fn only_land(self, global: bool) -> bool {
        self.only_use_land_sequences.unwrap_or(global)
    }
}

#[cfg(test)]
#[path = "phobos_land_sequences_tests.rs"]
mod tests;
