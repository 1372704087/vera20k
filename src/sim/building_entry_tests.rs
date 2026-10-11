//! Tests for the building-entry seeks. The pick replays native
//! EnterBioReactor's rows (`tools/team_recruit_oracle.py`'s `bio_reactor`,
//! under Unicorn); the production callers below are Rust regression tests.

use serde_json::{Value, json};

use super::*;
use crate::rules::ini_parser::IniFile;
use crate::rules::team_ai_ini::TeamAiDefinitionSource;
use crate::sim::house_state::HouseState;
use crate::sim::passenger::PassengerRole;
use crate::sim::pathfinding::PathGrid;
use crate::sim::team_script_vm::{TeamScriptAction, TeamScriptDefinition};

const RULES: &str = "\
[InfantryTypes]
0=INIT
1=OVERSIZE
[VehicleTypes]
0=LTNK
1=TRNS
[BuildingTypes]
0=YAPOWR
1=GAPOWR
2=UABSORB
[INIT]
Strength=100
Speed=4
Locomotor={4A582744-9839-11D1-B709-00A024DDAFD1}
[OVERSIZE]
Strength=100
Speed=4
Size=16
Locomotor={4A582744-9839-11D1-B709-00A024DDAFD1}
[LTNK]
Strength=300
Speed=6
Locomotor={4A582741-9839-11D1-B709-00A024DDAFD1}
[TRNS]
Strength=300
Speed=6
Passengers=2
Locomotor={4A582741-9839-11D1-B709-00A024DDAFD1}
[YAPOWR]
Strength=700
InfantryAbsorb=yes
Passengers=5
SizeLimit=15
[GAPOWR]
Strength=750
[UABSORB]
Strength=700
UnitAbsorb=yes
Passengers=5
SizeLimit=15
";

const ART: &str = "[YAPOWR]\nFoundation=2x2\n[GAPOWR]\nFoundation=2x2\n[UABSORB]\nFoundation=2x2\n";

fn rules() -> RuleSet {
    RuleSet::from_ini_with_fixed_art_for_test(&IniFile::from_str(RULES), &IniFile::from_str(ART))
        .expect("building entry rules")
}

/// The flat arena with the computer YuriCountry and the human Americans.
fn arena(rules: &RuleSet) -> (Simulation, PathGrid) {
    let mut sim = Simulation::with_seed(0x4DFB70);
    for (name, side, human) in [("YuriCountry", 2, false), ("Americans", 0, true)] {
        let id = sim.interner.intern(name);
        sim.houses
            .insert(id, HouseState::new(id, side, None, human, 0, 10));
        sim.session.house_order.push(id);
    }
    let grid = crate::sim::arena_fixture::flat_arena(&mut sim, rules);
    (sim, grid)
}

fn spawn(sim: &mut Simulation, rules: &RuleSet, kind: &str, owner: &str, rx: u16, ry: u16) -> u64 {
    sim.spawn_object_at_height(kind, owner, rx, ry, 0, 0, rules)
        .unwrap_or_else(|| panic!("{kind} spawns"))
}

/// Fills `count` of `building`'s passenger slots.
fn absorb(sim: &mut Simulation, rules: &RuleSet, building: u64, count: usize) {
    for _ in 0..count {
        let id = sim
            .construct_object_limbo_at_height("INIT", "YuriCountry", 1, 1, 0, 0, rules)
            .expect("absorbed infantry");
        sim.substrate.entities.get_mut(id).unwrap().passenger_role = PassengerRole::Inside {
            transport_id: building,
            open_topped: false,
        };
        let building = sim.substrate.entities.get_mut(building).unwrap();
        building
            .passenger_role
            .cargo_mut()
            .unwrap()
            .board_forced(id, 1);
    }
}

fn can_load(sim: &mut Simulation, rules: &RuleSet, from: u64, building: u64) -> RadioResponse {
    transmit(
        sim,
        from,
        building,
        RadioMessage::CanEnter,
        RadioPayload::default(),
        Some(rules),
        FrameEffects::default(),
    )
}

