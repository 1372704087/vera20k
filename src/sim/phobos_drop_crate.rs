//! Phobos `DropCrate=`: a techno that leaves a fixed crate on the cell it is
//! destroyed on.
//!
//! ```ini
//! DropCrate=          ; Nullable<Powerup>, unset   Ext/TechnoType/Body.cpp:1446
//! ```
//!
//! `TechnoExt::GetDropCrateIndex` (`Ext/Techno/Body.Internal.cpp:275-299`)
//! answers the type's `[Powerups]` slot. The two destruction hooks,
//! `TechnoClass::RegisterDestruction` (`0x702E4E`) and the Chronoshift path
//! (`0x70337D`), then call `TechnoExt::TryToCreateCrate` (`Ext/Techno/Hooks.cpp:667-703`):
//!
//! ```cpp
//! const int nSelectedPowerup = TechnoExt::GetDropCrateIndex(pVictim);
//! if (nSelectedPowerup >= 0)
//!     TechnoExt::TryToCreateCrate(pVictim->Location, static_cast<Powerup>(...));
//! ```
//!
//! `TryToCreateCrate` (`Ext/Techno/Body.cpp:579-656`) converts the location to
//! its cell and scans outward in rings of `currentRange = 0..10` — the upper
//! line `y - r` left to right, the lower line `y + r`, then the left and right
//! columns between them — calling `MapClass::PlacePowerupCrate` at each cell
//! until one accepts. VERA's [`crate::sim::crates::place_crate`] is that
//! 0x0056BEC0 callee, so the scan is the only new body here.

use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;

use super::world::{FrameEffects, Simulation};

/// `TechnoExt::TryToCreateCrate`'s ring limit (`Ext/Techno/Body.cpp:579`).
const DROP_CRATE_CELL_RANGE: i16 = 10;

impl Simulation {
    /// `TechnoExt::GetDropCrateIndex` plus `TryToCreateCrate`, run from the
    /// destruction transaction (`TechnoClass::RegisterDestruction`, hook
    /// `0x702E4E`, `Ext/Techno/Hooks.cpp:667-686`).
    ///
    /// RESIDUAL: the per-instance `TechnoExt::DropCrate` / `DropCrateType`
    /// override (`Ext/Techno/Body.Internal.cpp:285-295`) is only written by
    /// the Phobos trigger action `SetDropCrate=600`
    /// (`Ext/TAction/Body.cpp:690-720`), which VERA does not port yet. Trigger:
    /// a map that fires that action. Effect: the type key decides, an override
    /// is ignored. Frequency: mod-authored maps. Risk: a crate drops whose type
    /// the map meant to replace.
    pub(crate) fn drop_crate_on_destruction(
        &mut self,
        victim_id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        let Some(registry) = registry else {
            return;
        };
        let Some((type_ref, cell)) = self.substrate.entities.get(victim_id).map(|victim| {
            let location =
                crate::sim::movement::ground_pose::position_world_coord(&victim.position);
            (
                victim.type_ref(),
                (
                    crate::util::lepton::lepton_to_cell_packed(location.x),
                    crate::util::lepton::lepton_to_cell_packed(location.y),
                ),
            )
        }) else {
            return;
        };
        let Some(powerup) = self
            .interner
            .object_type(type_ref, rules)
            .and_then(|object| object.phobos_techno_extras.drop_crate)
        else {
            return;
        };
        try_to_create_crate(self, rules, registry, cell, powerup as u8, effects);
    }
}

/// `TechnoExt::TryToCreateCrate`'s outward scan
/// (`Ext/Techno/Body.cpp:585-650`): range `0..10`, each ring's upper line,
/// lower line, left column then right column, first accepted cell wins.
fn try_to_create_crate(
    sim: &mut Simulation,
    rules: &RuleSet,
    registry: &OverlayTypeRegistry,
    center: (i16, i16),
    powerup: u8,
    effects: FrameEffects<'_>,
) {
    for range in 0..DROP_CRATE_CELL_RANGE {
        let mut place = |x: i16, y: i16| -> bool {
            match crate::sim::crates::place_crate(
                sim,
                rules,
                registry,
                (x, y),
                Some(powerup),
                effects,
            ) {
                Ok(placed) => placed,
                Err(error) => {
                    log::error!("DropCrate placement failed: {error}");
                    false
                }
            }
        };
        for i in -range..=range {
            if place(center.0.wrapping_add(i), center.1.wrapping_sub(range)) {
                return;
            }
        }
        for i in -range..=range {
            if place(center.0.wrapping_add(i), center.1.wrapping_add(range)) {
                return;
            }
        }
        for j in (-range + 1)..range {
            if place(center.0.wrapping_sub(range), center.1.wrapping_add(j)) {
                return;
            }
        }
        for j in (-range + 1)..range {
            if place(center.0.wrapping_add(range), center.1.wrapping_add(j)) {
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "phobos_drop_crate_tests.rs"]
mod tests;
