//! Phobos `Slaved.OwnerWhenMasterKilled` on an `InfantryType`: who a slave
//! miner belongs to once its master dies.
//!
//! ```ini
//! Slaved.OwnerWhenMasterKilled=   ; Valueable<SlaveChangeOwnerType>, Killer
//!                                 ; Ext/InfantryType/Body.cpp:18
//! ```
//!
//! Declaration `Ext/InfantryType/Body.h:14`, constructor `Body.h:39`
//! (`SlaveChangeOwnerType::Killer`).
//!
//! `Utilities/Enum.h:178-184`:
//!
//! ```cpp
//! enum class SlaveChangeOwnerType { Killer = 0, Master = 1, Suicide = 2, Neutral = 4 };
//! ```
//!
//! Note `Neutral = 4`, not `3` — the values are what
//! `SlaveManagerClass_Killed_DecideOwner` (`Ext/Techno/Hooks.Misc.cpp:27-72`)
//! switches on, so they are preserved exactly.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

/// `SlaveChangeOwnerType` (`Utilities/Enum.h:178-184`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlaveChangeOwnerType {
    /// `Killer = 0`, the constructor default: the slave goes to the house
    /// that killed its master. When the master was sold or self-destroyed
    /// (no killer), the native fallback is the neutral house, else death.
    #[default]
    Killer,
    /// `Master = 1`: the slave keeps the master's house.
    Master,
    /// `Suicide = 2`: the slave is killed with its master.
    Suicide,
    /// `Neutral = 4`: the slave goes to the neutral house; if there is
    /// none, the `Killer` path runs instead.
    Neutral,
}

impl SlaveChangeOwnerType {
    /// The native discriminant, for tests and traces.
    pub const fn native_value(self) -> i32 {
        match self {
            Self::Killer => 0,
            Self::Master => 1,
            Self::Suicide => 2,
            Self::Neutral => 4,
        }
    }

    /// `Valueable<SlaveChangeOwnerType>::Read` (`detail::read`,
    /// `Utilities/TemplateDef.h`) is a case-insensitive scan of the enum's
    /// own names. An unknown name is a parse failure and leaves the
    /// `Valueable` at its current value — the constructor's `Killer`.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("killer") {
            Some(Self::Killer)
        } else if raw.eq_ignore_ascii_case("master") {
            Some(Self::Master)
        } else if raw.eq_ignore_ascii_case("suicide") {
            Some(Self::Suicide)
        } else if raw.eq_ignore_ascii_case("neutral") {
            Some(Self::Neutral)
        } else {
            None
        }
    }
}

/// `Slaved.OwnerWhenMasterKilled=` — a `Valueable`, so no `<default>` or
/// `<none>` reset token; an absent or unparseable read keeps `Killer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhobosSlaved {
    pub owner_when_master_killed: SlaveChangeOwnerType,
}

impl PhobosSlaved {
    pub fn from_section(section: &IniSection) -> Self {
        let defaults = Self::default();
        Self {
            owner_when_master_killed: section.read_string_with(
                "Slaved.OwnerWhenMasterKilled",
                0x200,
                defaults.owner_when_master_killed,
                |current, raw| SlaveChangeOwnerType::parse(raw).unwrap_or(current),
            ),
        }
    }
}

#[cfg(test)]
#[path = "phobos_slaved_tests.rs"]
mod tests;
