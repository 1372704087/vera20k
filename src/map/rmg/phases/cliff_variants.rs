//! Registered cliff-tile variation after the cliff materialization pass.
//!
//! Original gamemd.exe: selector `0x005A1350..0x005A1775`, its pointer/map
//! tables `0x005A1778..0x005A17ED`, and restamp pass
//! `0x005A17F0..0x005A19D3`. Native comparison:
//! `tools/rmg_oracle/cliff_materialization.py`, consumed by
//! `cliff_native_tests`.
//!
//! These are different CliffSet tile identities, not TMP suffix variants.
//! The existing MapGen RNG and unconditional tile stamper remain authoritative.

use crate::map::rmg::grid::{GridCell, RmgGrid};
use crate::map::rmg::rng::RmgRng;
use crate::map::rmg::scratch::RmgScratch;

use super::CellRef;
use super::carve::stamp_iso_block;
use super::shore::TileBlocks;

/// Original `0x005A17F0`: visit current cells in native order, then vary a
/// repeated neighboring cliff template south before east.
pub(in crate::map::rmg) fn run(
    grid: &mut RmgGrid,
    scratch: &mut RmgScratch,
    blocks: &dyn TileBlocks,
    rng: &mut RmgRng,
    cliff_base: i32,
) {
    for (x, y) in grid.native_cells() {
        let tile = grid.cell_native(x, y).tile;
        // Original 5A1819..5A182F is a signed 40-tile window, without a
        // separate absent-base guard. The theater owner supplies its base.
        if tile < cliff_base || tile >= cliff_base.wrapping_add(40) {
            continue;
        }

        // Original 5A1835..5A1844 obtains BOTH pointers before either branch.
        // A border pointer retains the shared dummy, not this lookup's coord:
        // the east lookup may overwrite the coordinate the south pointer sees.
        let neighbors = [4, 2].map(|direction| {
            let (nx, ny) = RmgGrid::step(x, y, direction);
            let (nx, ny) = (i32::from(nx as i16), i32::from(ny as i16));
            let held = CellRef::at(grid, nx, ny);
            grid.cell_native(nx, ny);
            held
        });

        for neighbor in neighbors {
            // The south stamp can change the current cell or held east cell.
            // Read the live owners again for the second branch (5A1906).
            let current = *grid.cell_native(x, y);
            let next = held_cell(grid, neighbor);
            if current.tile != next.tile || current.sub_tile <= next.sub_tile {
                continue;
            }

            let replacement = select_variant(current.tile, cliff_base, rng);
            let old_tile = grid.cell_native(x, y).tile;
            if replacement == old_tile {
                continue;
            }

            // Original 5A1886..5A18C7 / 5A193B..5A197A recovers the
            // NEIGHBOR's anchor using the CURRENT OLD tile's width. It does
            // not use the replacement's dimensions or the current subtile.
            let width = blocks
                .block(old_tile)
                .expect("cliff restamp requires the current tile's TMP block")
                .width;
            assert!(width > 0, "cliff restamp requires a positive TMP width");
            let sub_tile = i32::from(held_cell(grid, neighbor).sub_tile);
            let (nx, ny) = match neighbor {
                CellRef::Cell(nx, ny) => (nx, ny),
                CellRef::Border => {
                    let (nx, ny) = grid.border_coord();
                    (nx as i16, ny as i16)
                }
            };
            let origin = (
                i32::from(nx.wrapping_sub((sub_tile % width) as i16)),
                i32::from(ny.wrapping_sub((sub_tile / width) as i16)),
            );
            let region = scratch.get(origin.0, origin.1).region;

            // Original 5A1901 / 5A19B2 passes level=-1. Canonical 5A6C10
            // treats that as KEEP HEIGHT, while copying tile/subtile/slope
            // and scratch region. Some(0xFF) would incorrectly change height.
            stamp_iso_block(grid, scratch, blocks, replacement, origin, region, None);
        }
    }
}

/// Read a held pointer without a fresh lookup that would restamp dummy coords.
fn held_cell(grid: &mut RmgGrid, held: CellRef) -> GridCell {
    match held {
        CellRef::Cell(x, y) => *grid.cell_native(i32::from(x), i32::from(y)),
        CellRef::Border => *grid.border_cell_mut(),
    }
}

/// Original `0x005A1350`: choose a different registered sibling, or retain an
/// offset outside its seven groups without drawing randomness.
fn select_variant(tile: i32, cliff_base: i32, rng: &mut RmgRng) -> i32 {
    let offset = tile.wrapping_sub(cliff_base);
    // The first five triples use raw * K2 (+1) with K2=2*RANGE_K_BITS;
    // the last triple calls native 598030 directly. For these bounded u32,
    // span-two inputs, scaling the raw word by two is exact before the shared
    // K multiply, so the existing numeric/rejection owner serves both; the
    // native controls pin every group's values and RNG cursors.
    let selected = match offset {
        4 | 8 | 11 | 14 | 22 | 34 => offset.wrapping_add(rng.uniform(1, 2)),
        5 | 9 | 12 | 15 | 23 | 35 => offset.wrapping_sub(1).wrapping_add(2 * rng.uniform(0, 1)),
        6 | 10 | 13 | 16 | 24 | 36 => offset.wrapping_sub(2).wrapping_add(rng.uniform(0, 1)),
        28 => 29,
        29 => 28,
        _ => return tile,
    };
    cliff_base.wrapping_add(selected)
}

#[cfg(test)]
mod tests {
    #[test]
    fn registered_variant_selection_matches_original_values_and_cursors() {
        super::super::cliff_native_tests::compare_variants(super::select_variant);
    }
}
