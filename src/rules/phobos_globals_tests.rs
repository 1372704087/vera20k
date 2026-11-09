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
