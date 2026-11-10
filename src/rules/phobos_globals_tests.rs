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