fn seek(sim: &mut Simulation, rules: &RuleSet, id: u64) -> bool {
    sim.foot_enter_bio_reactor(id, rules, None, FrameEffects::default())
}

fn entity(sim: &Simulation, id: u64) -> &GameEntity {
    sim.substrate.entities.get(id).unwrap()
}

/// Runs the production frame until `done` holds, for at most `frames`
/// frames; answers whether it held.
fn run_until(
    sim: &mut Simulation,
    rules: &RuleSet,
    grid: &PathGrid,
    frames: usize,
    done: impl Fn(&Simulation) -> bool,
) -> bool {
    for _ in 0..frames {
        if done(sim) {
            return true;
        }
        let commands = sim.take_due_commands();
        sim.advance_tick(&commands, Some(rules), Some(grid), None, 33);
    }
    done(sim)
}

/// A team running script action 62, then 49, of `members`.
fn bio_reactor_team(sim: &mut Simulation, members: Vec<u64>) -> u64 {
    let script = sim.interner.intern("BIO SCRIPT");
    sim.team_script_vm.register_script(TeamScriptDefinition {
        id: script,
        source: TeamAiDefinitionSource::FixedAimd,
        actions: vec![
            TeamScriptAction {
                action_id: 62,
                argument: 0,
            },
            TeamScriptAction {
                action_id: 49,
                argument: 0,
            },
        ],
    });
    let owner = sim.interner.intern("YuriCountry");
    sim.team_script_vm.create_team(owner, script, members, 0)
}

/// Native EnterBioReactor (`0x004DFB70`), its CAN_LOAD answers supplied:
/// each absorber is asked from the list's last, and the pick, its reissue
/// (Queue_Mission(Enter, 0) and Assign_Destination(pick, 1)), the answer
/// and the latch are [`pick_bio_reactor`]'s over [`object_distance`]:
/// strictly nearer wins, ties to the later absorber, distances that differ
/// below a lepton tie, and a pick already the first radio contact or the
/// pending entry is not reissued.
#[test]
fn the_bio_reactor_pick_matches_native() {
    let oracle: Value =
        serde_json::from_str(crate::test_fixture::text("tools/team_recruit_oracle.json")).unwrap();
    let rows = oracle["bio_reactor"].as_array().unwrap();
    assert_eq!(rows.len(), 129);
    let enter = MissionId::from_known(MissionType::Enter).raw();
    let xyz = |value: &Value| -> [i32; 3] {
        let axes: Vec<i32> = value
            .as_array()
            .unwrap()
            .iter()
            .map(|axis| i32::try_from(axis.as_i64().unwrap()).unwrap())
            .collect();
        axes.try_into().unwrap()
    };
    let mut picked_rows = 0;
    for (number, row) in rows.iter().enumerate() {
        let events = row["events"].as_array().unwrap();
        let of = |kinds: &[&str]| -> Vec<Value> {
            events
                .iter()
                .filter(|event| kinds.iter().any(|kind| event[0] == *kind))
                .cloned()
                .collect()
        };
        let absorbers = row["absorbers"].as_array().unwrap();
        let walk: Vec<u64> = (0..absorbers.len() as u64).rev().collect();
        let asked: Vec<Value> = walk
            .iter()
            .map(|&index| json!(["transmit", RadioMessage::CanEnter as u8, index]))
            .collect();
        assert_eq!(of(&["transmit"]), asked, "row {number}: the walk");

        let foot = xyz(&row["foot"]);
        let (best, reissue) = pick_bio_reactor(
            walk.iter().map(|&index| {
                let absorber = &absorbers[index as usize];
                let roger = absorber["answer"] == RadioResponse::Roger.code();
                let distance = if roger {
                    object_distance(foot, xyz(&absorber["xyz"]), None)
                } else {
                    i32::MAX
                };
                (index, roger, distance)
            }),
            row["contact"].as_u64(),
            row["pending"].as_u64(),
        );
        let native_pick = of(&["picked"]).first().and_then(|event| event[1].as_u64());
        assert_eq!(best, native_pick, "row {number}: the pick");
        let reissued: Vec<Value> = best
            .filter(|_| reissue)
            .map(|best| {
                vec![
                    json!(["queue_mission", enter, 0]),
                    json!(["assign_destination", best, 1]),
                ]
            })
            .unwrap_or_default();
        assert_eq!(
            of(&["queue_mission", "assign_destination"]),
            reissued,
            "row {number}: the reissue"
        );
        assert_eq!(
            row["result"] == 1,
            best.is_some(),
            "row {number}: the answer"
        );
        assert_eq!(
            row["latch_after"] == 1,
            best.is_some(),
            "row {number}: the latch"
        );
        picked_rows += usize::from(best.is_some());
    }
    assert_eq!(picked_rows, 89);
}

