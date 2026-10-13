//! Rust regression tests for the reach block of `TechnoClass::AI_Update`
//! (`0x006FAAEF..0x006FABB8`, [`super::target_reach_step`]): a Foot outside a
//! team whose target lies in another zone runs Approach. Expected behaviour
//! comes from instruction reading; the search is Approach's own.

use super::{FrameEffects, RuleSet, Simulation, target_reach_step};
use crate::map::resolved_terrain::zone_class;
use crate::rules::ini_parser::IniFile;
use crate::sim::combat::{AttackTarget, TargetKind};
use crate::sim::components::NavTargetRef;
use crate::sim::mission::{MissionId, MissionType};

/// A tank with a 4-cell gun: 896 leptons of Approach reach (`0x004D5797`).
fn rules() -> RuleSet {
    RuleSet::from_ini(&IniFile::from_str(
        "[VehicleTypes]\n0=TANK\n\
         [TANK]\nStrength=300\nArmor=heavy\nSpeed=6\nSight=10\nPrimary=TankGun\n\
         Locomotor={4A582741-9839-11D1-B709-00A024DDAFD1}\n\
         [TankGun]\nDamage=10\nROF=20\nRange=4\nProjectile=InvisibleAll\nWarhead=HITWH\n\
         [InvisibleAll]\nInviso=yes\nAA=yes\nAG=yes\n\
         [HITWH]\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n",
    ))
    .expect("reach rules")
}

/// The flat arena, cut at x = 12..=13 by impassable cells when `split`, with
/// a human `Americans` tank at (8, 10) on `mission` holding a computer
/// `Russians` tank at (`target_x`, 10). Answers the tank and its target.
fn pair(
    rules: &RuleSet,
    split: bool,
    target_x: u16,
    mission: MissionType,
) -> (Simulation, u64, TargetKind) {
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 0x6FAAEF);
    if split {
        let terrain = sim.resolved_terrain.as_mut().unwrap();
        for ry in 0..32 {
            for rx in 12..=13 {
                let cell = terrain.cell_mut(rx, ry).unwrap();
                cell.is_cliff_like = true;
                cell.ground_walk_blocked = true;
                cell.base_ground_walk_blocked = true;
                cell.zone_type = zone_class::IMPASSABLE;
            }
        }
        assert!(sim.rebuild_dynamic_navigation(rules));
    }
    let tank = sim
        .spawn_object_at_height("TANK", "Americans", 8, 10, 0, 0, rules)
        .expect("tank spawns");
    let target = sim
        .spawn_object_at_height("TANK", "Russians", target_x, 10, 0, 0, rules)
        .expect("target spawns");
    let now = sim.session.binary_frame;
    sim.mission_assign_exact(tank, MissionId::from_known(mission), now)
        .unwrap();
    let _ = sim.mission_commence_exact(tank, now);
    sim.substrate.entities.get_mut(tank).unwrap().attack_target = Some(AttackTarget::new(target));
    (sim, tank, TargetKind::Entity(target))
}

fn reach(
    sim: &mut Simulation,
    rules: &RuleSet,
    tank: u64,
) -> (Option<TargetKind>, Option<NavTargetRef>) {
    target_reach_step(sim, tank, rules, None, FrameEffects::default());
    let tank = sim.substrate.entities.get(tank).unwrap();
    (
        tank.attack_target.as_ref().map(|attack| attack.target),
        tank.navigation.nav_com,
    )
}

/// A target in another zone is approached on any mission: from (8, 10) on
/// Guard the tank takes a cell on its own side within reach of the target
/// across the band at (14, 10), and keeps the target.
#[test]
fn a_guarding_tank_approaches_a_target_it_cannot_reach() {
    let rules = rules();
    let (mut sim, tank, held) = pair(&rules, true, 14, MissionType::Guard);

    let (target, nav_com) = reach(&mut sim, &rules, tank);

    assert_eq!(target, Some(held));
    let Some(NavTargetRef::Cell { rx, ry }) = nav_com else {
        panic!("Approach sets a firing cell: {nav_com:?}");
    };
    assert!(
        rx < 12,
        "the firing cell ({rx}, {ry}) lies on the tank's side"
    );
    let (dx, dy) = (i32::from(rx) - 14, i32::from(ry) - 10);
    assert!(
        (dx * dx + dy * dy) * 256 * 256 <= 896 * 896,
        "({rx}, {ry}) lies within reach"
    );
}

/// A reachable target ends the block (`0x006FAB52`): out of range on Guard,
/// the tank neither approaches it nor lets it go; Guard has no Approach.
#[test]
fn a_tank_leaves_a_reachable_target_to_its_mission() {
    let rules = rules();
    let (mut sim, tank, held) = pair(&rules, false, 16, MissionType::Guard);

    assert_eq!(reach(&mut sim, &rules, tank), (Some(held), None));
}

/// A team member (vt+0x290, `0x0041C050`) skips the block whatever its
/// target.
#[test]
fn a_team_member_leaves_an_unreachable_target_to_its_team() {
    let rules = rules();
    let (mut sim, tank, held) = pair(&rules, true, 14, MissionType::Guard);
    crate::sim::team_script_vm::join_team_for_test(&mut sim, &[tank], false, false);

    assert_eq!(reach(&mut sim, &rules, tank), (Some(held), None));
}

/// On Sticky, an out-of-range target is dropped by Approach
/// (`0x004D5716..0x004D5744`), which the block runs only for a target in
/// another zone: a reachable one is kept.
#[test]
fn a_sticky_tank_drops_only_a_target_it_cannot_reach() {
    let rules = rules();
    for (split, dropped) in [(true, true), (false, false)] {
        let (mut sim, tank, held) = pair(&rules, split, 14, MissionType::Sticky);

        let kept = (!dropped).then_some(held);
        assert_eq!(
            reach(&mut sim, &rules, tank),
            (kept, None),
            "split: {split}"
        );
    }
}
