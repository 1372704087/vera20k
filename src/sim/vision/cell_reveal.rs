//! MapCell4A9CA0 and Tactical6D8700 edge-cache authority for ordinary reveals.
//! Counters and ground knowledge stay with OwnerVisibility. These derived cache
//! bytes update only when this cell's native leaf is visited; neighbor writes
//! deliberately leave them stale. Native constructor47BD3A initializes -2/-2.

use super::{FogState, GROUND_OPEN, GROUND_VISIBLE, InternedId, OwnerVisibility, ShroudKnowledge};
use crate::map::cell_index::NativeCellIdentity;
use crate::map::resolved_terrain::NativeCellQuery;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

#[cfg(test)]
#[path = "cell_reveal_tests.rs"]
pub(crate) mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(super) struct MapCellEdgeCache {
    initialized: bool, // Cell140 bit2
    fog_open: bool,    // Cell140 bit1
    frames: [i8; 2],   // Cell120/121
}
impl Default for MapCellEdgeCache {
    fn default() -> Self {
        Self {
            initialized: false,
            fog_open: false,
            frames: [-2, -2],
        }
    }
}

/// The canonical map Dummy has one retained visibility record per represented
/// player plane, shared by every missing coordinate. Its coordinate/height
/// remain with NativeCellQuery; a miss does not invent a rectangular real cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(super) struct DummyVisibility {
    knowledge: ShroudKnowledge,
    ground: u8,
    cache: MapCellEdgeCache,
}

#[derive(Clone, Copy)]
enum VisibilityCell {
    Real(usize),
    Dummy,
}

/// Synchronous MapCell receiver arguments. The coordinate is read after both
/// edge queries from the cached Cell pointer. A Dummy may have been restamped.
pub(crate) struct MapCellNotification {
    pub(crate) cell: (u16, u16),
    pub(crate) fresh: bool,
}

impl OwnerVisibility {
    fn visibility_cell(
        &self,
        cell: (u16, u16),
        cells: Option<&NativeCellQuery<'_>>,
    ) -> VisibilityCell {
        let coord = match cells {
            Some(cells) => match cells.lookup((cell.0 as i16, cell.1 as i16)) {
                NativeCellIdentity::Real(identity) => {
                    let source = &cells.terrain().cells()[identity];
                    (source.rx, source.ry)
                }
                NativeCellIdentity::Dummy => return VisibilityCell::Dummy,
            },
            None => cell,
        };
        self.index(coord.0, coord.1)
            .map_or(VisibilityCell::Dummy, VisibilityCell::Real)
    }

    fn map_cell_parts(&self, key: VisibilityCell) -> (u8, MapCellEdgeCache) {
        match key {
            VisibilityCell::Real(index) => {
                (self.ground_flags[index], self.map_cell_edge_cache[index])
            }
            VisibilityCell::Dummy => (self.dummy_visibility.ground, self.dummy_visibility.cache),
        }
    }

    fn map_cell_parts_mut(
        &mut self,
        key: VisibilityCell,
    ) -> (&mut ShroudKnowledge, &mut u8, &mut MapCellEdgeCache) {
        match key {
            VisibilityCell::Real(index) => (
                &mut self.shroud_knowledge[index],
                &mut self.ground_flags[index],
                &mut self.map_cell_edge_cache[index],
            ),
            VisibilityCell::Dummy => (
                &mut self.dummy_visibility.knowledge,
                &mut self.dummy_visibility.ground,
                &mut self.dummy_visibility.cache,
            ),
        }
    }

    pub(crate) fn hash_map_cell_edge_cache(&self, hasher: &mut impl Hasher) {
        let populated = self
            .map_cell_edge_cache
            .iter()
            .filter(|cache| cache.initialized)
            .count();
        if populated != 0 {
            b"native-map-cell-edge-cache-v1".hash(hasher);
            (populated as u64).hash(hasher);
            for (index, cache) in self.map_cell_edge_cache.iter().enumerate() {
                if cache.initialized {
                    (index as u64).hash(hasher);
                    cache.hash(hasher);
                }
            }
        }
        if self.dummy_visibility != DummyVisibility::default() {
            b"native-dummy-visibility-v1".hash(hasher);
            self.dummy_visibility.hash(hasher);
        }
    }

    #[cfg(test)]
    pub(crate) fn map_cell_edge_cache_for_test(&self, cell: (u16, u16)) -> ([i8; 2], u32) {
        let cache = self.map_cell_edge_cache[self.index(cell.0, cell.1).unwrap()];
        (
            cache.frames,
            u32::from(cache.initialized) * 2 | u32::from(cache.fog_open),
        )
    }
}