/// `BuildingClass::Receive_Radio`'s CAN_LOAD absorber arm
/// (`0x0043C43B..0x0043C4F2`): ROGER while a slot is free, the later arms'
/// STATIC to a sender above `SizeLimit=` or when full, NEGATORY to a class
/// the absorber does not take.
#[test]
fn an_absorber_answers_can_load_by_room_size_and_class() {
    let rules = rules();
    let (mut sim, _) = arena(&rules);
    let reactor = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 16, 10);
    let infantry = spawn(&mut sim, &rules, "INIT", "YuriCountry", 10, 10);
    let tank = spawn(&mut sim, &rules, "LTNK", "YuriCountry", 10, 12);
    let oversize = spawn(&mut sim, &rules, "OVERSIZE", "YuriCountry", 10, 14);

    absorb(&mut sim, &rules, reactor, 4);
    assert_eq!(
        can_load(&mut sim, &rules, infantry, reactor),
        RadioResponse::Roger
    );
    assert_eq!(
        can_load(&mut sim, &rules, tank, reactor),
        RadioResponse::Negatory
    );
    assert_eq!(
        can_load(&mut sim, &rules, oversize, reactor),
        RadioResponse::None
    );
    absorb(&mut sim, &rules, reactor, 1);
    assert_eq!(
        can_load(&mut sim, &rules, infantry, reactor),
        RadioResponse::None
    );
}

/// The CAN_LOAD absorber arm refuses a sender controlling a captive
/// (`0x004722C0`), though a slot is free.
#[test]
fn an_absorber_refuses_a_controller() {
    let rules = rules();
    let (mut sim, _) = arena(&rules);
    let reactor = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 16, 10);
    let controller = spawn(&mut sim, &rules, "INIT", "YuriCountry", 10, 10);
    let captive = spawn(&mut sim, &rules, "INIT", "Americans", 10, 12);
    assert_eq!(
        can_load(&mut sim, &rules, controller, reactor),
        RadioResponse::Roger
    );

    sim.substrate
        .entities
        .get_mut(controller)
        .unwrap()
        .capture_manager = Some(
        crate::sim::capture_manager::CaptureManagerState::with_victims_for_test(
            1,
            false,
            &[captive],
        ),
    );

    assert_eq!(
        can_load(&mut sim, &rules, controller, reactor),
        RadioResponse::Negatory
    );
}

