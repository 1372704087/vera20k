//! Phobos `Harvester.CanGuardArea` (`Ext/Unit/Hooks.Harvester.cpp:75-94`)
//! through the Foot Area Guard dispatcher.

use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::house_state::HouseState;
use crate::sim::mission::state::MissionTestFixture;
use crate::sim::mission::{MissionDispatchTimer, MissionId, MissionType};
use crate::sim::world::{ObjectAiCtx, Simulation};

fn rules(miner_extra: &str) -> RuleSet {
    RuleSet::from_ini(&crate::sim::arena_fixture::clear_ground_ini(&format!(
        "[VehicleTypes]\n0=MINER\n\
         [MINER]\nStrength=300\nSight=8\nSpeed=4\nHarvester=yes\nPrimary=MinerGun\n{miner_extra}\
         [MinerGun]\nDamage=10\nRange=6\nROF=20\nProjectile=Invisible\nWarhead=GUARDWH\n\
         [Warheads]\n0=GUARDWH\n\
         [GUARDWH]\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n"
    )))
    .expect("harvester guard rules")
}

fn world(rules: &RuleSet, human: bool) -> Simulation {
    let mut sim = Simulation::new();
    crate::sim::arena_fixture::flat_ground(&mut sim, rules);
    let id = sim.intern("Americans");
    sim.houses
        .register(HouseState::new(id, 0, None, human, 0, 10));
    sim
}

fn area_guard(sim: &mut Simulation, id: u64) {
    sim.substrate
        .entities
        .get_mut(id)
        .unwrap()
        .mission
        .apply_test_fixture(MissionTestFixture {
            current: MissionId::from_known(MissionType::AreaGuard),
            suspended: MissionId::NONE,
            queued: MissionId::NONE,
            movement_bypass_latch: 0,
            handler_state: 0,
            mission_start_frame: 0,
            ai_counter: 0,
            dispatch_timer: MissionDispatchTimer::at_frame(0),
        });
}

/// One dispatcher visit's committed mission for a harvester set on Area
/// Guard.
fn visited_mission(miner_extra: &str, human: bool) -> MissionType {
    let rules = rules(miner_extra);
    let mut sim = world(&rules, human);
    let miner = sim
        .spawn_object("MINER", "Americans", 10, 10, 0, &rules)
        .expect("MINER spawns");
    area_guard(&mut sim, miner);
    crate::sim::world::dispatch_foot_mission(&mut sim, miner, &rules, ObjectAiCtx::default())
        .expect("the visit runs");
    sim.substrate
        .entities
        .get(miner)
        .unwrap()
        .mission
        .current()
        .known()
        .expect("a committed mission")
}

/// `0x4D6D34`: only a human-owned harvester whose type sets the key skips
/// the stock `Queue_Mission(Harvest, 0)` switch.
#[test]
fn a_human_harvester_with_the_key_keeps_area_guard() {
    assert_eq!(
        visited_mission("Harvester.CanGuardArea=yes\n", true),
        MissionType::AreaGuard
    );
    assert_eq!(
        visited_mission("", true),
        MissionType::Harvest,
        "without the key the stock switch runs"
    );
    assert_eq!(
        visited_mission("Harvester.CanGuardArea=yes\n", false),
        MissionType::Harvest,
        "a computer harvester keeps the stock switch"
    );
}

/// `RequireTarget`: with no threat in range the gate refuses and the stock
/// Harvest switch runs.
///
/// RESIDUAL (test only): this checkout's synthetic fixture cannot make the
/// shared Area Guard scanner form a candidate (the retail scanner
/// configuration is unavailable here), so the positive "threat keeps Area
/// Guard" arm is not exercised; the scan call itself is the same owner the
/// Area Guard body uses.
#[test]
fn require_target_without_a_threat_falls_back_to_harvest() {
    assert_eq!(
        visited_mission(
            "Harvester.CanGuardArea=yes\nHarvester.CanGuardArea.RequireTarget=yes\n",
            true
        ),
        MissionType::Harvest,
        "no threat: the scan refuses and Harvest starts"
    );
}
