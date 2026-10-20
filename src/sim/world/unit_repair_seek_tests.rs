use super::*;
use crate::sim::arena_fixture::{flood, rival_arena_with_size};
use crate::sim::pathfinding::PathGrid;

const ARENA: u16 = 48;

/// The retail-rules rival arena (`Russians` a computer house, `Americans`
/// human) at frame 160, a multiple of 16, with its path grid.
fn scene() -> Option<(Simulation, RuleSet, PathGrid)> {
    let rules = crate::rules::retail_ini_fixture::retail_battle_rules()?.rules;
    let mut sim = rival_arena_with_size(&rules, 0x0073_671F, ARENA);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.session
        .set_game_mode(crate::sim::scenario_session::ScenarioGameMode::from_legacy_nonzero(true));
    sim.session
        .set_current_house_for_test(sim.interner.get("Americans"));
    sim.session.set_binary_frame_for_test(160);
    let grid = sim
        .path_grid_snapshot()
        .map(|grid| (*grid).clone())
        .unwrap();
    Some((sim, rules, grid))
}

fn spawn(sim: &mut Simulation, rules: &RuleSet, kind: &str, owner: &str, x: u16, y: u16) -> u64 {
    sim.spawn_object_at_height(kind, owner, x, y, 0, 0, rules)
        .unwrap_or_else(|| panic!("{kind} spawns at ({x}, {y})"))
}

/// A tank of `owner` at `cell` on `mission`, at half its Strength.
fn damaged_tank(
    sim: &mut Simulation,
    rules: &RuleSet,
    kind: &str,
    owner: &str,
    cell: (u16, u16),
    mission: MissionType,
) -> u64 {
    let tank = spawn(sim, rules, kind, owner, cell.0, cell.1);
    let strength = rules.object(kind).unwrap().strength;
    let now = sim.session.binary_frame();
    let _ = sim.mission_assign_exact(tank, MissionId::from_known(mission), now);
    sim.substrate
        .entities
        .get_mut(tank)
        .unwrap()
        .health
        .set_for_test(strength / 2);
    tank
}

fn seek(sim: &mut Simulation, rules: &RuleSet, id: u64) {
    sim.unit_ai_seek_repair(id, rules, None, FrameEffects::default());
}

fn queued(sim: &Simulation, id: u64) -> MissionId {
    sim.substrate.entities.get(id).unwrap().mission.queued()
}

fn heads_for(sim: &Simulation, id: u64, station: u64) -> bool {
    let unit = sim.substrate.entities.get(id).unwrap();
    unit.navigation.nav_com() == Some(NavTargetRef::object(station))
        || unit.pending_entry() == Some(station)
        || unit.radio_contacts.slot(0) == Some(station)
}

#[test]
fn the_station_reach_is_twenty_cell_diagonals() {
    // ftol(Sqrt_Approx(131072) * 20): 20 * 362.04 leptons.
    assert_eq!(unit_repair_reach(), 7240);
}

/// A damaged computer tank on Guard archives its Cell, queues Enter, drops
/// its Target and takes its House's Service Depot as its destination.
#[test]
fn a_damaged_computer_tank_on_guard_heads_for_its_depot() {
    let Some((mut sim, rules, _)) = scene() else {
        return;
    };
    let depot = spawn(&mut sim, &rules, "GADEPT", "Russians", 24, 20);
    let tank = damaged_tank(
        &mut sim,
        &rules,
        "HTNK",
        "Russians",
        (14, 20),
        MissionType::Guard,
    );
    let _ = sim.assign_target_represented(
        tank,
        Some(TargetKind::Cell(30, 30)),
        Some(&rules),
        None,
        FrameEffects::default(),
    );

    seek(&mut sim, &rules, tank);

    let unit = sim.substrate.entities.get(tank).unwrap();
    assert_eq!(unit.archive_target(), Some(TargetKind::Cell(14, 20)));
    assert_eq!(unit.attack_target.target(), None);
    assert_eq!(
        sim.substrate.entities.get(tank).unwrap().mission.queued(),
        MissionId::from_known(MissionType::Enter)
    );
    assert!(heads_for(&sim, tank, depot));
}

