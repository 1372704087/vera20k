//! Unit What_Action's self-arm site tests (`0x0073FE95..0x00740115`) on
//! retail rules: a DeploysInto unit's placement with the unit lifted, and a
//! loaded transport's first-passenger land.

use crate::rules::ruleset::RuleSet;
use crate::rules::terrain_rules::LandType;
use crate::sim::world::Simulation;

fn arena(rules: &RuleSet) -> Simulation {
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 0x24);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    sim
}

/// On clear ground the MCV's own cell lies inside its Construction Yard's
/// foundation (origin one cell north-west), so only the lift admits it; a
/// tank on the foundation refuses it.
#[test]
fn an_mcv_offers_no_deploy_where_its_yard_cannot_stand() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let mut sim = arena(rules);
    let mcv = sim
        .spawn_object("AMCV", "Americans", 12, 12, 0, rules)
        .expect("mcv");
    assert!(!sim.unit_self_deploy_site_refused(mcv, rules, None));
    let yard = rules.object("GACNST").expect("yard");
    assert!(
        !crate::sim::build_site::can_place_building_at(
            &sim,
            rules,
            None,
            yard,
            (11, 11),
            None,
            None,
            None,
        ),
        "the unlifted MCV stands on its own foundation"
    );
    sim.spawn_object("HTNK", "Russians", 14, 14, 0, rules)
        .expect("tank on the foundation's far corner");
    assert!(sim.unit_self_deploy_site_refused(mcv, rules, None));
}

/// An Amphibious Transport (`[LCRF] SpeedType=Hover`) whose first passenger
/// is a GI refuses its unload on a Beach (Foot 0%), not on Clear ground or
/// on Water.
#[test]
fn a_loaded_transport_on_a_beach_offers_no_deploy() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let mut sim = arena(rules);
    let transport = sim
        .spawn_object("LCRF", "Americans", 12, 12, 0, rules)
        .expect("transport");
    let gi = sim
        .spawn_object("E1", "Americans", 20, 20, 0, rules)
        .expect("gi");
    let mut cargo = crate::sim::passenger::PassengerCargo::new(12, 0.0);
    cargo.board_forced_chain(&[(gi, 1.0)]);
    sim.entities_mut_for_test()
        .get_mut(transport)
        .unwrap()
        .passenger_role = crate::sim::passenger::PassengerRole::transport(cargo);
    let land = |sim: &mut Simulation, land: LandType| {
        sim.resolved_terrain_mut_for_test()
            .as_mut()
            .unwrap()
            .cell_mut_for_test(12, 12)
            .unwrap()
            .land_type = land.as_index();
    };
    assert!(!sim.unit_self_deploy_site_refused(transport, rules, None));
    land(&mut sim, LandType::Beach);
    assert!(sim.unit_self_deploy_site_refused(transport, rules, None));
    land(&mut sim, LandType::Water);
    assert!(!sim.unit_self_deploy_site_refused(transport, rules, None));
}
