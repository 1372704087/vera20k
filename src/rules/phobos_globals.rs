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
//! `Shrapnel.ObeyWarheadTriggerConditions` (`:972`),
//! `Vertical.AircraftFix` (`:990`),
//! `CanTarget.IronCurtained` (`Ext/Rules/Body.h:833`),
//! `AutoTarget.IronCurtained` (`:834`),
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

use crate::rules::ini_parser::{IniFile, IniSection};
use crate::rules::phobos_shields::AffectedHouse;
use crate::util::native_x87::NativeF64Bits;

/// `Leptons(-1)`'s constructor value for saves that predate the field.
fn minus_one_i32() -> i32 {
    -1
}

/// A `Valueable<bool>` global constructed `true`, for serde defaults.
fn true_bool() -> bool {
    true
}

/// A `Valueable<float>` global constructed 1.0, for serde defaults.
fn one_f32() -> crate::util::native_x87::NativeF32Bits {
    crate::util::native_x87::NativeF32Bits::ONE
}

/// A `Valueable<double>` global constructed 0.0, for serde defaults.
fn zero_f64() -> NativeF64Bits {
    NativeF64Bits::POSITIVE_ZERO
}

/// `section.read_phobos_double` kept as raw bits so `PhobosGlobals` stays
/// `Eq + Hash`; the parser and its failed-read fallback are unchanged.
fn read_double_bits(section: &IniSection, key: &str, current: NativeF64Bits) -> NativeF64Bits {
    NativeF64Bits::from_bits(
        section
            .read_phobos_double(key, f64::from_bits(current.bits()))
            .to_bits(),
    )
}

