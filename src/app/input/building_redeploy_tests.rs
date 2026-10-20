//! Building4436F0: archive the clicked Cell, then enqueue SELL16.
use super::*;
use crate::rules::ruleset::RuleSet;
use crate::sim::mission::MissionType;
use crate::sim::world::Simulation;

fn step(sim: &mut Simulation, rules: &RuleSet) {
    let commands = sim.take_due_commands();
    sim.advance_app_frame(
        &commands,
        Some(rules),
        None,
        33,
        crate::sim::world::TickLane::Ordinary,
        Default::default(),
    )
    .unwrap();
}

#[test]
fn retail_mcvs_repack_and_follow_the_ground_click_after_deployment() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let mut rules = retail.rules;
    for yard in ["GACNST", "NACNST", "YACNST"] {
        // This INI fixture does not load SHPs. Supply a finite build-up
        // animation binding; its timing is an authored test input.
        rules.set_buildup_control_for_test(yard, [0, 29, 1]);
    }
    for (mcv, yard_type, contact) in [("AMCV", "GACNST"), ("SMCV", "NACNST"), ("PCV", "YACNST")]
        .into_iter()
        .flat_map(|(mcv, yard)| [false, true].map(move |contact| (mcv, yard, contact)))
    {
        let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 0x4436F0);
        sim.resolve_type_handles(&rules);
        let owner = sim.interner.get("Americans").unwrap();
        sim.session.set_current_house_for_test(Some(owner));
        sim.session.set_game_mode(
            crate::sim::scenario_session::ScenarioGameMode::from_legacy_nonzero(true),
        );
        sim.session.game_options_mut_for_test().mcv_redeploy = true;
        sim.session.game_options_mut_for_test().short_game = false;
        sim.input_delay_ticks = 0;
        sim.spawn_object("GAPOWR", "Russians", 3, 3, 0, &rules)
            .unwrap();
        let actor = sim
            .spawn_object(mcv, "Americans", 12, 12, 96, &rules)
            .unwrap();
        assert!(sim.apply_command(
            "Americans",
            &Command::DeployMcv { entity_id: actor },
            Some(&rules)
        ));
        for _ in 0..120 {
            step(&mut sim, &rules);
        }
        let yard = sim
            .entities()
            .values()
            .find(|e| sim.interner.resolve(e.type_ref()) == yard_type)
            .unwrap_or_else(|| panic!("{mcv} must deploy into {yard_type}"))
            .stable_id();
        assert!(
            !sim.entities().get(yard).unwrap().building_up(),
            "{yard_type}"
        );
        let peer = sim
            .spawn_object("GAPOWR", "Americans", 5, 5, 0, &rules)
            .unwrap();
        if contact {
            // Supplied existing radio state. The sale owner must retire both
            // sides, rather than requiring an empty link at input time.
            sim.entities_mut()
                .get_mut(yard)
                .unwrap()
                .radio_contacts
                .insert(peer)
                .unwrap();
            sim.entities_mut()
                .get_mut(peer)
                .unwrap()
                .radio_contacts
                .insert(yard)
                .unwrap();
        }
        let (orders, announce) =
            building_cell_orders(&sim, &rules, None, &[yard], owner, (23, 20), false);
        assert!(!announce);
        assert!(
            matches!(orders.as_slice(), [Command::SetRally { .. }, Command::SellBuilding { entity_id }]
            if *entity_id == yard),
            "{yard_type}: {orders:?}"
        );
        let goal = match orders[0] {
            Command::SetRally { rx, ry, .. } => (rx, ry),
            _ => unreachable!(),
        };
        let envelopes: Vec<_> = orders
            .into_iter()
            .map(|payload| CommandEnvelope::new(owner, sim.session.tick(), payload))
            .collect();
        sim.queue_commands(envelopes);
        let mut converted = None;
        for _ in 0..80 {
            step(&mut sim, &rules);
            converted = sim
                .entities()
                .values()
                .find(|e| sim.interner.resolve(e.type_ref()) == mcv)
                .map(|e| e.stable_id());
            if converted.is_some() {
                break;
            }
        }
        assert!(
            sim.entities().get(yard).is_none(),
            "{yard_type} did not repack: {:?}",
            sim.entities().get(yard).map(|e| (
                e.mission.effective(),
                e.archive_target(),
                e.building_down()
            ))
        );
        let converted = converted.expect("MCV must return");
        assert!(sim.entities().get(peer).unwrap().radio_contacts.is_empty());
        let unit = sim.entities().get(converted).unwrap();
        assert!(
            [unit.mission.current(), unit.mission.queued()]
                .iter()
                .any(|mission| mission.known() == Some(MissionType::Move)),
            "{mcv} must receive the archived move"
        );
        for _ in 0..600 {
            step(&mut sim, &rules);
            let unit = sim.entities().get(converted).unwrap();
            if (unit.position.rx(), unit.position.ry()) == goal {
                break;
            }
        }
        let unit = sim.entities().get(converted).unwrap();
        assert_eq!((unit.position.rx(), unit.position.ry()), goal, "{mcv}");
    }
}

