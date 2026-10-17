//! Stock Carrier construction, launch, recall and reload feed the existing
//! spawn-count owner. Original drawing controls supply the expected frames;
//! they do not certify the whole native aircraft flight lifecycle.
use crate::rules::ruleset::RuleSet;
use crate::sim::command::Command;
use crate::sim::spawn_manager::SpawnSlotState;
use crate::sim::world::Simulation;
use std::collections::BTreeSet;

const OWNER: &str = "Americans";

fn frames(sim: &Simulation, rules: &RuleSet, carrier: u64, hover: Option<u64>) -> Vec<usize> {
    let entity = sim.entities().get(carrier).expect("Carrier survives");
    let before = (
        sim.state_hash(),
        sim.main_rng.native_state_hex(),
        sim.scenario_rng.native_state_hex(),
        sim.mapgen_rng.native_state_hex(),
        entity.spawn_manager.clone(),
    );
    let result = super::ui_overlays::scale_pip_draws(sim, entity, hover, Some(OWNER), rules)
        .into_iter()
        .map(|pip| pip.frame)
        .collect();
    assert_eq!(
        before,
        (
            sim.state_hash(),
            sim.main_rng.native_state_hex(),
            sim.scenario_rng.native_state_hex(),
            sim.mapgen_rng.native_state_hex(),
            entity.spawn_manager.clone(),
        ),
        "drawing reads retained nodes without advancing RNG, timers or targets"
    );
    result
}

fn native_frames(count: usize) -> Vec<usize> {
    let native = crate::test_fixture::json("tools/procedural_drawing_oracle/spawn_pips.json");
    native["raster_controls"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|row| {
            let actors = row["input"]["actors"].as_array()?;
            if actors.len() != 1
                || actors[0]["type_name"] != "CARRIER"
                || !row["input"]["scenario_patch"].is_null()
                || row["actor_results"][0]["spawn_count_docked"].as_u64() != Some(count as u64)
            {
                return None;
            }
            let frames: Vec<_> = row["draws"]
                .as_array()?
                .iter()
                .filter(|draw| draw["asset"] == "PIPS2.SHP")
                .map(|draw| draw["frame"].as_u64().unwrap() as usize)
                .collect();
            (frames.len() == 3).then_some(frames)
        })
        .expect("original admitted three-slot Carrier draw for this count")
}

#[test]
fn stock_carrier_pips_follow_construction_sortie_recall_reload_and_selection() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")
    else {
        return;
    };
    let rules = &retail.rules;
    let expected: [Vec<usize>; 4] = std::array::from_fn(native_frames);
    let mut sim = crate::sim::arena_fixture::rival_water_arena(rules, 1, 65);
    // Aircraft placement needs the actual Size diamond as well as LocalSize.
    sim.install_playfield_from_map_header(&crate::map::map_file::MapHeader {
        theater: "TEMPERATE".into(),
        fill: "Water".into(),
        level: 0,
        width: 32,
        height: 32,
        local_left: 2,
        local_top: 2,
        local_width: 28,
        local_height: 24,
    });
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    let owner = sim.interner.get(OWNER).unwrap();
    sim.fog.reveal_all_for_owner(owner);
    let carrier = sim
        .spawn_object("CARRIER", OWNER, 20, 20, 64, rules)
        .unwrap();
    let target = sim
        .spawn_object("LCRF", "Russians", 34, 20, 64, rules)
        .unwrap();
    let wing: Vec<_> = sim
        .entities()
        .get(carrier)
        .unwrap()
        .spawn_manager
        .as_ref()
        .unwrap()
        .slots()
        .iter()
        .map(|slot| slot.spawn().expect("constructor-owned Hornet"))
        .collect();
    assert_eq!(wing.len(), 3);
    assert!(
        wing.iter()
            .all(|id| sim.entities().get(*id).unwrap().lifecycle.in_limbo())
    );
    assert!(sim.apply_command(
        OWNER,
        &Command::Select {
            entity_ids: vec![carrier],
            additive: false
        },
        Some(rules)
    ));
    assert_eq!(frames(&sim, rules, carrier, None), expected[3]);
    assert!(sim.apply_command(
        OWNER,
        &Command::Attack {
            attacker_id: carrier,
            target_id: target
        },
        Some(rules)
    ));
    let hornet_bomb = sim
        .interner
        .intern(rules.object("HORNET").unwrap().weapon_at(0).unwrap());
    let mut counts = BTreeSet::from([3]);
    let mut fired = false;
    let mut recalled = false;
    let mut reloading = false;
    let mut ready_again = false;
    for _ in 0..4000 {
        let result = sim
            .advance_app_frame(
                &[],
                Some(rules),
                None,
                33,
                crate::sim::world::TickLane::Ordinary,
                None,
                crate::sim::world::FrameEffects::default(),
            )
            .expect("ordinary stock Carrier frame");
        fired |= result
            .fire_events
            .iter()
            .any(|shot| shot.weapon_id == hornet_bomb && wing.contains(&shot.attacker_id));
        let manager = sim
            .entities()
            .get(carrier)
            .unwrap()
            .spawn_manager
            .as_ref()
            .unwrap();
        let count = manager.count_docked_spawns();
        counts.insert(count);
        assert_eq!(frames(&sim, rules, carrier, None), expected[count]);
        reloading |= manager
            .slots()
            .iter()
            .any(|slot| slot.state() == SpawnSlotState::Reloading);
        if recalled
            && reloading
            && manager
                .slots()
                .iter()
                .all(|slot| slot.state() == SpawnSlotState::ReadyDocked)
        {
            assert!(wing.iter().all(|id| {
                sim.entities().get(*id).is_some_and(|child| {
                    child.lifecycle.in_limbo()
                        && child.ammo().is_some_and(|ammo| ammo.current() == 1)
                })
            }));
            ready_again = true;
            break;
        }
        if fired && !recalled {
            assert!(sim.apply_command(OWNER, &Command::Stop { entity_id: carrier }, Some(rules)));
            recalled = true;
        }
    }
    assert_eq!(
        counts,
        BTreeSet::from([0, 1, 2, 3]),
        "ordinary launches and landings expose every count"
    );
    assert!(
        fired && recalled && reloading && ready_again,
        "stock Hornets must fire, recall, count while reloading, and become ready again"
    );
    assert!(sim.apply_command(
        OWNER,
        &Command::Select {
            entity_ids: vec![],
            additive: false
        },
        Some(rules)
    ));
    assert!(frames(&sim, rules, carrier, None).is_empty());
    assert_eq!(frames(&sim, rules, carrier, Some(carrier)), expected[3]);
}
