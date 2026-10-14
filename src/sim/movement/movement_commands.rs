//! Destination acceptance shared by the class setters: admission, the
//! locomotor Move_To tails and the scheduling adapter they install.
//!
//! No order searches a route here. The class setters (`Simulation::
//! set_unit_destination`, `Simulation::assign_infantry_destination`) accept a
//! destination; the first ordinary Process owns the route search.
//!
//! ## Dependency rules
//! - Internal to sim/movement — called via re-exports in mod.rs.

use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::rules::locomotor_type::LocomotorKind;
use crate::sim::components::MovementTarget;
use crate::sim::entity_store::EntityStore;
use crate::sim::game_entity::GameEntity;
use crate::util::fixed_math::SimFixed;

/// Check if an entity can accept a new movement destination.
///
/// Prevents destination changes during special states: dying, deploying,
/// undeploying and falling.
pub(crate) fn can_accept_destination(entity: &GameEntity) -> bool {
    if entity.dying {
        return false;
    }
    if entity.building_up() || entity.building_down() {
        return false;
    }
    true
}

/// A committed head comes from the active locomotor after world callbacks;
/// the physical path and raw occupation metadata cannot reconstruct it.
pub(super) fn committed_movement_head(entity: &GameEntity) -> Option<(u16, u16)> {
    let head = if entity.locomotor.as_ref()?.kind == LocomotorKind::Walk {
        entity.locomotor.as_ref()?.step_head()?
    } else {
        super::track_head::committed_track_head(entity)?
    };
    Some(((head.x / 256) as u16, (head.y / 256) as u16))
}

/// Keep only physical movement already accepted by the locomotor. The caller
/// owns destination/queue writes: explicit Attack's null setter does not clear
/// NavQueue, whereas the Unit setter's null arm (`0x007423BE`) does.
pub(crate) fn retain_committed_movement(e: &mut GameEntity) {
    // Walk75BD29 samples current XYZ against its paid head; a Drive/Ship
    // curve finishes at its head after Foot+5E0 retires. Keep the scheduling
    // adapter while a head remains.
    if committed_movement_head(e).is_none() {
        e.movement_target = None;
    }
}

/// Read the movement goal for diagnostics without retaining another copy.
/// Drive/Ship display Foot's requested Cell, including a Move_To refusal
/// that leaves a different locomotor destination (track_destination native
/// warp controls). Object requests use the captured locomotor coordinate;
/// after Stop the paid head remains. Other families display Foot's NavCom
/// when it is a Cell (for a Jumpjet, the cell its `Move_To` placed it in),
/// and nothing for an object. This projection is for presentation; it makes
/// no simulation decision.
pub(crate) fn movement_goal_cell(entity: &GameEntity) -> Option<(u16, u16)> {
    entity.movement_target.as_ref()?;
    if entity.locomotor.as_ref().is_some_and(|loco| {
        matches!(
            loco.active_kind(),
            LocomotorKind::Drive | LocomotorKind::Ship
        )
    }) {
        if let Some(crate::sim::components::NavTargetRef::Cell { rx, ry }) =
            entity.navigation.nav_com
        {
            return Some((rx, ry));
        }
        return super::track_path::track_destination(entity)
            .or_else(|| super::track_head::committed_track_head(entity))
            .map(|coord| ((coord.x / 256) as u16, (coord.y / 256) as u16));
    }
    match entity.navigation.nav_com {
        Some(crate::sim::components::NavTargetRef::Cell { rx, ry }) => Some((rx, ry)),
        _ => None,
    }
}

/// The caller's native frame and configured Foot blocked timer duration.
/// Passing this context keeps accepted destination timers anchored even when
/// Scatter and the ordinary object turn both process one actor in a frame.
#[derive(Debug, Clone, Copy)]
pub struct DestinationTiming {
    pub binary_frame: u32,
    pub blockage_path_delay_ticks: i32,
}

impl DestinationTiming {
    pub const fn new(binary_frame: u32, blockage_path_delay_ticks: i32) -> Self {
        Self {
            binary_frame,
            blockage_path_delay_ticks,
        }
    }

    pub fn from_rules(binary_frame: u32, rules: Option<&crate::rules::ruleset::RuleSet>) -> Self {
        Self::new(
            binary_frame,
            rules.map_or(60, |r| r.general.blockage_path_delay_ticks),
        )
    }

    /// Set_Destination_Internal 0x004D94B0 tail 0x4D96C2..0x4D9707: +6B7 = 0,
    /// +668 = (frame, Rules+1768), +640 = (frame, 0) for every accepted
    /// setter. The setter never writes +64C; the no-head Process FindPath
    /// success continuation 0x75B2E2 owns that reset.
    pub(crate) fn accept(self, entity: &mut crate::sim::game_entity::GameEntity) {
        let path = &mut entity.navigation.path_runtime;
        path.start_movement(self.binary_frame, 0);
        path.start_blocked(self.binary_frame, self.blockage_path_delay_ticks);
        path.path_blocked = false;
    }
}

