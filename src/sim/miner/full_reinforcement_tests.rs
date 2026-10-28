//! Ordinary authored Full reinforcement feeds the existing storage consumers.
//! Native CreateMembers65DD30 fills slot0 at65DE53..65DE80 before AddMember;
//! the existing refinery/pip/snapshot owners consume that same hold below.

use crate::map::map_file::MapFile;
use crate::rules::ruleset::RuleSet;
use crate::rules::team_ai_ini::TeamAiIniRegistry;
use crate::rules::tiberium_type::TiberiumTypeId;
use crate::sim::combat::TargetKind;
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::house_state::HouseState;
use crate::sim::mission::MissionType;
use crate::sim::snapshot::GameSnapshot;
use crate::sim::world::{FrameEffects, Simulation, TickLane};

fn frame(sim: &mut Simulation, rules: &RuleSet, commands: &[CommandEnvelope]) {
    let result = sim
        .advance_app_frame(
            commands,
            rules,
            Some(rules.overlay_registry()),
            67,
            TickLane::Ordinary,
            FrameEffects::default(),
        )
        .expect("ordinary production frame");
    assert!(result.tick.frame_committed);
}

#[test]
fn full_harvester_reinforcement_retains_slot_zero_through_save_and_refinery_unload() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let object = rules.object("HARV").unwrap();
    assert_eq!(object.storage, 40, "selected retail HARV capacity");
    let capacity = usize::try_from(object.storage).unwrap();
    assert_eq!(
        rules.tiberium_types.get(TiberiumTypeId(0)).unwrap().section,
        "Riparius"
    );

    // Only the constructor-complete refinery/flat ground are scene priors.
    // The harvester under test must come from the map's real Event13/Spring.
    let mut sim = Simulation::with_seed(0x65DD_0030);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    crate::sim::arena_fixture::flat_arena(&mut sim, rules);
    sim.overlay_grid = Some(crate::sim::overlay_grid::OverlayGrid::new(32, 32));
    let owner = sim.interner.intern("Americans");
    sim.houses
        .register(HouseState::new(owner, 0, Some(owner), true, 0, 10));
    sim.session.set_current_house_for_test(Some(owner));
    let refinery = sim
        .spawn_object("NAREFN", "Americans", 6, 9, 0, rules)
        .expect("ordinary refinery constructor/Unlimbo");
    // A refinery can create its ordinary free HARV. Move that actual actor
    // off the pad; use Team membership below to identify the reinforcement.
    let leave_pad: Vec<_> = sim
        .entities()
        .iter_sorted()
        .filter(|(_, entity)| sim.interner.resolve(entity.type_ref()) == "HARV")
        .map(|(id, _)| {
            CommandEnvelope::new(
                owner,
                sim.session.tick(),
                Command::Move {
                    entity_id: id,
                    target_rx: 24,
                    target_ry: 24,
                },
            )
        })
        .collect();
    frame(&mut sim, rules, &leave_pad);
    let credits_before = sim.houses[&owner].economy().credits();

    let map = MapFile::from_bytes(b"[Map]\nTheater=TEMPERATE\nSize=0,0,2,1\nLocalSize=0,0,2,1\n\
        [IsoMapPack5]\n1=DwALABwBAAIA/////wAAABEAAA==\n[Basic]\nNewINIFormat=4\n\
        [Waypoints]\n4=16016\n[TeamTypes]\n0=FULL_MINER\n\
        [FULL_MINER]\nHouse=Americans\nScript=FULL_SCRIPT\nTaskForce=FULL_FORCE\nWaypoint=E\nFull=yes\n\
        [ScriptTypes]\n0=FULL_SCRIPT\n[FULL_SCRIPT]\n0=50,60\n\
        [TaskForces]\n0=FULL_FORCE\n[FULL_FORCE]\n0=1,HARV\n\
        [Triggers]\nARRIVAL=Americans,<none>,Full miner arrives,0,1,1,1,0\n\
        [Events]\nARRIVAL=1,13,0,0\n[Actions]\nARRIVAL=1,80,1,FULL_MINER,0,0,0,0,E\n\
        [Tags]\nARRIVAL_TAG=0,Full arrival,ARRIVAL\n").unwrap();
    let (_, assets) = crate::rules::retail_ini_fixture::retail_assets().unwrap();
    let teams = TeamAiIniRegistry::load_retail(&assets, &map.ini, true).unwrap();
    sim.initialize_trigger_definitions(&map, &teams, None, None, None);
    sim.install_team_ai_registry(&teams, rules).unwrap();
    sim.session.install_waypoints(&map.waypoints);
    sim.classify_map_triggers(rules);
    let team_type = sim.interner.get("FULL_MINER").unwrap();
    let mut reinforced = None;
    for _ in 0..32 {
        frame(&mut sim, rules, &[]);
        if let Some(team) = sim.team_script_vm.first_team_of_type(team_type) {
            assert_eq!(team.owner(), owner);
            let members: Vec<_> = team.members().collect();
            assert_eq!(members.len(), 1);
            reinforced = Some(members[0]);
            break;
        }
    }
    let reinforced = reinforced.expect("Full=yes must place its positive-Storage member");
    let assert_full = |sim: &Simulation| {
        let entity = sim.entities().get(reinforced).unwrap();
        assert!(!entity.lifecycle.in_limbo() && entity.lifecycle.cell_marked());
        assert_eq!(sim.interner.resolve(entity.type_ref()), "HARV");
        let cargo = entity.miner.as_ref().unwrap().cargo();
        assert_eq!(cargo.len(), capacity);
        assert!(
            cargo
                .iter()
                .all(|bale| bale.tiberium_type == TiberiumTypeId(0))
        );
        assert_eq!(
            entity.techno_pip_fill_level(object, rules),
            object.pip_max(rules)
        );
        assert_eq!(
            sim.houses[&owner].economy().credits(),
            credits_before,
            "fill is not payment"
        );
    };
    assert_full(&sim);
    for _ in 0..16 {
        frame(&mut sim, rules, &[]);
        if sim.entities().get(reinforced).unwrap().flashing.frames() > 0 {
            break;
        }
    }
    assert!(
        sim.entities().get(reinforced).unwrap().flashing.frames() > 0,
        "ordinary Team script ran"
    );
    let saved = GameSnapshot::save(
        &sim,
        0,
        rules.simulation_config_hash(),
        "Full reinforcement",
        0,
    );
    let mut restored = GameSnapshot::load(&saved).unwrap().sim;
    restored.retain_process_state_for_load(crate::sim::world::SnapshotProcessState::Live(&sim));
    restored.restore_after_snapshot_load().unwrap();
    restored.rebuild_caches_after_load(
        sim.resolved_terrain.as_ref().unwrap().clone(),
        sim.terrain_speed_config().clone(),
        rules,
    );
    restored
        .restore_map_authority_after_snapshot_load(rules, rules.overlay_registry(), None)
        .unwrap();
    let mut sim = restored;
    assert_full(&sim);
    for _ in 0..16 {
        frame(&mut sim, rules, &[]);
        if sim.team_script_vm.first_team_of_type(team_type).is_none() {
            break;
        }
    }
    assert!(
        sim.team_script_vm.first_team_of_type(team_type).is_none(),
        "script end releases the member"
    );
    let enter = CommandEnvelope::new(
        owner,
        sim.session.tick(),
        Command::clicked_mission(reinforced, MissionType::Enter, TargetKind::Entity(refinery)),
    );
    frame(&mut sim, rules, &[enter]);
    let mut saw_unload = false;
    for _ in 0..2000 {
        frame(&mut sim, rules, &[]);
        let miner = sim
            .entities()
            .get(reinforced)
            .unwrap()
            .miner
            .as_ref()
            .unwrap();
        saw_unload |= miner.unload_active();
        if miner.cargo().is_empty() {
            break;
        }
    }
    assert!(
        saw_unload,
        "the ordinary Enter/radio/Unload chain admitted the miner"
    );
    let entity = sim.entities().get(reinforced).unwrap();
    assert!(entity.miner.as_ref().unwrap().cargo().is_empty());
    assert_eq!(entity.techno_pip_fill_level(object, rules), 0);
    assert!(
        sim.houses[&owner].economy().credits() > credits_before,
        "the actual refinery owner received payment"
    );
}
