//! The `[CombatDamage] Shrapnel.*` globals (`Ext/Rules/Body.cpp`), whose
//! `Nullable` default reads `no`.
use super::PhobosGlobals;
use crate::rules::ini_parser::IniFile;

fn read(text: &str) -> PhobosGlobals {
    PhobosGlobals::from_ini(&IniFile::from_str(text))
}

/// `[General]` force-weapon fallbacks (`Ext/Rules/Body.cpp:230-232`,
/// `Ext/Rules/Body.h:674-676`).
#[test]
fn the_force_weapon_globals_default_and_read_from_general() {
    let defaults = PhobosGlobals::default();
    assert!(
        defaults.force_weapon_in_range_techno_only,
        "TechnoOnly constructs yes"
    );
    assert!(!defaults.force_weapon_in_range_apply_range_modifiers);
    assert!(!defaults.force_aa_weapon_in_range_apply_range_modifiers);

    let set = read(
        "[General]\nForceWeapon.InRange.TechnoOnly=no\n\
         ForceWeapon.InRange.ApplyRangeModifiers=yes\n\
         ForceAAWeapon.InRange.ApplyRangeModifiers=yes\n",
    );
    assert!(!set.force_weapon_in_range_techno_only);
    assert!(set.force_weapon_in_range_apply_range_modifiers);
    assert!(set.force_aa_weapon_in_range_apply_range_modifiers);
}

/// `[CombatDamage] PenetratesTransport.Level=`, default 10
/// (`Ext/Rules/Body.h:945`).
#[test]
fn the_penetrates_transport_level_defaults_and_reads() {
    assert_eq!(PhobosGlobals::default().penetrates_transport_level, 10);
    assert_eq!(
        read("[CombatDamage]\nPenetratesTransport.Level=3\n").penetrates_transport_level,
        3
    );
}

#[test]
fn the_shrapnel_globals_default_to_the_stock_admission() {
    // An absent section, an absent key and a later layer that names nothing all
    // leave the stock behaviour, which is why the unset value is `no`.
    assert_eq!(read(""), PhobosGlobals::default());
    assert_eq!(read("[CombatDamage]\n"), PhobosGlobals::default());
    assert!(!PhobosGlobals::default().shrapnel_affects_ground);
    assert!(!PhobosGlobals::default().shrapnel_affects_buildings);

    let set = read("[CombatDamage]\nShrapnel.AffectsGround=yes\nShrapnel.AffectsBuildings=yes\n");
    assert!(set.shrapnel_affects_ground);
    assert!(set.shrapnel_affects_buildings);

    // A later layer naming only one key keeps the other at its default.
    let half = read("[CombatDamage]\nShrapnel.AffectsBuildings=yes\n");
    assert!(!half.shrapnel_affects_ground);
    assert!(half.shrapnel_affects_buildings);

    // The AutoDeath scan layers default to the same `no`.
    assert!(!PhobosGlobals::default().auto_death_technos_exist_allow_limboed);
    assert!(!PhobosGlobals::default().auto_death_technos_dont_exist_allow_limboed);
    // `Ext/Rules/Body.h:1027` constructs this one `true`.
    assert!(PhobosGlobals::default().auto_death_allow_limboed);
    // `Ext/Rules/Body.h:923` constructs the invoker-state fallback `true`.
    assert!(PhobosGlobals::default().affects_invoker_only_ignore_invoker_state);
    assert!(
        !read(
            "[CombatDamage]
AffectsInvokerOnly.IgnoreInvokerState=no
"
        )
        .affects_invoker_only_ignore_invoker_state
    );
    let limbo = read(
        "[CombatDamage]\nAutoDeath.TechnosExist.AllowLimboed=yes\n\
         AutoDeath.TechnosDontExist.AllowLimboed=yes\n",
    );
    assert!(limbo.auto_death_technos_exist_allow_limboed);
    assert!(limbo.auto_death_technos_dont_exist_allow_limboed);
}

