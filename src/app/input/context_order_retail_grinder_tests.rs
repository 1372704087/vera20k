//! The Grinder's click chain on retail rules: CAN_LOAD (`0x0043C4F8`), the
//! Unit (`0x00740122..0x007401BC`) and Infantry (`0x0051F0AF..0x0051F111`)
//! What_Action rows, the Enter/NoEnter cursor rows and Foot ClickedAction's
//! mission Eaten (`0x004D774F`), then the existing Eaten walk and refund.

use super::*;
use crate::app::input::cursor::{capability_cursor_for_hover, cursor_id_for_feedback};
use crate::app::types::CursorId;
use crate::rules::ruleset::RuleSet;
use crate::sim::house_state::HouseState;
use crate::sim::world::Simulation;

fn arena(rules: &RuleSet) -> Simulation {
    let mut sim = Simulation::new();
    for (name, side) in [("Americans", 0), ("Russians", 1)] {
        let owner = sim.interner.intern(name);
        sim.houses
            .register(HouseState::new(owner, side, None, true, 0, 10));
    }
    sim.session
        .set_current_house_for_test(sim.interner.get("Americans"));
    sim.session
        .set_game_mode(crate::sim::scenario_session::ScenarioGameMode::from_legacy_nonzero(true));
    crate::sim::arena_fixture::flat_arena(&mut sim, rules);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    sim
}

/// The player's Grizzly and GI, and a Grinder of `grinder_house`.
fn scene(rules: &RuleSet, grinder_house: &str) -> (Simulation, u64, u64, u64) {
    let mut sim = arena(rules);
    if grinder_house != "Americans" {
        for (from, to) in [("AMERICANS", "RUSSIANS"), ("RUSSIANS", "AMERICANS")] {
            sim.house_alliances
                .entry(from.into())
                .or_default()
                .insert(to.into());
        }
    }
    // The Russians keep a building, so the skirmish has no winner and every
    // frame reaches its late delete drain.
    sim.spawn_object("NAPOWR", "Russians", 3, 3, 0, rules)
        .expect("russian power plant");
    let grinder = sim
        .spawn_object("YAGRND", grinder_house, 20, 16, 0, rules)
        .expect("grinder");
    let tank = sim
        .spawn_object("MTNK", "Americans", 13, 16, 0, rules)
        .expect("tank");
    let gi = sim
        .spawn_object("E1", "Americans", 13, 19, 0, rules)
        .expect("gi");
    (sim, grinder, tank, gi)
}

fn cursor(
    sim: &Simulation,
    rules: &RuleSet,
    registry: &crate::rules::overlay_types::OverlayTypeRegistry,
    actor: u64,
    grinder: u64,
) -> Option<CursorId> {
    cursor_id_for_feedback(capability_cursor_for_hover(
        sim,
        &[actor],
        Some(actor),
        &HoverTargetKindWithId {
            stable_id: grinder,
            kind: HoverTargetKind::FriendlyStructure,
        },
        Some(rules),
        Some(registry),
        OrderModifier::Normal,
        false,
    ))
}

fn eaten(entity_id: u64, grinder: u64) -> Command {
    Command::MegaMission {
        entity_id,
        mission: crate::sim::mission::MissionType::Eaten,
        target: None,
        destination: Some(TargetKind::Entity(grinder)),
    }
}

/// A player's vehicle and rifleman over the player's own Grinder show the
/// Enter row, and the click sends each mission Eaten; both walk in and are
/// ground for a refund.
#[test]
fn a_players_vehicle_and_rifleman_grind_at_their_own_grinder() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let registry = crate::rules::overlay_types::OverlayTypeRegistry::from_ini(
        &retail.processed_rules,
        Some(&retail.fixed_art),
    );
    assert!(rules.object("YAGRND").unwrap().grinding);
    let (mut sim, grinder, tank, gi) = scene(rules, "Americans");
    assert_eq!(
        cursor(&sim, rules, &registry, tank, grinder),
        Some(CursorId::Enter)
    );
    assert_eq!(
        cursor(&sim, rules, &registry, gi, grinder),
        Some(CursorId::Enter)
    );
    let hover = HoverTargetKindWithId {
        stable_id: grinder,
        kind: HoverTargetKind::FriendlyStructure,
    };
    let mut selected = vec![tank, gi];
    let orders = grinder_orders(&sim, rules, &mut selected, &hover).expect("both grind");
    assert!(selected.is_empty());
    assert_eq!(orders, [eaten(tank, grinder), eaten(gi, grinder)]);

    let owner = sim.interner.get("Americans").unwrap();
    let credits = sim.houses[&owner].economy().credits();
    for order in &orders {
        assert!(sim.apply_command("Americans", order, Some(rules)));
    }
    for _ in 0..900 {
        sim.advance_tick(&[], Some(rules), None, None, 67);
        if sim.entities().get(tank).is_none() && sim.entities().get(gi).is_none() {
            break;
        }
    }
    assert!(sim.entities().get(tank).is_none(), "the Grizzly is ground");
    assert!(sim.entities().get(gi).is_none(), "the GI is ground");
    assert!(sim.houses[&owner].economy().credits() > credits, "refunded");
}

/// An ally's Grinder admits the player's vehicle through CAN_LOAD, but the
/// Infantry row answers NoEnter for a Grinder the player does not own: the
/// cursor shows NoEnter and the click orders the GI nothing.
#[test]
fn an_allied_grinder_takes_a_vehicle_but_shows_a_rifleman_noenter() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let registry = crate::rules::overlay_types::OverlayTypeRegistry::from_ini(
        &retail.processed_rules,
        Some(&retail.fixed_art),
    );
    let (sim, grinder, tank, gi) = scene(rules, "Russians");
    assert_eq!(
        cursor(&sim, rules, &registry, tank, grinder),
        Some(CursorId::Enter)
    );
    assert_eq!(
        cursor(&sim, rules, &registry, gi, grinder),
        Some(CursorId::NoEnter)
    );
    let hover = HoverTargetKindWithId {
        stable_id: grinder,
        kind: HoverTargetKind::FriendlyStructure,
    };
    let mut selected = vec![tank, gi];
    assert_eq!(
        grinder_orders(&sim, rules, &mut selected, &hover),
        Some(vec![eaten(tank, grinder)])
    );
    assert!(selected.is_empty(), "the GI's NoEnter consumes the click");
}