impl FogState {
    /// Original MapCell4A9CA0..4A9DC9, with ordinary final0/1 counter leaves.
    /// Emit only when4A9D94 calls CellChangeNotify; fresh is its separate
    /// new-knowledge argument, independently of cache-change admission.
    /// Executed cell/cache/channel/Dummy controls:
    /// tools/spatial_oracle/trigger_action_values.{py,json,meta.json}.
    /// FogOfWar486BF0's fogged-object tail is outside the stock-clear path.
    pub(crate) fn reveal_map_cell(
        &mut self,
        viewer: InternedId,
        cell: (u16, u16),
        final_pass: bool,
        native_cells: Option<&NativeCellQuery<'_>>,
    ) -> Option<MapCellNotification> {
        let vis = self
            .by_owner
            .entry(viewer)
            .or_insert_with(|| OwnerVisibility::new(self.width, self.height));
        //4A9CA9 is distinct from each6D8713 center lookup below.
        let key = vis.visibility_cell(cell, native_cells);
        let (ground, old) = vis.map_cell_parts(key);
        let fresh = !(old.initialized && ground & GROUND_VISIBLE != 0);
        let (state, ground, cache) = vis.map_cell_parts_mut(key);
        cache.initialized = true;
        if final_pass {
            state.leave();
        } else {
            state.reveal();
        }
        state.transient_visible = matches!(key, VisibilityCell::Real(_));
        if state.open {
            *ground |= GROUND_VISIBLE | GROUND_OPEN;
        } else {
            *ground &= !(GROUND_VISIBLE | GROUND_OPEN);
        }
        if let VisibilityCell::Real(index) = key {
            vis.publish_knowledge(index);
        }
        let ground = self.map_cell_edge_frame(viewer, cell, false, native_cells);
        let vis = self.by_owner.get_mut(&viewer).unwrap();
        let (_, flags, cache) = vis.map_cell_parts_mut(key);
        cache.frames[0] = ground;
        if ground == -1 {
            *flags |= GROUND_OPEN;
        }
        let fog = self.map_cell_edge_frame(viewer, cell, true, native_cells);
        let vis = self.by_owner.get_mut(&viewer).unwrap();
        let (_, _, cache) = vis.map_cell_parts_mut(key);
        cache.frames[1] = fog;
        if fog == -1 {
            cache.fog_open = true;
        }
        if !fresh && old.frames == [ground, fog] {
            return None;
        }
        let cell = match (key, native_cells) {
            (VisibilityCell::Dummy, Some(cells)) => {
                let coord = cells.coord(NativeCellIdentity::Dummy);
                (coord.0 as u16, coord.1 as u16)
            }
            _ => cell,
        };
        Some(MapCellNotification { cell, fresh })
    }

    /// Original6D8700..6D8ADA: center lookup/gates, eight signed WORD
    /// neighbor lookups and sign-extended7F4194 table byte. Missing neighbors
    /// read the canonical Dummy's retained flags and stamp its coordinate.
    pub(crate) fn map_cell_edge_frame(
        &self,
        viewer: InternedId,
        cell: (u16, u16),
        fog_channel: bool,
        native_cells: Option<&NativeCellQuery<'_>>,
    ) -> i8 {
        let Some(vis) = self.by_owner.get(&viewer) else {
            return -2;
        };
        let center = vis.visibility_cell(cell, native_cells);
        let (ground, cache) = vis.map_cell_parts(center);
        let (any, eligible) = if fog_channel {
            (cache.initialized || cache.fog_open, cache.initialized)
        } else {
            (
                ground & (GROUND_VISIBLE | GROUND_OPEN) != 0,
                ground & GROUND_VISIBLE != 0,
            )
        };
        let fallback = if any { -1 } else { -2 };
        if !eligible {
            return fallback;
        }
        let mut mask = 0u8;
        for (dx, dy, bit) in [
            (-1i16, -1i16, 0x40),
            (0, -1, 0x80),
            (1, -1, 0x01),
            (-1, 0, 0x20),
            (1, 0, 0x02),
            (-1, 1, 0x10),
            (0, 1, 0x08),
            (1, 1, 0x04),
        ] {
            let neighbor = (
                (cell.0 as i16).wrapping_add(dx) as u16,
                (cell.1 as i16).wrapping_add(dy) as u16,
            );
            let key = vis.visibility_cell(neighbor, native_cells);
            let (ground, cache) = vis.map_cell_parts(key);
            let set = if fog_channel {
                cache.initialized
            } else {
                ground & GROUND_VISIBLE != 0
            };
            if !set {
                mask |= bit;
            }
        }
        SHROUD_EDGE_LUT[usize::from(mask)] as i8
    }

