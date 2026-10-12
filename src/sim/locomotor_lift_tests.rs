//! Rust regressions for the Magnetron's lift through the production frame on
//! retail rules (the local `ini/`; skipped without it), and for the state it
//! keeps. They check VERA's composition of the native bodies cited in
//! `locomotor_lift`, `jumpjet_flight` and `world::crash`; no native execution
//! backs them.

use super::LocomotorShotTarget;
use crate::rules::locomotor_type::LocomotorKind;
use crate::rules::ruleset::RuleSet;
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::game_entity::GameEntity;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::pathfinding::PathGrid;
use crate::sim::world::{FrameEffects, Simulation};

fn retail_rules() -> Option<RuleSet> {
    let (rules_ini, art_ini) = crate::rules::retail_ini_fixture::retail_rules_and_art()?;
    let mut rules = RuleSet::from_ini_with_fixed_art_for_test(&rules_ini, &art_ini).unwrap();
    rules.install_art_data(crate::rules::art_data::ArtRegistry::from_ini(&art_ini));
    Some(rules)
}

/// The flat arena: `Americans` (human) field the Magnetron, `Russians`
/// (computer) the tanks.
fn arena(rules: &RuleSet) -> (Simulation, PathGrid) {
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 0x1269);
    sim.input_delay_ticks = 0;
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    let grid = sim
        .path_grid_snapshot()
        .map(|grid| (*grid).clone())
        .unwrap();
    (sim, grid)
}

fn spawn(sim: &mut Simulation, rules: &RuleSet, kind: &str, owner: &str, rx: u16, ry: u16) -> u64 {
    sim.spawn_object(kind, owner, rx, ry, 0, rules)
        .unwrap_or_else(|| panic!("{kind} spawns"))
}

fn entity(sim: &Simulation, id: u64) -> &GameEntity {
    sim.substrate.entities.get(id).expect("live object")
}

fn order(sim: &mut Simulation, house: &str, command: Command) {
    let owner = sim.interner.get(house).unwrap();
    let tick = sim.session.tick;
    sim.queue_command(CommandEnvelope::new(owner, tick + 1, command));
}

/// Runs the production frame until `done` holds, for at most `frames`
/// frames; answers whether it held.
fn run_until(
    sim: &mut Simulation,
    rules: &RuleSet,
    grid: &PathGrid,
    frames: u32,
    mut done: impl FnMut(&Simulation) -> bool,
) -> bool {
    for _ in 0..frames {
        let commands = sim.take_due_commands();
        sim.advance_tick(&commands, Some(rules), Some(grid), None, 33);
        if done(sim) {
            return true;
        }
    }
    false
}

/// A Magnetron ordered to attack a tank lifts it: no damage, a Jumpjet over
/// its Drive and both links (Imbue `0x00710000`).
fn lift(rules: &RuleSet) -> (Simulation, PathGrid, u64, u64) {
    let (mut sim, grid) = arena(rules);
    let magnetron = spawn(&mut sim, rules, "TELE", "Americans", 10, 16);
    let tank = spawn(&mut sim, rules, "MTNK", "Russians", 16, 16);
    let health = entity(&sim, tank).health.current;
    order(
        &mut sim,
        "Americans",
        Command::Attack {
            attacker_id: magnetron,
            target_id: tank,
        },
    );
    assert!(
        run_until(&mut sim, rules, &grid, 120, |sim| {
            entity(sim, tank).foot_locomotor_swap_active
        }),
        "the beam lifts the tank"
    );
    let held = entity(&sim, tank);
    assert_eq!(held.locomotor_source, Some(magnetron));
    assert_eq!(entity(&sim, magnetron).locomotor_target, Some(tank));
    assert_eq!(held.health.current, health, "the beam deals no damage");
    let locomotor = held.locomotor.as_ref().unwrap();
    assert_eq!(locomotor.active_kind(), LocomotorKind::Jumpjet);
    assert!(locomotor.piggyback.is_some(), "the Drive is suspended");
    (sim, grid, magnetron, tank)
}

