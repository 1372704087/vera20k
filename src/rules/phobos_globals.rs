//! Phobos `RulesExt::LoadFromINI` globals, read from `[CombatDamage]` on every
//! rules pass (`Ext/Rules/Body.cpp:629-632` and the shrapnel block).
//!
//! Phobos reads these in its own extension reader, once per `Process`, with the
//! current field as the default; a later layer changes only the keys it names.
//! `ProcessedRulesLayers::ini()` is already that merged state, so one read here
//! is equivalent.
//!
//! The values are `Nullable<bool>` in Phobos and callers pass the `Nullable`
//! itself to a `Get(T)` fallback, which converts through `Valueable<T>::Value`:
//! an unset global therefore reads as its value default, `false`.
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
    }
}

#[cfg(test)]
#[path = "phobos_globals_tests.rs"]
mod tests;