/// The gates of `0x0073671F..0x0073679A`: the 16-frame cadence, a computer
/// House, Guard or AreaGuard with nothing queued, damage, no ArchiveTarget
/// unless on AreaGuard, and no `Harvester=`.
#[test]
fn only_a_damaged_idle_computer_vehicle_seeks_repair() {
    let Some((mut sim, rules, _)) = scene() else {
        return;
    };
    let _depot = spawn(&mut sim, &rules, "GADEPT", "Russians", 24, 20);
    let _player_depot = spawn(&mut sim, &rules, "GADEPT", "Americans", 24, 30);
    let enter = MissionId::from_known(MissionType::Enter);

    let off_cadence = damaged_tank(
        &mut sim,
        &rules,
        "HTNK",
        "Russians",
        (14, 14),
        MissionType::Guard,
    );
    sim.session.set_binary_frame_for_test(161);
    seek(&mut sim, &rules, off_cadence);
    assert_eq!(queued(&sim, off_cadence), MissionId::NONE);
    sim.session.set_binary_frame_for_test(176);
    seek(&mut sim, &rules, off_cadence);
    assert_eq!(queued(&sim, off_cadence), enter);

    let human = damaged_tank(
        &mut sim,
        &rules,
        "MTNK",
        "Americans",
        (14, 30),
        MissionType::Guard,
    );
    seek(&mut sim, &rules, human);
    assert_eq!(queued(&sim, human), MissionId::NONE);

    let whole = damaged_tank(
        &mut sim,
        &rules,
        "HTNK",
        "Russians",
        (14, 16),
        MissionType::Guard,
    );
    let strength = rules.object("HTNK").unwrap().strength;
    sim.substrate
        .entities
        .get_mut(whole)
        .unwrap()
        .health
        .set_for_test(strength);
    seek(&mut sim, &rules, whole);
    assert_eq!(queued(&sim, whole), MissionId::NONE);

    let hunting = damaged_tank(
        &mut sim,
        &rules,
        "HTNK",
        "Russians",
        (14, 18),
        MissionType::Hunt,
    );
    seek(&mut sim, &rules, hunting);
    assert_eq!(queued(&sim, hunting), MissionId::NONE);

    let posted = damaged_tank(
        &mut sim,
        &rules,
        "HTNK",
        "Russians",
        (14, 22),
        MissionType::Guard,
    );
    let post = Some(TargetKind::Cell(3, 3));
    sim.substrate
        .entities
        .get_mut(posted)
        .unwrap()
        .set_archive_target(post);
    seek(&mut sim, &rules, posted);
    assert_eq!(queued(&sim, posted), MissionId::NONE);

    // On AreaGuard an ArchiveTarget does not bar the seek.
    let area = damaged_tank(
        &mut sim,
        &rules,
        "HTNK",
        "Russians",
        (14, 24),
        MissionType::AreaGuard,
    );
    sim.substrate
        .entities
        .get_mut(area)
        .unwrap()
        .set_archive_target(post);
    seek(&mut sim, &rules, area);
    assert_eq!(queued(&sim, area), enter);

    let miner = damaged_tank(
        &mut sim,
        &rules,
        "HARV",
        "Russians",
        (14, 26),
        MissionType::Guard,
    );
    seek(&mut sim, &rules, miner);
    assert_eq!(queued(&sim, miner), MissionId::NONE);
}

/// `0x00455DD0` admits any nonzero CAN_LOAD answer: the Naval Yard answers a
/// land tank NEGATORY (`0x0043C3F3`), and being nearer it is the pick. A
/// station beyond the reach is not.
#[test]
fn a_refusing_naval_yard_nearer_than_the_depot_is_the_pick() {
    let Some((mut sim, rules, _)) = scene() else {
        return;
    };
    flood(
        &mut sim,
        &rules,
        ARENA,
        (30..40).flat_map(|y| (4..14).map(move |x| (x, y))),
    );
    let yard = spawn(&mut sim, &rules, "GAYARD", "Russians", 6, 32);
    let depot = spawn(&mut sim, &rules, "GADEPT", "Russians", 30, 20);
    let tank = damaged_tank(
        &mut sim,
        &rules,
        "HTNK",
        "Russians",
        (16, 28),
        MissionType::Guard,
    );
    assert_eq!(
        transmit(
            &mut sim,
            tank,
            yard,
            RadioMessage::CanEnter,
            RadioPayload::default(),
            Some(&rules),
            FrameEffects::default(),
        ),
        RadioResponse::Negatory
    );

    assert_eq!(
        sim.nearest_unit_repair_station(tank, &rules, FrameEffects::default()),
        Some(yard)
    );

    // Beyond 7240 leptons of both, nothing is chosen.
    let far = damaged_tank(
        &mut sim,
        &rules,
        "HTNK",
        "Russians",
        (46, 46),
        MissionType::Guard,
    );
    let _ = depot;
    assert_eq!(
        sim.nearest_unit_repair_station(far, &rules, FrameEffects::default()),
        None
    );
}

/// Through the production frame: the seek runs on a frame that is a
/// multiple of 16, after the Unit's second Ready/Commence, and the next
/// frame commences Enter toward the depot.
#[test]
fn the_production_frame_sends_a_damaged_computer_tank_to_its_depot() {
    let Some((mut sim, rules, grid)) = scene() else {
        return;
    };
    // A computer House with no base sells off and sends everyone hunting
    // (Strategy `0x004FD879..0x004FD904`); a passive one runs no Strategy.
    let russians = sim.interner.get("Russians").unwrap();
    sim.houses
        .get_mut(&russians)
        .unwrap()
        .set_multiplay_passive_for_test(true);
    let depot = spawn(&mut sim, &rules, "GADEPT", "Russians", 24, 20);
    let tank = damaged_tank(
        &mut sim,
        &rules,
        "HTNK",
        "Russians",
        (14, 20),
        MissionType::Guard,
    );
    let enter = MissionId::from_known(MissionType::Enter);
    let mut entered = false;
    for _ in 0..40 {
        let commands = sim.take_due_commands();
        sim.advance_tick(&commands, Some(&rules), Some(&grid), None, 33);
        let unit = sim.substrate.entities.get(tank).unwrap();
        if unit.mission.current() == enter || unit.mission.queued() == enter {
            entered = true;
            break;
        }
    }
    assert!(entered, "the seek runs within 16 frames");
    assert_eq!(
        sim.substrate.entities.get(tank).unwrap().archive_target(),
        Some(TargetKind::Cell(14, 20))
    );
    assert!(heads_for(&sim, tank, depot));
}