/// Held all the way, the tank is let go over the Magnetron (State 3's
/// arrival, `0x0054C1CB`), falls onto it and crashes there: the falling
/// damage destroys it and books the kill for the Magnetron's house.
#[test]
fn retail_magnetron_pulls_a_tank_onto_itself() {
    let Some(rules) = retail_rules() else {
        return;
    };
    let (mut sim, grid, magnetron, tank) = lift(&rules);
    let americans = sim.interner.get("Americans").unwrap();
    let kills = sim.houses[&americans].stats.units_killed();

    assert!(
        run_until(&mut sim, &rules, &grid, 600, |sim| {
            entity(sim, tank).being_manipulated
        }),
        "the tank arrives and is dropped"
    );
    let dropped = entity(&sim, tank);
    assert!(dropped.crashing);
    assert_eq!(dropped.manipulated_by, Some(magnetron));
    assert_eq!(dropped.warped_by_house, Some(americans));
    assert_eq!(dropped.locomotor_source, None);
    let holder = entity(&sim, magnetron);
    assert_eq!(holder.locomotor_target, None);
    assert!(
        holder.attack_target.is_none(),
        "the release clears its target"
    );
    let drop_cell = (dropped.position.rx, dropped.position.ry);
    assert_eq!(
        drop_cell,
        (holder.position.rx, holder.position.ry),
        "over the Magnetron"
    );

    assert!(
        run_until(&mut sim, &rules, &grid, 200, |sim| {
            !sim.substrate.entities.contains(tank)
        }),
        "the tank crashes onto the Magnetron"
    );
    assert_eq!(
        sim.houses[&americans].stats.units_killed(),
        kills + 1,
        "the drop's credit books the kill"
    );
}

/// Let go mid-flight, by a move order (the firer's own destination,
/// `0x004D9509`) or by the Magnetron's death (its UnInit, `0x004DE5F0`), the
/// tank is centred on its cell, falls, lands on the clear ground and, once
/// down, gets its Drive back unharmed (Jumpjet END `0x0054DA50`).
#[test]
fn retail_tank_let_go_over_clear_ground_lands_and_drives_again() {
    let Some(rules) = retail_rules() else {
        return;
    };
    for death in [false, true] {
        let (mut sim, grid, magnetron, tank) = lift(&rules);
        let health = entity(&sim, tank).health.current;
        assert!(
            run_until(&mut sim, &rules, &grid, 200, |sim| {
                crate::sim::movement::air_movement::current_fly_height(
                    entity(sim, tank),
                    sim.resolved_terrain.as_ref(),
                ) > 200
            }),
            "the tank climbs"
        );
        if death {
            sim.uninit_with_context(
                magnetron,
                crate::sim::world::UninitContext::new(Some(&rules), None),
            );
            assert!(!sim.substrate.entities.contains(magnetron));
        } else {
            order(
                &mut sim,
                "Americans",
                Command::Move {
                    entity_id: magnetron,
                    target_rx: 4,
                    target_ry: 16,
                    queue: false,
                },
            );
            assert!(
                run_until(&mut sim, &rules, &grid, 5, |sim| {
                    entity(sim, magnetron).locomotor_target.is_none()
                }),
                "the move order lets the tank go"
            );
        }
        let dropped = entity(&sim, tank);
        assert!(dropped.being_manipulated, "death {death}");
        assert_eq!(dropped.locomotor_source, None, "death {death}");
        assert_eq!(
            dropped.manipulated_by,
            (!death).then_some(magnetron),
            "an expiring Magnetron leaves no credit (death {death})"
        );

        assert!(
            run_until(&mut sim, &rules, &grid, 400, |sim| {
                !entity(sim, tank).foot_locomotor_swap_active
            }),
            "the tank lands and its lift ends (death {death})"
        );
        let landed = entity(&sim, tank);
        assert_eq!(landed.health.current, health, "death {death}");
        assert!(!landed.crashing && !landed.being_manipulated);
        assert!(!landed.lift_may_end);
        assert_eq!(landed.manipulated_by, None);
        assert_eq!(landed.warped_by_house, None);
        let locomotor = landed.locomotor.as_ref().unwrap();
        assert_eq!(locomotor.active_kind(), LocomotorKind::Drive);
        assert!(locomotor.piggyback.is_none());
        let location = crate::sim::movement::ground_pose::position_world_coord(&landed.position);
        assert_eq!(
            (location.x & 0xFF, location.y & 0xFF),
            (0x80, 0x80),
            "centred on its cell before the fall (death {death})"
        );
    }
}

