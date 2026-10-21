//! Objects that cannot act over an object or a Cell, on retail rules: a Unit
//! that is not IsActive (`UnitClass::What_Action_OnObject @ 0x0073FD50`,
//! `0x0073FE48`), and one being warped out (Techno `0x006FFF03`,
//! `0x00700731`).

use super::retail_entry_tests::{arena, retail};
use super::*;
use crate::sim::world::Simulation;

/// A Robot Tank whose control is offline (not IsActive, `0x007010D0`)
/// answers Move over an enemy GI it may crush and Select over an enemy tank
/// with Ctrl; an active one attacks both. Without Ctrl it answers 0: its
/// GetFireError (vt+0x3C0) is ILLEGAL, which Techno's attack rung refuses
/// without force (`0x00700542..0x0070055B`). Its Ctrl click on the GI sends
/// no order: `FootClass::ClickedAction_Object` (`0x004D74E0`) refuses Move
/// for an object that is not controllable (vt+0xA0). The one on the tank
/// selects.
#[test]
fn retail_inactive_robot_tank_crushes_or_selects_instead_of_attacking() {
    let Some((retail, registry)) = retail() else {
        return;
    };
    let rules = &retail.rules;
    let mut sim = arena(rules);
    // The Russians keep a building, so the skirmish has no winner.
    sim.spawn_object("NAPOWR", "Russians", 3, 3, 0, rules)
        .expect("russian power plant");
    let robot = sim
        .spawn_object("ROBO", "Americans", 16, 16, 0, rules)
        .expect("robot tank");
    let gi = sim
        .spawn_object("E1", "Russians", 17, 16, 0, rules)
        .expect("gi");
    let tank = sim
        .spawn_object("MTNK", "Russians", 16, 17, 0, rules)
        .expect("tank");
    let action = |sim: &Simulation, target, forced| {
        attack_action(
            sim,
            rules,
            robot,
            &TargetKind::Entity(target),
            forced,
            Some(&registry),
        )
    };
    for forced in [false, true] {
        assert_eq!(action(&sim, gi, forced), AttackAction::Attack);
        assert_eq!(action(&sim, tank, forced), AttackAction::Attack);
    }
    sim.entities_mut_for_test()
        .get_mut(robot)
        .unwrap()
        .deactivated
        .set_for_test(true);
    assert!(!sim.techno_is_active(robot, rules));
    assert_eq!(action(&sim, gi, false), AttackAction::None);
    assert_eq!(action(&sim, tank, false), AttackAction::None);
    assert_eq!(action(&sim, gi, true), AttackAction::Move);
    assert_eq!(action(&sim, tank, true), AttackAction::Select);
    let crush = attack_branch_orders(
        &sim,
        rules,
        &[robot],
        TargetKind::Entity(gi),
        true,
        &registry,
    );
    assert!(crush.consumed && !crush.select_fallback);
    assert!(crush.commands.is_empty(), "{:?}", crush.commands);
    let select = attack_branch_orders(
        &sim,
        rules,
        &[robot],
        TargetKind::Entity(tank),
        true,
        &registry,
    );
    assert!(select.select_fallback && select.commands.is_empty());
}

/// A tank being warped out (Temporal, vt+0x1D4) answers 0 over an enemy
/// (`0x006FFF03..0x006FFF8A`: armed, and another House's object) and on the ground with
/// Ctrl (`0x00700731`), and Select over an own unselected tank: no attack and
/// no force-fire.
#[test]
fn retail_tank_being_warped_out_neither_attacks_nor_fires() {
    let Some((retail, registry)) = retail() else {
        return;
    };
    let rules = &retail.rules;
    let mut sim = arena(rules);
    sim.spawn_object("NAPOWR", "Russians", 3, 3, 0, rules)
        .expect("russian power plant");
    let tank = sim
        .spawn_object("MTNK", "Americans", 16, 16, 0, rules)
        .expect("tank");
    let enemy = sim
        .spawn_object("MTNK", "Russians", 17, 16, 0, rules)
        .expect("enemy");
    let own = sim
        .spawn_object("MTNK", "Americans", 16, 17, 0, rules)
        .expect("own tank");
    let action = |sim: &Simulation, target: TargetKind| {
        attack_action(sim, rules, tank, &target, true, Some(&registry))
    };
    assert_eq!(
        action(&sim, TargetKind::Entity(enemy)),
        AttackAction::Attack
    );
    assert_eq!(action(&sim, TargetKind::Cell(18, 18)), AttackAction::Attack);
    sim.entities_mut_for_test()
        .get_mut(tank)
        .unwrap()
        .set_temporal_for_test(crate::sim::temporal::TemporalState::warped_by_for_test(99));
    assert!(sim.entities().get(tank).unwrap().is_warped_out());
    assert_eq!(action(&sim, TargetKind::Entity(enemy)), AttackAction::None);
    assert_eq!(action(&sim, TargetKind::Cell(18, 18)), AttackAction::None);
    assert_eq!(action(&sim, TargetKind::Entity(own)), AttackAction::Select);
    let fire = attack_branch_orders(
        &sim,
        rules,
        &[tank],
        TargetKind::Cell(18, 18),
        true,
        &registry,
    );
    assert!(fire.commands.is_empty(), "{:?}", fire.commands);
}