/// `[AudioVisual]` block (`Ext/Rules/Body.cpp:291-304`), whose `Valueable`
/// defaults come from `Ext/Rules/Body.h:725-732`.
#[test]
fn the_drain_display_globals_default_to_phobos_constructor_values() {
    assert_eq!(read(""), PhobosGlobals::default());
    let defaults = PhobosGlobals::default();
    assert!(!defaults.drain_money_display);
    assert_eq!(
        defaults.drain_money_display_houses,
        crate::rules::phobos_shields::AffectedHouse::ALL
    );
    assert!(!defaults.drain_money_display_on_target);
    assert!(
        defaults.drain_money_display_on_target_use_display_income,
        "UseDisplayIncome constructs yes"
    );
    assert_eq!(
        defaults.display_income_houses,
        crate::rules::phobos_shields::AffectedHouse::ALL
    );

    let set = read(
        "[AudioVisual]\nDrainMoneyDisplay=yes\nDrainMoneyDisplay.Houses=owner,enemies\n\
         DrainMoneyDisplay.OnTarget=yes\nDrainMoneyDisplay.OnTarget.UseDisplayIncome=no\n\
         DisplayIncome.Houses=allies\n",
    );
    assert!(set.drain_money_display);
    assert!(
        set.drain_money_display_houses
            .contains(crate::rules::phobos_shields::AffectedHouse::OWNER)
    );
    assert!(
        set.drain_money_display_houses
            .contains(crate::rules::phobos_shields::AffectedHouse::ENEMIES)
    );
    assert!(
        !set.drain_money_display_houses
            .contains(crate::rules::phobos_shields::AffectedHouse::ALLIES)
    );
    assert!(set.drain_money_display_on_target);
    assert!(!set.drain_money_display_on_target_use_display_income);
    assert_eq!(
        set.display_income_houses,
        crate::rules::phobos_shields::AffectedHouse::ALLIES
    );

    // The `[CombatDamage]` block keeps its own defaults in a layer that names
    // only `[AudioVisual]` keys.
    let audio_only = read("[AudioVisual]\nDrainMoneyDisplay=yes\n");
    assert!(audio_only.drain_money_display);
    assert!(!audio_only.shrapnel_affects_ground);
}

/// `[General] Convert.ResetMindControl=` (`Ext/Rules/Body.cpp:222`) and the
/// two `[General] Explodes.*` globals (`:470-471`, constructors
/// `Ext/Rules/Body.h:885-886`): all three read from General, and the two
/// Explodes booleans construct `true`.
#[test]
fn the_general_convert_and_explodes_globals_read_from_general() {
    let defaults = PhobosGlobals::default();
    assert!(!defaults.convert_reset_mind_control);
    assert!(defaults.explodes_kill_passengers);
    assert!(defaults.explodes_during_buildup);

    let set = read(
        "[General]\nConvert.ResetMindControl=yes\n\
         Explodes.KillPassengers=no\nExplodes.DuringBuildup=no\n",
    );
    assert!(set.convert_reset_mind_control);
    assert!(!set.explodes_kill_passengers);
    assert!(!set.explodes_during_buildup);

    // The keys belong to General; a CombatDamage read does not see them.
    let wrong = read(
        "[CombatDamage]\nConvert.ResetMindControl=yes\n\
         Explodes.KillPassengers=no\nExplodes.DuringBuildup=no\n",
    );
    assert!(!wrong.convert_reset_mind_control);
    assert!(wrong.explodes_kill_passengers);
    assert!(wrong.explodes_during_buildup);
}

/// `[CombatDamage]` interceptor globals (`Ext/Rules/Body.cpp:497-499`,
/// `Body.h:912-914`): two construct `false`, `ApplyFirepowerMult` constructs
/// `true`.
#[test]
fn the_interceptor_globals_default_and_read_from_combat_damage() {
    let defaults = PhobosGlobals::default();
    assert!(!defaults.projectile_interceptable);
    assert!(!defaults.interceptor_guard_range_is_cylindrical);
    assert!(defaults.interceptor_apply_firepower_mult);

    let set = read(
        "[CombatDamage]\nProjectileInterceptable=yes\n\
         Interceptor.GuardRange.IsCylindrical=yes\n\
         Interceptor.ApplyFirepowerMult=no\n",
    );
    assert!(set.projectile_interceptable);
    assert!(set.interceptor_guard_range_is_cylindrical);
    assert!(!set.interceptor_apply_firepower_mult);

    let wrong = read(
        "[General]\nProjectileInterceptable=yes\n\
         Interceptor.GuardRange.IsCylindrical=yes\n\
         Interceptor.ApplyFirepowerMult=no\n",
    );
    assert!(!wrong.projectile_interceptable);
    assert!(wrong.interceptor_apply_firepower_mult);
}

