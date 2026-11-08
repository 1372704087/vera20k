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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct AutoDeathTypeSettings {
    /// `AutoDeath.Behavior=`: `None` disables the mechanism.
    pub behavior: Option<AutoDeathBehavior>,
    /// `AutoDeath.AfterDelay=`, `Valueable<int>` default `0` — a non-positive
    /// value never starts the countdown (`Ext/Techno/Body.Update.cpp:173`).
    pub after_delay: i32,
    /// `AutoDeath.OnAmmoDepletion=`, `Valueable<bool>` default `no`: die once
    /// the type's `Ammo=` capacity is set and the count reaches zero or below
    /// (`:166`).
    pub on_ammo_depletion: bool,
    /// `AutoDeath.PlayerPowerState=`, `Valueable<PowerStatus>` default `none`
    /// (`:210-221`).
    pub player_power_state: AutoDeathPowerStatus,
    /// `AutoDeath.PlayerMoney.Max=`, `Valueable<int>` default `-1` (off).
    pub player_money_max: i32,
    /// `AutoDeath.PlayerMoney.Min=`, `Valueable<int>` default `-1` (off).
    pub player_money_min: i32,
    /// `AutoDeath.TechnosDontExist=`, default empty: die when the listed types
    /// are absent from the selected houses (`Ext/Techno/Body.Update.cpp:236-245`).
    pub technos_dont_exist: Vec<String>,
    /// `AutoDeath.TechnosDontExist.Any=`, default `no`. Native inverts it
    /// (`existTechnoTypes(..., !Any, ...)`, `:239`), so `no` asks "none of the
    /// listed types exists" and `yes` asks "not all of them exist".
    pub technos_dont_exist_any: bool,
    /// `AutoDeath.TechnosDontExist.AllowLimboed=`: `None` falls back to the
    /// `[CombatDamage]` global.
    pub technos_dont_exist_allow_limboed: Option<bool>,
    /// `AutoDeath.TechnosDontExist.Houses=`, default `owner`.
    pub technos_dont_exist_houses: crate::rules::phobos_shields::AffectedHouse,
    /// `AutoDeath.TechnosExist=`, default empty: die when the listed types
    /// exist (`:247-255`).
    pub technos_exist: Vec<String>,
    /// `AutoDeath.TechnosExist.Any=`, default `yes`.
    pub technos_exist_any: bool,
    /// `AutoDeath.TechnosExist.AllowLimboed=`, same fallback.
    pub technos_exist_allow_limboed: Option<bool>,
    /// `AutoDeath.TechnosExist.Houses=`, default `owner`.
    pub technos_exist_houses: crate::rules::phobos_shields::AffectedHouse,
}

/// `PowerStatus` (`Utilities/Enum.h:203-208`). Its parser
/// (`Utilities/TemplateDef.h:1762-1781`) accepts `consumer` as a synonym for
/// `low` and `normal` for `full`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum AutoDeathPowerStatus {
    #[default]
    None,
    Full,
    Low,
}

impl AutoDeathPowerStatus {
    pub fn parse(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("none") {
            Some(Self::None)
        } else if raw.eq_ignore_ascii_case("consumer") || raw.eq_ignore_ascii_case("low") {
            Some(Self::Low)
        } else if raw.eq_ignore_ascii_case("full") || raw.eq_ignore_ascii_case("normal") {
            Some(Self::Full)
        } else {
            None
        }
    }
}

impl AutoDeathTypeSettings {
    pub fn from_section(section: &IniSection) -> Self {
        Self {
            behavior: section
                .read_name("AutoDeath.Behavior", 2048)
                .and_then(|raw| AutoDeathBehavior::parse(&raw)),
            after_delay: section.read_int("AutoDeath.AfterDelay", 0),
            on_ammo_depletion: section.read_bool("AutoDeath.OnAmmoDepletion", false),
            player_power_state: section
                .read_name("AutoDeath.PlayerPowerState", 2048)
                .and_then(|raw| AutoDeathPowerStatus::parse(&raw))
                .unwrap_or_default(),
            player_money_max: section.read_int("AutoDeath.PlayerMoney.Max", -1),
            player_money_min: section.read_int("AutoDeath.PlayerMoney.Min", -1),
            technos_dont_exist: type_list(section, "AutoDeath.TechnosDontExist"),
            technos_dont_exist_any: section.read_bool("AutoDeath.TechnosDontExist.Any", false),
            technos_dont_exist_allow_limboed: section
                .read_phobos_nullable_bool("AutoDeath.TechnosDontExist.AllowLimboed", None),
            technos_dont_exist_houses: affected_house(section, "AutoDeath.TechnosDontExist.Houses"),
            technos_exist: type_list(section, "AutoDeath.TechnosExist"),
            technos_exist_any: section.read_bool("AutoDeath.TechnosExist.Any", true),
            technos_exist_allow_limboed: section
                .read_phobos_nullable_bool("AutoDeath.TechnosExist.AllowLimboed", None),
            technos_exist_houses: affected_house(section, "AutoDeath.TechnosExist.Houses"),
        }
    }
}

fn type_list(section: &IniSection, key: &str) -> Vec<String> {
    section
        .read_list(key, 0x800)
        .map(|tokens| {
            tokens
                .into_iter()
                .map(|token| token.trim().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// `Valueable<AffectedHouse>`: a failed or absent read keeps `Owner`
/// (`Utilities/TemplateDef.h:824..873`).
fn affected_house(section: &IniSection, key: &str) -> crate::rules::phobos_shields::AffectedHouse {
    section
        .read_name(key, 0x100)
        .and_then(|raw| crate::rules::phobos_shields::AffectedHouse::parse(&raw))
        .unwrap_or(crate::rules::phobos_shields::AffectedHouse::OWNER)
}

#[cfg(test)]
#[path = "phobos_auto_death_tests.rs"]
mod tests;
