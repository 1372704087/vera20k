//! Rust regression tests for `BuildingClass::AfterDestruction`'s rubble arm,
//! driven through the production damage receiver. Expectations follow the
//! instruction reading recorded in `building_rubble`; they are not native
//! goldens.

use std::fmt::Write as _;

use crate::rules::building_rubble::RubbleArt;
use crate::rules::ini_parser::IniFile;
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::{
    AttackTarget, EntityDamageEvent, RAD_NO_ATTACKER, ReceiverCallFlags, TargetKind,
};
use crate::sim::house_state::HouseState;
use crate::sim::overlay_grid::{OverlayGrid, RUBBLE_OVERLAY_INDEX};
use crate::sim::world::{FrameEffects, Simulation};

const RULES: &str = "\
[InfantryTypes]
[VehicleTypes]
[AircraftTypes]
[BuildingTypes]
0=RUIN
1=PLAIN
2=SHORT
[Warheads]
0=KILLWH
[RUIN]
Strength=100
LeaveRubble=yes
[PLAIN]
Strength=100
[SHORT]
Strength=100
LeaveRubble=yes
[KILLWH]
Verses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%
";

const ART: &str = "\
[RUIN]
Foundation=2x2
[PLAIN]
Foundation=2x2
[SHORT]
Foundation=2x2
";

fn rules() -> RuleSet {
    let mut rules = RuleSet::from_ini_with_fixed_art_for_test(
        &IniFile::from_str(RULES),
        &IniFile::from_str(ART),
    )
    .expect("rubble rules");
    // RUIN's image carries rubble; SHORT's is too short (no art bound).
    rules.set_building_rubble_art_for_test(
        "RUIN",
        RubbleArt {
            shp: "RUIN.SHP".into(),
            body_frame: 3,
        },
    );
    rules
}

/// Dense table whose index 0xEF is an `IsRubble=yes` type, as in retail.
fn registry() -> OverlayTypeRegistry {
    let mut ini = String::from("[OverlayTypes]\n");
    for id in 0..=u16::from(RUBBLE_OVERLAY_INDEX) {
        let name = if id == u16::from(RUBBLE_OVERLAY_INDEX) {
            "RUBBLE_OVERLAY".to_owned()
        } else {
            format!("OV{id:03}")
        };
        writeln!(ini, "{id}={name}").unwrap();
    }
    ini.push_str("[RUBBLE_OVERLAY]\nIsRubble=yes\nDrawFlat=true\nNoUseTileLandType=true\n");
    OverlayTypeRegistry::from_ini(&IniFile::from_str(&ini), None)
}

fn sim() -> Simulation {
    let mut sim = Simulation::with_seed(7);
    let owner = sim.interner.intern("Americans");
    sim.houses
        .insert(owner, HouseState::new(owner, 0, None, false, 0, 10));
    sim.session.house_order.push(owner);
    sim.overlay_grid = Some(OverlayGrid::new(32, 32));
    sim.resolved_terrain = Some(crate::map::resolved_terrain::test_grid(
        32,
        32,
        crate::map::resolved_terrain::test_clear_cell,
    ));
    sim
}

fn kill(sim: &mut Simulation, rules: &RuleSet, registry: &OverlayTypeRegistry, id: u64) {
    let warhead = sim.interner.intern("KILLWH");
    let hit = EntityDamageEvent::direct_receiver(
        id,
        100_000,
        0,
        RAD_NO_ATTACKER,
        None,
        warhead,
        ReceiverCallFlags {
            ignore_defenses: false,
            arg6: false,
        },
    );
    sim.commit_noncombat_aoe_hits(rules, Some(registry), &[hit], FrameEffects::default());
}

fn overlay(sim: &Simulation, cell: (u16, u16)) -> crate::sim::overlay_grid::OverlayCell {
    *sim.overlay_grid.as_ref().unwrap().cell(cell.0, cell.1)
}

/// Every foundation cell takes overlay 0xEF with its data byte kept; only the
/// north-west cell records the type the overlay draws read.
#[test]
fn a_destroyed_leave_rubble_building_leaves_rubble_on_its_foundation() {
    let rules = rules();
    let registry = registry();
    let mut sim = sim();
    let ruin = sim
        .spawn_object_at_height("RUIN", "Americans", 10, 12, 0, 0, &rules)
        .expect("RUIN spawns");
    // A listener force-firing a foundation cell, and installed navigation so
    // the per-cell publish and zone repair run.
    let listener = sim
        .spawn_object_at_height("PLAIN", "Americans", 20, 20, 0, 0, &rules)
        .expect("PLAIN spawns");
    sim.substrate
        .entities
        .get_mut(listener)
        .unwrap()
        .attack_target = Some(AttackTarget {
        target: TargetKind::Cell(11, 13),
    });
    sim.overlay_grid
        .as_mut()
        .unwrap()
        .write_literal_bridge_state(sim.resolved_terrain.as_mut().unwrap(), 11, 13, 5);
    assert!(sim.rebuild_dynamic_navigation(&rules));

    kill(&mut sim, &rules, &registry, ruin);

    assert!(
        sim.substrate
            .entities
            .get(ruin)
            .is_none_or(|entity| !entity.lifecycle.object_alive),
        "the building is gone"
    );
    let ruin_type = sim.interner.get("RUIN").expect("interned type");
    for cell in [(10, 12), (11, 12), (10, 13), (11, 13)] {
        let live = overlay(&sim, cell);
        assert_eq!(live.overlay_id, Some(RUBBLE_OVERLAY_INDEX), "{cell:?}");
        assert_eq!(
            live.rubble_building_type,
            (cell == (10, 12)).then_some(ruin_type),
            "{cell:?}"
        );
    }
    assert_eq!(
        overlay(&sim, (11, 13)).overlay_data,
        5,
        "+0x11E is not written"
    );
    assert_eq!(
        overlay(&sim, (12, 12)).overlay_id,
        None,
        "outside the foundation"
    );
    assert!(
        sim.substrate
            .entities
            .get(listener)
            .unwrap()
            .attack_target
            .is_none(),
        "the cell detach sweep (0x0070D4A0) clears a target on a foundation cell"
    );
    assert!(sim.zone_grid.is_some() && sim.path_grid().is_some());
}

/// No rubble without `LeaveRubble=yes`, and none when the type's image is too
/// short to carry a rubble frame (`0x0045F160` returns false).
#[test]
fn no_rubble_without_leave_rubble_or_rubble_art() {
    let rules = rules();
    let registry = registry();
    for kind in ["PLAIN", "SHORT"] {
        let mut sim = sim();
        let id = sim
            .spawn_object_at_height(kind, "Americans", 10, 12, 0, 0, &rules)
            .expect("building spawns");
        kill(&mut sim, &rules, &registry, id);
        for cell in [(10, 12), (11, 12), (10, 13), (11, 13)] {
            assert_eq!(overlay(&sim, cell).overlay_id, None, "{kind} {cell:?}");
        }
    }
}