/// EnterBioReactor over its house's absorbers only: a power plant, a full
/// reactor and another house's reactor stand nearer than the reactor with
/// room that takes the Foot. The Foot latches, queues Enter, heads for it
/// and boards through the adapter (module residual).
#[test]
fn the_nearest_own_reactor_with_room_takes_the_foot() {
    let rules = rules();
    let (mut sim, _) = arena(&rules);
    let foot = spawn(&mut sim, &rules, "INIT", "YuriCountry", 10, 10);
    let _plant = spawn(&mut sim, &rules, "GAPOWR", "YuriCountry", 12, 12);
    let _enemy = spawn(&mut sim, &rules, "YAPOWR", "Americans", 7, 10);
    let full = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 13, 8);
    absorb(&mut sim, &rules, full, 5);
    let room = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 18, 10);
    let _far = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 24, 10);

    assert!(seek(&mut sim, &rules, foot));

    let entity = sim.substrate.entities.get(foot).unwrap();
    assert!(entity.building_entry.should_enter_absorber());
    assert_eq!(
        entity.mission.queued(),
        MissionId::from_known(MissionType::Enter)
    );
    assert_eq!(entity.navigation.nav_com, Some(NavTargetRef::object(room)));
    assert!(matches!(
        entity.passenger_role,
        PassengerRole::Boarding { target_transport_id } if target_transport_id == room
    ));
}

/// Equidistant absorbers through the production walk: the house's list is
/// walked from its last and only a strictly nearer absorber replaces the
/// pick, so the one built later takes the Foot, whichever side it stands.
/// The seeker is a vehicle, which stands at its cell's centre (an
/// infantryman's sub-cell spot would break the mirror).
#[test]
fn of_equidistant_absorbers_the_later_built_takes_the_foot() {
    let rules = rules();
    for cells in [[(13, 8), (8, 13)], [(8, 13), (13, 8)]] {
        let (mut sim, _) = arena(&rules);
        let foot = spawn(&mut sim, &rules, "LTNK", "YuriCountry", 10, 10);
        let reactors =
            cells.map(|(rx, ry)| spawn(&mut sim, &rules, "UABSORB", "YuriCountry", rx, ry));
        let coords = |id| {
            let coords = object_get_coords(entity(&sim, id), sim.resolved_terrain.as_ref());
            [coords.x, coords.y, coords.z]
        };
        let [first, later] =
            reactors.map(|reactor| object_distance(coords(foot), coords(reactor), None));
        assert_eq!(first, later, "the absorbers stand equally far");

        assert!(seek(&mut sim, &rules, foot));

        assert_eq!(
            entity(&sim, foot).navigation.nav_com,
            Some(NavTargetRef::object(reactors[1]))
        );
    }
}

/// A Foot that is a transport keeps its cargo: the seek orders it to the
/// absorber, but the Boarding adapter, which keeps one role per object, is
/// not given it (module residual).
#[test]
fn a_transport_sent_to_an_absorber_keeps_its_cargo() {
    let rules = rules();
    let (mut sim, _) = arena(&rules);
    let absorber = spawn(&mut sim, &rules, "UABSORB", "YuriCountry", 16, 10);
    let transport = spawn(&mut sim, &rules, "TRNS", "YuriCountry", 10, 10);
    assert!(entity(&sim, transport).passenger_role.cargo().is_some());

    assert!(seek(&mut sim, &rules, transport));

    let entity = entity(&sim, transport);
    assert!(entity.building_entry.should_enter_absorber());
    assert_eq!(
        entity.mission.queued(),
        MissionId::from_known(MissionType::Enter)
    );
    assert_eq!(
        entity.navigation.nav_com,
        Some(NavTargetRef::object(absorber))
    );
    assert!(entity.passenger_role.cargo().is_some());
}

/// The latch is saved and, while set, hashed.
#[test]
fn the_latch_is_saved_and_hashed() {
    let rules = rules();
    let (mut sim, _) = arena(&rules);
    let _reactor = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 16, 10);
    let foot = spawn(&mut sim, &rules, "INIT", "YuriCountry", 10, 10);
    assert!(seek(&mut sim, &rules, foot));

    let bytes = crate::sim::snapshot::GameSnapshot::save_validated(&sim, 0, 0, "Seek", 0);
    let mut restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    restored.restore_after_snapshot_load().unwrap();

    let entity = restored.substrate.entities.get(foot).unwrap();
    assert!(entity.building_entry.should_enter_absorber());
    let latched = restored.state_hash();
    set_should_enter_absorber_for_test(restored.substrate.entities.get_mut(foot).unwrap(), false);
    assert_ne!(restored.state_hash(), latched);
}

