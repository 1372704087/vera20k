//! Per-SpeedType land-row grid, published beside the navigation views.
//!
//! Each cell carries its land-type row for one SpeedType (0 = impassable,
//! otherwise a whole-percent speed). No simulation decision reads the values.
//! The path search asks the Foot's `+0x1AC` class (`world::object_entry`), which
//! reads the cell's own land row (`UnitClass::Can_Enter_Cell` at `0x0073FAB5`,
//! `InfantryClass::Can_Enter_Cell` at `0x0051C750`); the runtime speed chain
//! reads the cell's row itself (`sim::pathfinding::terrain_speed`). App
//! diagnostics display the grid, and the Recalc publication gates read its
//! emptiness as "navigation installed".
//!
//! ## Dependency rules
//! - Part of sim/ — depends on map/ (MapCell, TilesetLookup).
//! - sim/ NEVER depends on render/, ui/, audio/, net/.

use std::collections::BTreeMap;

use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::rules::locomotor_type::SpeedType;

/// Normal terrain speed — no bonus, no penalty.
const COST_NORMAL: u8 = 100;
/// Rough terrain — slower movement for tracked/wheeled.
const COST_ROUGH: u8 = 75;
/// Blocked / impassable for this SpeedType.
const COST_BLOCKED: u8 = 0;

/// Per-cell land row for one SpeedType.
///
/// Values: 0 = impassable, otherwise a whole-percent speed. Built once per
/// SpeedType from map data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainCostGrid {
    /// The row with an elevated deck overriding the terrain beneath it: the
    /// *deck* answer on a stamped cell.
    costs: Vec<u8>,
    /// Row of the cell's own land type with no deck override — what
    /// `UnitClass::Can_Enter_Cell` @ `0x0073FAB5` and
    /// `InfantryClass::Can_Enter_Cell` @ `0x0051C750` read for a mover on the
    /// ground plane beneath a span. Identical to `costs` off-deck.
    ground_costs: Vec<u8>,
    width: u16,
    height: u16,
}

/// Build every ground/naval terrain-cost row.
/// Winged movement deliberately has no grid because it ignores terrain.
pub(crate) fn build_canonical_terrain_cost_grids(
    terrain: &ResolvedTerrainGrid,
) -> BTreeMap<SpeedType, TerrainCostGrid> {
    SpeedType::ALL_WITH_COSTS
        .iter()
        .copied()
        .map(|speed_type| {
            (
                speed_type,
                TerrainCostGrid::from_resolved_terrain(terrain, speed_type),
            )
        })
        .collect()
}

/// Publish one recalculated terrain cell into the canonical cost grids. A
/// missing, partial or mis-sized set (loading) is rebuilt whole instead.
pub(crate) fn refresh_canonical_terrain_costs_at(
    grids: &mut BTreeMap<SpeedType, TerrainCostGrid>,
    terrain: &ResolvedTerrainGrid,
    (rx, ry): (u16, u16),
) {
    let installed = grids.len() == SpeedType::ALL_WITH_COSTS.len()
        && grids
            .values()
            .all(|costs| costs.width() == terrain.width() && costs.height() == terrain.height());
    let refreshed = installed
        && terrain.cell(rx, ry).is_some_and(|cell| {
            grids
                .iter_mut()
                .all(|(&speed_type, costs)| costs.refresh_resolved_cell(cell, speed_type))
        });
    if !refreshed {
        *grids = build_canonical_terrain_cost_grids(terrain);
    }
}

