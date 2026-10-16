//! Test-only flat arena: a clear 32x32 map with its playfield, zones and path
//! grid, for fixtures that order units around through `advance_tick`, and
//! the native map inputs a component fixture's Foot Process reads.

use crate::rules::ruleset::RuleSet;
use crate::sim::pathfinding::PathGrid;
use crate::sim::world::Simulation;

/// Explicit rules prior for a synthetic clear-ground component scene.
/// Native LandType defaults are not buildable; an arena intended to host
/// ordinary buildings must supply the retail Clear `Buildable=yes` input.
/// Merge the scene last so its deliberate overrides still take precedence.
pub fn clear_ground_ini(source: &str) -> crate::rules::ini_parser::IniFile {
    let mut ini = crate::rules::ini_parser::IniFile::from_str("[Clear]\nBuildable=yes\n");
    ini.merge(&crate::rules::ini_parser::IniFile::from_str(source));
    ini
}

/// The arena's playfield, wide enough to hold every arena cell (and the shared
/// dummy just past its edge).
pub(crate) const OPEN_PLAYFIELD: crate::sim::cell_rect::PlayfieldBounds =
    crate::sim::cell_rect::PlayfieldBounds {
        base: 20,
        off_fc: -128,
        off_100: -128,
        off_104: 256,
        off_108: 256,
    };

/// The arena's ground with two rival houses, not allied: `Russians`, a Normal
/// computer house of the skirmish IQ (`[IQ] MaxIQLevels`), and `Americans`, a
/// human house.
pub(crate) fn rival_arena(rules: &RuleSet, seed: u64) -> Simulation {
    rival_arena_with_size(rules, seed, 32)
}

/// The same two-house arena with enough room for a mechanism's footprint.
pub(crate) fn rival_arena_with_size(rules: &RuleSet, seed: u64, size: u16) -> Simulation {
    let mut sim = Simulation::with_seed(seed);
    flat_ground_with_size(&mut sim, rules, size);
    for (name, human) in [("Russians", false), ("Americans", true)] {
        let id = sim.interner.intern(name);
        let mut house = crate::sim::house_state::HouseState::new(id, 0, None, human, 0, 10);
        house.difficulty = crate::sim::house_state::HouseDifficulty::Normal;
        house.current_iq = if human {
            0
        } else {
            rules.general.max_iq_levels
        };
        sim.houses.insert(id, house);
        sim.session.house_order.push(id);
    }
    sim
}

/// The rival-house arena with flat Water cells and the production rules'
/// land semantics. Ships enter through ordinary Unlimbo; no movement tuning
/// or missing-terrain placement bypass is needed.
pub fn rival_water_arena(rules: &RuleSet, seed: u64, size: u16) -> Simulation {
    use crate::rules::terrain_rules::LandType;

    let mut sim = rival_arena_with_size(rules, seed, size);
    let water = rules
        .terrain_rules
        .semantics_for_land_type(LandType::Water.as_index())
        .expect("Water land semantics");
    let mut terrain = clear_ground(size);
    for y in 0..size {
        for x in 0..size {
            let cell = terrain.cell_mut(x, y).unwrap();
            cell.land_type = LandType::Water.as_index();
            cell.yr_cell_land_type = cell.land_type;
            cell.terrain_class = water.terrain_class;
            cell.speed_costs = water.speed_costs;
            cell.is_water = water.water;
            cell.is_cliff_like = water.cliff_like;
            cell.is_rough = water.rough;
            cell.is_road = water.road;
            cell.ground_walk_blocked = water.ground_blocked;
            cell.base_ground_walk_blocked = water.ground_blocked;
            cell.base_build_blocked = !water.buildable;
            cell.base_land_type = cell.land_type;
            cell.base_yr_cell_land_type = cell.yr_cell_land_type;
            cell.base_terrain_class = cell.terrain_class;
            cell.base_speed_costs = cell.speed_costs;
            cell.zone_type = crate::map::resolved_terrain::recalc_zone_type(
                false,
                None,
                cell.land_type,
                cell.speed_costs.wheel,
                None,
            );
        }
    }
    sim.install_resolved_terrain_for_new_map(terrain);
    assert!(sim.rebuild_dynamic_navigation(rules));
    sim
}