/// Ordinary Unit741970 -> Foot4D94B0 -> Drive4AFD40/Ship69F450 accepts
/// before any FindPath, preserves power and a paid head, and clears only the
/// live path word (741E88) unless Enter lacks a radio contact
/// (tools/spatial_oracle/track_path_continuation, Enter rows); the setter's
/// flag-gated NavQueue Clear (7422E8..7422F4) empties the waypoint queue
/// (track_destination NavQueue rows). Neither map availability nor an A* result admits the
/// destination: the first no-queue Process owns the request (Drive 4B28A3,
/// Ship 6A1EF3). Native evidence: track_destination unit rows and
/// track_order_path. The class preprocessing (the Teleporter arm, +1F8 and
/// the Foot+0x6AC skip) belongs to `Simulation::set_unit_destination`; this
/// tail's remaining caller, the deferred Restore setter
/// (`Simulation::complete_pending_order`), has none of it, nor the
/// radio-building and deploy-byte arms.
pub(crate) fn prepare_track_destination(
    entity: &mut GameEntity,
    target: (u16, u16),
    object_destination: Option<(
        crate::sim::components::NavTargetRef,
        crate::sim::components::DriveCoord,
    )>,
    speed: SimFixed,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
    timing: crate::sim::movement::DestinationTiming,
) {
    clear_destination_path_head(entity);
    entity.navigation.nav_queue.clear();
    if let Some((reference, coord)) = object_destination {
        super::navcom::set_destination_internal_coord(
            entity,
            reference,
            coord,
            resolved_terrain,
            timing.binary_frame,
        );
    } else {
        super::navcom::set_destination_internal_cell(
            entity,
            target,
            resolved_terrain,
            timing.binary_frame,
        );
    }
    timing.accept(entity);
    prepare_destination_execution(entity, speed);
}

/// Accepted destination -> Walk75ACB0, without searching or advancing
/// Process. The class caller owns preceding admission and its path-head
/// write; the captured coordinate comes from the shared target +4C owner.
/// No PathGrid is needed until the next ordinary Process (or the immediate
/// Process specifically required by NULL-source Scatter51D478).
pub(crate) fn prepare_walk_destination(
    entities: &mut EntityStore,
    entity_id: u64,
    destination: (
        crate::sim::components::NavTargetRef,
        crate::sim::components::DriveCoord,
    ),
    speed: SimFixed,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
    timing: crate::sim::movement::DestinationTiming,
) -> bool {
    let Some(entity) = entities.get_mut(entity_id) else {
        return false;
    };
    if !entity
        .locomotor
        .as_ref()
        .is_some_and(|l| l.kind == LocomotorKind::Walk)
    {
        return false;
    }
    let (reference, coord) = destination;
    super::navcom::set_destination_internal_coord(
        entity,
        reference,
        coord,
        resolved_terrain,
        timing.binary_frame,
    );
    timing.accept(entity);
    prepare_destination_execution(entity, speed);
    true
}

/// The setters' one-word PathHead clear, which Enter (current or queued)
/// without a radio contact skips: Infantry 51AC25..51AD17, Unit
/// 741C4F..741C78 -> 741E88 (both the NULL and the non-NULL destination).
/// NavQueue, suffix and reference survive.
pub(crate) fn clear_destination_path_head(entity: &mut GameEntity) {
    if (entity.mission.effective().raw() != 7 && entity.mission.queued().raw() != 7)
        || !entity.radio_contacts.is_empty()
    {
        entity.navigation.path_replay.clear_live_head();
    }
}

/// Install the empty-route scheduling adapter for a Drive/Ship Foot whose
/// locomotor destination was set outside the ordinary setter (the outer
/// Process NavCom reissue). Process_Movement follows +34 without it; the
/// adapter's readers (the bunker install) see the Foot as moving.
pub(super) fn schedule_track_process(entity: &mut GameEntity, speed: SimFixed) {
    prepare_destination_execution(entity, speed);
}

/// A track terminal that found the Foot+5E0 queue empty (Drive 0x4B280D
/// compares it with -1) short of the retained locomotor destination (+34):
/// the adapter stays while +34 does, as the next Process_Movement reaches the
/// no-queue arm (Drive 0x4B281C / Ship 0x6A1E75), in the same Process after
/// the track end (`track_continuation`) or on the next visit. With no
/// destination left it retires.
pub(super) fn spend_track_route(entity: &mut GameEntity) {
    let destination = entity.locomotor.as_ref().and_then(|loco| {
        loco.track_destination(super::track_process::TrackFamily::from_kind(
            loco.active_kind(),
        )?)
    });
    if destination.is_none() {
        entity.movement_target = None;
    }
}

/// MovementTarget is only the scheduling adapter. Retain a paid head, never
/// turn or search at order time. The ordinary Process owns the first route
/// and subsequent head selection for Walk, Drive and Ship. Every family keeps
/// its goal in its locomotor destination and Foot's NavCom, not here.
pub(super) fn prepare_destination_execution(entity: &mut GameEntity, speed: SimFixed) {
    entity.movement_target = Some(MovementTarget { speed });
}