/// One cell's `(row, ground_row)`: the first carries the elevated-deck
/// override, the second is the land row of the terrain itself. They differ only
/// on an elevated deck cell.
fn resolved_cell_costs(
    cell: &crate::map::resolved_terrain::ResolvedTerrainCell,
    speed_type: SpeedType,
) -> (u8, u8) {
    let ramp_passable = cell.canonical_ramp.is_some();
    // Retail terrain-object occupation is a sub-cell mask on the ground
    // occupation plane, and only the infantry entry gate reads it: the
    // cell closes to infantry only when every functional sub-cell bit is
    // set. Vehicles keep the whole-cell block, so the relaxation is
    // scoped to the Foot row.
    let terrain_object_blocked = if speed_type == SpeedType::Foot {
        cell.terrain_object_occupation.is_some_and(|ini_bits| {
            super::core::terrain_object_blocks_infantry(
                crate::sim::terrain_object::terrain_raw_occupation_mask(ini_bits),
            )
        })
    } else {
        cell.terrain_object_blocks
    };
    let hard_blocked =
        (cell.is_cliff_like && !ramp_passable) || cell.overlay_blocks || terrain_object_blocked;
    let ground = if hard_blocked {
        COST_BLOCKED
    } else if ramp_passable {
        COST_NORMAL
    } else if let Some(resolved) = cell.speed_costs.cost_for_speed_type(speed_type) {
        // The parsed land row is the primary source — rules.ini's
        // [Clear], [Rough], [Tiberium] … sections, one whole-percent
        // value per SpeedType. 0 = impassable, >0 = passable.
        resolved
    } else {
        classify_terrain_cost(
            speed_type,
            cell.is_water,
            // `ground_walk_blocked` folds in the terrain object; for the
            // Foot row the sub-cell rule above already decided that.
            if speed_type == SpeedType::Foot {
                cell.base_ground_walk_blocked || cell.overlay_blocks
            } else {
                cell.ground_walk_blocked
            },
            cell.is_rough,
            cell.is_road,
        )
    };
    // Bridge deck overrides underlying terrain (water/cliff) for ground units.
    // Units walk on the bridge surface, not the terrain below.
    let planner = if cell.is_elevated_bridge_cell() && !cell.overlay_blocks {
        COST_NORMAL
    } else {
        ground
    };
    (planner, ground)
}

impl TerrainCostGrid {
    /// Build a terrain cost grid from resolved terrain metadata.
    ///
    /// Uses the cell's INI-backed land row as the terrain substrate. When no row
    /// was parsed, [`classify_terrain_cost`] answers instead — a VERA-internal
    /// fallback, see its own note; reduced-zone matrix rows are not SpeedType
    /// data.
    pub fn from_resolved_terrain(terrain: &ResolvedTerrainGrid, speed_type: SpeedType) -> Self {
        let size: usize = terrain.width() as usize * terrain.height() as usize;
        let mut costs: Vec<u8> = vec![COST_BLOCKED; size];
        let mut ground_costs: Vec<u8> = vec![COST_BLOCKED; size];

        for cell in terrain.iter() {
            let idx: usize = cell.ry as usize * terrain.width() as usize + cell.rx as usize;
            if idx >= costs.len() {
                continue;
            }
            let (planner, ground) = resolved_cell_costs(cell, speed_type);
            costs[idx] = planner;
            ground_costs[idx] = ground;
        }

        Self {
            costs,
            ground_costs,
            width: terrain.width(),
            height: terrain.height(),
        }
    }

    /// Project one live Recalc cell without refreshing unrelated cached rows.
    pub(crate) fn refresh_resolved_cell(
        &mut self,
        cell: &crate::map::resolved_terrain::ResolvedTerrainCell,
        speed_type: SpeedType,
    ) -> bool {
        if cell.rx >= self.width || cell.ry >= self.height {
            return false;
        }
        let index = usize::from(cell.ry) * usize::from(self.width) + usize::from(cell.rx);
        let (planner, ground) = resolved_cell_costs(cell, speed_type);
        self.costs[index] = planner;
        self.ground_costs[index] = ground;
        true
    }

    /// Get the speed modifier for a cell (0 = blocked, 100 = normal).
    pub fn cost_at(&self, x: u16, y: u16) -> u8 {
        if x >= self.width || y >= self.height {
            return COST_BLOCKED;
        }
        self.costs[y as usize * self.width as usize + x as usize]
    }

    /// The cell's own land row, ignoring any elevated deck above it — the
    /// answer for a mover on the ground plane beneath a span. Equal to
    /// [`Self::cost_at`] everywhere else.
    pub fn ground_cost_at(&self, x: u16, y: u16) -> u8 {
        if x >= self.width || y >= self.height {
            return COST_BLOCKED;
        }
        self.ground_costs[y as usize * self.width as usize + x as usize]
    }

    /// Map width in cells.
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Map height in cells.
    pub fn height(&self) -> u16 {
        self.height
    }
}

