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
//! the rules INI. VERA reads it at the per-pass fixed-ART owner
//! (`InfantryArt::read_pass`, `src/rules/object_type.rs`, the
//! `0x005246A1..0x0052472D` reads followed by the `0x52473F` hook) and
//! projects it onto `ObjectType::phobos_land_sequences`; a rules section
//! never supplies it.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct PhobosLandSequences {
    /// `OnlyUseLandSequences=`, `Nullable`: `None` keeps `[General]`.
    pub only_use_land_sequences: Option<bool>,
}

impl PhobosLandSequences {
    /// One fresh read from an ART Image section
    /// (`Ext/InfantryType/Body.cpp:32`).
    pub fn from_section(section: &IniSection) -> Self {
        let mut this = Self::default();
        this.read_pass(section);
        this
    }

    /// One reached pass's `Nullable` read with the current value as the
    /// default, from the type Image's ART section. The per-pass owner calls
    /// this from `InfantryArt::read_pass`.
    pub fn read_pass(&mut self, section: &IniSection) {
        self.only_use_land_sequences =
            section.read_phobos_nullable_bool("OnlyUseLandSequences", self.only_use_land_sequences);
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
