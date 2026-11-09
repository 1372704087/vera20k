//! Phobos `RulesExt::LoadFromINI` globals, read from `[CombatDamage]`,
//! `[AudioVisual]` and `[General]` on every rules pass
//! (`Ext/Rules/Body.cpp:230-232`, `:291-304`, `:629-632` and the shrapnel
//! block).
//!
//! Phobos reads these in its own extension reader, once per `Process`, with the
//! current field as the default; a later layer changes only the keys it names.
//! `ProcessedRulesLayers::ini()` is already that merged state, so one read here
//! is equivalent.
//!
//! The globals are `Valueable`; the booleans construct `false`
//! (`Ext/Rules/Body.h:464-465`, `:968-969`, `:729`, `:731`) except
//! `AutoDeath.AllowLimboed` (`:1027`),
//! `AffectsInvokerOnly.IgnoreInvokerState` (`:923`) and
//! `DrainMoneyDisplay.OnTarget.UseDisplayIncome` (`:732`), which construct
//! `true`; the `AffectedHouse` globals construct `All` (`:728`, `:730`).
//! The type-level keys are `Nullable`, and their consumer passes the
//! global through `Valueable<T>::operator const T&`, so an unset type key
//! falls back to that global's value. A `Valueable` global has no unset state
//! and no `none`/`<default>` clearing token; do not give these reads
//! `Nullable` semantics.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependencies on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniFile;
use crate::rules::phobos_shields::AffectedHouse;

/// `Leptons(-1)`'s constructor value for saves that predate the field.
fn minus_one_i32() -> i32 {
    -1
}

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
    /// `[AudioVisual] DrainMoneyDisplay=`, default `no`: the drainer-side
    /// flying credit label (`Ext/Rules/Body.cpp:301`, consumed at
    /// `Ext/Techno/Hooks.cpp:199`).
    pub drain_money_display: bool,
    /// `[AudioVisual] DrainMoneyDisplay.Houses=`, default `All`
    /// (`Ext/Rules/Body.h:730`): the label's House filter.
    pub drain_money_display_houses: AffectedHouse,
    /// `[AudioVisual] DrainMoneyDisplay.OnTarget=`, default `no`
    /// (`Ext/Rules/Body.h:731`): the drained object's own label.
    pub drain_money_display_on_target: bool,
    /// `[AudioVisual] DrainMoneyDisplay.OnTarget.UseDisplayIncome=`, default
    /// `yes` (`Ext/Rules/Body.h:732`): the target label takes the building's
    /// `DisplayIncome.Houses`/`.Offset` instead of the drain keys.
    pub drain_money_display_on_target_use_display_income: bool,
    /// `[AudioVisual] DisplayIncome.Houses=`, default `All`
    /// (`Ext/Rules/Body.h:728`): fallback for a building's own
    /// `DisplayIncome.Houses` in the drain target label.
    pub display_income_houses: AffectedHouse,
    /// `[General] ForceWeapon.InRange.TechnoOnly=`, default **`yes`**
    /// (`Ext/Rules/Body.h:674`): the per-type fallback for the in-range
    /// force-weapon gate.
    pub force_weapon_in_range_techno_only: bool,
    /// `[General] ForceWeapon.InRange.ApplyRangeModifiers=`, default `no`
    /// (`Ext/Rules/Body.h:675`).
    pub force_weapon_in_range_apply_range_modifiers: bool,
    /// `[General] ForceAAWeapon.InRange.ApplyRangeModifiers=`, default `no`
    /// (`Ext/Rules/Body.cpp:232`, constructor `Body.h:676`).
    pub force_aa_weapon_in_range_apply_range_modifiers: bool,
    /// `[CombatDamage] PenetratesTransport.Level=`, default 10
    /// (`Ext/Rules/Body.h:945`): the fallback for a transport type whose own
    /// level is unset.
    pub penetrates_transport_level: i32,
    /// `[General] BuildLimitGroup.ContentIfAnyMatch=`, default `no`
    /// (`Ext/Rules/Body.cpp:223`, `Body.h:130,667`): fallback for an unset
    /// type-level `BuildLimitGroup.ContentIfAnyMatch`; `yes` makes any one
    /// group member reaching its limit block the whole group.
    #[serde(default)]
    pub build_limit_group_content_if_any_match: bool,
    /// `[General] BuildLimitGroup.NotBuildableIfQueueMatch=`, default `no`
    /// (`Ext/Rules/Body.cpp:224`, `Body.h:131,668`): fallback for an unset
    /// type-level `BuildLimitGroup.NotBuildableIfQueueMatch`; `yes` blocks
    /// even when only queued (not owned) copies reach the limit.
    #[serde(default)]
    pub build_limit_group_not_buildable_if_queue_match: bool,
    /// `[General] Spawner.AttackImmediately=`, default `no`
    /// (`Ext/Rules/Body.cpp:208`, `Body.h:115,652`): fallback for an unset
    /// type-level key; `yes` sends a fresh child straight at the target.
    #[serde(default)]
    pub spawner_attack_immediately: bool,
    /// `[General] Spawner.UseTurretFacing=`, default `no`
    /// (`Ext/Rules/Body.cpp:209`, `Body.h:116,653`): launch facing uses the
    /// owner's turret instead of its body.
    #[serde(default)]
    pub spawner_use_turret_facing: bool,
    /// `[General] Spawner.RecycleRange=`, `Valueable<Leptons>`, default
    /// `Leptons(-1)` (`Ext/Rules/Body.cpp:210`, `Body.h:117,654`): cells ×
    /// 256; a negative value selects the legacy dock test.
    #[serde(default = "minus_one_i32")]
    pub spawner_recycle_range: i32,
    /// `[General] Spawner.RecycleOnTurret=`, default `no`
    /// (`Ext/Rules/Body.cpp:211`, `Body.h:118,655`): `RecycleCoord` rotates
    /// with the turret.
    #[serde(default)]
    pub spawner_recycle_on_turret: bool,
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
            drain_money_display: false,
            drain_money_display_houses: AffectedHouse::ALL,
            drain_money_display_on_target: false,
            drain_money_display_on_target_use_display_income: true,
            display_income_houses: AffectedHouse::ALL,
            force_weapon_in_range_techno_only: true,
            force_weapon_in_range_apply_range_modifiers: false,
            force_aa_weapon_in_range_apply_range_modifiers: false,
            penetrates_transport_level: 10,
            build_limit_group_content_if_any_match: false,
            build_limit_group_not_buildable_if_queue_match: false,
            spawner_attack_immediately: false,
            spawner_use_turret_facing: false,
            spawner_recycle_range: -1,
            spawner_recycle_on_turret: false,
        }
    }
}

