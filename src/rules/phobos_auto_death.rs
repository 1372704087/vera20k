//! Phobos `AutoDeath.*` (`Ext/TechnoType/Body.cpp:817-836`).
//!
//! `AutoDeath.Behavior=` is a `Nullable<AutoDeathBehavior>`: the parser at
//! `Utilities/TemplateDef.h:1063-1083` matches `kill`/`sell`/`vanish` with
//! `_strcmpi`, and an absent key, a blank value or an unknown name leaves the
//! `Nullable` unset. `Ext/Techno/Body.Update.cpp:142` then turns the whole
//! mechanism off, so `AfterDelay` alone does nothing.
//!
//! ## Scope
//!
//! This reader covers the timed arm only. The other triggers
//! (`OnAmmoDepletion`, `OnOwnerChange.*`, `TechnosDontExist.*`, `TechnosExist.*`,
//! `PlayerPowerState`, `PlayerMoney.*`), the `VanishAnimation` list and the
//! `AllowLimboed` gate (type key plus the `[CombatDamage]` global at
//! `Ext/Rules/Body.cpp:629-632`) are unported; they keep their gap entries in
//! the chain plan so a later chain reads each from the layer native does.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependencies on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

/// `AutoDeathBehavior` (`Utilities/Enum.h:196`), in declaration order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum AutoDeathBehavior {
    /// `0`: the default death option. Deals the object's own Health as damage
    /// with no attacker (`Ext/Techno/Body.Update.cpp:820`).
    #[default]
    Kill,
    /// `1`: play a vanish animation and erase the object without a death.
    Vanish,
    /// `2`: sell the building, or kill it when it has no buildup.
    Sell,
}

impl AutoDeathBehavior {
    /// The `_strcmpi` names from `Utilities/TemplateDef.h:1069-1071`.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("kill") {
            Some(Self::Kill)
        } else if raw.eq_ignore_ascii_case("sell") {
            Some(Self::Sell)
        } else if raw.eq_ignore_ascii_case("vanish") {
            Some(Self::Vanish)
        } else {
            None
        }
    }
}

/// The type-level `AutoDeath.*` keys this chain reads.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct AutoDeathTypeSettings {
    /// `AutoDeath.Behavior=`: `None` disables the mechanism.
    pub behavior: Option<AutoDeathBehavior>,
    /// `AutoDeath.AfterDelay=`, `Valueable<int>` default `0` — a non-positive
    /// value never starts the countdown (`Ext/Techno/Body.Update.cpp:173`).
    pub after_delay: i32,
}

impl AutoDeathTypeSettings {
    pub fn from_section(section: &IniSection) -> Self {
        Self {
            behavior: section
                .read_name("AutoDeath.Behavior", 2048)
                .and_then(|raw| AutoDeathBehavior::parse(&raw)),
            after_delay: section.read_int("AutoDeath.AfterDelay", 0),
        }
    }
}

#[cfg(test)]
#[path = "phobos_auto_death_tests.rs"]
mod tests;
