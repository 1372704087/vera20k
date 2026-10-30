//! The firing blink of a `DisguiseWhenStill=` Unit (`UnitClass::Fire_At @
//! 0x0074143E..0x00741471`) and Evaluate_Candidate's disguise arm
//! (`0x006F84B1..0x006F854B`), through the production fire and scan owners on
//! retail rules.

use super::*;
use crate::sim::combat::threat_range::ScanMission;
use crate::sim::rng::SimRng;
use crate::sim::world::Simulation;

/// A Mirage of `mirage_house` disguised by its own UpdateDisguise, and a
/// Rhino of the other house two Cells away, on the rival arena (Russians a
/// computer house at Normal, Americans the human player).
fn scene(rules: &RuleSet, mirage_house: &str) -> (Simulation, u64, u64) {
    let rhino_house = if mirage_house == "Americans" {
        "Russians"
    } else {
        "Americans"
    };
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 1);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    let mirage = sim
        .spawn_object("MGTK", mirage_house, 16, 16, 64, rules)
        .expect("mirage");
    let rhino = sim
        .spawn_object("HTNK", rhino_house, 18, 16, 64, rules)
        .expect("rhino");
    sim.session.set_binary_frame_for_test(100);
    sim.update_unit_disguise(mirage, rules).unwrap();
    let observer = sim.interner.get(rhino_house).unwrap();
    assert!(sim.substrate.entities.get(mirage).unwrap().is_disguised_to(
        observer,
        None,
        None,
        &sim.interner
    ));
    (sim, mirage, rhino)
}

fn blink(sim: &mut Simulation, mirage: u64, duration: i32) {
    let frame = sim.session.binary_frame();
    sim.substrate
        .entities
        .get_mut(mirage)
        .unwrap()
        .disguise
        .as_mut()
        .unwrap()
        .start_fire_blink(frame, duration);
}

/// Retail `[MirageGun] DisguiseFakeBlinkTime=15` (`MirageGunE` 5) and
/// `[General] DisabledDisguiseDetectionPercent=15,5,2` through the production
/// readers; a Mirage's launched shot starts its blink at the firing frame for
/// its weapon's time.
#[test]
fn a_retail_mirage_shot_starts_its_blink_for_its_weapons_fake_blink_time() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    assert_eq!(
        rules.weapon("MirageGun").unwrap().disguise_fake_blink_time,
        15
    );
    assert_eq!(
        rules.weapon("MirageGunE").unwrap().disguise_fake_blink_time,
        5
    );
    assert_eq!(
        rules.general.disabled_disguise_detection_percent,
        [15, 5, 2]
    );

    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 1);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    let mirage = sim
        .spawn_object("MGTK", "Americans", 16, 16, 64, rules)
        .expect("mirage");
    let rhino = sim
        .spawn_object("HTNK", "Russians", 19, 16, 64, rules)
        .expect("rhino");
    assert!(sim.apply_command(
        "Americans",
        &crate::sim::command::Command::Attack {
            attacker_id: mirage,
            target_id: rhino,
        },
        Some(rules)
    ));
    let timer = |sim: &Simulation| {
        sim.substrate
            .entities
            .get(mirage)
            .unwrap()
            .disguise
            .as_ref()
            .unwrap()
            .blink_timer()
    };
    let mut fired = None;
    for _ in 0..90 {
        let frame = sim.session.binary_frame();
        sim.advance_tick(&[], Some(rules), None, None, 67);
        if timer(&sim).duration() != 0 {
            fired = Some(frame);
            break;
        }
    }
    let fired = fired.expect("the Mirage fires at the Rhino");
    let timer = timer(&sim);
    assert_eq!(timer.duration(), 15);
    assert!(
        (fired as i32..=sim.session.binary_frame() as i32).contains(&timer.start_frame()),
        "the blink starts in the firing frame"
    );
}

