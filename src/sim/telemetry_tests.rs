//! Tests for the dashboard telemetry snapshot.
//!
//! Separated from telemetry.rs to stay within the 400-line file limit.

use crate::map::entities::EntityCategory;
use crate::rules::art_data::ArtRegistry;
use crate::rules::infantry_sequence::parse_infantry_sequence_registry;
use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::Health;
use crate::sim::house_state::HouseState;
use crate::sim::telemetry::{
    CategoryCounts, HousePopulation, TELEMETRY_SCHEMA_VERSION, TelemetrySnapshot,
};
use crate::sim::world::Simulation;

/// Minimal one-vehicle rules fixture, mirroring the animation tests' spawn
/// setup: a voxel Unit with the fields the constructor reads.
fn vehicle_rules() -> RuleSet {
    let rules_ini = IniFile::from_str(
        "[VehicleTypes]\n0=ACTOR\n[ACTOR]\nImage=IMAGE\nStrength=100\nWalkRate=3\nIdleRate=4\n",
    );
    let art = IniFile::from_str("[IMAGE]\nVoxel=yes\nFacings=8\nWalkFrames=6\nStandingFrames=1\n");
    let mut rules = RuleSet::from_ini_with_fixed_art_for_test(&rules_ini, &art).unwrap();
    rules.install_art_data(ArtRegistry::from_ini(&art));
    rules.bind_animation_sequences(&parse_infantry_sequence_registry(&art));
    rules
}

fn house_row<'a>(snapshot: &'a TelemetrySnapshot, name: &str) -> &'a HousePopulation {
    snapshot
        .houses
        .iter()
        .find(|house| house.name == name)
        .unwrap_or_else(|| panic!("no house row for {name} in {snapshot:?}"))
}

fn house_index(snapshot: &TelemetrySnapshot, name: &str) -> u16 {
    snapshot
        .houses
        .iter()
        .position(|house| house.name == name)
        .unwrap_or_else(|| panic!("no house row for {name} in {snapshot:?}")) as u16
}

