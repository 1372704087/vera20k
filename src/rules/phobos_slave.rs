//! Phobos InfantryType slave keys: `SlavesFreeSound=` and
//! `Slaved.OwnerWhenMasterKilled=` (`Ext/InfantryType/Body.cpp:18,31`,
//! declarations `Body.h:14,27`, constructors `Body.h:38-40`).
//!
//! Consumers: `SlaveManagerClass::Killed` (`0x6B0B9C`/`0x6B0BA4`,
//! `Ext/Techno/Hooks.Misc.cpp:31-74`) decides each freed slave's owner from
//! the slave's own type; `FreeSlaves` (`0x6B0C2C`, `:8-21`) plays the first
//! freed slave's type sound over `[AudioVisual] SlavesFreeSound=`.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

/// Phobos `SlaveChangeOwnerType` (`Utilities/Enum.h:178-184`): what happens
/// to one freed slave when its master dies.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Default,
)]
pub enum SlaveChangeOwnerType {
    /// `Killer` (0, the constructor value): the killer's House, else the
    /// explicit House, else the neutral House, else death.
    #[default]
    Killer,
    /// `Master` (1): the slave keeps its current owner.
    Master,
    /// `Suicide` (2): the slave is damaged to death with `C4Warhead`.
    Suicide,
    /// `Neutral` (4): the neutral House when one exists, else `Killer`.
    Neutral,
}

impl SlaveChangeOwnerType {
    fn parse(raw: &str) -> Option<Self> {
        Some(match raw.to_ascii_lowercase().as_str() {
            "killer" => Self::Killer,
            "master" => Self::Master,
            "suicide" => Self::Suicide,
            "neutral" => Self::Neutral,
            _ => return None,
        })
    }
}

/// The InfantryType's slave keys, read by the ART-independent type pass.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PhobosSlaved {
    pub owner_when_master_killed: SlaveChangeOwnerType,
    /// `SlavesFreeSound=` (`Nullable<VocClass>`): `None` inherits the fixed
    /// `[AudioVisual] SlavesFreeSound=`.
    pub slaves_free_sound: Option<String>,
}

impl PhobosSlaved {
    pub fn from_section(section: &IniSection) -> Self {
        Self {
            owner_when_master_killed: section.read_string_with(
                "Slaved.OwnerWhenMasterKilled",
                0x40,
                SlaveChangeOwnerType::Killer,
                |current, raw| SlaveChangeOwnerType::parse(raw).unwrap_or(current),
            ),
            slaves_free_sound: section
                .read_phobos_nullable_with("SlavesFreeSound", None, |raw| Some(raw.to_owned())),
        }
    }

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
#[path = "phobos_slave_tests.rs"]
mod tests;
