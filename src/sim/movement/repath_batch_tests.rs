//! Rust regression tests for the batched code-2 re-path searches
//! ([`super`]). A batched answer is checked against Find_Path's own search
//! on the same world; no native behavior is claimed.
use super::*;
use crate::rules::ini_parser::IniFile;
use crate::sim::components::NavTargetRef;
use crate::sim::movement::FacingClass;
use crate::sim::movement::locomotor::MovementLayer;
use crate::sim::overlay_grid::OverlayGrid;
use crate::sim::world::FrameEffects;
use crate::util::fixed_math::SIM_ONE;

const EAST: u16 = 0x4000;
const WEST: u16 = 0xC000;
const WEST_STEP: u8 = 6;

/// A tank, a tank that cannot move, an infantryman and a one-cell building.
fn rules() -> RuleSet {
    RuleSet::from_ini(&IniFile::from_str(
        "[VehicleTypes]\n0=TANK\n1=STUCK\n[InfantryTypes]\n0=E1\n[BuildingTypes]\n0=HUT\n\
         [TANK]\nStrength=300\nSpeed=6\n\
         Locomotor={4A582741-9839-11D1-B709-00A024DDAFD1}\n\
         [STUCK]\nStrength=300\nSpeed=0\n\
         Locomotor={4A582741-9839-11D1-B709-00A024DDAFD1}\n\
         [E1]\nStrength=100\nSpeed=4\n\
         Locomotor={4A582744-9839-11D1-B709-00A024DDAFD1}\n\
         [HUT]\nStrength=100\n",
    ))
    .expect("batch rules")
}

/// The flat arena with an overlay grid, which the batch needs.
fn arena(rules: &RuleSet) -> (Simulation, PathGrid) {
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 0x4D3920);
    sim.overlay_grid = Some(OverlayGrid::new(32, 32));
    let grid = sim
        .path_grid_snapshot()
        .map(|grid| (*grid).clone())
        .expect("arena grid");
    (sim, grid)
}

fn spawn(sim: &mut Simulation, rules: &RuleSet, type_name: &str, cell: (u16, u16)) -> u64 {
    sim.spawn_object_at_height(type_name, "Americans", cell.0, cell.1, 0, 0, rules)
        .expect("spawns")
}

fn send(sim: &mut Simulation, rules: &RuleSet, id: u64, cell: (u16, u16)) {
    sim.assign_destination_represented(
        id,
        Some(NavTargetRef::cell(cell.0, cell.1)),
        Some(rules),
        None,
        FrameEffects::default(),
    )
    .expect("takes the destination");
}

/// A Foot rolling at full speed along `facing`.
fn rolling(sim: &mut Simulation, id: u64, facing: u16) {
    let foot = sim.substrate.entities.get_mut(id).unwrap();
    foot.foot_speed.set_speed_fraction(SIM_ONE);
    foot.body_facing = FacingClass::new(facing, 0);
}

/// A tank at (10,10) and an infantryman at (10,14), both rolling east toward
/// cells ten east, each with huts north and south of its next cell and, in
/// that cell, a friendly tank rolling west into the mover's own cell: the
/// code-2 chain from there runs back through the mover. Lifted, the mover
/// clears it and the straight route is cheapest; listed, it jams and the
/// route goes round the huts.
fn jam(rules: &RuleSet) -> (Simulation, [u64; 2]) {
    let (mut sim, _) = arena(rules);
    let mut movers = [0; 2];
    for (mover, (type_name, y)) in [("TANK", 10), ("E1", 14)].into_iter().enumerate() {
        spawn(&mut sim, rules, "HUT", (11, y - 1));
        spawn(&mut sim, rules, "HUT", (11, y + 1));
        let id = spawn(&mut sim, rules, type_name, (10, y));
        send(&mut sim, rules, id, (20, y));
        rolling(&mut sim, id, EAST);
        let blocker = spawn(&mut sim, rules, "TANK", (11, y));
        send(&mut sim, rules, blocker, (2, y));
        rolling(&mut sim, blocker, WEST);
        // A path west, replayed by the urgency-2 markers.
        sim.substrate
            .entities
            .get_mut(blocker)
            .unwrap()
            .navigation
            .path_replay
            .directions = vec![WEST_STEP; 3];
        movers[mover] = id;
    }
    (sim, movers)
}

/// The jam's chains run back through the movers: listed they jam, skipped
/// they clear.
#[test]
fn the_jam_chains_run_through_the_movers() {
    let rules = rules();
    let (sim, movers) = jam(&rules);
    for (mover, y) in movers.into_iter().zip([10, 14]) {
        let jams = |unlisted| {
            sim.astar_code2_chain_jams((11, y), MovementLayer::Ground, unlisted)
                .unwrap()
        };
        assert!(jams(None), "row {y}: the chain runs back through the mover");
        assert!(!jams(Some(mover)), "row {y}: without the mover it clears");
    }
}

