//! Phobos `RulesExt::LoadFromINI` globals, read from `[CombatDamage]` on every
//! rules pass (`Ext/Rules/Body.cpp:629-632` and the shrapnel block).
//!
//! Phobos reads these in its own extension reader, once per `Process`, with the
//! current field as the default; a later layer changes only the keys it names.
//! `ProcessedRulesLayers::ini()` is already that merged state, so one read here
//! is equivalent.
//!
//! The globals are `Valueable<bool>`; all construct `false`
//! (`Ext/Rules/Body.h:464-465`, `:968-969`) except
//! `AutoDeath.AllowLimboed` (`:1027`) and
//! `AffectsInvokerOnly.IgnoreInvokerState` (`:923`), which construct `true`.
//! The type-level keys are `Nullable<bool>`, and their consumer passes the
//! global through `Valueable<T>::operator const T&`, so an unset type key
//! falls back to that global's value. A `Valueable` global has no unset state
//! and no `none`/`<default>` clearing token; do not give these reads
//! `Nullable` semantics.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependencies on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniFile;

/// `[CombatDamage]` globals this port reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    /// the child weapon's own target eligibility (`:309-342`).
    pub shrapnel_use_weapon_targeting: bool,
    /// `Overload.ParticleSysCount=`, default 5 (`Ext/Rules/Body.h:670`):
    /// the number of spark systems a damaging overload row throws.
    pub overload_particle_sys_count: i32,
    /// `AffectsInvokerOnly.IgnoreInvokerState=`, default **`yes`**
    /// (`Ext/Rules/Body.h:923`): the fallback for a warhead whose own key is
    /// unset when its damage has no invoker.
    pub affects_invoker_only_ignore_invoker_state: bool,
    /// `AutoDeath.OnOwnerChange.IgnoreRevertOnExit=`, default `no`
    /// (`Ext/Rules/Body.cpp:630`).
    pub auto_death_on_owner_change_ignore_revert_on_exit: bool,
    /// `AutoDeath.AllowLimboed=`, **default `yes`**: Phobos constructs it
    /// `true` (`Ext/Rules/Body.h:1027`) and `Ext/Rules/Body.cpp:629` reads it.
    pub auto_death_allow_limboed: bool,
}

impl Default for PhobosGlobals {
    fn default() -> Self {
        Self {
            shrapnel_affects_ground: false,
            shrapnel_affects_buildings: false,
            auto_death_technos_exist_allow_limboed: false,
            auto_death_technos_dont_exist_allow_limboed: false,
            shrapnel_ignore_hit_buildings: false,
            shrapnel_use_weapon_targeting: false,
            overload_particle_sys_count: 5,
            affects_invoker_only_ignore_invoker_state: true,
            auto_death_on_owner_change_ignore_revert_on_exit: false,
            auto_death_allow_limboed: true,
        }
    }
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
        overload_particle_sys_count: section.read_int(
            "Overload.ParticleSysCount",
            defaults.overload_particle_sys_count,
        ),
        affects_invoker_only_ignore_invoker_state: section.read_bool(
            "AffectsInvokerOnly.IgnoreInvokerState",
            defaults.affects_invoker_only_ignore_invoker_state,
        ),
        auto_death_on_owner_change_ignore_revert_on_exit: section.read_bool(
            "AutoDeath.OnOwnerChange.IgnoreRevertOnExit",
            defaults.auto_death_on_owner_change_ignore_revert_on_exit,
        ),
        auto_death_allow_limboed: section
            .read_bool("AutoDeath.AllowLimboed", defaults.auto_death_allow_limboed),
    }
}

#[cfg(test)]
#[path = "phobos_globals_tests.rs"]
mod tests;