/// The `Nullable<double>` counterpart of [`read_double_bits`].
fn read_nullable_double_bits(
    section: &IniSection,
    key: &str,
    current: Option<NativeF64Bits>,
) -> Option<NativeF64Bits> {
    section
        .read_phobos_nullable_double(key, current.map(|value| f64::from_bits(value.bits())))
        .map(|value| NativeF64Bits::from_bits(value.to_bits()))
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
    /// `Shrapnel.ObeyWarheadTriggerConditions=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:557`, constructor `Ext/Rules/Body.h:972`): a
    /// bullet whose type leaves the key unset applies the warhead's health,
    /// veterancy, neutral and invoker conditions before its whole body
    /// (`Ext/Bullet/Hooks.DetonateLogics.cpp:22-29`); failing them keeps only
    /// the impact extras.
    #[serde(default = "true_bool")]
    pub shrapnel_obey_warhead_trigger_conditions: bool,
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
    /// `[General] ArtImageSwap=`, default `no` (`Phobos.INI.cpp:262`):
    /// Phobos' ART `Image=` redirect for TechnoTypes. VERA's body SHP source
    /// already applies that redirect, so the SHP UnitType turret file name
    /// (`RuleSet::unit_turret_shp_id`) tracks the same base and this flag
    /// changes no name; it is read for parity only.
    #[serde(default)]
    pub art_image_swap: bool,
    /// `[General] KeepRange.EarlyStopFrame=`, default 0 (`:271`, `Body.h:189`).
    pub keep_range_early_stop: i32,
    /// `[AudioVisual] Parasite.GrappleAnim=`, `NullableIdx<AnimTypeClass>`
    /// read by declared-name `FindIndex` (`Ext/Rules/Body.cpp:425`): the
    /// fallback for a warhead's key; `SQDG` applies when both are unset.
    pub parasite_grapple_anim: Option<String>,
    /// `[General] Parasite.AllowWaterExit=`, `Nullable<bool>`
    /// (`Ext/Rules/Body.cpp:426`): the fallback for a UnitType's key.
    pub parasite_allow_water_exit: Option<bool>,
    /// `[CombatDamage] Parasite.DisableParticleSystem=`, default `no`
    /// (`Ext/Rules/Body.cpp:495`, constructor `Ext/Rules/Body.h:405`).
    pub parasite_disable_particle_system: bool,
    /// `[General] Cloak.KickOutParasite=`, default `no`
    /// (`Ext/Rules/Body.cpp:289`, constructor `Ext/Rules/Body.h:1056`): the
    /// fallback for a TechnoType's key.
    pub cloak_kick_out_parasite: bool,
    /// `[AudioVisual] CloakAnims=`, `ValueableVector<AnimTypeClass*>`
    /// (`Ext/Rules/Body.cpp:287`): the cloaking anim list fallback.
    #[serde(default)]
    pub cloak_anims: Vec<String>,
    /// `[AudioVisual] DecloakAnims=` (`Ext/Rules/Body.cpp:288`).
    #[serde(default)]
    pub decloak_anims: Vec<String>,
    /// `[General] Sinkable.SquidGrab=`, default `yes`
    /// (`Ext/Rules/Body.cpp:485`, constructor `Ext/Rules/Body.h:900`): the
    /// fallback for a UnitType's key.
    pub sinkable_squid_grab: bool,
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
    /// `[General] ExtendedAircraftMissions=`, default `no`
    /// (`Ext/Rules/Body.cpp:191`, constructor `Ext/Rules/Body.h:97`): the
    /// master switch an AircraftType's own `ExtendedAircraftMissions=`
    /// overrides, and the fallback for its `SmoothMoving`, `EarlyDescend`,
    /// `RearApproach` and `FastScramble` keys
    /// (`rules::phobos_extended_aircraft_missions`).
    pub extended_aircraft_missions: bool,
    /// `[General] ExtendedAircraftMissions.UnlandDamage=`, default `0`
    /// (`Ext/Rules/Body.cpp:192`, `Ext/Rules/Body.h:98`): the damage an
    /// aircraft that cannot land takes, or the kill threshold when negative.
    pub extended_aircraft_missions_unland_damage: i32,
    /// `[General] AmbientDamage.IgnoreTarget=`, default `no`
    /// (`Ext/Rules/Body.cpp:268`, constructor `Ext/Rules/Body.h:710`): the
    /// fallback for an unset weapon key; `yes` leaves the primary target out
    /// of the railgun/blast-wave ambient damage.
    #[serde(default)]
    pub ambient_damage_ignore_target: bool,
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
    /// `[General] ApproachTarget.StopWhenInRange=`, default `no`
    /// (`Ext/Rules/Body.cpp:642-643`, `Body.h:537,1036`): fallback for an
    /// unset `TechnoType` `ApproachTarget.StopWhenInRange`; `yes` stops a
    /// close-enough, really moving foot on its approach.
    #[serde(default)]
    pub approach_target_stop_when_in_range: bool,
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
    /// `[CombatDamage] ProjectileInterceptable=`, default `no`
    /// (`Ext/Rules/Body.cpp:497`, `Body.h:912`): the fallback for a bullet
    /// type's `Interceptable`.
    #[serde(default)]
    pub projectile_interceptable: bool,
    /// `[CombatDamage] Interceptor.GuardRange.IsCylindrical=`, default `no`
    /// (`Ext/Rules/Body.cpp:498`, `Body.h:913`).
    #[serde(default)]
    pub interceptor_guard_range_is_cylindrical: bool,
    /// `[CombatDamage] Interceptor.ApplyFirepowerMult=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:499`, `Body.h:914`).
    #[serde(default = "true_bool")]
    pub interceptor_apply_firepower_mult: bool,
    /// `[CombatDamage] ReturnWeapon.ApplyFirepowerMult=`, default `no`
    /// (`Ext/Rules/Body.cpp:559`, constructor `Ext/Rules/Body.h:973`): the
    /// fallback a `BulletType`'s unset `ReturnWeapon.ApplyFirepowerMult=`
    /// keeps, scaling the returned shot's `Damage=` by the dying bullet's
    /// `FirepowerMult` (`Ext/Bullet/Hooks.DetonateLogics.cpp:490`).
    #[serde(default)]
    pub return_weapon_apply_firepower_mult: bool,
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
    /// `[CombatDamage] OpenTransport.RangeBonus=`, default 0
    /// (`Ext/Rules/Body.cpp:463`, constructor `Ext/Rules/Body.h:878`): the
    /// rider-side fallback, added to the carrier's bonus.
    #[serde(default)]
    pub open_transport_range_bonus: i32,
    /// `[CombatDamage] OpenTransport.DamageMultiplier=`, default 1.0
    /// (`:464`, `Body.h:879`), a `Valueable<float>`.
    #[serde(default = "one_f32")]
    pub open_transport_damage_multiplier: crate::util::native_x87::NativeF32Bits,
    /// `[General] OpenTransport.FireWhileMoving=`, default `yes`
    /// (`:465`, `Body.h:880`).
    #[serde(default = "true_bool")]
    pub open_transport_fire_while_moving: bool,
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
    /// `[General] NoTurret.TrackTarget=`, default `no`
    /// (`Ext/Rules/Body.cpp:393`, constructor `Body.h:812`): the fallback for
    /// an unset UnitType key; `yes` lets a turretless Unit in REARM keep
    /// checking its facing and turn toward the target while cooling down.
    #[serde(default)]
    pub no_turret_track_target: bool,
    /// `[General] BuildingStartFacing=`, default 0
    /// (`Ext/Rules/Body.cpp:626`, constructor `Body.h:1024`): the fallback for
    /// an unset BuildingType `StartFacing`, on a 256-point circle.
    #[serde(default)]
    pub building_start_facing: i32,
    /// `[General] BuildingStartFacing.Random=`, default `no`
    /// (`Ext/Rules/Body.cpp:627`, constructor `Body.h:1025`): the fallback for
    /// an unset BuildingType `StartFacing.Random`.
    #[serde(default)]
    pub building_start_facing_random: bool,
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
    /// `[General] AlternateFLH.OnTurret=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:218`, constructor `Ext/Rules/Body.h:662`): the
    /// fallback for an unset TechnoType ART `AlternateFLH.OnTurret`; `no`
    /// keeps an alternate FLH hull-relative while a turret fires.
    #[serde(default = "true_bool")]
    pub alternate_flh_on_turret: bool,
    /// `[General] AlternateFLH.ApplyVehicle=`, default `no`
    /// (`Ext/Rules/Body.cpp:219`, constructor `Ext/Rules/Body.h:663`): the
    /// fallback for an unset carrier ART `AlternateFLH.ApplyVehicle`; `yes`
    /// lets a Unit passenger fire from the open-topped carrier's AlternateFLH.
    #[serde(default)]
    pub alternate_flh_apply_vehicle: bool,
    /// `[General] Vertical.AircraftFix=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:581`, constructor `Ext/Rules/Body.h:990`): the
    /// fallback for an unset BulletType `Vertical.AircraftFix`; `yes` keeps a
    /// `Vertical=` bullet's FireAt velocity instead of the aircraft's ROT
    /// course (`0x415F25`, `Misc/Hooks.BugFixes.cpp:2423`).
    #[serde(default = "true_bool")]
    pub vertical_aircraft_fix: bool,
    /// `[CombatDamage] CanTarget.IronCurtained=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:438`, constructor `Ext/Rules/Body.h:833`): the
    /// human fallback for an unset WeaponType `CanTarget.IronCurtained`; `no`
    /// refuses a shot at an iron-curtained target
    /// (`Ext/Techno/Hooks.Firing.cpp:367`).
    #[serde(default = "true_bool")]
    pub can_target_iron_curtained: bool,
    /// `[CombatDamage] CanTargetAI.IronCurtained=`, default `no` (`:437`,
    /// constructor `Body.h:832`): the computer-house fallback, preserving the
    /// stock AI refusal.
    #[serde(default)]
    pub can_target_ai_iron_curtained: bool,
    /// `[CombatDamage] AutoTarget.IronCurtained=`, default **`yes`** (`:439`,
    /// constructor `Body.h:834`): the human fallback for an unset WeaponType
    /// `AutoTarget.IronCurtained` in retaliation and auto-target
    /// (`Hooks.Firing.cpp:1246`, `Hooks.Targeting.cpp:63`).
    #[serde(default = "true_bool")]
    pub auto_target_iron_curtained: bool,
    /// `[General] CylinderRangefinding=`, default `no`
    /// (`Ext/Rules/Body.cpp:525`, constructor `Body.h:442`): the fallback for
    /// an unset weapon `CylinderRangefinding`; `yes` measures direct range in
    /// the plan (dx²+dy²) for every techno, not only aircraft
    /// (`Ext/Techno/Hooks.Firing.cpp:1265-1274`).
    #[serde(default)]
    pub cylinder_rangefinding: bool,
    /// `[General] Strafing.SimulateBurst=`, default `no`
    /// (`Ext/Rules/Body.cpp:264`, constructor `Ext/Rules/Body.h:706`): the
    /// fallback for an unset weapon `Strafing.SimulateBurst`; `yes` alternates
    /// the firing offset across a strafing round's single shots
    /// (`Ext/Aircraft/Body.cpp:22,26-27`).
    #[serde(default)]
    pub strafing_simulate_burst: bool,
    /// `[General] Strafing.UseAmmoPerShot=`, default `no`
    /// (`Ext/Rules/Body.cpp:265`, constructor `Body.h:707`): the fallback for
    /// an unset weapon key; `yes` debits one Ammo after every strafing shot
    /// instead of once per round (`Ext/Aircraft/Body.cpp:36-45`).
    #[serde(default)]
    pub strafing_use_ammo_per_shot: bool,
    /// `[General] Strafing.TargetCell=`, default `no`
    /// (`Ext/Rules/Body.cpp:266`, constructor `Body.h:708`): the fallback for
    /// an unset weapon key; `yes` turns the round's Target into its cell after
    /// the release (`Ext/Aircraft/Hooks.cpp:234-235`).
    #[serde(default)]
    pub strafing_target_cell: bool,
    /// `[General] ExtraThreat.IsThreat=`, default 0.0
    /// (`Ext/Rules/Body.cpp:535`, constructor `Ext/Rules/Body.h:952`): the
    /// fallback for an unset TechnoType key.
    #[serde(default = "zero_f64")]
    pub extra_threat_is_threat: NativeF64Bits,
    /// `[General] ExtraThreat.InRange=`, default 0.0 (`:536`, `:953`).
    #[serde(default = "zero_f64")]
    pub extra_threat_in_range: NativeF64Bits,
    /// `[General] ExtraThreatCoefficient.InRangeDistance=`, default 0.0
    /// (`:537`, `:954`).
    #[serde(default = "zero_f64")]
    pub extra_threat_coefficient_in_range_distance: NativeF64Bits,
    /// `[General] ExtraThreatCoefficient.Facing=`, default 0.0 (`:538`,
    /// `:955`).
    #[serde(default = "zero_f64")]
    pub extra_threat_coefficient_facing: NativeF64Bits,
    /// `[General] ExtraThreatCoefficient.DistanceToLastTarget=`, default 0.0
    /// (`:539`, `:956`).
    #[serde(default = "zero_f64")]
    pub extra_threat_coefficient_distance_to_last_target: NativeF64Bits,
    /// `[AudioVisual] CombatAlert=`, the master switch, default **`no`**
    /// (`Ext/Rules/Body.cpp:355`, constructor `Ext/Rules/Body.h:783`).
    #[serde(default)]
    pub combat_alert: bool,
    /// `[AudioVisual] CombatAlert.Default=`, `Nullable<bool>`
    /// (`Ext/Rules/Body.cpp:356`, constructor `Body.h:784` unset): the
    /// fallback for an unset TechnoType `CombatAlert`; unset in turn admits
    /// `!Insignificant && !Spawned` (`Ext/Techno/Hooks.ReceiveDamage.cpp:101`).
    #[serde(default)]
    pub combat_alert_default: Option<bool>,
    /// `[AudioVisual] CombatAlert.IgnoreBuilding=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:357`, constructor `Body.h:785`): an ordinary
    /// building is skipped unless its type is vehicle-like.
    #[serde(default = "true_bool")]
    pub combat_alert_ignore_building: bool,
    /// `[AudioVisual] CombatAlert.SuppressIfInScreen=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:358`, constructor `Body.h:786`). Presentation-only:
    /// the simulation has no tactical view, so VERA cannot apply it
    /// (`docs/phobos-combat-alert.md` records the residual).
    #[serde(default = "true_bool")]
    pub combat_alert_suppress_if_in_screen: bool,
    /// `[AudioVisual] CombatAlert.Interval=`, `Valueable<int>` frames,
    /// default 150 (`Ext/Rules/Body.cpp:359`, constructor `Body.h:787`).
    #[serde(default = "combat_alert_interval_default")]
    pub combat_alert_interval: i32,
    /// `[AudioVisual] CombatAlert.SuppressIfAllyDamage=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:360`, constructor `Body.h:788`).
    #[serde(default = "true_bool")]
    pub combat_alert_suppress_if_ally_damage: bool,
    /// `[AudioVisual] CombatAlert.MakeAVoice=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:361`, constructor `Body.h:789`): `no` keeps the
    /// radar ping but plays no voice or EVA line
    /// (`Ext/Techno/Hooks.ReceiveDamage.cpp:125`).
    #[serde(default = "true_bool")]
    pub combat_alert_make_a_voice: bool,
    /// `[AudioVisual] CombatAlert.UseFeedbackVoice=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:362`, constructor `Body.h:790`): the fallback for
    /// the type's key.
    #[serde(default = "true_bool")]
    pub combat_alert_use_feedback_voice: bool,
    /// `[AudioVisual] CombatAlert.UseAttackVoice=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:363`, constructor `Body.h:791`).
    #[serde(default = "true_bool")]
    pub combat_alert_use_attack_voice: bool,
    /// `[AudioVisual] CombatAlert.UseEVA=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:364`, constructor `Body.h:792`).
    #[serde(default = "true_bool")]
    pub combat_alert_use_eva: bool,
    /// `[General] DestroyAnim.Random=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:220`, constructor `Body.h:664`): a destroyed
    /// unit with a `DestroyAnim=` list picks an entry at random, otherwise
    /// the facing-based entry (`Ext/AnimType/Body.cpp:21-43`).
    #[serde(default = "true_bool")]
    pub destroy_anim_random: bool,
    /// `[AudioVisual] VisualScatter.Min=`, `Valueable<Leptons>` default
    /// **`8`** (`Ext/Rules/Body.cpp:325`, constructor `Ext/Rules/Body.h:758`):
    /// the low end of the radius a `VisualScatter=` weapon's drawn laser, bolt
    /// or beam may end away from its real target
    /// (`Ext/Techno/Hooks.WeaponEffects.cpp:234-235`).
    #[serde(default = "visual_scatter_min_default")]
    pub visual_scatter_min: i32,
    /// `[AudioVisual] VisualScatter.Max=`, `Valueable<Leptons>` default
    /// **`32`** (`Ext/Rules/Body.cpp:326`, constructor `Ext/Rules/Body.h:759`).
    #[serde(default = "visual_scatter_max_default")]
    pub visual_scatter_max: i32,
    /// `[General] AutoTarget.NoThreatBuildings=`, default `no`
    /// (`Ext/Rules/Body.cpp:517`, constructor `Ext/Rules/Body.h:931`): whether
    /// a **human** scanner may auto-target an enemy building that is unarmed
    /// or poses no threat (`Ext/Techno/Hooks.cpp:1850-1852`). A weapon's own
    /// `AttackNoThreatBuildings=` overrides it.
    #[serde(default)]
    pub auto_target_no_threat_buildings: bool,
    /// `[General] AutoTargetAI.NoThreatBuildings=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:518`, constructor `Ext/Rules/Body.h:932`): the
    /// computer's half of the same gate.
    #[serde(default = "true_bool")]
    pub auto_target_ai_no_threat_buildings: bool,
    /// `[CombatDamage] DamageOwnerMultiplier=`, `Valueable<double>` default
    /// **`1.0`** (`Ext/Rules/Body.cpp:340`, constructor `Ext/Rules/Body.h:770`):
    /// the fallback for a warhead's own `DamageOwnerMultiplier=`
    /// (`Ext/Techno/Hooks.ReceiveDamage.cpp:73-74`).
    #[serde(default = "one_f64")]
    pub damage_owner_multiplier: NativeF64Bits,
    /// `[CombatDamage] DamageAlliesMultiplier=`, default **`1.0`**
    /// (`Ext/Rules/Body.cpp:341`, constructor `Ext/Rules/Body.h:771`).
    #[serde(default = "one_f64")]
    pub damage_allies_multiplier: NativeF64Bits,
    /// `[CombatDamage] DamageEnemiesMultiplier=`, default **`1.0`**
    /// (`Ext/Rules/Body.cpp:342`, constructor `Ext/Rules/Body.h:772`); also
    /// the last fallback of the two berserk chains.
    #[serde(default = "one_f64")]
    pub damage_enemies_multiplier: NativeF64Bits,
    /// `[CombatDamage] DamageOwnerMultiplier.NotAffectsEnemies=`,
    /// `Nullable<double>` (`Ext/Rules/Body.cpp:343`, `Body.h:773`): the owner
    /// column of a warhead whose `AffectsEnemies=no` when the warhead names
    /// no owner multiplier of its own.
    #[serde(default)]
    pub damage_owner_multiplier_not_affects_enemies: Option<NativeF64Bits>,
    /// `[CombatDamage] DamageAlliesMultiplier.NotAffectsEnemies=`
    /// (`Ext/Rules/Body.cpp:344`, `Body.h:774`).
    #[serde(default)]
    pub damage_allies_multiplier_not_affects_enemies: Option<NativeF64Bits>,
    /// `[CombatDamage] DamageOwnerMultiplier.Berzerk=`,
    /// `Nullable<double>` (`Ext/Rules/Body.cpp:345`, `Body.h:775`).
    #[serde(default)]
    pub damage_owner_multiplier_berzerk: Option<NativeF64Bits>,
    /// `[CombatDamage] DamageAlliesMultiplier.Berzerk=`
    /// (`Ext/Rules/Body.cpp:346`, `Body.h:776`).
    #[serde(default)]
    pub damage_allies_multiplier_berzerk: Option<NativeF64Bits>,
    /// `[CombatDamage] DamageEnemiesMultiplier.Berzerk=`
    /// (`Ext/Rules/Body.cpp:347`, `Body.h:777`).
    #[serde(default)]
    pub damage_enemies_multiplier_berzerk: Option<NativeF64Bits>,
    /// `[AudioVisual] RemoveMindControl.Silent=`, default `no`
    /// (`Ext/Rules/Body.cpp:597`, constructor `Ext/Rules/Body.h:998`): a
    /// warhead's `RemoveMindControl=` frees its victims without the
    /// `MindClearedSound` (`Ext/CaptureManager/Body.cpp:141`).
    #[serde(default)]
    pub remove_mind_control_silent: bool,
    /// `[CombatDamage] MergeBuildingDamage=`, default `no`
    /// (`Ext/Rules/Body.cpp:503`, constructor `Ext/Rules/Body.h:918`): one
    /// area detonation damages a building once, with its per-cell distance
    /// multipliers summed (`Ext/WarheadType/Hooks.cpp:412-487`).
    #[serde(default)]
    pub merge_building_damage: bool,
    /// `[General] RadarJamHouses=`, `Nullable<AffectedHouse>` default
    /// **`Enemies`** (`Ext/Rules/Body.cpp:213`, constructor
    /// `Ext/Rules/Body.h:657`): which Houses a `RadarJamRadius=` jammer may
    /// jam (`New/Entity/Ares/RadarJammerClass.cpp:17-18`).
    #[serde(default = "radar_jam_houses_default")]
    pub radar_jam_houses: crate::rules::phobos_shields::AffectedHouse,
    /// `[General] RadarJamDelay=`, `Valueable<int>` default **`30`**
    /// (`Ext/Rules/Body.cpp:214`, constructor `Ext/Rules/Body.h:658`): frames
    /// between two jammer scans (`RadarJammerClass.cpp:10`).
    #[serde(default = "radar_jam_delay_default")]
    pub radar_jam_delay: i32,
    /// `[General] NoQueueUpToEnter=`, default `no`
    /// (`Ext/Rules/Body.cpp:197`, constructor `Ext/Rules/Body.h:640`).
    #[serde(default)]
    pub no_queue_up_to_enter: bool,
    /// `[General] NoQueueUpToEnter.BoardDistance=`, `Valueable<int>` default
    /// **`384`** leptons (`Ext/Rules/Body.cpp:198`, constructor
    /// `Ext/Rules/Body.h:641`).
    #[serde(default = "no_queue_board_distance_default")]
    pub no_queue_up_to_enter_board_distance: i32,
    /// `[General] NoQueueUpToEnter.Buildings=`, `Nullable<bool>`
    /// (`Ext/Rules/Body.cpp:200`, constructor `Ext/Rules/Body.h:643`).
    #[serde(default)]
    pub no_queue_up_to_enter_buildings: Option<bool>,
    /// `[General] VeteranReload=`, default **`1.0`**
    /// (`Ext/Rules/Body.cpp:388`, constructor `Ext/Rules/Body.h:808`).
    #[serde(default = "one_f64")]
    pub veteran_reload: NativeF64Bits,
    /// `[General] VeteranEmptyReload=`, `Nullable<double>`
    /// (`Ext/Rules/Body.cpp:389`, constructor `Ext/Rules/Body.h:809`).
    #[serde(default)]
    pub veteran_empty_reload: Option<NativeF64Bits>,
    /// `[AudioVisual] IsVoiceCreatedGlobal=`, default `no`
    /// (`Ext/Rules/Body.cpp:306`, constructor `Ext/Rules/Body.h:738`): every
    /// produced object plays its `VoiceCreated=`.
    #[serde(default)]
    pub is_voice_created_global: bool,
    /// `[General] UseDisguiseMovementSpeed=`, `Nullable<bool>`
    /// (`Ext/Rules/Body.cpp:221`): a disguised object that names no own
    /// answer moves at its disguise's speed.
    #[serde(default)]
    pub use_disguise_movement_speed: Option<bool>,
    /// `[General] HoverDrownable=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:475`, constructor `Ext/Rules/Body.h:890`): a hover
    /// vehicle over deep water drowns unless its type says otherwise.
    #[serde(default = "true_bool")]
    pub hover_drownable: bool,
    /// `[General] OnlyUseLandSequences=`, default `no`
    /// (`Ext/Rules/Body.cpp:622`, constructor `Ext/Rules/Body.h:1017`): the
    /// fallback an `InfantryType`'s unset art `OnlyUseLandSequences=` keeps,
    /// making infantry play their land sequences while in water
    /// (`Ext/Techno/Hooks.Misc.cpp:890`).
    #[serde(default)]
    pub only_use_land_sequences: bool,
    /// `[General] AnimCraterDestroyTiberium=`, default **`yes`**
    /// (`Ext/Rules/Body.cpp:418`, constructor `Ext/Rules/Body.h:844`): the
    /// fallback an AnimType's unset art `Crater.DestroyTiberium=` keeps for
    /// `0x4250E1`'s `CellClass::Reduce_Tiberium(6)` call
    /// (`Ext/Anim/Hooks.cpp:707`).
    #[serde(default = "true_bool")]
    pub anim_crater_destroy_tiberium: bool,
}

