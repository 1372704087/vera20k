//! `BuildingClass::AfterDestruction @ 0x00441F60`: the rubble a destroyed
//! `LeaveRubble=yes` building leaves on its foundation.
//!
//! Native callers run it right after the dying building's UnInit (`vt+0xF8`):
//! `BuildingClass::ReceiveDamage` (`0x004426A2`) when DestructionEffects
//! armed a running death timer, and `BuildingClass::Update` (`0x004400E5`)
//! when that timer expires. VERA UnInits every dying building at once (see
//! `crew_survival`), so it runs here from the fatal lifecycle's
//! AfterDeathEffects stage, after `uninit_with_context`.
//!
//! When the type's rubble art answers ([`crate::rules::building_rubble`],
//! `0x0045F160`), each foundation cell in `vt+0x108(0)` order is redrawn
//! (`CellClass::MarkForRedraw @ 0x00486E70`: tactical rect and radar cell),
//! takes overlay 0xEF with a null rubble type, runs `RecalcAttributes(-1)`
//! (`0x0047D2B0`), `AssignOrphanedCellZone` (`0x0056D460`) and the local
//! graph update (`0x00584550`), then the cell-target detach sweep
//! (`0x0070D4A0`). The origin cell (`vt+0xAC`, Location minus 0x80, the
//! north-west cell) then records the type for the overlay draws.
//!
//! Evidence: instruction reading of `0x00441F60..0x0044222D` and its callees
//! named above. No RNG draws, timer writes or object detach calls besides the
//! cell sweep.
//!
//! RESIDUALS:
//! - The `CrateBeneath=` tail (`0x004421A4..0x0044222D`), which runs whether
//!   or not the rubble gate passed: `MapClass::Place_Crate @ 0x0056BEC0` at
//!   the origin cell, money (`0`) for `CrateBeneathIsMoney=`, else a random
//!   powerup (`0x14`). Unported: VERA has no targeted crate placer yet.
//!   Trigger: destroying a retail `CrateBeneath=true` type (fourteen civilian
//!   buildings; CARUS07 and CAWASH17 take the random arm). Effect: no crate
//!   appears. Frequency: maps with those buildings. Risk: crate slots, the
//!   placer's Find_Nearby_Passable_Cell and timer draws on the Scenario
//!   stream.
//! - The screen rect AfterDestruction dirties for the origin cell
//!   (`0x00442085..0x0044219F`) is presentation; VERA's overlay upsert
//!   redraws it.

use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use std::collections::BTreeSet;

use crate::sim::intern::InternedId;

use super::{FrameEffects, Simulation, mark_wall_radar_dirty_cell};

impl Simulation {
    /// `BuildingClass::AfterDestruction @ 0x00441F60`'s rubble arm for a
    /// building of `type_id` whose north-west cell was `origin`.
    pub(crate) fn leave_building_rubble(
        &mut self,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        type_id: InternedId,
        origin: (u16, u16),
        effects: FrameEffects<'_>,
    ) {
        let Some(registry) = registry else {
            return;
        };
        let Some(object) = rules.object(self.interner.resolve(type_id)) else {
            return;
        };
        if rules.building_rubble_art(&object.id).is_none() {
            return;
        }
        let cells =
            crate::sim::crew_survival::foundation_cells(origin.0, origin.1, &object.foundation);
        // The building is already unmarked, so presence cannot change between
        // cells. Before the first navigation build there is nothing to publish
        // or repair ([`Simulation::publish_recalculated_cells`]).
        let navigation_installed =
            self.path_grid.is_some() || self.zone_grid.is_some() || !self.terrain_costs.is_empty();
        let blocked = if navigation_installed {
            super::navigation::structure_blocked_among(
                &self.substrate.entities,
                &self.interner,
                rules,
                &cells,
            )
        } else {
            BTreeSet::new()
        };
        for cell in cells {
            mark_wall_radar_dirty_cell(
                &mut self.radar_terrain_dirty_cells,
                &mut self.radar_terrain_dirty_generation,
                cell,
            );
            self.tactical_dirty_cells.push(cell);
            let (Some(grid), Some(terrain)) =
                (self.overlay_grid.as_mut(), self.resolved_terrain.as_mut())
            else {
                return;
            };
            if !grid.write_rubble_fields(terrain, registry, cell.0, cell.1) {
                continue;
            }
            grid.recalculate_runtime_cell(terrain, registry, cell);
            if navigation_installed {
                self.publish_cell_and_assign_orphaned_zone(rules, cell, &blocked);
            }
            self.stop_all_targeting_cell(cell.0, cell.1, Some(rules), Some(registry), effects);
        }
        if let Some(grid) = self.overlay_grid.as_mut() {
            grid.set_rubble_building_type(origin.0, origin.1, type_id);
        }
    }
}
