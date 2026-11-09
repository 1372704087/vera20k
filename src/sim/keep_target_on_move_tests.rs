//! `KeepTargetOnMove.*` through the production move-order and Unit AI owners.
use super::{move_order_keeps_target, update_keep_target_on_move};
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::TargetKind;
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::intern::InternedId;
use crate::sim::mission::MissionId;
use crate::sim::mission::MissionType;
use crate::sim::world::{FrameEffects, Simulation};

/// A `TANK` (Americans) with `KeepTargetOnMove` as asked and an enemy
/// `DUMMY` two cells east, on flat ground.
fn scene(keep: bool) -> (Simulation, RuleSet, InternedId, u64, u64) {
    let key = if keep { "KeepTargetOnMove=yes\n" } else { "" };
    let ini = crate::sim::arena_fixture::clear_ground_ini(&format!(
        "[Countries]\n0=Americans\n1=Russians\n\
         [VehicleTypes]\n0=TANK\n1=DUMMY\n\
         [WeaponTypes]\n0=GUN\n\
         [GUN]\nDamage=10\nROF=20\nRange=9.5\nSpeed=100\nProjectile=INVIS\nWarhead=HIT\n\
         [INVIS]\nInviso=yes\nAG=yes\n\
         [HIT]\nCellSpread=0\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n\
         [TANK]\nStrength=500\nArmor=heavy\nSpeed=6\nPrimary=GUN\n{key}\
         [DUMMY]\nStrength=1000\nArmor=heavy\nCanPassiveAquire=no\n"
    ));
    let rules = RuleSet::from_ini(&ini).expect("rules");
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 0x4B_EE_70);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.session
        .set_game_mode(crate::sim::scenario_session::ScenarioGameMode::from_legacy_nonzero(true));
    let owner = sim.interner.intern("Americans");
    sim.session.set_current_house_for_test(Some(owner));
    sim.session.set_binary_frame_for_test(100);
    let tank = sim
        .spawn_object("TANK", "Americans", 10, 10, 0, &rules)
        .expect("tank");
    let target = sim
        .spawn_object("DUMMY", "Russians", 12, 10, 0, &rules)
        .expect("target");
    for id in [tank, target] {
        let entity = sim
            .entities_mut_for_test()
            .get_mut(id)
            .expect("placed object");
        entity.lifecycle.set_in_limbo_for_test(false);
    }
    assert!(
        sim.assign_target_represented(
            tank,
            Some(TargetKind::Entity(target)),
            &rules,
            None,
            FrameEffects::empty(),
        )
        .is_ok(),
        "the tank holds the target"
    );
    (sim, rules, owner, tank, target)
}

/// Issue a production `Command::Move` and run the queued order.
fn issue_move(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: crate::sim::intern::InternedId,
    tank: u64,
) {
    let commands = vec![CommandEnvelope::new(
        owner,
        1,
        Command::Move {
            entity_id: tank,
            target_rx: 20,
            target_ry: 20,
        },
    )];
    assert!(sim.issue_local_commands(commands));
    sim.transfer_local_commands();
    sim.apply_due_commands(&[], rules, 100, None, Default::default());
}

#[test]
fn a_move_order_keeps_an_in_range_target_only_with_the_key() {
    for keep in [true, false] {
        let (mut sim, rules, owner, tank, target) = scene(keep);
        issue_move(&mut sim, &rules, owner, tank);
        let entity = sim.entities().get(tank).unwrap();
        assert_eq!(
            entity.attack_target.target(),
            keep.then_some(TargetKind::Entity(target)),
            "KeepTargetOnMove={keep}: the move order clears or keeps the target"
        );
        assert_eq!(
            entity.phobos_keep_target_on_move(),
            keep,
            "the latch follows the kept target"
        );
        assert_eq!(
            entity.mission.queued(),
            MissionId::from_known(MissionType::Move),
            "the move mission is still queued"
        );
        assert_eq!(
            entity.navigation.nav_com(),
            Some(crate::sim::components::NavTargetRef::cell(20, 20)),
            "the order destination is stored"
        );
    }
}

#[test]
fn the_unit_ai_drops_a_kept_target_once_it_leaves_range() {
    let (mut sim, rules, _owner, tank, target) = scene(true);
    assert!(move_order_keeps_target(&mut sim, tank, &rules, None));
    let entity = sim.entities().get(tank).unwrap();
    assert_eq!(
        entity.attack_target.target(),
        Some(TargetKind::Entity(target))
    );
    assert!(entity.phobos_keep_target_on_move());
    // The tank's mission must be Move for the update to consider the range.
    sim.entities_mut_for_test()
        .get_mut(tank)
        .unwrap()
        .mission
        .set_current_for_test(MissionId::from_known(MissionType::Move), 0);
    // Move the target beyond the 9.5-cell range (2432 leptons).
    sim.entities_mut_for_test()
        .get_mut(target)
        .unwrap()
        .position
        .put_world_xy([10 * 256 + 128 + 4000, 10 * 256 + 128]);
    update_keep_target_on_move(&mut sim, tank, &rules, None, FrameEffects::empty());
    let entity = sim.entities().get(tank).unwrap();
    assert!(
        entity.attack_target.target().is_none(),
        "a target past the weapon range is dropped"
    );
    assert!(!entity.phobos_keep_target_on_move());
}