/// A batched answer is the search Find_Path makes itself on the same world:
/// at urgency 1, whose markers fall back to the code-2 chain, and at 2, for
/// the tank whose chain runs through its own cell and for the infantryman.
#[test]
fn a_batched_answer_is_the_serial_search() {
    let rules = rules();
    for urgency in [1, 2] {
        for mover in 0..2 {
            let search = |batched: bool| {
                let (mut sim, movers) = jam(&rules);
                let id = movers[mover];
                let frame = sim.session.binary_frame;
                let runtime = &mut sim
                    .substrate
                    .entities
                    .get_mut(id)
                    .unwrap()
                    .navigation
                    .path_runtime;
                runtime.path_blocked = urgency == 2;
                runtime.start_blocked(frame, 0);
                if batched {
                    sim.batch_repath_searches();
                    assert!(sim.defer_code2_search(id));
                    sim.answer_batched_repaths(Some(&rules), None);
                }
                let request = sim.code2_ladder_request(id).expect("a ladder request");
                assert_eq!(request.urgency, urgency);
                let found = sim
                    .foot_find_path(&request, &rules, None, FrameEffects::default())
                    .expect("Find_Path runs");
                if batched {
                    let batch = sim.repath_batch.as_ref().unwrap();
                    assert_eq!(batch.used, 1, "Find_Path used the answer");
                    assert_eq!(batch.ready.get(&id), Some(&None));
                }
                let route = sim.substrate.entities.get(id).unwrap();
                (found, route.navigation.path_replay.directions.clone())
            };
            let serial = search(false);
            assert!(
                !serial.1.is_empty(),
                "urgency {urgency}, mover {mover} routes"
            );
            assert_eq!(search(true), serial, "urgency {urgency}, mover {mover}");
        }
    }
}

/// An answer serves only the request it was computed for: a ladder whose
/// blockage timer latched after the batch asks at urgency 2 and searches for
/// itself.
#[test]
fn an_answer_for_another_request_is_not_used() {
    let rules = rules();
    let (mut sim, [tank, _]) = jam(&rules);
    sim.batch_repath_searches();
    assert!(sim.defer_code2_search(tank));
    sim.answer_batched_repaths(Some(&rules), None);
    assert!(
        !sim.defer_code2_search(tank),
        "an answered Foot no longer waits"
    );
    let frame = sim.session.binary_frame;
    let runtime = &mut sim
        .substrate
        .entities
        .get_mut(tank)
        .unwrap()
        .navigation
        .path_runtime;
    runtime.path_blocked = true;
    runtime.start_blocked(frame, 0);
    let request = sim.code2_ladder_request(tank).unwrap();
    assert_eq!(request.urgency, 2);
    sim.foot_find_path(&request, &rules, None, FrameEffects::default())
        .expect("Find_Path runs");
    let batch = sim.repath_batch.as_ref().unwrap();
    assert_eq!(batch.used, 0);
    assert!(
        batch.ready[&tank].is_some(),
        "the unused answer is left for the frame"
    );
}

/// A request whose target redirects (a building on the destination) is left
/// to the Foot's own Find_Path, which retargets its NavCom before searching.
#[test]
fn a_redirected_request_is_left_to_find_path() {
    let rules = rules();
    let (mut sim, [tank, _]) = jam(&rules);
    spawn(&mut sim, &rules, "HUT", (20, 10));
    let destination = sim.code2_ladder_request(tank).unwrap().destination;
    let answer = sim
        .find_path_target_answer(tank, destination, &rules, None)
        .unwrap()
        .expect("the precheck admits the search");
    assert!(
        sim.find_path_redirect(tank, destination, answer, &rules)
            .unwrap()
            .is_some(),
        "the building on the destination redirects (code {answer})"
    );
    sim.batch_repath_searches();
    assert!(sim.defer_code2_search(tank));
    sim.answer_batched_repaths(Some(&rules), None);
    assert_eq!(
        sim.repath_batch.as_ref().unwrap().ready.get(&tank),
        Some(&None)
    );
    let request = sim.code2_ladder_request(tank).unwrap();
    sim.foot_find_path(&request, &rules, None, FrameEffects::default())
        .expect("Find_Path runs");
    assert_eq!(sim.repath_batch.as_ref().unwrap().used, 0);
}

/// Each answer is its search's own, whatever thread ran it: the same
/// searches one at a time on this thread agree. Movers along the map edge
/// stamp the shared Dummy from their finishing passes.
#[test]
fn batched_answers_do_not_depend_on_the_threads() {
    let rules = rules();
    let (mut sim, _) = arena(&rules);
    let mut movers = Vec::new();
    for i in 0..12u16 {
        let tank = spawn(&mut sim, &rules, "TANK", (2 + 2 * i, 1));
        send(&mut sim, &rules, tank, (29 - 2 * i, 1));
        let infantry = spawn(&mut sim, &rules, "E1", (1, 3 + 2 * i));
        send(&mut sim, &rules, infantry, (1, 29 - 2 * i));
        movers.extend([tank, infantry]);
    }
    sim.batch_repath_searches();
    for &id in &movers {
        assert!(sim.defer_code2_search(id));
    }
    sim.answer_batched_repaths(Some(&rules), None);
    let grid = sim.path_grid_snapshot().unwrap();
    let counts = sim.movement_pass_cache.current_blocker_plane();
    let dummy = sim.resolved_terrain.as_ref().unwrap().shared_cell_dummy();
    let ready = &sim.repath_batch.as_ref().unwrap().ready;
    let mut routes = 0;
    for &id in &movers {
        let alone = dummy.detached(|| sim.batched_search(id, &rules, None, &grid, counts));
        routes += usize::from(alone.as_ref().is_some_and(|answer| answer.found.is_ok()));
        assert_eq!(ready[&id], alone, "mover {id}");
    }
    assert_eq!(routes, movers.len(), "every mover routes");
}

