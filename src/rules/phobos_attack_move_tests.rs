//! `AttackMove.*` reads (`Ext/TechnoType/Body.cpp:1086-1087`,
//! `Ext/Rules/Body.cpp:377-378,423`).
use super::PhobosAttackMove;
use crate::rules::ini_parser::IniFile;
use crate::rules::phobos_globals::PhobosGlobals;

#[test]
fn attack_move_type_keys_default_to_unset_with_false_globals() {
    let ini = IniFile::from_str("[MTNK]\n");
    let section = ini.section_or_empty("MTNK");
    let set = PhobosAttackMove::from_section(&section);
    assert_eq!(set, PhobosAttackMove::default());
    assert_eq!((set.aggressive, set.update_target), (None, None));

    let globals = PhobosGlobals::default();
    assert!(!globals.attack_move_aggressive);
    assert!(!globals.attack_move_update_target);
    assert!(!globals.attack_move_ignore_weapon_check);
    // Both unset type keys answer the `[General]` fallback, which is `no`.
    assert!(!set.aggressive(&globals));
    assert!(!set.update_target(&globals));
}

#[test]
fn attack_move_type_keys_override_and_clear_back_to_the_global() {
    let globals = PhobosGlobals {
        attack_move_aggressive: true,
        attack_move_update_target: true,
        ..Default::default()
    };

    let set = PhobosAttackMove::from_section(
        IniFile::from_str("[MTNK]\nAttackMove.Aggressive=yes\nAttackMove.UpdateTarget=no\n")
            .section_or_empty("MTNK"),
    );
    assert_eq!(
        (set.aggressive, set.update_target),
        (Some(true), Some(false))
    );
    assert!(
        set.aggressive(&globals),
        "the type key wins over the global"
    );
    assert!(
        !set.update_target(&globals),
        "an explicit `no` is not the global `yes`"
    );

    // `Phobos Nullable` clears on `<default>`/`none`, so the fallback returns.
    let cleared = PhobosAttackMove::from_section(
        IniFile::from_str(
            "[MTNK]\nAttackMove.Aggressive=<default>\nAttackMove.UpdateTarget=none\n",
        )
        .section_or_empty("MTNK"),
    );
    assert_eq!((cleared.aggressive, cleared.update_target), (None, None));
    assert!(cleared.aggressive(&globals));
    assert!(cleared.update_target(&globals));
}

#[test]
fn attack_move_general_keys_read_the_three_flags() {
    let globals = PhobosGlobals::from_ini(&IniFile::from_str(
        "[General]\nAttackMove.Aggressive=yes\nAttackMove.UpdateTarget=yes\n\
         AttackMove.IgnoreWeaponCheck=yes\n",
    ));
    assert!(globals.attack_move_aggressive);
    assert!(globals.attack_move_update_target);
    assert!(globals.attack_move_ignore_weapon_check);

    // An absent key keeps the constructor's `false`.
    let empty = PhobosGlobals::from_ini(&IniFile::from_str("[General]\n"));
    assert!(!empty.attack_move_aggressive);
    assert!(!empty.attack_move_update_target);
    assert!(!empty.attack_move_ignore_weapon_check);
}