/// Without a running blink a disguised Mirage is refused without a draw. While
/// it blinks, a computer attacker draws once on the Scenario stream and takes
/// the Mirage when the draw is at most its Normal entry (5).
#[test]
fn a_computer_attacker_sees_a_blinking_mirage_only_on_its_detection_roll() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let pick = |sim: &mut Simulation, rhino| {
        sim.greatest_threat_represented(rules, None, rhino, ScanMission::Guard, None)
    };

    let (mut sim, _, rhino) = scene(rules, "Americans");
    let before = sim.scenario_rng.logical_state();
    assert_eq!(pick(&mut sim, rhino), None, "no blink: refused");
    assert_eq!(
        sim.scenario_rng.logical_state(),
        before,
        "no blink: no draw"
    );

    for (roll, admitted) in [(5, true), (6, false), (0, true), (99, false)] {
        let (mut sim, mirage, rhino) = scene(rules, "Americans");
        blink(&mut sim, mirage, 15);
        sim.scenario_rng = SimRng::answering(0, 99, roll);
        let mut once = sim.scenario_rng.clone();
        once.next_range_i32_inclusive(0, 99);
        assert_eq!(
            pick(&mut sim, rhino),
            admitted.then_some(mirage),
            "roll {roll}"
        );
        assert_eq!(
            sim.scenario_rng.logical_state(),
            once.logical_state(),
            "roll {roll}: exactly one draw"
        );
    }

    // The blink ends after its 15 frames.
    let (mut sim, mirage, rhino) = scene(rules, "Americans");
    blink(&mut sim, mirage, 15);
    sim.session.set_binary_frame_for_test(115);
    sim.scenario_rng = SimRng::answering(0, 99, 0);
    let before = sim.scenario_rng.logical_state();
    assert_eq!(pick(&mut sim, rhino), None);
    assert_eq!(sim.scenario_rng.logical_state(), before);
}

/// IsControlledByHuman (`0x0050B730`) of the attacking house: the human
/// player's tank never draws and never sees through the disguise.
#[test]
fn a_human_attacker_never_rolls_against_a_blinking_mirage() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let (mut sim, mirage, rhino) = scene(rules, "Russians");
    blink(&mut sim, mirage, 15);
    sim.scenario_rng = SimRng::answering(0, 99, 0);
    let before = sim.scenario_rng.logical_state();
    assert_eq!(
        sim.greatest_threat_represented(rules, None, rhino, ScanMission::Guard, None),
        None
    );
    assert_eq!(sim.scenario_rng.logical_state(), before);
}

/// A Hunt's flat walk over 256 or more objects is scored side by side
/// ([`flat_scores`]); the blinking Mirage's roll is drawn again on the shared
/// stream, so the pick and the stream equal the one-by-one walk's.
#[test]
fn a_side_by_side_hunt_walk_rolls_like_the_one_by_one_walk() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let run = |serial: bool| {
        let mut sim = crate::sim::arena_fixture::rival_arena_with_size(rules, 1, 64);
        sim.intern_rule_type_ids(rules);
        sim.resolve_type_handles(rules);
        let mirage = sim
            .spawn_object("MGTK", "Americans", 16, 16, 64, rules)
            .expect("mirage");
        let rhino = sim
            .spawn_object("HTNK", "Russians", 18, 16, 64, rules)
            .expect("rhino");
        for index in 0..300u16 {
            sim.spawn_object(
                "E1",
                "Americans",
                20 + index % 30,
                30 + index / 30,
                64,
                rules,
            )
            .expect("rifleman");
        }
        sim.session.set_binary_frame_for_test(100);
        sim.update_unit_disguise(mirage, rules).unwrap();
        blink(&mut sim, mirage, 15);
        sim.scenario_rng = SimRng::answering(0, 99, 0);
        SERIAL_FLAT_SCORES.set(serial);
        let walks = FLAT_WALKS_SIDE_BY_SIDE.get();
        let pick = sim.greatest_threat_represented(rules, None, rhino, ScanMission::Hunt, None);
        SERIAL_FLAT_SCORES.set(false);
        (
            pick,
            sim.scenario_rng.logical_state(),
            FLAT_WALKS_SIDE_BY_SIDE.get() != walks,
        )
    };
    let one_by_one = run(true);
    let side_by_side = run(false);
    assert!(!one_by_one.2 && side_by_side.2, "the walks took both paths");
    assert_eq!(side_by_side.0, one_by_one.0);
    assert_eq!(side_by_side.1, one_by_one.1);
    let mut once = SimRng::answering(0, 99, 0);
    once.next_range_i32_inclusive(0, 99);
    assert_eq!(one_by_one.1, once.logical_state(), "exactly one roll");
}