    pub(super) fn is_dummy_ground_unshrouded(&self, viewer: InternedId) -> bool {
        self.by_owner
            .get(&viewer)
            .is_some_and(|vis| vis.dummy_visibility.ground & GROUND_VISIBLE != 0)
    }
}

/// Shroud edge frame lookup table.
///
/// Indexed by 8-bit neighbor bitmask (see `FogState::shroud_edge_mask_8bit`).
/// Values: 0xFF = no edge needed, 0xFE = fully surrounded, 0-46 = frame index.
#[rustfmt::skip]
pub const SHROUD_EDGE_LUT: [u8; 256] = [
    // 0x00–0x0F
    0xFF, 0x21, 0x02, 0x02, 0x22, 0x25, 0x02, 0x02,
    0x04, 0x1A, 0x06, 0x06, 0x04, 0x1A, 0x06, 0x06,
    // 0x10–0x1F
    0x23, 0x2D, 0x11, 0x11, 0x26, 0x29, 0x11, 0x11,
    0x04, 0x1A, 0x06, 0x06, 0x04, 0x1A, 0x06, 0x06,
    // 0x20–0x2F
    0x08, 0x15, 0x0A, 0x0A, 0x1B, 0x1F, 0x0A, 0x0A,
    0x0C, 0x17, 0x0E, 0x0E, 0x0C, 0x17, 0x0E, 0x0E,
    // 0x30–0x3F
    0x08, 0x15, 0x0A, 0x0A, 0x1B, 0x1F, 0x0A, 0x0A,
    0x0C, 0x17, 0x0E, 0x0E, 0x0C, 0x17, 0x0E, 0x0E,
    // 0x40–0x4F
    0x20, 0x24, 0x19, 0x19, 0x2C, 0x28, 0x19, 0x19,
    0x13, 0x1E, 0x14, 0x14, 0x13, 0x1E, 0x14, 0x14,
    // 0x50–0x5F
    0x27, 0x2B, 0x1D, 0x1D, 0x2A, 0x2E, 0x1D, 0x1D,
    0x13, 0x1E, 0x14, 0x14, 0x13, 0x1E, 0x14, 0x14,
    // 0x60–0x6F
    0x08, 0x15, 0x0A, 0x0A, 0x1B, 0x1F, 0x0A, 0x0A,
    0x0C, 0x17, 0x0E, 0x0E, 0x0C, 0x17, 0x0E, 0x0E,
    // 0x70–0x7F
    0x08, 0x15, 0x0A, 0x0A, 0x1B, 0x1F, 0x0A, 0x0A,
    0x0C, 0x17, 0x0E, 0x0E, 0x0C, 0x17, 0x0E, 0x0E,
    // 0x80–0x8F
    0x01, 0x01, 0x03, 0x03, 0x10, 0x10, 0x03, 0x03,
    0x05, 0x05, 0x07, 0x07, 0x05, 0x05, 0x07, 0x07,
    // 0x90–0x9F
    0x18, 0x18, 0x12, 0x12, 0x1C, 0x1C, 0x12, 0x12,
    0x05, 0x05, 0x07, 0x07, 0x05, 0x05, 0x07, 0x07,
    // 0xA0–0xAF
    0x09, 0x09, 0x0B, 0x0B, 0x16, 0x16, 0x0B, 0x0B,
    0x0D, 0x0D, 0xFE, 0xFE, 0x0D, 0x0D, 0xFE, 0xFE,
    // 0xB0–0xBF
    0x09, 0x09, 0x0B, 0x0B, 0x16, 0x16, 0x0B, 0x0B,
    0x0D, 0x0D, 0xFE, 0xFE, 0x0D, 0x0D, 0xFE, 0xFE,
    // 0xC0–0xCF
    0x01, 0x01, 0x03, 0x03, 0x10, 0x10, 0x03, 0x03,
    0x05, 0x05, 0x07, 0x07, 0x05, 0x05, 0x07, 0x07,
    // 0xD0–0xDF
    0x18, 0x18, 0x12, 0x12, 0x1C, 0x1C, 0x12, 0x12,
    0x05, 0x05, 0x07, 0x07, 0x05, 0x05, 0x07, 0x07,
    // 0xE0–0xEF
    0x09, 0x09, 0x0B, 0x0B, 0x16, 0x16, 0x0B, 0x0B,
    0x0D, 0x0D, 0xFE, 0xFE, 0x0D, 0x0D, 0xFE, 0xFE,
    // 0xF0–0xFF
    0x09, 0x09, 0x0B, 0x0B, 0x16, 0x16, 0x0B, 0x0B,
    0x0D, 0x0D, 0xFE, 0xFE, 0x0D, 0x0D, 0xFE, 0xFE,
];