#[test]
fn category_counts_add_covers_every_category() {
    let mut counts = CategoryCounts::default();
    counts.add(EntityCategory::Unit);
    counts.add(EntityCategory::Infantry);
    counts.add(EntityCategory::Aircraft);
    counts.add(EntityCategory::Structure);
    assert_eq!(
        (
            counts.unit,
            counts.infantry,
            counts.aircraft,
            counts.structure
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(counts.total(), 4);
}

#[test]
fn fresh_simulation_snapshot_is_empty() {
    let sim = Simulation::new();
    let snapshot = sim.telemetry_snapshot();
    assert_eq!(snapshot.version, TELEMETRY_SCHEMA_VERSION);
    assert_eq!(snapshot.tick, 0);
    assert_eq!(snapshot.entities_total, 0);
    assert!(snapshot.houses.is_empty());
    assert!(snapshot.spots.is_empty());
    assert_eq!(snapshot.timing, None);
    assert_eq!(snapshot.map_width, sim.session.map_width);
    assert_eq!(snapshot.map_height, sim.session.map_height);
}

#[test]
fn snapshot_tallies_entities_per_house() {
    let rules = vehicle_rules();
    let mut sim = Simulation::with_seed(31);
    let americans = sim.interner.intern("Americans");
    let soviets = sim.interner.intern("Soviets");
    sim.houses.insert(
        americans,
        HouseState::new(americans, 0, None, true, 1000, 10),
    );
    sim.houses
        .insert(soviets, HouseState::new(soviets, 1, None, false, 1000, 10));

    for _ in 0..2 {
        sim.spawn_object_limbo_at_height("ACTOR", "Americans", 3, 3, 0, 0, &rules)
            .expect("vehicle spawn");
    }
    sim.spawn_object_limbo_at_height("ACTOR", "Soviets", 40, 40, 0, 0, &rules)
        .expect("vehicle spawn");
    // An owner with no house row: counted in the total, absent from the
    // house rows — the anomaly stays visible instead of being hidden.
    sim.spawn_object_limbo_at_height("ACTOR", "French", 60, 60, 0, 0, &rules)
        .expect("vehicle spawn");

    let snapshot = sim.telemetry_snapshot();
    assert_eq!(snapshot.entities_total, 4);
    assert_eq!(snapshot.houses.len(), 2);

    let americans_row = house_row(&snapshot, "Americans");
    assert_eq!(americans_row.counts.unit, 2);
    assert_eq!(americans_row.counts.total(), 2);
    assert_eq!(americans_row.side_index, 0);
    assert!(americans_row.is_human);

    let soviets_row = house_row(&snapshot, "Soviets");
    assert_eq!(soviets_row.counts.unit, 1);
    assert_eq!(soviets_row.side_index, 1);
    assert!(!soviets_row.is_human);

    let housed: u32 = snapshot.houses.iter().map(|h| h.counts.total()).sum();
    assert_eq!(housed, 3, "house rows miss the orphan owner by design");
}

#[test]
fn house_rows_carry_economy_and_roster() {
    let rules = vehicle_rules();
    let mut sim = Simulation::with_seed(37);
    let americans = sim.interner.intern("Americans");
    sim.houses.insert(
        americans,
        HouseState::new(americans, 0, None, true, 1000, 10),
    );
    for _ in 0..3 {
        sim.spawn_object_limbo_at_height("ACTOR", "Americans", 3, 3, 0, 0, &rules)
            .expect("vehicle spawn");
    }
    if let Some(house) = sim.houses.get_mut(&americans) {
        house.economy.add_credits(500);
        house.economy.add_score(42);
    }

    let snapshot = sim.telemetry_snapshot();
    let row = house_row(&snapshot, "Americans");
    assert_eq!(row.credits, 1500, "seeded 1000 plus the 500 deposit");
    assert_eq!(row.score, 42);
    assert_eq!(row.spent, 0);
    assert!(!row.defeated);
    assert_eq!(row.roster.len(), 1, "one type in this fixture");
    assert_eq!(row.roster[0].type_name, "ACTOR");
    assert_eq!(row.roster[0].category, EntityCategory::Unit);
    assert_eq!(row.roster[0].count, 3);
}

#[test]
fn spots_carry_world_positions_and_owner_indices() {
    // The diagnostic insert keeps its requested cell (unlike the limbo
    // spawn, whose native CoordStruct default is (0,0) off-map).
    let mut sim = Simulation::with_seed(31);
    let americans = sim.interner.intern("Americans");
    sim.houses.insert(
        americans,
        HouseState::new(americans, 0, None, true, 1000, 10),
    );
    let french = sim.interner.intern("French");
    let type_ref = sim.interner.intern("ACTOR");
    for (owner, cell) in [(americans, 3u16), (french, 60u16)] {
        sim.insert_synthetic_techno_for_diagnostics(
            cell,
            cell,
            0,
            0,
            owner,
            Health::new(1000),
            type_ref,
            EntityCategory::Unit,
            0,
            6,
            false,
        );
    }

    let snapshot = sim.telemetry_snapshot();
    assert_eq!(snapshot.spots.len(), 2);
    let americans_index = house_index(&snapshot, "Americans");

    let americans_spot = snapshot
        .spots
        .iter()
        .find(|spot| spot.owner_index == americans_index)
        .expect("the American's spot");
    let orphan_spot = snapshot
        .spots
        .iter()
        .find(|spot| spot.owner_index == u16::MAX)
        .expect("the orphan owner's spot");

    for (spot, cell) in [(americans_spot, 3.0), (orphan_spot, 60.0)] {
        assert_eq!(spot.category, EntityCategory::Unit);
        assert_eq!(spot.z, 0);
        assert!(
            (cell..cell + 1.0).contains(&spot.x),
            "x {} is inside cell {} with its sub-cell offset",
            spot.x,
            cell
        );
        assert!(
            (cell..cell + 1.0).contains(&spot.y),
            "y {} is inside cell {} with its sub-cell offset",
            spot.y,
            cell
        );
    }
}

#[test]
fn snapshot_round_trips_through_bincode() {
    let rules = vehicle_rules();
    let mut sim = Simulation::with_seed(31);
    let owner = sim.interner.intern("Americans");
    sim.houses
        .insert(owner, HouseState::new(owner, 0, None, true, 1000, 10));
    sim.spawn_object_limbo_at_height("ACTOR", "Americans", 3, 3, 0, 0, &rules)
        .expect("vehicle spawn");

    let snapshot = sim.telemetry_snapshot();
    let bytes = bincode::serialize(&snapshot).expect("serialize");
    let restored: TelemetrySnapshot = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(snapshot, restored);
}

#[test]
fn houses_carry_their_difficulty() {
    use crate::sim::house_state::HouseDifficulty;

    let mut sim = Simulation::with_seed(41);
    let tough = sim.interner.intern("Computer1");
    let mut tough_house = HouseState::new(tough, 1, None, false, 1000, 10);
    tough_house.difficulty = HouseDifficulty::Hard;
    sim.houses.insert(tough, tough_house);

    let easy = sim.interner.intern("Computer2");
    let mut easy_house = HouseState::new(easy, 1, None, false, 1000, 10);
    easy_house.difficulty = HouseDifficulty::Easy;
    sim.houses.insert(easy, easy_house);

    let snapshot = sim.telemetry_snapshot();
    assert_eq!(snapshot.houses[0].difficulty, "hard");
    assert_eq!(snapshot.houses[1].difficulty, "easy");
    // The app fills the side's [Sides] spelling from the loaded rules.
    assert_eq!(snapshot.houses[0].side_name, None);
}

#[test]
fn repeated_captures_are_equal() {
    let rules = vehicle_rules();
    let mut sim = Simulation::with_seed(31);
    let owner = sim.interner.intern("Americans");
    sim.houses
        .insert(owner, HouseState::new(owner, 0, None, true, 1000, 10));
    sim.spawn_object_limbo_at_height("ACTOR", "Americans", 3, 3, 0, 0, &rules)
        .expect("vehicle spawn");

    assert_eq!(sim.telemetry_snapshot(), sim.telemetry_snapshot());
}

#[test]
fn debug_panel_round_trips_through_bincode() {
    use crate::sim::telemetry::{
        DebugEventEntry, DebugPanel, EntityDesc, LandRow, MinerDebug, OverlayFlags, PathStep,
        SelectedDebug,
    };
    let debug = DebugPanel {
        cursor_cell: Some((12, 34)),
        cursor_elevation: 1,
        grid: Some((120, 120)),
        cursor_walkable: Some(true),
        land_rows: vec![LandRow {
            speed_type: "Foot".to_string(),
            cost: 100,
        }],
        active_speed_type: "Track".to_string(),
        vxl_sprites: Some(42),
        entities_at_cursor: vec![EntityDesc {
            type_name: "E1".to_string(),
            category: "Inf".to_string(),
            owner: "Americans".to_string(),
            hp: 90,
            max_hp: 125,
        }],
        footprint: Some("GAWALL (2x1) @ (10,20) Americans".to_string()),
        selected: vec![SelectedDebug {
            stable_id: 7,
            type_name: "HARV".to_string(),
            cell: (30, 40),
            sub: (128, 200),
            moving: true,
            path_steps_left: 3,
            path_blocked: true,
            goal: Some((35, 42)),
            path_preview: vec![PathStep {
                cell: (31, 40),
                walkable: false,
                first: true,
            }],
            miner: Some(MinerDebug {
                kind: "Ore".to_string(),
                state: "Harvest".to_string(),
                cargo: 3,
                capacity: 7,
                stage: 5,
                rate: 1,
                harvesting: true,
                archived_ore: Some((50, 60)),
                teleport_active: false,
            }),
            inspector_active: true,
            events: vec![DebugEventEntry {
                tick: 99,
                text: "[99] Repath: blocked (12 steps)".to_string(),
            }],
        }],
        overlays: OverlayFlags {
            terrain_costs: true,
            heightmap: false,
            cell_grid: true,
            fog: true,
            unit_inspector: false,
            foot_path: false,
            paused: false,
        },
    };
    let bytes = bincode::serialize(&debug).expect("serialize");
    let restored: DebugPanel = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(debug, restored);
}