/// With no absorber of its house taking it, EnterBioReactor clears the
/// latch, answers false and gives no order.
#[test]
fn no_taker_clears_the_latch() {
    let rules = rules();
    let (mut sim, _) = arena(&rules);
    let tank = spawn(&mut sim, &rules, "LTNK", "YuriCountry", 10, 12);
    let _reactor = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 16, 10);
    set_should_enter_absorber_for_test(sim.substrate.entities.get_mut(tank).unwrap(), true);
    let mission = sim.substrate.entities.get(tank).unwrap().mission;

    assert!(!seek(&mut sim, &rules, tank));

    let entity = sim.substrate.entities.get(tank).unwrap();
    assert!(!entity.building_entry.seeking());
    assert_eq!(entity.mission, mission);
    assert_eq!(entity.navigation.nav_com, None);
    assert!(matches!(entity.passenger_role, PassengerRole::None));
}

/// Script action 62 (`0x006E9E83`) through the team pass: the member a
/// reactor takes leaves the team, the one none takes stays, and the action
/// is finished.
#[test]
fn script_action_62_sends_the_members_a_reactor_takes() {
    let rules = rules();
    let (mut sim, _) = arena(&rules);
    let _reactor = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 18, 10);
    let infantry = spawn(&mut sim, &rules, "INIT", "YuriCountry", 10, 10);
    let tank = spawn(&mut sim, &rules, "LTNK", "YuriCountry", 10, 12);
    let team = bio_reactor_team(&mut sim, vec![infantry, tank]);

    sim.run_team_ai_pass(&rules, None, FrameEffects::default());

    let state = sim.team_script_vm.team(team).unwrap();
    assert_eq!(state.members().collect::<Vec<_>>(), [tank]);
    assert!(state.advance_pending());
    assert_eq!(sim.team_script_vm.team_for_member(infantry), None);
    let entity = sim.substrate.entities.get(infantry).unwrap();
    assert!(entity.building_entry.seeking());
    assert_eq!(
        entity.mission.queued(),
        MissionId::from_known(MissionType::Enter)
    );
    assert!(
        !sim.substrate
            .entities
            .get(tank)
            .unwrap()
            .building_entry
            .seeking()
    );
}

/// `TeamClass::Can_Add` refuses a Foot seeking a building's entry
/// (`Is_Seeking_Building_Entry`, `0x006EA79B`).
#[test]
fn a_seeking_foot_is_not_recruited() {
    let rules = rules();
    let (mut sim, _) = arena(&rules);
    let infantry = spawn(&mut sim, &rules, "INIT", "YuriCountry", 10, 10);
    let team = bio_reactor_team(&mut sim, Vec::new());
    assert_eq!(sim.team_can_add(team, infantry, true, &rules), Some(0));

    set_should_enter_absorber_for_test(sim.substrate.entities.get_mut(infantry).unwrap(), true);

    assert_eq!(sim.team_can_add(team, infantry, true, &rules), None);
}

/// Through the production frame: a team's action 62 walks its infantryman
/// into the house's Bio Reactor, and the emptied team is destroyed.
#[test]
fn a_bio_reactor_team_member_walks_into_the_reactor() {
    let rules = rules();
    let (mut sim, grid) = arena(&rules);
    let reactor = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 16, 10);
    let infantry = spawn(&mut sim, &rules, "INIT", "YuriCountry", 10, 10);
    let team = bio_reactor_team(&mut sim, vec![infantry]);

    assert!(run_until(&mut sim, &rules, &grid, 300, |sim| {
        entity(sim, infantry).passenger_role.inside_transport_id() == Some(reactor)
    }));

    let cargo = entity(&sim, reactor).passenger_role.cargo().unwrap();
    assert_eq!(cargo.count(), 1);
    assert!(sim.team_script_vm.team(team).is_none());
}