/// Whole frames do not depend on the thread count: tanks and infantrymen
/// crossing each other's rows re-path through the batch, and every frame's
/// state hash is the same on one worker thread as on four.
#[test]
fn whole_frames_do_not_depend_on_the_thread_count() {
    let rules = rules();
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("a worker pool");
        pool.install(|| {
            let (mut sim, grid) = arena(&rules);
            for i in 0..12u16 {
                let tank = spawn(&mut sim, &rules, "TANK", (2 + 2 * i, 4));
                send(&mut sim, &rules, tank, (29 - 2 * i, 4));
                let infantry = spawn(&mut sim, &rules, "E1", (4, 3 + 2 * i));
                send(&mut sim, &rules, infantry, (4, 29 - 2 * i));
            }
            sim.batch_repath_searches();
            let hashes: Vec<u64> = (0..150)
                .map(|_| {
                    sim.advance_tick(&[], Some(&rules), Some(&grid), None, 67);
                    sim.state_hash()
                })
                .collect();
            (hashes, sim.repath_batch.as_ref().unwrap().used)
        })
    };
    let (one, used) = run(1);
    assert!(used > 0, "the crossing movers re-path through the batch");
    assert_eq!(run(4), (one, used));
}

/// A tank, then an infantryman, follows a tank that cannot move down a
/// corridor and stays blocked behind it with code 2. Each pass answers
/// exactly the previous frame's registrations, a Foot that waited did not
/// search that frame, and its ladder (Drive's, then Walk's) uses the answer.
#[test]
fn a_waiting_ladder_uses_its_answer_the_next_frame() {
    let rules = rules();
    for follower in ["TANK", "E1"] {
        let (mut sim, grid) = arena(&rules);
        for x in 6..18 {
            spawn(&mut sim, &rules, "HUT", (x, 9));
            spawn(&mut sim, &rules, "HUT", (x, 11));
        }
        let mover = spawn(&mut sim, &rules, follower, (8, 10));
        let stuck = spawn(&mut sim, &rules, "STUCK", (12, 10));
        send(&mut sim, &rules, mover, (24, 10));
        send(&mut sim, &rules, stuck, (24, 10));
        sim.batch_repath_searches();
        let mut waited = 0;
        for _ in 0..300 {
            let mut registered = sim.repath_batch.as_ref().unwrap().pending.clone();
            registered.sort_unstable();
            registered.dedup();
            sim.advance_tick(&[], Some(&rules), Some(&grid), None, 67);
            let batch = sim.repath_batch.as_ref().unwrap();
            let answered: Vec<u64> = batch.ready.keys().copied().collect();
            assert_eq!(answered, registered, "{follower}");
            let frame = sim.session.binary_frame as i32;
            for id in &batch.pending {
                let runtime = &sim
                    .substrate
                    .entities
                    .get(*id)
                    .unwrap()
                    .navigation
                    .path_runtime;
                assert!(
                    runtime.movement_timer.expired(frame),
                    "{follower}: a Foot that waits has not searched"
                );
                waited += 1;
            }
        }
        let used = sim.repath_batch.as_ref().unwrap().used;
        assert!(
            waited > 0 && used > 0,
            "{follower}: waited {waited}, used {used}"
        );
    }
}

/// A saved game keeps the batching and the Foots waiting for the next
/// frame's batch; this frame's answers are not saved.
#[test]
fn registrations_survive_a_save_and_answers_do_not() {
    let rules = rules();
    let (mut sim, [tank, infantry]) = jam(&rules);
    sim.batch_repath_searches();
    assert!(sim.defer_code2_search(tank));
    sim.answer_batched_repaths(Some(&rules), None);
    assert!(sim.defer_code2_search(infantry));
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "repath_batch", 0);
    let restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .expect("the save loads")
        .sim;
    let batch = restored.repath_batch.as_ref().expect("still batching");
    assert_eq!(batch.pending, [infantry]);
    assert!(batch.ready.is_empty());
    let serial = crate::sim::snapshot::GameSnapshot::save(&arena(&rules).0, 0, 0, "serial", 0);
    let serial = crate::sim::snapshot::GameSnapshot::load(&serial).expect("the save loads");
    assert!(
        serial.sim.repath_batch.is_none(),
        "a serial game stays serial"
    );
}
