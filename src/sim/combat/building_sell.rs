//! Phobos sell/undeploy on impact (`Ext/WarheadType/Detonate.cpp:213-214,
//! 266-352`), the last of `DetonateOnOneUnit`'s house-changing arms: after
//! `RemoveMindControl`, `ChangeOwner` and `ApplyConvert`, a hit building is
//! sold or packed up.
//!
//! `ApplyBuildingUndeploy` reads:
//!
//! 1. Only a `BuildingClass` target proceeds.
//! 2. `BuildingSell=` outranks `BuildingUndeploy=` and returns either way: a
//!    building that `CanBeSold()` and is not strange sells
//!    (`:270-279`), unless `BuildingSell.IgnoreUnsellable=` overrides both
//!    gates.
//! 3. `BuildingUndeploy=` refuses a `Construction` or `Selling` mission and a
//!    type with no `UndeploysInto=`, or a `ConstructionYard` while the game
//!    mode forbids MCV redeploy, then sells through the undeploy order
//!    (`:284-304`).
//!
//! ## Dependency rules
//! - Part of sim/ — depends on rules/ and sim/ only; never audio/ or ui/.

use crate::map::entities::EntityCategory;
use crate::rules::ruleset::RuleSet;
use crate::rules::warhead_type::WarheadType;
use crate::sim::mission::MissionType;
use crate::sim::production::SellOrder;
use crate::sim::world::Simulation;
use crate::sim::world::frame_effects::FrameEffects;

/// Sells or undeploys `target_id` when the warhead asks for it.
pub(crate) fn apply(
    world: &mut Simulation,
    rules: &RuleSet,
    registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
    warhead: &WarheadType,
    target_id: u64,
    effects: FrameEffects<'_>,
) {
    let extras = &warhead.phobos_extras;
    if !(extras.building_sell || extras.building_undeploy) {
        return;
    }
    let Some(target) = world.substrate.entities.get(target_id) else {
        return;
    };
    // `abstract_cast<BuildingClass*, true>(pTarget)` (`:268`).
    if target.category() != EntityCategory::Structure {
        return;
    }
    let type_ref = target.type_ref();
    let strange = world
        .interner
        .object_type(type_ref, rules)
        .is_some_and(|object| object.is_1x1_with_undeploy());
    if extras.building_sell {
        // `if ((pBuilding->CanBeSold() && !pBuilding->IsStrange()) ||
        // this->BuildingSell_IgnoreUnsellable)` (`:271-273`).
        let sellable =
            crate::sim::production::can_sell_building(world, rules, target_id) && !strange;
        if extras.building_sell_ignore_unsellable || sellable {
            // `pBuilding->SetArchiveTarget(nullptr)` then `Sell(1)` (`:275-277`).
            clear_archive_target(world, target_id);
            crate::sim::production::sell_back(
                world,
                rules,
                target_id,
                SellOrder::Player,
                registry,
                effects,
            );
        }
        // `return;` (`:279`): a sell arm never undeploys.
        return;
    }
    if !extras.building_undeploy {
        return;
    }
    // `// CanBeSold() will also check this` (`:282-287`).
    let mission = target.mission.effective().known();
    if matches!(
        mission,
        Some(MissionType::Construction | MissionType::Selling)
    ) {
        return;
    }
    // `!pType->UndeploysInto || (pType->ConstructionYard && !MCVRedeploy)`
    // (`:291-292`) is [`Simulation::can_undeploy_building_runtime`].
    if !world.can_undeploy_building_runtime(target_id, rules) {
        return;
    }
    // `pBuilding->SetArchiveTarget(pCell)` then `Sell(1)` (`:351-353`): the
    // pack-up runs the undeploy order and produces the `UndeploysInto=` unit.
    world.undeploy_building(target_id, rules, registry, effects);
}

/// `pBuilding->SetArchiveTarget(nullptr)` (`:275`), so the pack-up does not
/// drive the resulting unit anywhere.
fn clear_archive_target(world: &mut Simulation, target_id: u64) {
    if let Some(target) = world.substrate.entities.get_mut(target_id) {
        target.set_archive_target(None);
    }
}