/// Original Building4436F0 admits both empty and populated RadioClass
/// contacts; option/mode/House/mind-control gates are44374F..443778.
/// Contact membership is a supplied state here, not a stock producer claim.
#[test]
fn construction_yard_ground_orders_match_native_redeploy_admission() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = retail.rules;
    for (name, enabled, skirmish, human, contact, admitted) in [
        ("enabled", true, true, true, false, true),
        ("radio contact", true, true, true, true, true),
        ("disabled", false, true, true, false, false),
        ("campaign", true, false, true, false, false),
        ("computer", true, true, false, false, false),
    ] {
        let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 0x4436F0);
        sim.resolve_type_handles(&rules);
        let owner = sim.interner.get("Americans").unwrap();
        sim.houses
            .get_mut(&owner)
            .unwrap()
            .set_is_human_for_test(human);
        sim.session.set_game_mode(
            crate::sim::scenario_session::ScenarioGameMode::from_legacy_nonzero(skirmish),
        );
        sim.session.game_options_mut_for_test().mcv_redeploy = enabled;
        let yard = sim
            .spawn_object("GACNST", "Americans", 12, 12, 0, &rules)
            .unwrap();
        let peer = sim
            .spawn_object("GAPOWR", "Americans", 5, 5, 0, &rules)
            .unwrap();
        if contact {
            sim.entities_mut()
                .get_mut(yard)
                .unwrap()
                .radio_contacts
                .insert(peer)
                .unwrap();
        }
        assert_eq!(
            sim.takes_rally_point(yard, "Americans", &rules),
            admitted,
            "{name}"
        );
        let (orders, _) = building_cell_orders(&sim, &rules, None, &[yard], owner, (23, 20), false);
        assert_eq!(!orders.is_empty(), admitted, "{name}");
    }
}

/// A retail rival arena where the Americans are the player, in a skirmish
/// with MCV redeploy on, with its lower right corner flooded.
fn yard_arena(rules: &RuleSet) -> (Simulation, InternedId) {
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 0x447540);
    crate::sim::arena_fixture::flood(
        &mut sim,
        rules,
        32,
        (24..32).flat_map(|y| (24..32).map(move |x| (x, y))),
    );
    sim.resolve_type_handles(rules);
    let owner = sim.interner.get("Americans").unwrap();
    sim.session.set_current_house_for_test(Some(owner));
    sim.session
        .set_game_mode(crate::sim::scenario_session::ScenarioGameMode::from_legacy_nonzero(true));
    sim.session.game_options_mut_for_test().mcv_redeploy = true;
    (sim, owner)
}

