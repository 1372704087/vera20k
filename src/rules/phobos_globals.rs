//! Phobos `RulesExt::LoadFromINI` globals, read from `[CombatDamage]`,
//! `[AudioVisual]` and `[General]` on every rules pass
//! (`Ext/Rules/Body.cpp:230-232`, `:291-304`, `:470-471`, `:629-632` and the
//! shrapnel block).
//!
//! Phobos reads these in its own extension reader, once per `Process`, with the
//! current field as the default; a later layer changes only the keys it names.
//! `ProcessedRulesLayers::ini()` is already that merged state, so one read here
//! is equivalent.
//!
//! The globals are `Valueable`; the booleans construct `false`
//! (`Ext/Rules/Body.h:464-465`, `:968-969`, `:729`, `:731`) except
//! `AutoDeath.AllowLimboed` (`:1027`),
//! `AffectsInvokerOnly.IgnoreInvokerState` (`:923`),
//! `DrainMoneyDisplay.OnTarget.UseDisplayIncome` (`:732`) and both
//! `Explodes.*` globals (`:885-886`), which construct `true`; the
//! `AffectedHouse` globals construct `All` (`:728`, `:730`).
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

/// A `Valueable<bool>` global constructed `true`, for serde defaults.
fn true_bool() -> bool {
    true
}

