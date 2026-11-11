//! Regression tests for Phobos `DropCrate=` through the production
//! destruction transaction (`Simulation::record_the_kill`, the
//! `0x702E4E` hook site). Expectations follow the instruction reading in
//! `phobos_drop_crate`; they are not native goldens.

use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::{KillCallback, KillSource};
use crate::sim::overlay_grid::OverlayGrid;
use crate::sim::world::{Simulation, UninitContext};

const RULES: &str = "\
[InfantryTypes]
[VehicleTypes]
0=GIFT
[AircraftTypes]
[BuildingTypes]
[CrateRules]
CrateImg=SILVER
WoodCrateImg=WOOD
WaterCrateImg=WATER
[GIFT]
Name=Gift
Strength=100
";

fn rules(gift_keys: &str) -> RuleSet {
    RuleSet::from_ini(&crate::sim::arena_fixture::clear_ground_ini(&format!(
        "{RULES}{gift_keys}\n"
    )))
    .expect("drop-crate rules")
}

fn registry() -> OverlayTypeRegistry {
    crate::sim::crates::tests::crate_registry()
}

fn sim(rules: &RuleSet) -> Simulation {
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 11);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    sim.overlay_grid = Some(OverlayGrid::new(32, 32));
    sim
}

fn destroy(sim: &mut Simulation, rules: &RuleSet, registry: &OverlayTypeRegistry, id: u64) {
    sim.record_the_kill(
        id,
        KillSource::Techno(None),
        KillCallback::Terminal,
        UninitContext::new(rules, Some(registry)),
    );
}

/// `TechnoExt::GetDropCrateIndex` + `TryToCreateCrate`
/// (`Ext/Techno/Hooks.cpp:667-686`, `Ext/Techno/Body.cpp:579-656`): a
/// `DropCrate=Money` victim leaves a crate carrying the `[Powerups]` slot on
/// the cell it died on.
#[test]
fn drop_crate_places_its_powerup_on_the_death_cell() {
    let rules = rules("DropCrate=Money");
    let registry = registry();
    let mut sim = sim(&rules);
    let victim = sim
        .spawn_object("GIFT", "Americans", 10, 10, 0, &rules)
        .expect("victim");

    destroy(&mut sim, &rules, &registry, victim);

    let cell = sim.overlay_grid.as_ref().unwrap().cell(10, 10);
    assert!(cell.overlay_id.is_some(), "the death cell carries a crate");
    assert_eq!(
        cell.overlay_data,
        crate::rules::powerups::POWERUP_MONEY as u8,
        "the fixed [Powerups] slot is written to the crate's data byte"
    );
}

/// An unset `DropCrate=` leaves nothing: `GetDropCrateIndex` answers `-1`
/// and the hook places no crate.
#[test]
fn an_unset_drop_crate_places_nothing() {
    let rules = rules("");
    let registry = registry();
    let mut sim = sim(&rules);
    let victim = sim
        .spawn_object("GIFT", "Americans", 10, 10, 0, &rules)
        .expect("victim");

    destroy(&mut sim, &rules, &registry, victim);

    let grid = sim.overlay_grid.as_ref().unwrap();
    assert!(
        (9..=11).all(|x| (9..=11).all(|y| grid.cell(x, y).overlay_id.is_none())),
        "no crate appears around the death cell"
    );
}
