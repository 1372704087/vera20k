//! LAT auto-tiling fixup over the generated grid (RNG-free).
//!
//! Ported from the LAT half of `CellClass::ApplyLAT_and_SlopeFixup`. Four ground
//! groups run in fixed order — **Rough → Sand → Green → Pave** — on every cell.
//! A cell whose tile is a group's base *or* an existing LAT variant gets a 4-bit
//! mask over its four **cardinal** neighbours (bit 0 = N, 1 = E, 2 = S, 3 = W,
//! from `CELL_DELTAS[0, 2, 4, 6]`); a neighbour sets its bit unless it is
//! in the same group (base or LAT range) or in one of the group's hardcoded
//! exemption ranges. Mask 0 → the base tile; otherwise `lat_base + mask` (a
//! variant in `1..=15`). Off-band neighbours read as the map-edge sentinel
//! (tile 0), so a base cell at the diamond edge always gets its edge variant.
//!
//! Group membership is closed under this rewrite (a base tile becomes a LAT
//! variant that is still inside the same base∪LAT range), so the four passes are
//! order-independent between cells — the in-place walk matches the original.
//!
//! Each cell then runs the slope half from its raw `GridCell.slope` id. RMG has
//! populated all slope ids before these recalc passes, so one live-grid walk is
//! sufficient for each scheduled recalc.

use crate::map::lat::{SlopeFixupConfig, slope_fixed_tile};
use crate::map::rmg::grid::RmgGrid;
use crate::map::rmg::tiles::TileIds;

/// LAT variant range length past the base: `[lat_base, lat_base + 0xF]`.
const LAT_LEN: i32 = 0xF;
/// The four cardinal directions as `CELL_DELTAS` indices (N, E, S, W).
/// Bit `i` of the mask comes from `CARDINAL_DIRS[i]`.
const CARDINAL_DIRS: [usize; 4] = [0, 2, 4, 6];
/// Off-band / off-map neighbour tile (`Get_CellClass(off_map)` → tile 0).
const EDGE_SENTINEL: i32 = 0;

/// Green exemption span past the base: shore pieces `+0x29` (42 tiles),
/// water bridge `+1` (2 tiles).
const SHORE_LEN: i32 = 0x29;
const WATER_BRIDGE_LEN: i32 = 1;
/// Pave exemption spans: misc-pave `+0xD` (14), medians `+0xD` (14),
/// paved roads `+0x14` (21).
const MISC_PAVE_LEN: i32 = 0xD;
const MEDIANS_LEN: i32 = 0xD;
const PAVED_ROADS_LEN: i32 = 0x14;

/// An inclusive exemption range `[lo, hi]`, disabled (never matches a real
/// tile) when its base is `-1`.
#[derive(Clone, Copy)]
struct Exempt {
    lo: i32,
    hi: i32,
}

impl Exempt {
    fn new(base: i32, len: i32) -> Self {
        if base == -1 {
            Self { lo: -1, hi: -1 }
        } else {
            Self {
                lo: base,
                hi: base + len,
            }
        }
    }

    fn contains(&self, tile: i32) -> bool {
        self.lo != -1 && self.lo <= tile && tile <= self.hi
    }
}

/// Apply the four-group LAT fixup to every in-band grid cell in place.
pub fn run(grid: &mut RmgGrid, ids: &TileIds) {
    for (x, y) in grid.native_cells().collect::<Vec<_>>() {
        apply_cell(grid, ids, x, y);
    }
}

/// Run all four group passes on one cell, in the original's fixed order.
fn apply_cell(grid: &mut RmgGrid, ids: &TileIds, x: i32, y: i32) {
    // Rough — unguarded: it runs even when its LAT base is undefined (in which
    // case the match test only accepts the bare rough base tile).
    lat_group(grid, x, y, ids.rough, ids.rough_lat, &[]);

    // Sand — guarded, no exemptions.
    if ids.sand_lat != -1 {
        lat_group(grid, x, y, ids.sand, ids.sand_lat, &[]);
    }

    // Green — guarded; exempts shore pieces and water-bridge connectors.
    if ids.green_lat != -1 {
        let exemptions = [
            Exempt::new(ids.shore, SHORE_LEN),
            Exempt::new(ids.water_bridge, WATER_BRIDGE_LEN),
        ];
        lat_group(grid, x, y, ids.green, ids.green_lat, &exemptions);
    }

    // Pave — guarded; exempts misc-pave, medians and paved roads.
    if ids.pave_lat != -1 {
        let exemptions = [
            Exempt::new(ids.misc_pave, MISC_PAVE_LEN),
            Exempt::new(ids.medians, MEDIANS_LEN),
            Exempt::new(ids.paved_roads, PAVED_ROADS_LEN),
        ];
        lat_group(grid, x, y, ids.pave, ids.pave_lat, &exemptions);
    }

    slope_fixup(grid, ids, x, y);
}

fn slope_fixup(grid: &mut RmgGrid, ids: &TileIds, x: i32, y: i32) {
    let Some(cell) = grid.get(x, y).copied() else {
        return;
    };
    let cardinal_slopes = CARDINAL_DIRS.map(|dir| {
        let (nx, ny) = RmgGrid::step(x, y, dir);
        grid.get(nx, ny).map_or(0, |neighbor| neighbor.slope)
    });
    let tile = slope_fixed_tile(
        cell.tile,
        cell.slope,
        cardinal_slopes,
        SlopeFixupConfig {
            ramp_base: ids.ramp_base,
            ramp_smooth: ids.ramp_smooth,
        },
    );
    if let Some(cell) = grid.get_mut(x, y) {
        cell.tile = tile;
    }
}

/// Whether `tile` belongs to a group (its base tile or its LAT variant range).
fn in_group(tile: i32, base: i32, lat_base: i32) -> bool {
    tile == base || (lat_base != -1 && lat_base <= tile && tile <= lat_base + LAT_LEN)
}

/// One group pass on one cell: if the cell is in the group, rebuild its tile
/// from the cardinal-neighbour mask.
fn lat_group(grid: &mut RmgGrid, x: i32, y: i32, base: i32, lat_base: i32, exemptions: &[Exempt]) {
    let tile = grid.get(x, y).map_or(EDGE_SENTINEL, |cell| cell.tile);
    if !in_group(tile, base, lat_base) {
        return;
    }

    let mut mask = 0u32;
    for (bit, &dir) in CARDINAL_DIRS.iter().enumerate() {
        let (nx, ny) = RmgGrid::step(x, y, dir);
        let neighbor = grid.get(nx, ny).map_or(EDGE_SENTINEL, |cell| cell.tile);
        // The neighbour sets the bit unless it reads as the same ground — in
        // this group, or inside an exemption range.
        let same =
            in_group(neighbor, base, lat_base) || exemptions.iter().any(|r| r.contains(neighbor));
        if !same {
            mask |= 1 << bit;
        }
    }

    let new_tile = if mask == 0 {
        base
    } else {
        lat_base + mask as i32
    };
    if let Some(cell) = grid.get_mut(x, y) {
        cell.tile = new_tile;
    }
}
