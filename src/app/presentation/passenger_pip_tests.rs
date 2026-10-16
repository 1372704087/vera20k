//! Stock FV boarding/departure through the production passenger and display
//! owners. The drawing oracle separately executes the original pip consumer.
use crate::rules::ruleset::RuleSet;
use crate::sim::command::Command;
use crate::sim::world::Simulation;

const OWNER: &str = "Americans";

fn frames(sim: &Simulation, rules: &RuleSet, ifv: u64, hover: Option<u64>) -> Vec<usize> {
    let hash = sim.state_hash();
    let result = super::ui_overlays::scale_pip_frames(
        sim,
        sim.entities().get(ifv).unwrap(),
        hover,
        Some(OWNER),
        rules,
    );
    assert_eq!(
        sim.state_hash(),
        hash,
        "status queries cannot change retained simulation state"
    );
    result
}

#[test]
fn stock_ifv_pip_follows_real_boarding_departure_and_selection() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")
    else {
        return;
    };
    let rules = &retail.rules;
    for (passenger_type, native_frame) in [("E1", 3), ("ENGINEER", 5)] {
        let mut sim = Simulation::new();
        let owner = sim.interner.intern(OWNER);
        sim.houses.insert(
            owner,
            crate::sim::house_state::HouseState::new(owner, 0, None, true, 0, 10),
        );
        sim.session.house_order.push(owner);
        crate::sim::arena_fixture::flat_arena(&mut sim, rules);
        sim.intern_rule_type_ids(rules);
        sim.resolve_type_handles(rules);
        sim.fog.reveal_all_for_owner(owner);
        let ifv = sim.spawn_object("FV", OWNER, 16, 16, 64, rules).unwrap();
        let passenger = sim
            .spawn_object(passenger_type, OWNER, 16, 15, 64, rules)
            .unwrap();
        assert!(sim.apply_command(
            OWNER,
            &Command::Select {
                entity_ids: vec![ifv],
                additive: false,
            },
            Some(rules)
        ));
        assert_eq!(frames(&sim, rules, ifv, None), [0]);
        assert!(sim.apply_command(
            OWNER,
            &Command::EnterTransport {
                passenger_id: passenger,
                transport_id: ifv,
            },
            Some(rules)
        ));
        let grid = sim.path_grid_snapshot().unwrap();
        for _ in 0..240 {
            sim.advance_tick(&[], Some(rules), Some(&grid), None, 33);
            if sim
                .entities()
                .get(passenger)
                .unwrap()
                .passenger_role
                .is_inside_transport()
            {
                break;
            }
        }
        let rider = sim.entities().get(passenger).unwrap();
        assert_eq!(rider.passenger_role.inside_transport(), Some((ifv, false)));
        assert!(rider.lifecycle.in_limbo());
        let hold = sim
            .entities()
            .get(ifv)
            .unwrap()
            .passenger_role
            .cargo()
            .unwrap();
        assert_eq!(hold.passengers(), [passenger]);
        assert_eq!(hold.total_size(), 1);
        assert_eq!(frames(&sim, rules, ifv, None), [native_frame]);

        assert!(sim.apply_command(
            OWNER,
            &Command::UnloadPassengers { transport_id: ifv },
            Some(rules)
        ));
        for _ in 0..240 {
            sim.advance_tick(&[], Some(rules), Some(&grid), None, 33);
            if !sim
                .entities()
                .get(passenger)
                .unwrap()
                .passenger_role
                .is_inside_transport()
            {
                break;
            }
        }
        let rider = sim.entities().get(passenger).unwrap();
        assert!(!rider.passenger_role.is_inside_transport());
        assert!(!rider.lifecycle.in_limbo());
        assert_eq!(
            sim.entities()
                .get(ifv)
                .unwrap()
                .passenger_role
                .passenger_count(),
            0
        );
        assert_eq!(frames(&sim, rules, ifv, None), [0]);
        assert!(sim.apply_command(
            OWNER,
            &Command::Select {
                entity_ids: vec![],
                additive: false
            },
            Some(rules)
        ));
        assert!(frames(&sim, rules, ifv, None).is_empty());
        assert_eq!(frames(&sim, rules, ifv, Some(ifv)), [0]);
    }
}

/// Controlled storage-consumer priors; this does not establish harvesting.
/// Native executions supply expected frames, including PC53/chop half ties.
#[test]
fn shared_harvester_scale_matches_native_independent_ore_and_gem_counts() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")
    else {
        return;
    };
    let rules = &retail.rules;
    let mut sim = Simulation::new();
    crate::sim::arena_fixture::flat_arena(&mut sim, rules);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    let miner = sim.spawn_object("HARV", OWNER, 16, 16, 64, rules).unwrap();
    assert!(sim.apply_command(
        OWNER,
        &Command::Select {
            entity_ids: vec![miner],
            additive: false,
        },
        Some(rules)
    ));
    let native = crate::test_fixture::json("tools/procedural_drawing_oracle/passenger_pips.json");
    for row in native["raster_controls"].as_array().unwrap() {
        let actor = &row["input"]["actors"][0];
        if actor["type_name"] != "HARV" {
            continue;
        }
        crate::sim::arena_fixture::install_status_miner_cargo(
            sim.entities_mut().get_mut(miner).unwrap(),
            rules.object("HARV").unwrap(),
            rules,
            actor["ore_bales"].as_u64().unwrap() as usize,
            actor["gem_bales"].as_u64().unwrap() as usize,
        );
        let expected: Vec<_> = row["draws"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|draw| draw["asset"] == "PIPS2.SHP")
            .map(|draw| draw["frame"].as_u64().unwrap() as usize)
            .collect();
        assert_eq!(
            frames(&sim, rules, miner, None),
            expected,
            "{}",
            row["name"]
        );
    }
}
