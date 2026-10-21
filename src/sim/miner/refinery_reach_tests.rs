//! The refinery scanner's zone gate (`FUN_004DEE80`, `0x004DEF29..0x004DEFDD`):
//! `MapClass::Can_Reach_Zone` from the cell of the miner's `vt+0x4C`
//! coordinate to the cell of the refinery's GetCoords.

use super::ResourceType;
use super::miner_system::find_docking_bay;
use super::pad_nudge_tests::{scene, spawn};
use crate::sim::tiberium::test_support::place_tiberium_on_map;

/// A War Miner standing in an ore field west of a water channel over the
/// columns `channel`, a refinery at NW (22, 10) and a farther one on the
/// miner's shore. Answers the refinery the scanner picks and the two
/// candidates.
fn pick(channel: std::ops::RangeInclusive<u16>) -> Option<(Option<u64>, u64, u64)> {
    let (mut sim, rules, _) = scene()?;
    crate::sim::arena_fixture::flood(
        &mut sim,
        &rules,
        40,
        (0..40).flat_map(|y| channel.clone().map(move |x| (x, y))),
    );
    let near = spawn(&mut sim, &rules, "NAREFN", "Russians", 22, 10);
    let far = spawn(&mut sim, &rules, "NAREFN", "Russians", 4, 24);
    let miner = spawn(&mut sim, &rules, "HARV", "Russians", 14, 12);
    sim.overlay_grid = Some(crate::sim::overlay_grid::OverlayGrid::new(40, 40));
    for y in 11..=13 {
        for x in 13..=15 {
            place_tiberium_on_map(&mut sim, (x, y), ResourceType::Ore, 5);
        }
    }
    assert!(sim.rebuild_dynamic_navigation(&rules));
    let found = find_docking_bay(
        &mut sim,
        &rules,
        miner,
        false,
        false,
        crate::sim::world::FrameEffects::default(),
    );
    Some((found, near, far))
}

/// Across water the nearer refinery's zone is unreachable and the scanner
/// takes the farther one. The miner's own Cell is ore, which keeps its zone.
#[test]
fn a_miner_skips_a_refinery_across_water() {
    let Some((found, _, far)) = pick(18..=20) else {
        return;
    };
    assert_eq!(found, Some(far));
}

/// The gate measures to the refinery's GetCoords Cell, its foundation centre
/// (22 + 1, 10 + 1), not its dock pad (25, 11) on the foundation's east
/// edge: with water just past the pad the centre is still on the miner's
/// shore, and the nearer refinery wins.
#[test]
fn the_zone_gate_reads_the_refinerys_centre_cell() {
    let Some((found, near, _)) = pick(26..=27) else {
        return;
    };
    assert_eq!(found, Some(near));
}