/// The arm's admission (`0x004694CB..0x00469700`): the shot must exceed the
/// target's `Size=` (`FCOMP`, equal refuses); only a Unit is lifted; a shot
/// at another tank first lets the held one go (`0x004695FD`), on the ground
/// a release that lets its lift end. A computer house's lifted tank queues
/// Hunt (`0x007103D5`).
#[test]
fn retail_locomotor_arm_admission() {
    let Some(rules) = retail_rules() else {
        return;
    };
    assert_eq!(rules.object("MTNK").unwrap().size, 3);
    for (strength, lifted) in [(3, false), (4, true)] {
        let (mut sim, _) = arena(&rules);
        let magnetron = spawn(&mut sim, &rules, "TELE", "Americans", 10, 16);
        let tank = spawn(&mut sim, &rules, "MTNK", "Russians", 16, 16);
        sim.locomotor_detonation(
            magnetron,
            LocomotorShotTarget::Object(tank),
            strength,
            LocomotorKind::Jumpjet,
            &rules,
            None,
            FrameEffects::default(),
        );
        assert_eq!(entity(&sim, tank).foot_locomotor_swap_active, lifted);
        assert_eq!(
            entity(&sim, magnetron).locomotor_target,
            lifted.then_some(tank)
        );
        if lifted {
            assert_eq!(
                entity(&sim, tank).mission.queued(),
                MissionId::from_known(MissionType::Hunt)
            );
        }
    }

    let (mut sim, _) = arena(&rules);
    let magnetron = spawn(&mut sim, &rules, "TELE", "Americans", 10, 16);
    let gi = spawn(&mut sim, &rules, "E1", "Russians", 16, 16);
    let first = spawn(&mut sim, &rules, "MTNK", "Russians", 16, 18);
    let second = spawn(&mut sim, &rules, "MTNK", "Russians", 16, 20);
    let shoot = |sim: &mut Simulation, target| {
        sim.locomotor_detonation(
            magnetron,
            target,
            5000,
            LocomotorKind::Jumpjet,
            &rules,
            None,
            FrameEffects::default(),
        );
    };
    shoot(&mut sim, LocomotorShotTarget::Object(gi));
    shoot(&mut sim, LocomotorShotTarget::Cell);
    assert!(!entity(&sim, gi).foot_locomotor_swap_active);
    assert_eq!(entity(&sim, magnetron).locomotor_target, None);

    shoot(&mut sim, LocomotorShotTarget::Object(first));
    assert_eq!(entity(&sim, magnetron).locomotor_target, Some(first));
    shoot(&mut sim, LocomotorShotTarget::Object(first));
    assert_eq!(entity(&sim, first).locomotor_source, Some(magnetron));

    shoot(&mut sim, LocomotorShotTarget::Object(second));
    assert_eq!(entity(&sim, magnetron).locomotor_target, Some(second));
    assert_eq!(entity(&sim, second).locomotor_source, Some(magnetron));
    let released = entity(&sim, first);
    assert_eq!(released.locomotor_source, None);
    assert!(
        released.lift_may_end,
        "a grounded release lets the lift end"
    );
    assert!(!released.crashing && !released.being_manipulated);
}

/// The links and the drop's credit are saved, and each one moves the world
/// hash.
#[test]
fn lift_links_and_drop_credit_are_saved_and_hashed() {
    let rules = RuleSet::from_ini(&crate::rules::ini_parser::IniFile::from_str(
        "[VehicleTypes]\n0=TANK\n[TANK]\nStrength=300\nSpeed=7\n",
    ))
    .unwrap();
    let (mut sim, _) = arena(&rules);
    let magnetron = spawn(&mut sim, &rules, "TANK", "Americans", 10, 16);
    let tank = spawn(&mut sim, &rules, "TANK", "Russians", 16, 16);
    let house = sim.interner.get("Americans").unwrap();
    sim.scenario_rng = crate::sim::rng::SimRng::new(0);
    type Write = fn(&mut GameEntity, u64, crate::sim::intern::InternedId);
    let writes: [(u64, Write); 6] = [
        (magnetron, |entity, other, _| {
            entity.locomotor_target = Some(other)
        }),
        (tank, |entity, other, _| {
            entity.locomotor_source = Some(other)
        }),
        (tank, |entity, _, _| entity.lift_may_end = true),
        (tank, |entity, _, _| entity.being_manipulated = true),
        (tank, |entity, other, _| entity.manipulated_by = Some(other)),
        (tank, |entity, _, house| {
            entity.warped_by_house = Some(house)
        }),
    ];
    for (id, write) in writes {
        let before = sim.state_hash();
        let other = if id == tank { magnetron } else { tank };
        write(sim.substrate.entities.get_mut(id).unwrap(), other, house);
        assert_ne!(sim.state_hash(), before);
    }
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "magnetron_lift", 0);
    let loaded = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    assert_eq!(loaded.state_hash(), sim.state_hash());
    let held = entity(&loaded, tank);
    assert_eq!(held.locomotor_source, Some(magnetron));
    assert!(held.lift_may_end && held.being_manipulated);
    assert_eq!(held.manipulated_by, Some(magnetron));
    assert_eq!(held.warped_by_house, Some(house));
    assert_eq!(entity(&loaded, magnetron).locomotor_target, Some(tank));
}