/// Install the arena on `sim`, with commands executing on the next tick, and
/// return its path grid.
pub fn flat_arena(sim: &mut Simulation, rules: &RuleSet) -> PathGrid {
    sim.input_delay_ticks = 0;
    flat_ground(sim, rules)
}

/// Install the arena's ground, playfield and navigation on `sim` and return
/// its path grid.
pub fn flat_ground(sim: &mut Simulation, rules: &RuleSet) -> PathGrid {
    flat_ground_with_size(sim, rules, 32)
}

fn flat_ground_with_size(sim: &mut Simulation, rules: &RuleSet, size: u16) -> PathGrid {
    sim.session.map_width = size;
    sim.session.map_height = size;
    sim.install_resolved_terrain_for_new_map(clear_ground(size));
    sim.playfield_bounds = Some(OPEN_PLAYFIELD);
    sim.playfield_size_height = Some(20);
    install_empty_arena_navigation(sim, rules, size);
    assert!(sim.rebuild_dynamic_navigation(rules));
    sim.path_grid_snapshot()
        .map(|grid| (*grid).clone())
        .expect("navigation grid")
}

/// Before an empty arena's first navigation rebuild, install the zones and
/// path grid that rebuild makes, and the Dummy cell state it leaves: every
/// empty arena of one size gets the same ones, so they are built once per
/// test process for each size and starting Dummy state. The rebuild keeps an
/// installed copy because its path grid and movement classes still match. An
/// arena that already holds objects, bridges or navigation builds its own.
fn install_empty_arena_navigation(sim: &mut Simulation, rules: &RuleSet, size: u16) {
    if !(sim.substrate.entities.is_empty()
        && sim.bridge_state.is_none()
        && sim.zone_grid.is_none()
        && sim.path_grid.is_none())
    {
        return;
    }
    let dummy = sim
        .resolved_terrain
        .as_ref()
        .expect("arena terrain")
        .shared_cell_dummy();
    let (zones, path, built) =
        crate::test_fixture::once_per_process(("empty arena", size, dummy.state_words()), || {
            let mut probe = Simulation::with_seed(0);
            probe.install_resolved_terrain_for_new_map(clear_ground(size));
            probe.playfield_bounds = Some(OPEN_PLAYFIELD);
            let probe_dummy = probe.resolved_terrain.as_ref()?.shared_cell_dummy();
            probe_dummy.adopt_prepared_load_state(&dummy);
            assert!(probe.rebuild_dynamic_navigation(rules));
            Some((
                probe.zone_grid?,
                probe.path_grid?,
                probe_dummy.fork_query_identity(),
            ))
        })
        .expect("arena navigation");
    dummy.adopt_prepared_load_state(&built);
    sim.zone_grid = Some(zones);
    sim.path_grid = Some(path);
}

/// A `size` x `size` grid of clear ground cells.
fn clear_ground(size: u16) -> crate::map::resolved_terrain::ResolvedTerrainGrid {
    let clear = crate::rules::terrain_rules::SpeedCostProfile {
        foot: Some(100),
        track: Some(100),
        wheel: Some(100),
        float: None,
        amphibious: Some(80),
        float_beach: None,
        hover: Some(50),
    };
    let cell = |x, y| {
        let mut cell = crate::map::resolved_terrain::test_flat_cell(x, y);
        cell.speed_costs = clear;
        cell.base_speed_costs = clear;
        cell
    };
    crate::map::resolved_terrain::ResolvedTerrainGrid::from_cells(
        size,
        size,
        (0..size)
            .flat_map(|y| (0..size).map(move |x| cell(x, y)))
            .collect(),
    )
}