/// `[CombatDamage]` globals this port reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    /// `[General] Convert.ResetMindControl=`, default `no`
    /// (`Ext/Rules/Body.cpp:222`, constructor `Ext/Rules/Body.h:666`): the
    /// fallback for a type's `Convert.ResetMindControl` when it is unset.
    pub convert_reset_mind_control: bool,
    /// `[General] Explodes.KillPassengers=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:470`, constructor `Ext/Rules/Body.h:885`): the
    /// fallback for a type's key; `no` leaves a destroyed object's passengers
    /// out of the death arm's kill.
    #[serde(default = "true_bool")]
    pub explodes_kill_passengers: bool,
    /// `[General] Explodes.DuringBuildup=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:471`, constructor `Ext/Rules/Body.h:886`): the
    /// fallback for a BuildingType's key; `no` makes a building destroyed
    /// while under Construction or Selling behave as if `Explodes=` were
    /// false.
    #[serde(default = "true_bool")]
    pub explodes_during_buildup: bool,
    /// `[General] NotHuman.RandomDeathSequence=`, default `no`
    /// (`Ext/Rules/Body.cpp:621`, constructor `Ext/Rules/Body.h:1016`): the
    /// fallback for an InfantryType's key.
    pub not_human_random_death_sequence: bool,
    /// `[General] KeepRange.AllowAI=`, default `no` (`Ext/Rules/Body.cpp:269`,
    /// constructor `Ext/Rules/Body.h:187`): the fallback for a weapon's key.
    pub keep_range_allow_ai: bool,
    /// `[General] KeepRange.AllowPlayer=`, default `no` (`:270`, `Body.h:188`).
    pub keep_range_allow_player: bool,
    /// `[General] KeepRange.EarlyStopFrame=`, default 0 (`:271`, `Body.h:189`).
    pub keep_range_early_stop: i32,
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
    /// `[CombatDamage] DriverKilled.KeptPassengers=`, default `no`
    /// (`Ext/Rules/Body.cpp:533`, `Body.h:949`): the fallback for a vehicle
    /// type whose own key is unset; `yes` keeps the passengers of a
    /// driver-killed vehicle owned by a neutral house aboard
    /// (`Ext/Techno/Body.cpp:930-1017`).
    #[serde(default)]
    pub driver_killed_kept_passengers: bool,
    /// `[CombatDamage] DriverKilled.KillPassengers=`, default `no`
    /// (`Ext/Rules/Body.cpp:534`, `Body.h:950`): the fallback that destroys
    /// each departing passenger instead of ejecting it.
    #[serde(default)]
    pub driver_killed_kill_passengers: bool,
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
    /// `[General] AttackMove.Aggressive=`, default `no`
    /// (`Ext/Rules/Body.cpp:377`, `Body.h:282,800`): fallback for an unset
    /// `TechnoType` `AttackMove.Aggressive`; `yes` lets an attack-mover treat
    /// an unarmed enemy building as a legal auto-target.
    #[serde(default)]
    pub attack_move_aggressive: bool,
    /// `[General] AttackMove.UpdateTarget=`, default `no`
    /// (`Ext/Rules/Body.cpp:378`, `Body.h:283,801`): fallback for an unset
    /// `TechnoType` `AttackMove.UpdateTarget`; `yes` lets the arrival handler
    /// drop an unusable held target and rescan.
    #[serde(default)]
    pub attack_move_update_target: bool,
    /// `[General] AttackMove.IgnoreWeaponCheck=`, default `no`
    /// (`Ext/Rules/Body.cpp:423`, `Body.h:341,853`): global only — drops the
    /// native "`Primary=` must name a weapon" test from
    /// `TechnoTypeClass::Can_Attack_Move @ 0x00711E90`.
    #[serde(default)]
    pub attack_move_ignore_weapon_check: bool,
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
    /// `[General] Promote.IncludeSpawns=`, default `no`
    /// (`Ext/Rules/Body.cpp:212`, `Body.h:119,656`): fallback for an unset
    /// type-level `Promote.IncludeSpawns`.
    #[serde(default)]
    pub promote_include_spawns: bool,
    /// `[AudioVisual] Promote.VeteranAnimation=`,
    /// `ValueableVector<AnimTypeClass*>`, constructed empty
    /// (`Ext/Rules/Body.cpp:316`, `Body.h:239,748`): fallback for a type with
    /// no veteran list.
    #[serde(default)]
    pub promote_veteran_animation: Vec<String>,
    /// `[AudioVisual] Promote.EliteAnimation=`, constructed empty
    /// (`Ext/Rules/Body.cpp:317`, `Body.h:240,749`).
    #[serde(default)]
    pub promote_elite_animation: Vec<String>,
    /// `[General] OpenTopped.IgnoreRangefinding=`, default `no`
    /// (`Ext/Rules/Body.cpp:455`, `Body.h:870`): the per-type fallback for
    /// skipping the passenger-min weapon-range clamp in `WeaponRange`.
    #[serde(default)]
    pub open_topped_ignore_rangefinding: bool,
    /// `[General] OpenTopped.AllowFiringIfDeactivated=`, default **`yes`**
    /// (`Body.cpp:456`, `Body.h:871`).
    #[serde(default = "true_bool")]
    pub open_topped_allow_firing_if_deactivated: bool,
    /// `[General] OpenTopped.AllowFiringIfAttackedByLocomotor=`, default
    /// **`yes`** (`Body.cpp:457`, `Body.h:872`).
    #[serde(default = "true_bool")]
    pub open_topped_allow_firing_if_attacked_by_locomotor: bool,
    /// `[General] OpenTopped.ShareTransportTarget=`, default **`yes`**
    /// (`Body.cpp:458`, `Body.h:873`).
    #[serde(default = "true_bool")]
    pub open_topped_share_transport_target: bool,
    /// `[General] OpenTopped.UseTransportRangeModifiers=`, default `no`
    /// (`Body.cpp:459`, `Body.h:874`). Also read into `AttachEffectGlobals`.
    #[serde(default)]
    pub open_topped_use_transport_range_modifiers: bool,
    /// `[General] OpenTopped.CheckTransportDisableWeapons=`, default `no`
    /// (`Body.cpp:460`, `Body.h:875`). Also read into `AttachEffectGlobals`.
    #[serde(default)]
    pub open_topped_check_transport_disable_weapons: bool,
    /// `[General] OpenTopped.DecloakToFire=`, default `no`
    /// (`Body.cpp:461`, `Body.h:876`). The docs at
    /// `docs/New-or-Enhanced-Logics.md:1912` claim `yes`; the constructor is
    /// `{ false }`, so the source wins.
    #[serde(default)]
    pub open_topped_decloak_to_fire: bool,
    /// `[General] OpenTopped.FireWhileMoving=`, default **`yes`**
    /// (`Body.cpp:462`, `Body.h:877`).
    #[serde(default = "true_bool")]
    pub open_topped_fire_while_moving: bool,
    /// `[General] NoRearm.UnderEMP=`, default `no` (`Ext/Rules/Body.cpp:383`,
    /// constructor `Ext/Rules/Body.h:804`): the fallback for an unset type key;
    /// `yes` holds a running rearm timer while the techno is EMP'd or
    /// deactivated.
    #[serde(default)]
    pub no_rearm_under_emp: bool,
    /// `[General] NoRearm.Temporal=`, default `no` (`Ext/Rules/Body.cpp:384`,
    /// constructor `Body.h:805`): the same hold while a Temporal weapon warps
    /// the techno.
    #[serde(default)]
    pub no_rearm_temporal: bool,
    /// `[General] NoReload.UnderEMP=`, default `no` (`Ext/Rules/Body.cpp:385`,
    /// constructor `Body.h:806`): the ammo-reload counterpart of
    /// `no_rearm_under_emp`.
    #[serde(default)]
    pub no_reload_under_emp: bool,
    /// `[General] NoReload.Temporal=`, default `no` (`Ext/Rules/Body.cpp:386`,
    /// constructor `Body.h:807`): the ammo-reload counterpart of
    /// `no_rearm_temporal`.
    #[serde(default)]
    pub no_reload_temporal: bool,
    /// `[General] Passengers.SyncOwner=`, default `no`
    /// (`Ext/Rules/Body.cpp:467`, constructor `Ext/Rules/Body.h:882`): the
    /// fallback for an unset transport-type key; `yes` re-houses the cargo
    /// when the transport's house changes.
    #[serde(default)]
    pub passengers_sync_owner: bool,
    /// `[General] Passengers.SyncOwner.RevertOnExit=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:468`, constructor `Body.h:883`): the fallback for
    /// an unset transport-type key; `yes` puts an exiting cargo member back
    /// on the house it held when it boarded.
    #[serde(default = "true_bool")]
    pub passengers_sync_owner_revert_on_exit: bool,
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
            convert_reset_mind_control: false,
            explodes_kill_passengers: true,
            explodes_during_buildup: true,
            not_human_random_death_sequence: false,
            keep_range_allow_ai: false,
            keep_range_allow_player: false,
            keep_range_early_stop: 0,
            drain_money_display: false,
            drain_money_display_houses: AffectedHouse::ALL,
            drain_money_display_on_target: false,
            drain_money_display_on_target_use_display_income: true,
            display_income_houses: AffectedHouse::ALL,
            force_weapon_in_range_techno_only: true,
            force_weapon_in_range_apply_range_modifiers: false,
            force_aa_weapon_in_range_apply_range_modifiers: false,
            penetrates_transport_level: 10,
            driver_killed_kept_passengers: false,
            driver_killed_kill_passengers: false,
            build_limit_group_content_if_any_match: false,
            build_limit_group_not_buildable_if_queue_match: false,
            attack_move_aggressive: false,
            attack_move_update_target: false,
            attack_move_ignore_weapon_check: false,
            spawner_attack_immediately: false,
            spawner_use_turret_facing: false,
            spawner_recycle_range: -1,
            spawner_recycle_on_turret: false,
            promote_include_spawns: false,
            promote_veteran_animation: Vec::new(),
            promote_elite_animation: Vec::new(),
            open_topped_ignore_rangefinding: false,
            open_topped_allow_firing_if_deactivated: true,
            open_topped_allow_firing_if_attacked_by_locomotor: true,
            open_topped_share_transport_target: true,
            open_topped_use_transport_range_modifiers: false,
            open_topped_check_transport_disable_weapons: false,
            open_topped_decloak_to_fire: false,
            open_topped_fire_while_moving: true,
            no_rearm_under_emp: false,
            no_rearm_temporal: false,
            no_reload_under_emp: false,
            no_reload_temporal: false,
            passengers_sync_owner: false,
            passengers_sync_owner_revert_on_exit: true,
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
        // `[AudioVisual] Promote.VeteranAnimation=` / `.EliteAnimation=`
        // (`Ext/Rules/Body.cpp:316-317`): `ValueableVector<AnimTypeClass*>`
        // reads, so an absent or empty key keeps the list and any other value
        // replaces it. Names stay until a consumer resolves them against the
        // AnimType registry (`has_anim_type`).
        let keep = |name: &str| Some(name.to_owned());
        globals.promote_veteran_animation =
            crate::rules::native_processing::read_valueable_type_vector(
                audio_visual,
                "Promote.VeteranAnimation",
                &globals.promote_veteran_animation,
                keep,
            );
        globals.promote_elite_animation =
            crate::rules::native_processing::read_valueable_type_vector(
                audio_visual,
                "Promote.EliteAnimation",
                &globals.promote_elite_animation,
                keep,
            );
        // `[General]` force-weapon defaults (`Ext/Rules/Body.cpp:230-232`);
        // the three sections share no key, so their order does not matter.
        let general = ini.section_or_empty("General");
        // `Ext/Rules/Body.cpp:222`: a `[General]` read, not CombatDamage.
        globals.convert_reset_mind_control = general.read_bool(
            "Convert.ResetMindControl",
            globals.convert_reset_mind_control,
        );
        // `Ext/Rules/Body.cpp:470-471`, both constructed `true`.
        globals.explodes_kill_passengers =
            general.read_bool("Explodes.KillPassengers", globals.explodes_kill_passengers);
        globals.explodes_during_buildup =
            general.read_bool("Explodes.DuringBuildup", globals.explodes_during_buildup);
        globals.force_weapon_in_range_techno_only = general.read_bool(
            "ForceWeapon.InRange.TechnoOnly",
            globals.force_weapon_in_range_techno_only,
        );
        globals.not_human_random_death_sequence = general.read_bool(
            "NotHuman.RandomDeathSequence",
            globals.not_human_random_death_sequence,
        );
        globals.keep_range_allow_ai =
            general.read_bool("KeepRange.AllowAI", globals.keep_range_allow_ai);
        globals.keep_range_allow_player =
            general.read_bool("KeepRange.AllowPlayer", globals.keep_range_allow_player);
        globals.keep_range_early_stop =
            general.read_ares_int("KeepRange.EarlyStopFrame", globals.keep_range_early_stop);
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
        globals.attack_move_aggressive =
            general.read_bool("AttackMove.Aggressive", globals.attack_move_aggressive);
        globals.attack_move_update_target =
            general.read_bool("AttackMove.UpdateTarget", globals.attack_move_update_target);
        globals.attack_move_ignore_weapon_check = general.read_bool(
            "AttackMove.IgnoreWeaponCheck",
            globals.attack_move_ignore_weapon_check,
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
        globals.promote_include_spawns =
            general.read_bool("Promote.IncludeSpawns", globals.promote_include_spawns);
        // The eight `[General] OpenTopped.*` `Valueable<bool>` switches
        // (`Ext/Rules/Body.cpp:455-462`); `Valueable` has no unset state, so
        // these are plain `read_bool` reads from the constructor defaults.
        globals.open_topped_ignore_rangefinding = general.read_bool(
            "OpenTopped.IgnoreRangefinding",
            globals.open_topped_ignore_rangefinding,
        );
        globals.open_topped_allow_firing_if_deactivated = general.read_bool(
            "OpenTopped.AllowFiringIfDeactivated",
            globals.open_topped_allow_firing_if_deactivated,
        );
        globals.open_topped_allow_firing_if_attacked_by_locomotor = general.read_bool(
            "OpenTopped.AllowFiringIfAttackedByLocomotor",
            globals.open_topped_allow_firing_if_attacked_by_locomotor,
        );
        globals.open_topped_share_transport_target = general.read_bool(
            "OpenTopped.ShareTransportTarget",
            globals.open_topped_share_transport_target,
        );
        globals.open_topped_use_transport_range_modifiers = general.read_bool(
            "OpenTopped.UseTransportRangeModifiers",
            globals.open_topped_use_transport_range_modifiers,
        );
        globals.open_topped_check_transport_disable_weapons = general.read_bool(
            "OpenTopped.CheckTransportDisableWeapons",
            globals.open_topped_check_transport_disable_weapons,
        );
        globals.open_topped_decloak_to_fire = general.read_bool(
            "OpenTopped.DecloakToFire",
            globals.open_topped_decloak_to_fire,
        );
        globals.open_topped_fire_while_moving = general.read_bool(
            "OpenTopped.FireWhileMoving",
            globals.open_topped_fire_while_moving,
        );
        // The four `NoRearm.*` / `NoReload.*` `Valueable<bool>` globals
        // (`Ext/Rules/Body.cpp:383-386`); `Valueable` has no unset state, so
        // these are plain `read_bool` reads from the constructor defaults.
        globals.no_rearm_under_emp =
            general.read_bool("NoRearm.UnderEMP", globals.no_rearm_under_emp);
        globals.no_rearm_temporal =
            general.read_bool("NoRearm.Temporal", globals.no_rearm_temporal);
        globals.no_reload_under_emp =
            general.read_bool("NoReload.UnderEMP", globals.no_reload_under_emp);
        globals.no_reload_temporal =
            general.read_bool("NoReload.Temporal", globals.no_reload_temporal);
        // The two `Passengers.SyncOwner` `Valueable<bool>` globals
        // (`Ext/Rules/Body.cpp:467-468`); `Valueable` has no unset state, so
        // these are plain `read_bool` reads from the constructor defaults.
        globals.passengers_sync_owner =
            general.read_bool("Passengers.SyncOwner", globals.passengers_sync_owner);
        globals.passengers_sync_owner_revert_on_exit = general.read_bool(
            "Passengers.SyncOwner.RevertOnExit",
            globals.passengers_sync_owner_revert_on_exit,
        );
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
        driver_killed_kept_passengers: section.read_bool(
            "DriverKilled.KeptPassengers",
            defaults.driver_killed_kept_passengers,
        ),
        driver_killed_kill_passengers: section.read_bool(
            "DriverKilled.KillPassengers",
            defaults.driver_killed_kill_passengers,
        ),
        // The `[AudioVisual]` block belongs to `from_ini`; a `CombatDamage`
        // section leaves these at their constructor values.
        ..defaults
    }
}

#[cfg(test)]
#[path = "phobos_globals_tests.rs"]
mod tests;