/// `RadarJamHouses=`'s constructor value (`Ext/Rules/Body.h:657`).
fn radar_jam_houses_default() -> crate::rules::phobos_shields::AffectedHouse {
    crate::rules::phobos_shields::AffectedHouse::ENEMIES
}

/// `RadarJamDelay=`'s constructor value (`Ext/Rules/Body.h:658`).
fn radar_jam_delay_default() -> i32 {
    30
}

/// `NoQueueUpToEnter.BoardDistance=`'s constructor value
/// (`Ext/Rules/Body.h:641`).
fn no_queue_board_distance_default() -> i32 {
    384
}

/// `1.0`, the constructor value of the three plain `[CombatDamage]` damage
/// multipliers (`Ext/Rules/Body.h:770-772`) and `[General] VeteranReload=`
/// (`Ext/Rules/Body.h:808`), stored as bits so `PhobosGlobals` keeps `Eq`.
fn one_f64() -> NativeF64Bits {
    NativeF64Bits::ONE
}

/// `CombatAlert.Interval=`'s constructor value (`Ext/Rules/Body.h:787`).
fn combat_alert_interval_default() -> i32 {
    150
}

/// `VisualScatter.Min=`'s constructor value (`Ext/Rules/Body.h:758`).
fn visual_scatter_min_default() -> i32 {
    8
}