/// `[AudioVisual] VisualScatter.Min/Max` (`Ext/Rules/Body.cpp:325-326`),
/// constructed 8/32 leptons (`Ext/Rules/Body.h:758-759`).
#[test]
fn the_visual_scatter_bounds_default_and_read_from_audio_visual() {
    let defaults = PhobosGlobals::default();
    assert_eq!(defaults.visual_scatter_min, 8);
    assert_eq!(defaults.visual_scatter_max, 32);

    let set = read("[AudioVisual]\nVisualScatter.Min=2\nVisualScatter.Max=5\n");
    assert_eq!(set.visual_scatter_min, 2);
    assert_eq!(set.visual_scatter_max, 5);

    // A later layer naming only one keeps the other at its constructor value.
    let half = read("[AudioVisual]\nVisualScatter.Max=5\n");
    assert_eq!(half.visual_scatter_min, 8);
    assert_eq!(half.visual_scatter_max, 5);
}

/// `[CombatDamage] Damage*Multiplier` (`Ext/Rules/Body.cpp:340-347`): the
/// three plain columns construct 1.0 and the five `.NotAffectsEnemies`/
/// `.Berzerk` columns construct unset (`Ext/Rules/Body.h:770-777`).
#[test]
fn the_damage_multiplier_globals_default_and_read_from_combat_damage() {
    let defaults = PhobosGlobals::default();
    assert_eq!(defaults.damage_owner_multiplier, 1.0);
    assert_eq!(defaults.damage_allies_multiplier, 1.0);
    assert_eq!(defaults.damage_enemies_multiplier, 1.0);
    assert_eq!(defaults.damage_owner_multiplier_not_affects_enemies, None);
    assert_eq!(defaults.damage_allies_multiplier_not_affects_enemies, None);
    assert_eq!(defaults.damage_owner_multiplier_berzerk, None);
    assert_eq!(defaults.damage_allies_multiplier_berzerk, None);
    assert_eq!(defaults.damage_enemies_multiplier_berzerk, None);
    assert_eq!(read(""), defaults);

    let set = read(
        "[CombatDamage]\nDamageOwnerMultiplier=0.5\nDamageAlliesMultiplier=0.25\n\
         DamageEnemiesMultiplier=2\nDamageOwnerMultiplier.NotAffectsEnemies=0.1\n\
         DamageAlliesMultiplier.NotAffectsEnemies=0.2\nDamageOwnerMultiplier.Berzerk=3\n\
         DamageAlliesMultiplier.Berzerk=4\nDamageEnemiesMultiplier.Berzerk=5\n",
    );
    assert_eq!(set.damage_owner_multiplier, 0.5);
    assert_eq!(set.damage_allies_multiplier, 0.25);
    assert_eq!(set.damage_enemies_multiplier, 2.0);
    assert_eq!(set.damage_owner_multiplier_not_affects_enemies, Some(0.1));
    assert_eq!(set.damage_allies_multiplier_not_affects_enemies, Some(0.2));
    assert_eq!(set.damage_owner_multiplier_berzerk, Some(3.0));
    assert_eq!(set.damage_allies_multiplier_berzerk, Some(4.0));
    assert_eq!(set.damage_enemies_multiplier_berzerk, Some(5.0));

    // The keys belong to `[CombatDamage]`, not `[General]`.
    let wrong = read("[General]\nDamageEnemiesMultiplier=9\n");
    assert_eq!(wrong.damage_enemies_multiplier, 1.0);
}

/// `[AudioVisual] RemoveMindControl.Silent=` and
/// `[CombatDamage] MergeBuildingDamage=` (`Ext/Rules/Body.cpp:597`, `:503`),
/// both constructed `no` (`Ext/Rules/Body.h:998`, `:918`).
#[test]
fn the_mind_control_silent_and_merge_building_damage_globals() {
    let defaults = PhobosGlobals::default();
    assert!(!defaults.remove_mind_control_silent);
    assert!(!defaults.merge_building_damage);
    assert_eq!(read(""), defaults);

    let set = read(
        "[AudioVisual]\nRemoveMindControl.Silent=yes\n[CombatDamage]\nMergeBuildingDamage=yes\n",
    );
    assert!(set.remove_mind_control_silent);
    assert!(set.merge_building_damage);

    // Each key belongs to its own section.
    let wrong = read(
        "[CombatDamage]\nRemoveMindControl.Silent=yes\n[AudioVisual]\nMergeBuildingDamage=yes\n",
    );
    assert!(!wrong.remove_mind_control_silent);
    assert!(!wrong.merge_building_damage);
}