/// Supply whatever native map input a component fixture lacks for a Foot
/// Process that searches: map cells (clear 64x64 ground), zones and Map Size
/// for those cells, generous LocalSize bounds, and the path grid. Inputs the
/// fixture already installed are kept, including a zone grid built before a
/// later terrain change; rebuild zones after editing cells.
pub fn supply_native_map(sim: &mut Simulation) {
    use crate::sim::pathfinding::zone_map::ZoneGrid;

    let supplies_ground = sim.resolved_terrain.is_none();
    if supplies_ground {
        // A newly supplied map also owns its saved real CellClass values.
        // Keep already installed terrain and deliberate native priors intact.
        sim.install_resolved_terrain_for_new_map(clear_ground(64));
    }
    let terrain = sim.resolved_terrain.as_ref().unwrap();
    let (width, height) = (terrain.width(), terrain.height());
    let grid = PathGrid::from_resolved_terrain(terrain);
    let size = (i32::from(width), i32::from(height));
    if sim.zone_grid.is_none() {
        // Every supplied clear ground has the same zones; a fixture builds
        // them once per test process and installs a copy.
        static CLEAR_GROUND_ZONES: std::sync::OnceLock<ZoneGrid> = std::sync::OnceLock::new();
        sim.zone_grid = Some(if supplies_ground {
            CLEAR_GROUND_ZONES
                .get_or_init(|| native_zones(&grid, terrain))
                .clone()
        } else {
            native_zones(&grid, terrain)
        });
    }
    if sim.playfield_bounds.is_none() {
        let span = size.0.max(size.1);
        sim.playfield_bounds = Some(crate::sim::cell_rect::PlayfieldBounds {
            base: size.0,
            off_fc: -span,
            off_100: -span,
            off_104: span * 2,
            off_108: span * 2,
        });
    }
    sim.playfield_size_height.get_or_insert(size.1);
    sim.install_fixture_path_grid(Some(&grid));
}

/// [`supply_native_map`] for a fixture whose installed terrain is a function
/// of `terrain_key` alone: the zones, and the Dummy cell state their build
/// leaves, are built once per test process for each key and starting Dummy
/// state.
pub fn supply_native_map_keyed<K: Ord + Send + 'static>(sim: &mut Simulation, terrain_key: K) {
    if sim.zone_grid.is_none()
        && let Some(terrain) = sim.resolved_terrain.as_ref()
    {
        let dummy = terrain.shared_cell_dummy();
        let (zones, built) =
            crate::test_fixture::once_per_process((terrain_key, dummy.state_words()), || {
                let zones = native_zones(&PathGrid::from_resolved_terrain(terrain), terrain);
                Some((zones, dummy.fork_query_identity()))
            })
            .expect("keyed fixture zones");
        dummy.adopt_prepared_load_state(&built);
        sim.zone_grid = Some(zones);
    }
    supply_native_map(sim);
}

/// The zones [`supply_native_map`] builds for `terrain`, whose path grid is
/// `grid`.
fn native_zones(
    grid: &PathGrid,
    terrain: &crate::map::resolved_terrain::ResolvedTerrainGrid,
) -> crate::sim::pathfinding::zone_map::ZoneGrid {
    let (width, height) = (terrain.width(), terrain.height());
    crate::sim::pathfinding::zone_map::ZoneGrid::build_with_native_bridge_geometry(
        grid,
        terrain,
        &[],
        width,
        height,
        Some((i32::from(width), i32::from(height))),
    )
}

#[cfg(any(test, feature = "test-support"))]
pub mod mastermind;

/// Explicit drawing-consumer prior, using the existing private cargo setter.
/// It establishes no harvesting, movement, deposit or full native lifecycle.
pub fn install_status_miner_cargo(
    entity: &mut crate::sim::game_entity::GameEntity,
    object: &crate::rules::object_type::ObjectType,
    rules: &RuleSet,
    ore: usize,
    gems: usize,
) {
    use crate::sim::miner::{CargoBale, Miner, MinerConfig, ResourceType, miner_kind_for_object};
    let config = MinerConfig::from_rules(rules);
    let kind = miner_kind_for_object(object).expect("native prior is a miner");
    let mut miner = Miner::new(kind, &config, object.storage.max(0) as u16);
    miner.set_cargo_for_test(
        std::iter::repeat_n(
            CargoBale {
                resource_type: ResourceType::Ore,
                value: config.ore_bale_value,
            },
            ore,
        )
        .chain(std::iter::repeat_n(
            CargoBale {
                resource_type: ResourceType::Gem,
                value: config.gem_bale_value,
            },
            gems,
        ))
        .collect(),
    );
    entity.miner = Some(miner);
}

/// Explicit retained-count prior for a drawing comparison. Capture and release
/// gameplay remains covered by the production Mastermind fixture.
pub fn install_status_capture_count(
    entity: &mut crate::sim::game_entity::GameEntity,
    capacity: i32,
    infinite: bool,
    count: u32,
) {
    let victims: Vec<_> = (0..count).map(|index| u64::from(index) + 10_000).collect();
    entity.capture_manager = Some(
        crate::sim::capture_manager::CaptureManagerState::with_victims_for_test(
            capacity, infinite, &victims,
        ),
    );
}