/// Building What_Action_OnObject (`0x00447210`) over another House's unit
/// is the Techno tail's Select for an unarmed factory or Construction Yard,
/// so the click selects the unit; Building ClickedAction_Object
/// (`0x00443410`) neither rallies nor packs up. With Alt the force-move rung
/// answers Move: the war factory rallies at the unit's Cell, and the yard
/// only archives it, so it still does not pack up.
#[test]
fn a_selected_factory_or_yard_selects_another_houses_unit_instead_of_rallying() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = retail.rules;
    let (mut sim, owner) = yard_arena(&rules);
    let factory = sim
        .spawn_object("GAWEAP", "Americans", 6, 6, 0, &rules)
        .unwrap();
    let yard = sim
        .spawn_object("GACNST", "Americans", 12, 12, 0, &rules)
        .unwrap();
    let enemy = sim
        .spawn_object("MTNK", "Russians", 20, 18, 64, &rules)
        .unwrap();
    let registry = crate::rules::overlay_types::OverlayTypeRegistry::empty();
    for selection in [vec![factory], vec![yard], vec![factory, yard]] {
        let release = |alt| {
            building_object_release(
                &sim, &rules, &registry, &selection, &selection, enemy, false, alt,
            )
        };
        let orders = |alt| {
            building_click_orders(
                &sim,
                &rules,
                Some(&registry),
                &selection,
                owner,
                Some(enemy),
                (20, 17),
                false,
                alt,
                false,
            )
            .0
        };
        assert_eq!(release(false), LeftRelease::Select, "{selection:?}");
        assert!(orders(false).is_empty(), "{selection:?}");
        assert_eq!(release(true), LeftRelease::Dispatch, "{selection:?}");
        let alt = orders(true);
        assert!(
            !alt.iter()
                .any(|order| matches!(order, Command::SellBuilding { .. })),
            "{selection:?}: {alt:?}"
        );
        assert_eq!(
            alt.iter()
                .filter_map(|order| match order {
                    Command::SetRally { producer_ids, .. } => Some(producer_ids.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            if selection.contains(&factory) {
                vec![vec![factory]]
            } else {
                Vec::new()
            },
            "{selection:?}"
        );
    }
    // The factory's rally comes from the unit's own Cell, not the clicked one.
    let rally = building_object_orders(&sim, &rules, &[factory], enemy, true).0;
    let expected = sim
        .factory_rally_cell_input(factory, (20, 18), &rules)
        .unwrap();
    assert!(
        matches!(rally.as_slice(), [Command::SetRally { rx, ry, .. }] if Some((*rx, *ry)) == expected),
        "{rally:?} vs {expected:?}"
    );
    // Ctrl+Alt over an allied object answers 0x1A, which sends nothing.
    let ally = sim
        .spawn_object("MTNK", "Americans", 20, 10, 64, &rules)
        .unwrap();
    assert!(
        building_click_orders(
            &sim,
            &rules,
            Some(&registry),
            &[factory],
            owner,
            Some(ally),
            (20, 10),
            false,
            true,
            true,
        )
        .0
        .is_empty()
    );
}

/// Building What_Action_OnCell (`0x00447540`) asks an UndeploysInto type's
/// own Can_Enter_Cell (`0x00449440`) even for the cursor: a Construction
/// Yard over water answers NoMove, so the click neither archives nor packs
/// up (`0x004436F0`). Shift answers Move before that test, and a Slave
/// Miner's yard, which undeploys into a ResourceGatherer, always moves.
#[test]
fn a_yard_neither_archives_nor_packs_up_over_a_cell_it_cannot_stand_on() {
    use crate::sim::world::MoveAction;
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = retail.rules;
    let (mut sim, owner) = yard_arena(&rules);
    let yard = sim
        .spawn_object("GACNST", "Americans", 12, 12, 0, &rules)
        .unwrap();
    let slave_yard = sim
        .spawn_object("YAREFN", "Americans", 4, 20, 0, &rules)
        .unwrap();
    let enemy_yard = sim
        .spawn_object("GACNST", "Russians", 4, 4, 0, &rules)
        .unwrap();
    let action = |id, cell, shift| {
        sim.building_cell_move_action(owner, id, cell, shift, &rules, None)
            .unwrap()
    };
    let (land, water) = ((20, 12), (27, 27));
    assert_eq!(action(yard, land, false), MoveAction::Move);
    assert_eq!(action(yard, water, false), MoveAction::NoMove);
    assert_eq!(action(yard, water, true), MoveAction::Move);
    assert_eq!(action(slave_yard, water, false), MoveAction::Move);
    assert_eq!(action(enemy_yard, land, false), MoveAction::None);

    let orders = |cell| building_cell_orders(&sim, &rules, None, &[yard], owner, cell, false).0;
    assert!(
        matches!(orders(land).as_slice(), [Command::SetRally { .. }, Command::SellBuilding { entity_id }]
            if *entity_id == yard),
        "{:?}",
        orders(land)
    );
    assert!(orders(water).is_empty(), "{:?}", orders(water));
}
