//! The `[CombatDamage] Shrapnel.*` globals (`Ext/Rules/Body.cpp`), whose
//! `Nullable` default reads `no`.
use super::PhobosGlobals;
use crate::rules::ini_parser::IniFile;

fn read(text: &str) -> PhobosGlobals {
    PhobosGlobals::from_ini(&IniFile::from_str(text))
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
