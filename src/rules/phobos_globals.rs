//! Phobos `RulesExt::LoadFromINI` globals, read from `[CombatDamage]` on every
//! rules pass (`Ext/Rules/Body.cpp:629-632` and the shrapnel block).
//!
//! Phobos reads these in its own extension reader, once per `Process`, with the
//! current field as the default; a later layer changes only the keys it names.
//! `ProcessedRulesLayers::ini()` is already that merged state, so one read here
//! is equivalent.
//!
//! The globals are `Valueable<bool>` with constructor defaults `false`
//! (`Ext/Rules/Body.h:464-465`, `:968-969`). The type-level keys are
//! `Nullable<bool>`, and their consumer passes the global through
//! `Valueable<T>::operator const T&`, so an unset type key and an unset global
//! both read `false`. A `Valueable` global has no unset state and no
//! `none`/`<default>` clearing token; do not give these reads `Nullable`
//! semantics.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependencies on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniFile;

/// `[CombatDamage]` globals this port reads.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct PhobosGlobals {
    /// `Shrapnel.AffectsGround=`, default `no`: admit shrapnel when the impact
    /// cell holds no ground object (`Ext/Bullet/Hooks.cpp:276`).
    pub shrapnel_affects_ground: bool,
    /// `Shrapnel.AffectsBuildings=`, default `no`: admit shrapnel when the
    /// impact cell's first ground object is a building (`:270`).
    pub shrapnel_affects_buildings: bool,
    /// `AutoDeath.TechnosExist.AllowLimboed=`, default `no`: count limboed
    /// instances in `AutoDeath.TechnosExist=` scans
    /// (`Ext/Rules/Body.cpp:632`, consumed at `Ext/Techno/Body.Update.cpp:250`).
    pub auto_death_technos_exist_allow_limboed: bool,
    /// `AutoDeath.TechnosDontExist.AllowLimboed=`, default `no` (`:631`,
    /// consumed at `:239`).
    pub auto_death_technos_dont_exist_allow_limboed: bool,
    /// `Shrapnel.IgnoreHitBuildings=`, default `no`: a building already hit by
    /// one of this emission's children is skipped for the rest
    /// (`Ext/Bullet/Hooks.cpp:295,303,348`).
    pub shrapnel_ignore_hit_buildings: bool,
    /// `Shrapnel.UseWeaponTargeting=`, default `no`: replace the ally test with
    /// the child weapon's own target eligibility (`:309-342`). Parsed here so
    /// the key has one owner; the consumer is its own chain.
    pub shrapnel_use_weapon_targeting: bool,
}

impl PhobosGlobals {
    pub fn from_ini(ini: &IniFile) -> Self {
        section_settings(ini.section_or_empty("CombatDamage"))
    }
}

fn section_settings(section: &crate::rules::ini_parser::IniSection) -> PhobosGlobals {
    let defaults = PhobosGlobals::default();
    PhobosGlobals {
        shrapnel_affects_ground: section
            .read_bool("Shrapnel.AffectsGround", defaults.shrapnel_affects_ground),
        shrapnel_affects_buildings: section.read_bool(
            "Shrapnel.AffectsBuildings",
            defaults.shrapnel_affects_buildings,
        ),
        auto_death_technos_exist_allow_limboed: section.read_bool(
            "AutoDeath.TechnosExist.AllowLimboed",
            defaults.auto_death_technos_exist_allow_limboed,
        ),
        auto_death_technos_dont_exist_allow_limboed: section.read_bool(
            "AutoDeath.TechnosDontExist.AllowLimboed",
            defaults.auto_death_technos_dont_exist_allow_limboed,
        ),
        shrapnel_ignore_hit_buildings: section.read_bool(
            "Shrapnel.IgnoreHitBuildings",
            defaults.shrapnel_ignore_hit_buildings,
        ),
        shrapnel_use_weapon_targeting: section.read_bool(
            "Shrapnel.UseWeaponTargeting",
            defaults.shrapnel_use_weapon_targeting,
        ),
    }
}

#[cfg(test)]
#[path = "phobos_globals_tests.rs"]
mod tests;