/// `VisualScatter.Max=`'s constructor value (`Ext/Rules/Body.h:759`).
fn visual_scatter_max_default() -> i32 {
    32
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
            shrapnel_obey_warhead_trigger_conditions: true,
            projectile_interceptable: false,
            interceptor_guard_range_is_cylindrical: false,
            interceptor_apply_firepower_mult: true,
            return_weapon_apply_firepower_mult: false,
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
            art_image_swap: false,
            keep_range_early_stop: 0,
            parasite_grapple_anim: None,
            parasite_allow_water_exit: None,
            parasite_disable_particle_system: false,
            cloak_kick_out_parasite: false,
            cloak_anims: Vec::new(),
            decloak_anims: Vec::new(),
            sinkable_squid_grab: true,
            drain_money_display: false,
            drain_money_display_houses: AffectedHouse::ALL,
            drain_money_display_on_target: false,
            drain_money_display_on_target_use_display_income: true,
            display_income_houses: AffectedHouse::ALL,
            extended_aircraft_missions: false,
            extended_aircraft_missions_unland_damage: 0,
            ambient_damage_ignore_target: false,
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
            approach_target_stop_when_in_range: false,
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
            open_transport_range_bonus: 0,
            open_transport_damage_multiplier: crate::util::native_x87::NativeF32Bits::ONE,
            open_transport_fire_while_moving: true,
            no_rearm_under_emp: false,
            no_rearm_temporal: false,
            no_reload_under_emp: false,
            no_reload_temporal: false,
            no_turret_track_target: false,
            building_start_facing: 0,
            building_start_facing_random: false,
            passengers_sync_owner: false,
            passengers_sync_owner_revert_on_exit: true,
            alternate_flh_on_turret: true,
            alternate_flh_apply_vehicle: false,
            vertical_aircraft_fix: true,
            can_target_iron_curtained: true,
            can_target_ai_iron_curtained: false,
            auto_target_iron_curtained: true,
            cylinder_rangefinding: false,
            strafing_simulate_burst: false,
            strafing_use_ammo_per_shot: false,
            strafing_target_cell: false,
            extra_threat_is_threat: NativeF64Bits::POSITIVE_ZERO,
            extra_threat_in_range: NativeF64Bits::POSITIVE_ZERO,
            extra_threat_coefficient_in_range_distance: NativeF64Bits::POSITIVE_ZERO,
            extra_threat_coefficient_facing: NativeF64Bits::POSITIVE_ZERO,
            extra_threat_coefficient_distance_to_last_target: NativeF64Bits::POSITIVE_ZERO,
            combat_alert: false,
            combat_alert_default: None,
            combat_alert_ignore_building: true,
            combat_alert_suppress_if_in_screen: true,
            combat_alert_interval: combat_alert_interval_default(),
            combat_alert_suppress_if_ally_damage: true,
            combat_alert_make_a_voice: true,
            combat_alert_use_feedback_voice: true,
            combat_alert_use_attack_voice: true,
            combat_alert_use_eva: true,
            destroy_anim_random: true,
            visual_scatter_min: visual_scatter_min_default(),
            visual_scatter_max: visual_scatter_max_default(),
            auto_target_no_threat_buildings: false,
            auto_target_ai_no_threat_buildings: true,
            damage_owner_multiplier: NativeF64Bits::ONE,
            damage_allies_multiplier: NativeF64Bits::ONE,
            damage_enemies_multiplier: NativeF64Bits::ONE,
            damage_owner_multiplier_not_affects_enemies: None,
            damage_allies_multiplier_not_affects_enemies: None,
            damage_owner_multiplier_berzerk: None,
            damage_allies_multiplier_berzerk: None,
            damage_enemies_multiplier_berzerk: None,
            remove_mind_control_silent: false,
            merge_building_damage: false,
            radar_jam_houses: radar_jam_houses_default(),
            radar_jam_delay: radar_jam_delay_default(),
            no_queue_up_to_enter: false,
            no_queue_up_to_enter_board_distance: no_queue_board_distance_default(),
            no_queue_up_to_enter_buildings: None,
            veteran_reload: NativeF64Bits::ONE,
            veteran_empty_reload: None,
            is_voice_created_global: false,
            use_disguise_movement_speed: None,
            hover_drownable: true,
            only_use_land_sequences: false,
            anim_crater_destroy_tiberium: true,
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
        globals.parasite_grapple_anim = audio_visual
            .read_type_name("Parasite.GrappleAnim", 0x100)
            .map(str::to_owned);
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
        // `Ext/Rules/Body.cpp:325-326`: the `VisualScatter=` radius pair,
        // read just ahead of the `CombatAlert*` block (`:355-364`).
        // `Ext/Rules/Body.cpp:597`: an `[AudioVisual]` read, not CombatDamage.
        globals.remove_mind_control_silent = audio_visual.read_bool(
            "RemoveMindControl.Silent",
            globals.remove_mind_control_silent,
        );
        globals.visual_scatter_min =
            audio_visual.read_int("VisualScatter.Min", globals.visual_scatter_min);
        globals.visual_scatter_max =
            audio_visual.read_int("VisualScatter.Max", globals.visual_scatter_max);
        // `[AudioVisual] CombatAlert*` (`Ext/Rules/Body.cpp:355-364`): the
        // master switch, the nullable default and interval, and the three
        // type-key fallbacks. `Valueable` reads from the constructor values;
        // `CombatAlert.Default` is `Nullable<bool>` and stays unset until a
        // layer names it.
        globals.combat_alert = audio_visual.read_bool("CombatAlert", globals.combat_alert);
        globals.combat_alert_default = audio_visual
            .read_phobos_nullable_bool("CombatAlert.Default", globals.combat_alert_default);
        globals.combat_alert_ignore_building = audio_visual.read_bool(
            "CombatAlert.IgnoreBuilding",
            globals.combat_alert_ignore_building,
        );
        globals.combat_alert_suppress_if_in_screen = audio_visual.read_bool(
            "CombatAlert.SuppressIfInScreen",
            globals.combat_alert_suppress_if_in_screen,
        );
        globals.combat_alert_interval =
            audio_visual.read_ares_int("CombatAlert.Interval", globals.combat_alert_interval);
        globals.combat_alert_suppress_if_ally_damage = audio_visual.read_bool(
            "CombatAlert.SuppressIfAllyDamage",
            globals.combat_alert_suppress_if_ally_damage,
        );
        globals.combat_alert_make_a_voice =
            audio_visual.read_bool("CombatAlert.MakeAVoice", globals.combat_alert_make_a_voice);
        globals.combat_alert_use_feedback_voice = audio_visual.read_bool(
            "CombatAlert.UseFeedbackVoice",
            globals.combat_alert_use_feedback_voice,
        );
        globals.combat_alert_use_attack_voice = audio_visual.read_bool(
            "CombatAlert.UseAttackVoice",
            globals.combat_alert_use_attack_voice,
        );
        globals.combat_alert_use_eva =
            audio_visual.read_bool("CombatAlert.UseEVA", globals.combat_alert_use_eva);
        // `Ext/Rules/Body.cpp:306`: an `[AudioVisual]` read, not General.
        globals.is_voice_created_global =
            audio_visual.read_bool("IsVoiceCreatedGlobal", globals.is_voice_created_global);
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
        globals.ambient_damage_ignore_target = general.read_bool(
            "AmbientDamage.IgnoreTarget",
            globals.ambient_damage_ignore_target,
        );
        // `Ext/Rules/Body.cpp:418`: the AnimType ART key's `[General]`
        // fallback, constructed true.
        globals.anim_crater_destroy_tiberium = general.read_bool(
            "AnimCraterDestroyTiberium",
            globals.anim_crater_destroy_tiberium,
        );
        globals.force_weapon_in_range_techno_only = general.read_bool(
            "ForceWeapon.InRange.TechnoOnly",
            globals.force_weapon_in_range_techno_only,
        );
        // `Ext/Rules/Body.cpp:197-200`: the boarding-queue pair and its
        // Building override, all `[General]`.
        globals.no_queue_up_to_enter =
            general.read_bool("NoQueueUpToEnter", globals.no_queue_up_to_enter);
        globals.no_queue_up_to_enter_board_distance = general.read_int(
            "NoQueueUpToEnter.BoardDistance",
            globals.no_queue_up_to_enter_board_distance,
        );
        globals.no_queue_up_to_enter_buildings = general.read_phobos_nullable_bool(
            "NoQueueUpToEnter.Buildings",
            globals.no_queue_up_to_enter_buildings,
        );
        // `Ext/Rules/Body.cpp:213-214`: the Ares radar-jam scan's House filter
        // (`Valueable<AffectedHouse>`, default Enemies) and interval.
        globals.radar_jam_houses = crate::rules::phobos_shields::read_affected_house(
            general,
            "RadarJamHouses",
            globals.radar_jam_houses,
        );
        globals.radar_jam_delay = general.read_int("RadarJamDelay", globals.radar_jam_delay);
        // `Ext/Rules/Body.cpp:221`: a nullable `[General]` read.
        globals.use_disguise_movement_speed = general.read_phobos_nullable_bool(
            "UseDisguiseMovementSpeed",
            globals.use_disguise_movement_speed,
        );
        // `Ext/Rules/Body.cpp:388-389`.
        globals.veteran_reload =
            read_double_bits(general, "VeteranReload", globals.veteran_reload);
        globals.veteran_empty_reload = read_nullable_double_bits(
            general,
            "VeteranEmptyReload",
            globals.veteran_empty_reload,
        );
        // `Ext/Rules/Body.cpp:475`, a `Valueable<bool>` default `yes`.
        globals.hover_drownable = general.read_bool("HoverDrownable", globals.hover_drownable);
        globals.only_use_land_sequences =
            general.read_bool("OnlyUseLandSequences", globals.only_use_land_sequences);
        // `Ext/Rules/Body.cpp:218-219`, `yes`/`no` constructor defaults.
        globals.alternate_flh_on_turret =
            general.read_bool("AlternateFLH.OnTurret", globals.alternate_flh_on_turret);
        globals.alternate_flh_apply_vehicle = general.read_bool(
            "AlternateFLH.ApplyVehicle",
            globals.alternate_flh_apply_vehicle,
        );
        globals.destroy_anim_random =
            general.read_bool("DestroyAnim.Random", globals.destroy_anim_random);
        globals.not_human_random_death_sequence = general.read_bool(
            "NotHuman.RandomDeathSequence",
            globals.not_human_random_death_sequence,
        );
        globals.cloak_kick_out_parasite =
            general.read_bool("Cloak.KickOutParasite", globals.cloak_kick_out_parasite);
        globals.cloak_anims = audio_visual
            .read_list("CloakAnims", 0x800)
            .map(|tokens| tokens.into_iter().map(str::to_owned).collect())
            .unwrap_or(globals.cloak_anims);
        globals.decloak_anims = audio_visual
            .read_list("DecloakAnims", 0x800)
            .map(|tokens| tokens.into_iter().map(str::to_owned).collect())
            .unwrap_or(globals.decloak_anims);
        globals.sinkable_squid_grab =
            general.read_bool("Sinkable.SquidGrab", globals.sinkable_squid_grab);
        globals.keep_range_allow_ai =
            general.read_bool("KeepRange.AllowAI", globals.keep_range_allow_ai);
        globals.keep_range_allow_player =
            general.read_bool("KeepRange.AllowPlayer", globals.keep_range_allow_player);
        globals.art_image_swap = general.read_bool("ArtImageSwap", globals.art_image_swap);
        globals.keep_range_early_stop =
            general.read_ares_int("KeepRange.EarlyStopFrame", globals.keep_range_early_stop);
        globals.parasite_allow_water_exit = general.read_phobos_nullable_bool(
            "Parasite.AllowWaterExit",
            globals.parasite_allow_water_exit,
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
        globals.attack_move_aggressive =
            general.read_bool("AttackMove.Aggressive", globals.attack_move_aggressive);
        globals.attack_move_update_target =
            general.read_bool("AttackMove.UpdateTarget", globals.attack_move_update_target);
        globals.approach_target_stop_when_in_range = general.read_bool(
            "ApproachTarget.StopWhenInRange",
            globals.approach_target_stop_when_in_range,
        );
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
        globals.open_transport_fire_while_moving = general.read_bool(
            "OpenTransport.FireWhileMoving",
            globals.open_transport_fire_while_moving,
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
        // `Valueable<bool>` (`Ext/Rules/Body.cpp:393`): no unset state.
        globals.no_turret_track_target =
            general.read_bool("NoTurret.TrackTarget", globals.no_turret_track_target);
        // The StartFacing pair (`Ext/Rules/Body.cpp:626-627`): a Valueable<int>
        // and a Valueable<bool>, both read with their constructor value.
        globals.building_start_facing =
            general.read_int("BuildingStartFacing", globals.building_start_facing);
        globals.building_start_facing_random = general.read_bool(
            "BuildingStartFacing.Random",
            globals.building_start_facing_random,
        );
        // The two `Passengers.SyncOwner` `Valueable<bool>` globals
        // (`Ext/Rules/Body.cpp:467-468`); `Valueable` has no unset state, so
        // these are plain `read_bool` reads from the constructor defaults.
        globals.passengers_sync_owner =
            general.read_bool("Passengers.SyncOwner", globals.passengers_sync_owner);
        globals.passengers_sync_owner_revert_on_exit = general.read_bool(
            "Passengers.SyncOwner.RevertOnExit",
            globals.passengers_sync_owner_revert_on_exit,
        );
        globals.vertical_aircraft_fix =
            general.read_bool("Vertical.AircraftFix", globals.vertical_aircraft_fix);
        // `Ext/Rules/Body.cpp:517-518`, both `[General]` `Valueable<bool>`:
        // a human scanner keeps `no`, the computer's scanners keep `yes`.
        globals.auto_target_no_threat_buildings = general.read_bool(
            "AutoTarget.NoThreatBuildings",
            globals.auto_target_no_threat_buildings,
        );
        globals.auto_target_ai_no_threat_buildings = general.read_bool(
            "AutoTargetAI.NoThreatBuildings",
            globals.auto_target_ai_no_threat_buildings,
        );
        // The `ExtendedAircraftMissions` pair (`Ext/Rules/Body.cpp:191-192`).
        // The switch is a `Valueable<bool>`; UnlandDamage a `Valueable<int>`,
        // both read with their constructor value as the default.
        globals.extended_aircraft_missions = general.read_bool(
            "ExtendedAircraftMissions",
            globals.extended_aircraft_missions,
        );
        globals.extended_aircraft_missions_unland_damage = general.read_int(
            "ExtendedAircraftMissions.UnlandDamage",
            globals.extended_aircraft_missions_unland_damage,
        );
        // The three `[General] Strafing.*` `Valueable<bool>` fallbacks
        // (`Ext/Rules/Body.cpp:264-266`, constructor `Body.h:706-708`).
        globals.strafing_simulate_burst =
            general.read_bool("Strafing.SimulateBurst", globals.strafing_simulate_burst);
        globals.strafing_use_ammo_per_shot = general.read_bool(
            "Strafing.UseAmmoPerShot",
            globals.strafing_use_ammo_per_shot,
        );
        globals.strafing_target_cell =
            general.read_bool("Strafing.TargetCell", globals.strafing_target_cell);
        // `Ext/Rules/Body.cpp:535-539`: five `Valueable<double>` fallbacks,
        // all constructed `0.0` (`Ext/Rules/Body.h:952-956`).
        // `Valueable<double>` through `parser.ReadDouble` (the installed Ares
        // Parser<double>), not the stock `%f` ReadDouble.
        globals.extra_threat_is_threat =
            general.read_ares_double_bits("ExtraThreat.IsThreat", globals.extra_threat_is_threat);
        globals.extra_threat_in_range =
            general.read_ares_double_bits("ExtraThreat.InRange", globals.extra_threat_in_range);
        globals.extra_threat_coefficient_in_range_distance = general.read_ares_double_bits(
            "ExtraThreatCoefficient.InRangeDistance",
            globals.extra_threat_coefficient_in_range_distance,
        );
        globals.extra_threat_coefficient_facing = general.read_ares_double_bits(
            "ExtraThreatCoefficient.Facing",
            globals.extra_threat_coefficient_facing,
        );
        globals.extra_threat_coefficient_distance_to_last_target = general.read_ares_double_bits(
            "ExtraThreatCoefficient.DistanceToLastTarget",
            globals.extra_threat_coefficient_distance_to_last_target,
        );
        globals
    }

    /// `[General] ExtraThreat.IsThreat=`.
    pub fn extra_threat_is_threat(&self) -> f64 {
        f64::from_bits(self.extra_threat_is_threat.bits())
    }

    /// `[General] ExtraThreat.InRange=`.
    pub fn extra_threat_in_range(&self) -> f64 {
        f64::from_bits(self.extra_threat_in_range.bits())
    }

    /// `[General] ExtraThreatCoefficient.InRangeDistance=`.
    pub fn extra_threat_in_range_distance(&self) -> f64 {
        f64::from_bits(self.extra_threat_coefficient_in_range_distance.bits())
    }

    /// `[General] ExtraThreatCoefficient.Facing=`.
    pub fn extra_threat_coefficient_facing(&self) -> f64 {
        f64::from_bits(self.extra_threat_coefficient_facing.bits())
    }

    /// `[General] ExtraThreatCoefficient.DistanceToLastTarget=`.
    pub fn extra_threat_coefficient_distance_to_last_target(&self) -> f64 {
        f64::from_bits(self.extra_threat_coefficient_distance_to_last_target.bits())
    }

    /// `[CombatDamage] DamageOwnerMultiplier=`.
    pub fn damage_owner_multiplier(&self) -> f64 {
        f64::from_bits(self.damage_owner_multiplier.bits())
    }

    /// `[CombatDamage] DamageAlliesMultiplier=`.
    pub fn damage_allies_multiplier(&self) -> f64 {
        f64::from_bits(self.damage_allies_multiplier.bits())
    }

    /// `[CombatDamage] DamageEnemiesMultiplier=`.
    pub fn damage_enemies_multiplier(&self) -> f64 {
        f64::from_bits(self.damage_enemies_multiplier.bits())
    }

    /// `[CombatDamage] DamageOwnerMultiplier.NotAffectsEnemies=`.
    pub fn damage_owner_multiplier_not_affects_enemies(&self) -> Option<f64> {
        self.damage_owner_multiplier_not_affects_enemies
            .map(|value| f64::from_bits(value.bits()))
    }

    /// `[CombatDamage] DamageAlliesMultiplier.NotAffectsEnemies=`.
    pub fn damage_allies_multiplier_not_affects_enemies(&self) -> Option<f64> {
        self.damage_allies_multiplier_not_affects_enemies
            .map(|value| f64::from_bits(value.bits()))
    }

    /// `[CombatDamage] DamageOwnerMultiplier.Berzerk=`.
    pub fn damage_owner_multiplier_berzerk(&self) -> Option<f64> {
        self.damage_owner_multiplier_berzerk
            .map(|value| f64::from_bits(value.bits()))
    }

    /// `[CombatDamage] DamageAlliesMultiplier.Berzerk=`.
    pub fn damage_allies_multiplier_berzerk(&self) -> Option<f64> {
        self.damage_allies_multiplier_berzerk
            .map(|value| f64::from_bits(value.bits()))
    }

    /// `[CombatDamage] DamageEnemiesMultiplier.Berzerk=`.
    pub fn damage_enemies_multiplier_berzerk(&self) -> Option<f64> {
        self.damage_enemies_multiplier_berzerk
            .map(|value| f64::from_bits(value.bits()))
    }

    /// `[General] VeteranReload=`.
    pub fn veteran_reload(&self) -> f64 {
        f64::from_bits(self.veteran_reload.bits())
    }

    /// `[General] VeteranEmptyReload=`.
    pub fn veteran_empty_reload(&self) -> Option<f64> {
        self.veteran_empty_reload
            .map(|value| f64::from_bits(value.bits()))
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
        shrapnel_obey_warhead_trigger_conditions: section.read_bool(
            "Shrapnel.ObeyWarheadTriggerConditions",
            defaults.shrapnel_obey_warhead_trigger_conditions,
        ),
        projectile_interceptable: section
            .read_bool("ProjectileInterceptable", defaults.projectile_interceptable),
        interceptor_guard_range_is_cylindrical: section.read_bool(
            "Interceptor.GuardRange.IsCylindrical",
            defaults.interceptor_guard_range_is_cylindrical,
        ),
        interceptor_apply_firepower_mult: section.read_bool(
            "Interceptor.ApplyFirepowerMult",
            defaults.interceptor_apply_firepower_mult,
        ),
        return_weapon_apply_firepower_mult: section.read_bool(
            "ReturnWeapon.ApplyFirepowerMult",
            defaults.return_weapon_apply_firepower_mult,
        ),
        can_target_iron_curtained: section.read_bool(
            "CanTarget.IronCurtained",
            defaults.can_target_iron_curtained,
        ),
        can_target_ai_iron_curtained: section.read_bool(
            "CanTargetAI.IronCurtained",
            defaults.can_target_ai_iron_curtained,
        ),
        auto_target_iron_curtained: section.read_bool(
            "AutoTarget.IronCurtained",
            defaults.auto_target_iron_curtained,
        ),
        cylinder_rangefinding: section
            .read_bool("CylinderRangefinding", defaults.cylinder_rangefinding),
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
        parasite_disable_particle_system: section.read_bool(
            "Parasite.DisableParticleSystem",
            defaults.parasite_disable_particle_system,
        ),
        open_transport_range_bonus: section.read_int(
            "OpenTransport.RangeBonus",
            defaults.open_transport_range_bonus,
        ),
        open_transport_damage_multiplier: crate::util::native_x87::NativeF32Bits::from_bits(
            section
                .read_float(
                    "OpenTransport.DamageMultiplier",
                    f32::from_bits(defaults.open_transport_damage_multiplier.bits()),
                )
                .to_bits(),
        ),
        driver_killed_kill_passengers: section.read_bool(
            "DriverKilled.KillPassengers",
            defaults.driver_killed_kill_passengers,
        ),
        // `Ext/Rules/Body.cpp:340-347`: the eight `[CombatDamage]` damage
        // multipliers, read in this order. The three plain values are
        // `Valueable<double>` 1.0; the five nullable columns stay unset until
        // a layer names them.
        damage_owner_multiplier: read_double_bits(
            section,
            "DamageOwnerMultiplier",
            defaults.damage_owner_multiplier,
        ),
        damage_allies_multiplier: read_double_bits(
            section,
            "DamageAlliesMultiplier",
            defaults.damage_allies_multiplier,
        ),
        damage_enemies_multiplier: read_double_bits(
            section,
            "DamageEnemiesMultiplier",
            defaults.damage_enemies_multiplier,
        ),
        damage_owner_multiplier_not_affects_enemies: read_nullable_double_bits(
            section,
            "DamageOwnerMultiplier.NotAffectsEnemies",
            defaults.damage_owner_multiplier_not_affects_enemies,
        ),
        damage_allies_multiplier_not_affects_enemies: read_nullable_double_bits(
            section,
            "DamageAlliesMultiplier.NotAffectsEnemies",
            defaults.damage_allies_multiplier_not_affects_enemies,
        ),
        damage_owner_multiplier_berzerk: read_nullable_double_bits(
            section,
            "DamageOwnerMultiplier.Berzerk",
            defaults.damage_owner_multiplier_berzerk,
        ),
        damage_allies_multiplier_berzerk: read_nullable_double_bits(
            section,
            "DamageAlliesMultiplier.Berzerk",
            defaults.damage_allies_multiplier_berzerk,
        ),
        damage_enemies_multiplier_berzerk: read_nullable_double_bits(
            section,
            "DamageEnemiesMultiplier.Berzerk",
            defaults.damage_enemies_multiplier_berzerk,
        ),
        // `Ext/Rules/Body.cpp:503`: the merge switch, a `Valueable<bool>`
        // default `no`.
        merge_building_damage: section
            .read_bool("MergeBuildingDamage", defaults.merge_building_damage),
        // The `[AudioVisual]` block belongs to `from_ini`; a `CombatDamage`
        // section leaves these at their constructor values.
        ..defaults
    }
}

#[cfg(test)]
#[path = "phobos_globals_tests.rs"]
mod tests;
