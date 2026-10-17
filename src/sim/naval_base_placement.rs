//! Native naval branch of HouseClass AI base-placement selection.
//!
//! This module owns only the stock-active `Naval=yes` fast path. The ordinary
//! perimeter search is `sim::ai_base_site`, which dispatches here; sharing
//! either path would add gates and ordering that
//! `HouseClass__AI_FindBasePlacement` does not execute.

use crate::map::entities::EntityCategory;
use crate::rules::locomotor_type::{MovementZone, SpeedType};
use crate::rules::object_type::ObjectType;
use crate::rules::ruleset::RuleSet;
use crate::sim::find_nearby_cell::{
    NearbyFootprint, NearbyQuery, PassabilityArgs, find_nearby_passable_cell, map_owned_radius_cap,
};
use crate::sim::house_state::HouseState;
use crate::sim::pathfinding::PathGrid;
use crate::sim::world::Simulation;

/// Find the one naval ready-Building site selected by the native fast path.
///
/// gamemd-derived: `HouseClass__AI_FindBasePlacement @ 0x005060B0`, naval
/// branch `0x005060E8..0x00506213`; type selection delegates to
/// `HouseClass__FirstBuildableFromArray @ 0x005051E0`.
pub(crate) fn find_naval_base_placement(
    sim: &Simulation,
    rules: &RuleSet,
    owner: crate::sim::intern::InternedId,
) -> Option<(u16, u16)> {
    let house = sim.houses.get(&owner)?;

    // Native invokes the source-ordered selector twice rather than retaining
    // the first pointer. Keep the two calls independent even though ordinary
    // immutable Rules yield the same type both times.
    let width_type = first_buildable_shipyard(rules, sim, house)?;
    let width = crate::rules::foundation::foundation_dimensions(&width_type.foundation).0;
    let height_type = first_buildable_shipyard(rules, sim, house)?;
    let height = crate::rules::foundation::foundation_dimensions(&height_type.foundation).1;

    // The native fallback is literal `(0,0)` and reaches normal FNPC/MapClass
    // admission rather than becoming an early Rust-side failure.
    let origin = house.base_origin();

    // This is an active MapClass query. Missing PathGrid, real CellClass
    // terrain, diamond bounds, or Size-height authority cannot be replaced by
    // the compatibility projection without changing bridge pools or Z.
    let path_grid = sim.path_grid()?;
    let resolved_terrain = sim.resolved_terrain.as_ref()?;
    let playfield_bounds = sim.playfield_bounds?;
    let size_height = sim.playfield_size_height?;
    let query = naval_query(
        path_grid,
        resolved_terrain,
        sim.overlay_grid.as_ref(),
        sim.zone_grid.as_ref(),
        playfield_bounds,
        playfield_bounds.base,
        size_height,
        i32::from(width).wrapping_add(2),
        i32::from(height).wrapping_add(2),
    );
    let candidate = find_nearby_passable_cell(
        (i32::from(origin.0 as i16), i32::from(origin.1 as i16)),
        &query,
        sim.session.binary_frame,
    )?;
    if candidate == (0, 0) {
        return None;
    }

    first_yard_distance_accepts(sim, rules, house, candidate, resolved_terrain).then_some(candidate)
}

/// Exact `FirstBuildableFromArray` specialization used for `[General]
/// Shipyard=`. No generic build-option eligibility is consulted.
fn first_buildable_shipyard<'a>(
    rules: &'a RuleSet,
    sim: &Simulation,
    house: &HouseState,
) -> Option<&'a ObjectType> {
    let country = house.country?;
    let country_name = sim.interner.resolve(country);
    crate::sim::ai_buildable::first_buildable_from_array(
        rules,
        &rules.shipyard_types,
        country_name,
        house.side_index,
        sim.session.game_options.super_weapons,
    )
}

fn naval_query<'a>(
    path_grid: &'a PathGrid,
    resolved_terrain: &'a crate::map::resolved_terrain::ResolvedTerrainGrid,
    overlay_grid: Option<&'a crate::sim::overlay_grid::OverlayGrid>,
    zone_grid: Option<&'a crate::sim::pathfinding::zone_map::ZoneGrid>,
    playfield_bounds: crate::sim::cell_rect::PlayfieldBounds,
    size_width: i32,
    size_height: i32,
    footprint_width: i32,
    footprint_height: i32,
) -> NearbyQuery<'a> {
    NearbyQuery {
        native_cells: None,
        raw_occupation: None,
        passability: PassabilityArgs {
            speed_type: SpeedType::Float,
            required_zone_id: None,
            movement_zone: MovementZone::Normal,
            bridge_aware_zone: false,
        },
        footprint: NearbyFootprint::new(footprint_width, footprint_height),
        allow_bridge_cells: true,
        check_height: false,
        check_occupancy: false,
        radius_cap: map_owned_radius_cap(size_width, size_height),
        target_cell: None,
        path_grid: Some(path_grid),
        resolved_terrain: Some(resolved_terrain),
        overlay_grid,
        occupancy: None,
        entities: None,
        zone_grid,
        playfield_bounds: Some(playfield_bounds),
    }
}

/// Compare the chosen CellClass center against exactly the first acquired live
/// BuildConst Building. An empty vector bypasses the cap; a stale first ID is an
/// invariant failure and never silently advances to a later yard.
///
/// gamemd-derived: `HouseClass__AI_FindBasePlacement @ 0x005061BE..0x00506213`,
/// `BuildingClass::GetCoords @ 0x00447AC0`,
/// `CellClass__Get_Center_Coords @ 0x00480A30`, and
/// `CoordStruct__Distance3D @ 0x0041C380`.
fn first_yard_distance_accepts(
    sim: &Simulation,
    rules: &RuleSet,
    house: &HouseState,
    candidate: (u16, u16),
    terrain: &crate::map::resolved_terrain::ResolvedTerrainGrid,
) -> bool {
    let Some(&yard_id) = house.build_const_order.first() else {
        return true;
    };
    let Some(yard) = sim.substrate.entities.get(yard_id) else {
        return false;
    };
    if yard.category() != EntityCategory::Structure
        || !yard.type_facts.build_const_eligible()
        || !yard.lifecycle.object_alive()
        || yard.lifecycle.in_limbo()
        || !yard.lifecycle.cell_marked()
    {
        return false;
    }

    let Some(cell) = terrain.cell(candidate.0, candidate.1) else {
        return false;
    };
    let Some(center) = crate::sim::cell_rect::CellRef::Real(cell).center_coordinate() else {
        return false;
    };

    let yard_coord = crate::sim::movement::ground_pose::object_get_coords(yard, Some(terrain));
    let distance = crate::util::native_x87::distance_3d_leptons(
        [center.x, center.y, center.z],
        [yard_coord.x, yard_coord.y, yard_coord.z],
    );
    let cap = rules.ai_naval_yard_adjacency.wrapping_shl(8);
    distance <= cap
}