/// `[General] RadarJamHouses=` / `RadarJamDelay=` (`Ext/Rules/Body.cpp:213-214`,
/// constructors `Ext/Rules/Body.h:657-658`) default `Enemies` and `30`, which
/// is the Ares scan interval they replace.
#[test]
fn the_radar_jam_globals_default_and_read_from_general() {
    use crate::rules::phobos_shields::AffectedHouse;

    let defaults = PhobosGlobals::default();
    assert_eq!(defaults.radar_jam_houses, AffectedHouse::ENEMIES);
    assert_eq!(defaults.radar_jam_delay, 30);
    assert_eq!(read(""), defaults);

    let set = read("[General]\nRadarJamHouses=all\nRadarJamDelay=7\n");
    assert_eq!(set.radar_jam_houses, AffectedHouse::ALL);
    assert_eq!(set.radar_jam_delay, 7);

    // `RadarJamHouses` is a `Valueable<AffectedHouse>` here: an unknown token
    // keeps the live value instead of clearing it.
    assert_eq!(
        read("[General]\nRadarJamHouses=bogus\n").radar_jam_houses,
        AffectedHouse::ENEMIES
    );

    // The keys belong to `[General]`, not `[AudioVisual]`.
    assert_eq!(read("[AudioVisual]\nRadarJamDelay=7\n").radar_jam_delay, 30);
}

/// `[General] NoQueueUpToEnter*` (`Ext/Rules/Body.cpp:197-200`) and
/// `[General] VeteranReload*` (`:388-389`), constructors `Ext/Rules/Body.h:640-643`
/// and `:808-809`.
#[test]
fn the_board_queue_and_veteran_reload_globals() {
    let defaults = PhobosGlobals::default();
    assert!(!defaults.no_queue_up_to_enter);
    assert_eq!(defaults.no_queue_up_to_enter_board_distance, 384);
    assert_eq!(defaults.no_queue_up_to_enter_buildings, None);
    assert_eq!(defaults.veteran_reload, 1.0);
    assert_eq!(defaults.veteran_empty_reload, None);
    assert_eq!(read(""), defaults);

    let set = read(
        "[General]\nNoQueueUpToEnter=yes\nNoQueueUpToEnter.BoardDistance=512\n\
         NoQueueUpToEnter.Buildings=yes\nVeteranReload=0.5\nVeteranEmptyReload=2\n",
    );
    assert!(set.no_queue_up_to_enter);
    assert_eq!(set.no_queue_up_to_enter_board_distance, 512);
    assert_eq!(set.no_queue_up_to_enter_buildings, Some(true));
    assert_eq!(set.veteran_reload, 0.5);
    assert_eq!(set.veteran_empty_reload, Some(2.0));
}

/// `[General] UseDisguiseMovementSpeed=` (`Ext/Rules/Body.cpp:221`) and
/// `[AudioVisual] IsVoiceCreatedGlobal=` (`:306`) default `no`, and
/// `[General] HoverDrownable=` (`:475`, constructor `Ext/Rules/Body.h:890`)
/// defaults `yes`.
#[test]
fn the_disguise_voice_and_hover_globals() {
    let defaults = PhobosGlobals::default();
    assert_eq!(defaults.use_disguise_movement_speed, None);
    assert!(!defaults.is_voice_created_global);
    assert!(defaults.hover_drownable, "HoverDrownable constructs yes");
    assert_eq!(read(""), defaults);

    let set = read(
        "[General]\nUseDisguiseMovementSpeed=yes\nHoverDrownable=no\n\
         [AudioVisual]\nIsVoiceCreatedGlobal=yes\n",
    );
    assert_eq!(set.use_disguise_movement_speed, Some(true));
    assert!(!set.hover_drownable);
    assert!(set.is_voice_created_global);

    // `IsVoiceCreatedGlobal` is an `[AudioVisual]` read, not a `[General]` one.
    assert!(!read("[General]\nIsVoiceCreatedGlobal=yes\n").is_voice_created_global);
}