/// Determine the terrain cost for a cell given its SpeedType and tile classification.
///
/// **VERA-internal, and it deliberately disagrees with gamemd.** This answers
/// for a cell whose land-type section was absent from the INI. Its non-zero
/// constants (Foot rough 90, Track rough 75, Wheel rough 60) have no gamemd
/// source at all, and its *permissiveness* is the opposite of retail:
/// `RulesClass::ReadSpeedTypeLandTypeTable` @ `0x00674000` skips a row whose
/// section `INIClass::FindSectionByName` cannot find, and the block at
/// `0x0089EA40` is all zero bytes in the image with no other initializer, so an
/// unauthored land type is **impassable to every SpeedType** in gamemd. Only a
/// missing *key* inside a present section defaults to `1.0`.
///
/// Not aligned deliberately: `SpeedCostProfile::default()` — all rows `None` —
/// is also what every synthetic cell in the codebase carries, so making the
/// unparsed case impassable would close every fixture-built map rather than
/// reproduce a retail behaviour. Trigger: a mod or map INI that omits one of the
/// twelve land-type sections. Player effect: retail refuses to move onto that
/// land type at all; VERA treats it as ordinary ground. Frequency: **zero in
/// ordinary skirmish** — `rulesmd.ini` carries all twelve, and `parse_cost`
/// never returns `None` for a section that exists, so no cell reaches this arm.
/// Downstream risk: none while the A* reads the value as a predicate only.
fn classify_terrain_cost(
    speed_type: SpeedType,
    is_water: bool,
    is_cliff: bool,
    is_rough: bool,
    _is_road: bool,
) -> u8 {
    match speed_type {
        // Reduced fallback only: no native invalid-index terrain behavior is
        // claimed. Live Foot entry reports an unavailable speed row instead.
        SpeedType::Invalid => COST_BLOCKED,
        SpeedType::Foot => {
            if is_water || is_cliff {
                COST_BLOCKED
            } else if is_rough {
                // Infantry handle rough terrain better than vehicles.
                90
            } else {
                COST_NORMAL
            }
        }
        SpeedType::Track => {
            if is_water || is_cliff {
                COST_BLOCKED
            } else if is_rough {
                COST_ROUGH
            } else {
                COST_NORMAL
            }
        }
        SpeedType::Wheel => {
            if is_water || is_cliff {
                COST_BLOCKED
            } else if is_rough {
                // Wheeled vehicles are even slower on rough terrain.
                60
            } else {
                COST_NORMAL
            }
        }
        SpeedType::Float | SpeedType::FloatBeach | SpeedType::Hover => {
            // Hover/float units can cross water.
            if is_cliff { COST_BLOCKED } else { COST_NORMAL }
        }
        SpeedType::Amphibious => {
            // Amphibious units cross both land and water.
            if is_cliff { COST_BLOCKED } else { COST_NORMAL }
        }
        SpeedType::Winged => {
            // Aircraft ignore all terrain.
            COST_NORMAL
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::resolved_terrain::ResolvedTerrainCell;
    use crate::map::resolved_terrain::ResolvedTerrainGrid;
    use crate::rules::terrain_rules::SpeedCostProfile;

    #[test]
    fn test_track_on_water_is_blocked() {
        let cost = classify_terrain_cost(SpeedType::Track, true, false, false, false);
        assert_eq!(cost, COST_BLOCKED);
    }

    #[test]
    fn test_track_on_road_is_normal() {
        let cost = classify_terrain_cost(SpeedType::Track, false, false, false, true);
        assert_eq!(cost, COST_NORMAL);
    }

    #[test]
    fn test_track_on_rough_is_slow() {
        let cost = classify_terrain_cost(SpeedType::Track, false, false, true, false);
        assert_eq!(cost, COST_ROUGH);
    }

    #[test]
    fn test_float_on_water_is_passable() {
        let cost = classify_terrain_cost(SpeedType::Float, true, false, false, false);
        assert_eq!(cost, COST_NORMAL);
    }

    #[test]
    fn test_winged_ignores_terrain() {
        assert_eq!(
            classify_terrain_cost(SpeedType::Winged, true, true, true, false),
            COST_NORMAL
        );
    }

    #[test]
    fn test_foot_on_rough_is_less_penalized() {
        let foot = classify_terrain_cost(SpeedType::Foot, false, false, true, false);
        let track = classify_terrain_cost(SpeedType::Track, false, false, true, false);
        assert!(
            foot > track,
            "Infantry handle rough terrain better than vehicles"
        );
    }

    #[test]
    fn test_wheel_on_rough_is_most_penalized() {
        let wheel = classify_terrain_cost(SpeedType::Wheel, false, false, true, false);
        let track = classify_terrain_cost(SpeedType::Track, false, false, true, false);
        assert!(
            wheel < track,
            "Wheeled vehicles suffer more on rough terrain"
        );
    }

    #[test]
    fn test_from_resolved_terrain_uses_resolved_surface_classes() {
        use crate::sim::pathfinding::passability::LandType;
        let terrain = ResolvedTerrainGrid::from_cells(
            2,
            2,
            vec![
                ResolvedTerrainCell {
                    is_road: true,
                    land_type: LandType::Road.as_index(),
                    speed_costs: SpeedCostProfile {
                        track: Some(120),
                        ..SpeedCostProfile::default()
                    },
                    ..make_resolved_cell(0, 0)
                },
                ResolvedTerrainCell {
                    is_rough: true,
                    land_type: LandType::Rough.as_index(),
                    speed_costs: SpeedCostProfile {
                        track: Some(75),
                        wheel: Some(60),
                        foot: Some(90),
                        ..SpeedCostProfile::default()
                    },
                    ..make_resolved_cell(1, 0)
                },
                ResolvedTerrainCell {
                    is_water: true,
                    land_type: LandType::Water.as_index(),
                    ground_walk_blocked: true,
                    speed_costs: SpeedCostProfile {
                        track: Some(0),
                        hover: Some(100),
                        ..SpeedCostProfile::default()
                    },
                    ..make_resolved_cell(0, 1)
                },
                ResolvedTerrainCell {
                    is_cliff_like: true,
                    land_type: LandType::Rock.as_index(),
                    ground_walk_blocked: true,
                    ..make_resolved_cell(1, 1)
                },
            ],
        );
        let track = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Track);
        let hover = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Hover);
        // Road cell uses INI speed cost (Track=120) — no road bonus in A*.
        assert_eq!(track.cost_at(0, 0), 120);
        assert_eq!(track.cost_at(1, 0), COST_ROUGH);
        assert_eq!(track.cost_at(0, 1), COST_BLOCKED);
        assert_eq!(hover.cost_at(0, 1), COST_NORMAL);
        assert_eq!(hover.cost_at(1, 1), COST_BLOCKED);
    }

    #[test]
    fn gsi_04_13_synthetic_low_bridge_uses_ground_surface_cost_not_deck_override() {
        use crate::sim::pathfinding::passability::LandType;

        let custom_surface = SpeedCostProfile {
            track: Some(55),
            ..SpeedCostProfile::default()
        };
        let terrain = ResolvedTerrainGrid::from_cells(
            2,
            1,
            vec![
                ResolvedTerrainCell {
                    level: 2,
                    has_bridge_deck: true,
                    bridge_walkable: false,
                    bridge_deck_level: 2,
                    land_type: LandType::Rough.as_index(),
                    is_rough: true,
                    speed_costs: custom_surface,
                    ..make_resolved_cell(0, 0)
                },
                ResolvedTerrainCell {
                    level: 0,
                    has_bridge_deck: true,
                    bridge_walkable: true,
                    bridge_deck_level: 4,
                    land_type: LandType::Rough.as_index(),
                    is_rough: true,
                    speed_costs: custom_surface,
                    ..make_resolved_cell(1, 0)
                },
            ],
        );
        let track = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Track);

        assert_eq!(track.cost_at(0, 0), 55, "ground-level bridge uses TMP Land");
        assert_eq!(
            track.cost_at(1, 0),
            COST_NORMAL,
            "elevated deck overrides TMP Land"
        );
        assert_eq!(
            track.ground_cost_at(1, 0),
            55,
            "the ground plane beneath the deck keeps the TMP Land row"
        );
        assert_eq!(track.ground_cost_at(0, 0), track.cost_at(0, 0));
    }

    /// The row a ground-plane mover reads beneath a span is the terrain's own:
    /// water under an elevated deck closes to Track while the deck stays open.
    #[test]
    fn elevated_deck_over_water_keeps_a_closed_ground_row() {
        use crate::sim::pathfinding::passability::LandType;

        let terrain = ResolvedTerrainGrid::from_cells(
            1,
            1,
            vec![ResolvedTerrainCell {
                level: 1,
                is_water: true,
                has_bridge_deck: true,
                bridge_walkable: true,
                bridge_deck_level: 5,
                land_type: LandType::Water.as_index(),
                ground_walk_blocked: true,
                speed_costs: SpeedCostProfile {
                    track: Some(0),
                    hover: Some(100),
                    ..SpeedCostProfile::default()
                },
                ..make_resolved_cell(0, 0)
            }],
        );
        let track = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Track);
        let hover = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Hover);
        assert_eq!(track.cost_at(0, 0), COST_NORMAL);
        assert_eq!(track.ground_cost_at(0, 0), COST_BLOCKED);
        assert_eq!(hover.cost_at(0, 0), COST_NORMAL);
        assert_eq!(hover.ground_cost_at(0, 0), COST_NORMAL);
        assert_eq!(track.ground_cost_at(1, 0), COST_BLOCKED, "out of range");
    }

    #[test]
    fn test_canonical_ramp_is_not_blocked_by_cliff_like_rock_land() {
        use crate::map::resolved_terrain::RampDirection;
        use crate::sim::pathfinding::passability::LandType;
        let terrain = ResolvedTerrainGrid::from_cells(
            1,
            1,
            vec![ResolvedTerrainCell {
                is_cliff_like: true,
                land_type: LandType::Rock.as_index(),
                canonical_ramp: Some(RampDirection::North),
                ground_walk_blocked: false,
                ..make_resolved_cell(0, 0)
            }],
        );
        let track = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Track);

        assert_eq!(track.cost_at(0, 0), COST_NORMAL);
    }

    /// A terrain object with `TemperateOccupationBits` short of the full
    /// sub-cell mask blocks vehicles but not infantry. 56 of 60 stock temperate
    /// terrain types are `=4`, i.e. a single sub-cell bit.
    fn tree_cell(rx: u16, ry: u16, bits: u8) -> ResolvedTerrainCell {
        ResolvedTerrainCell {
            terrain_object_occupation: Some(bits),
            terrain_object_blocks: bits != 0,
            // resolved terrain folds the terrain object into ground_walk_blocked;
            // base_ground_walk_blocked is the value without it.
            ground_walk_blocked: bits != 0,
            base_ground_walk_blocked: false,
            ..make_resolved_cell(rx, ry)
        }
    }

    #[test]
    fn partial_terrain_occupation_blocks_vehicles_but_not_infantry() {
        use crate::sim::pathfinding::PathGrid;
        let terrain = ResolvedTerrainGrid::from_cells(
            2,
            1,
            vec![tree_cell(0, 0, 4), make_resolved_cell(1, 0)],
        );
        let foot = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Foot);
        let track = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Track);
        assert_ne!(foot.cost_at(0, 0), COST_BLOCKED);
        assert_eq!(track.cost_at(0, 0), COST_BLOCKED);

        let grid = PathGrid::from_resolved_terrain(&terrain);
        assert!(grid.is_walkable_for_infantry(0, 0));
        assert!(!grid.is_walkable(0, 0));
        assert_eq!(grid.terrain_object_cell_bits_at(0, 0), 0x10);
    }

    #[test]
    fn full_terrain_occupation_blocks_infantry_too() {
        use crate::sim::pathfinding::PathGrid;
        let terrain = ResolvedTerrainGrid::from_cells(1, 1, vec![tree_cell(0, 0, 7)]);
        let foot = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Foot);
        assert_eq!(foot.cost_at(0, 0), COST_BLOCKED);

        let grid = PathGrid::from_resolved_terrain(&terrain);
        assert!(!grid.is_walkable_for_infantry(0, 0));
        assert!(!grid.is_walkable(0, 0));
        assert_eq!(grid.terrain_object_cell_bits_at(0, 0), 0x1C);
    }

    #[test]
    fn terrain_occupation_five_and_six_leave_one_subcell_for_infantry() {
        use crate::sim::pathfinding::PathGrid;
        for bits in [5u8, 6u8] {
            let terrain = ResolvedTerrainGrid::from_cells(1, 1, vec![tree_cell(0, 0, bits)]);
            let foot = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Foot);
            assert_ne!(foot.cost_at(0, 0), COST_BLOCKED, "bits={bits}");
            let grid = PathGrid::from_resolved_terrain(&terrain);
            assert!(grid.is_walkable_for_infantry(0, 0), "bits={bits}");
            assert!(!grid.is_walkable(0, 0), "bits={bits}");
        }
    }

    #[test]
    fn terrain_relaxation_never_reopens_a_cliff_cell() {
        use crate::sim::pathfinding::PathGrid;
        let cliff_with_tree = ResolvedTerrainCell {
            is_cliff_like: true,
            base_ground_walk_blocked: true,
            ..tree_cell(0, 0, 4)
        };
        let terrain = ResolvedTerrainGrid::from_cells(1, 1, vec![cliff_with_tree]);
        let grid = PathGrid::from_resolved_terrain(&terrain);
        assert!(!grid.is_walkable_for_infantry(0, 0));
    }

    fn make_resolved_cell(rx: u16, ry: u16) -> ResolvedTerrainCell {
        ResolvedTerrainCell {
            ..crate::map::resolved_terrain::test_flat_cell(rx, ry)
        }
    }
}