impl PhobosGlobals {
    pub fn from_ini(ini: &IniFile) -> Self {
        let mut globals = section_settings(ini.section_or_empty("CombatDamage"));
        // `RulesExt::LoadFromINIFile` reads this block in the same pass
        // (`Ext/Rules/Body.cpp:291-304`); the two sections share no key, so
        // their relative order does not matter.
        let audio_visual = ini.section_or_empty("AudioVisual");
        globals.drain_money_display =
            audio_visual.read_bool("DrainMoneyDisplay", globals.drain_money_display);
        globals.drain_money_display_houses = crate::rules::phobos_shields::read_affected_house(
            audio_visual,
            "DrainMoneyDisplay.Houses",
            globals.drain_money_display_houses,
        );
        globals.drain_money_display_on_target = audio_visual.read_bool(
            "DrainMoneyDisplay.OnTarget",
            globals.drain_money_display_on_target,
        );
        globals.drain_money_display_on_target_use_display_income = audio_visual.read_bool(
            "DrainMoneyDisplay.OnTarget.UseDisplayIncome",
            globals.drain_money_display_on_target_use_display_income,
        );
        globals.display_income_houses = crate::rules::phobos_shields::read_affected_house(
            audio_visual,
            "DisplayIncome.Houses",
            globals.display_income_houses,
        );
        // `[General]` force-weapon defaults (`Ext/Rules/Body.cpp:230-232`);
        // the three sections share no key, so their order does not matter.
        let general = ini.section_or_empty("General");
        globals.force_weapon_in_range_techno_only = general.read_bool(
            "ForceWeapon.InRange.TechnoOnly",
            globals.force_weapon_in_range_techno_only,
        );
        globals.force_weapon_in_range_apply_range_modifiers = general.read_bool(
            "ForceWeapon.InRange.ApplyRangeModifiers",
            globals.force_weapon_in_range_apply_range_modifiers,
        );
        globals.force_aa_weapon_in_range_apply_range_modifiers = general.read_bool(
            "ForceAAWeapon.InRange.ApplyRangeModifiers",
            globals.force_aa_weapon_in_range_apply_range_modifiers,
        );
        globals.build_limit_group_content_if_any_match = general.read_bool(
            "BuildLimitGroup.ContentIfAnyMatch",
            globals.build_limit_group_content_if_any_match,
        );
        globals.build_limit_group_not_buildable_if_queue_match = general.read_bool(
            "BuildLimitGroup.NotBuildableIfQueueMatch",
            globals.build_limit_group_not_buildable_if_queue_match,
        );
        globals.spawner_attack_immediately = general.read_bool(
            "Spawner.AttackImmediately",
            globals.spawner_attack_immediately,
        );
        globals.spawner_use_turret_facing =
            general.read_bool("Spawner.UseTurretFacing", globals.spawner_use_turret_facing);
        // `Valueable<Leptons>`: cells × 256 (F2I); a failed read keeps -1.
        globals.spawner_recycle_range =
            general.read_phobos_leptons("Spawner.RecycleRange", globals.spawner_recycle_range);
        globals.spawner_recycle_on_turret =
            general.read_bool("Spawner.RecycleOnTurret", globals.spawner_recycle_on_turret);
        globals
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
        penetrates_transport_level: section.read_ares_int(
            "PenetratesTransport.Level",
            defaults.penetrates_transport_level,
        ),
        // The `[AudioVisual]` block belongs to `from_ini`; a `CombatDamage`
        // section leaves these at their constructor values.
        ..defaults
    }
}

#[cfg(test)]
#[path = "phobos_globals_tests.rs"]
mod tests;