/// A reactor destroyed while the Foot walks to it: the adapter lets the
/// Foot go, Mission_Enter's no-target arm idles it (`0x004D9425..
/// 0x004D9466`), and its Guard head seeks again, finds no taker and clears
/// the latch, so a team may recruit it again.
#[test]
fn a_seeker_whose_reactor_dies_gives_up_and_can_be_recruited() {
    let rules = rules();
    let (mut sim, grid) = arena(&rules);
    let reactor = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 24, 10);
    let infantry = spawn(&mut sim, &rules, "INIT", "YuriCountry", 10, 10);
    let _team = bio_reactor_team(&mut sim, vec![infantry]);
    assert!(run_until(&mut sim, &rules, &grid, 30, |sim| {
        matches!(
            entity(sim, infantry).passenger_role,
            PassengerRole::Boarding { target_transport_id } if target_transport_id == reactor
        )
    }));

    sim.despawn_entity(reactor);

    assert!(run_until(&mut sim, &rules, &grid, 300, |sim| {
        !entity(sim, infantry).building_entry.seeking()
    }));
    let foot = entity(&sim, infantry);
    assert!(matches!(foot.passenger_role, PassengerRole::None));
    assert_ne!(
        foot.mission.current(),
        MissionId::from_known(MissionType::Enter)
    );
    let team = bio_reactor_team(&mut sim, Vec::new());
    assert!(sim.team_can_add(team, infantry, true, &rules).is_some());
}

/// Two seekers for a reactor's last slot: the first in takes it, the
/// adapter turns the other away on arrival (module residual), Mission_Enter
/// idles it, and its Guard head finds no taker and clears the latch.
#[test]
fn a_seeker_beaten_to_the_last_slot_gives_up() {
    let rules = rules();
    let (mut sim, grid) = arena(&rules);
    let reactor = spawn(&mut sim, &rules, "YAPOWR", "YuriCountry", 16, 10);
    absorb(&mut sim, &rules, reactor, 4);
    let near = spawn(&mut sim, &rules, "INIT", "YuriCountry", 12, 10);
    let far = spawn(&mut sim, &rules, "INIT", "YuriCountry", 8, 10);
    let _team = bio_reactor_team(&mut sim, vec![near, far]);
    assert!(run_until(&mut sim, &rules, &grid, 30, |sim| {
        [near, far]
            .iter()
            .all(|&id| entity(sim, id).building_entry.seeking())
    }));

    assert!(run_until(&mut sim, &rules, &grid, 900, |sim| {
        entity(sim, near).passenger_role.inside_transport_id() == Some(reactor)
            && !entity(sim, far).building_entry.seeking()
    }));
    assert_eq!(
        entity(&sim, reactor)
            .passenger_role
            .cargo()
            .unwrap()
            .count(),
        5
    );
    let foot = entity(&sim, far);
    assert!(matches!(foot.passenger_role, PassengerRole::None));
    assert_ne!(
        foot.mission.current(),
        MissionId::from_known(MissionType::Enter)
    );
}

/// Retail rules (every layer, gamemd-exact): the Bio Reactor takes a Yuri
/// Initiate (`Size=1` within `SizeLimit=15`) and refuses a Lasher (no
/// `UnitAbsorb=`).
#[test]
fn a_retail_bio_reactor_takes_infantry_only() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")
    else {
        return;
    };
    let rules = &retail.rules;
    let (mut sim, _) = arena(rules);
    let reactor = spawn(&mut sim, rules, "YAPOWR", "YuriCountry", 16, 10);
    let initiate = spawn(&mut sim, rules, "INIT", "YuriCountry", 10, 10);
    let lasher = spawn(&mut sim, rules, "LTNK", "YuriCountry", 10, 12);

    assert_eq!(
        can_load(&mut sim, rules, initiate, reactor),
        RadioResponse::Roger
    );
    assert_eq!(
        can_load(&mut sim, rules, lasher, reactor),
        RadioResponse::Negatory
    );
}