/// A side-by-side Hunt walk rolls each candidate on its worker's copy of the
/// stream, whose draws depend on how the list was split, so only the
/// candidates' rolls on the shared stream may count. Two blinking Mirages
/// roll: the first fails, the second passes on the second draw and stamps the
/// Dummy from its missing Cell, so the walk is scored again one by one from
/// the stream it found; a later rifleman's missing Cell stamps the Dummy back.
/// One worker or four, the pick, the stream (two draws) and the Dummy equal
/// the one-by-one walk's.
#[test]
fn a_side_by_side_hunt_walk_does_not_depend_on_the_worker_count() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    // The Normal roll (at most 5) fails on the first draw, passes on the second.
    let stream = (0..)
        .map(SimRng::new)
        .find(|rng| {
            let mut rng = rng.clone();
            rng.next_range_i32_inclusive(0, 99) > 5 && rng.next_range_i32_inclusive(0, 99) <= 5
        })
        .unwrap();
    let (missing_mirage, missing_rifleman) = ((60, 60), (62, 60));
    let run = |workers: usize, serial: bool| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            let mut sim = crate::sim::arena_fixture::rival_arena_with_size(rules, 1, 64);
            sim.intern_rule_type_ids(rules);
            sim.resolve_type_handles(rules);
            let rhino = sim
                .spawn_object("HTNK", "Russians", 18, 16, 64, rules)
                .expect("rhino");
            // In list order: a Mirage, 99 riflemen, the Mirage and a
            // rifleman on missing Cells, then 200 riflemen.
            let mut mirages = Vec::new();
            for index in 0..302u16 {
                let (kind, (x, y)) = match index {
                    0 => ("MGTK", (16, 16)),
                    100 => ("MGTK", missing_mirage),
                    101 => ("E1", missing_rifleman),
                    _ => ("E1", (20 + index % 30, 30 + index / 30)),
                };
                let id = sim
                    .spawn_object(kind, "Americans", x, y, 64, rules)
                    .expect(kind);
                if kind == "MGTK" {
                    mirages.push(id);
                }
            }
            sim.session.set_binary_frame_for_test(100);
            for mirage in mirages {
                sim.update_unit_disguise(mirage, rules).unwrap();
                blink(&mut sim, mirage, 15);
            }
            let terrain = sim.resolved_terrain.as_mut().unwrap();
            let allocated: Vec<_> = (0..64)
                .flat_map(|y| (0..64).map(move |x| (x, y)))
                .filter(|&cell| cell != missing_mirage && cell != missing_rifleman)
                .collect();
            terrain.test_set_native_allocated_cells(&allocated);
            let dummy = terrain.shared_cell_dummy();
            dummy.stamp_coord(missing_rifleman.0.into(), missing_rifleman.1.into());
            sim.scenario_rng = stream.clone();
            SERIAL_FLAT_SCORES.set(serial);
            let pick = sim.greatest_threat_represented(rules, None, rhino, ScanMission::Hunt, None);
            SERIAL_FLAT_SCORES.set(false);
            (
                pick,
                sim.scenario_rng.logical_state(),
                dummy.snapshot().coord,
            )
        })
    };
    let one_by_one = run(1, true);
    let mut twice = stream.clone();
    twice.next_range_i32_inclusive(0, 99);
    twice.next_range_i32_inclusive(0, 99);
    assert_eq!(one_by_one.1, twice.logical_state(), "both Mirages rolled");
    assert_eq!(one_by_one.2, (62, 60), "the rifleman stamped last");
    assert_eq!(run(1, false), one_by_one, "one worker");
    assert_eq!(run(4, false), one_by_one, "four workers");
}
