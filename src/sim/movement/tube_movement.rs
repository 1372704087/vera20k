//! Native TubeClass movement for explicit map tubes.
//!
//! gamemd: `UnitClass::TubeMovement` 0x007359F0, which reads the cell's tube
//! through `CellClass::GetTubeAtCell` 0x00484F20 and, on exit, drives
//! `FacingClass::UpdateFacing` from the record's +0x2C direction field.
//!
//! Automatic tunnel/low-bridge records have a zero-length path. They are
//! predicate and zone metadata only: the retail producer divides by path
//! length, so a zero-step shell must never become active movement state.

use super::infantry_entry::InfantryEntryArgs;
use super::locomotor::LocomotorState;
use super::track_process::TrackFamily;
use super::{MovementTickStats, PerCellReason, walk_head};
use crate::map::entities::EntityCategory;
use crate::map::resolved_terrain::{NativeCellQuery, ResolvedTerrainGrid};
use crate::map::retail_trig::TrigTable;
use crate::map::tube_facts::{TubeFact, TubeId, TubeSource};
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::cell_kernel;
use crate::sim::components::{DriveCoord, NavTargetRef, Position};
use crate::sim::entity_store::EntityStore;
use crate::sim::game_entity::GameEntity;
use crate::sim::intern::StringInterner;
use crate::sim::movement::ScatterFlags;
use crate::sim::movement::ground_pose;
use crate::sim::movement::locomotor::MovementLayer;
use crate::sim::occupancy::{
    CellListInsertion, CellOccupationGrid, OccupancyGrid, RawCellOccupationGrid,
    VEHICLE_OCCUPATION_BIT, infantry_raw_occupation_mask,
};
use crate::sim::world::{FrameAdvanceError, FrameEffects, Simulation};
use crate::util::fixed_math::{SIM_ONE, SIM_ZERO};
use crate::util::native_x87::{NativeF32Bits, NativeF64Bits, X87Chop53, distance_3d_leptons};

/// FootClass-owned active TubeMovement payload.
///
/// `Some` also means the mover is absent from CellClass object lists and raw
/// occupation, but inside an exit: the Infantry exit lists and marks the
/// infantryman (`0x0051BA22`, `0x0051BA45`) and runs its self-Scatter before
/// it clears the tube (`0x0051BA8D`), as
/// [`Simulation::infantry_tube_exit`] does. Immutable path bytes remain in
/// the map-owned [`TubeFact`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LowBridgeTubeMovementState {
    tube_id: TubeId,
    cursor: u8,
    target: DriveCoord,
}

impl LowBridgeTubeMovementState {
    /// The tube (FootClass+0x684).
    pub fn tube_id(&self) -> TubeId {
        self.tube_id
    }

    /// The step along the tube's path.
    pub fn cursor(&self) -> u8 {
        self.cursor
    }

    /// The retained transit target.
    pub fn target(&self) -> DriveCoord {
        self.target
    }

    /// Install a fixture's tube.
    #[cfg(test)]
    pub(crate) fn set_tube_id_for_test(&mut self, tube_id: TubeId) {
        self.tube_id = tube_id;
    }

    /// Install a fixture's cursor.
    #[cfg(test)]
    pub(crate) fn set_cursor_for_test(&mut self, cursor: u8) {
        self.cursor = cursor;
    }

    /// Install a fixture's transit target.
    #[cfg(test)]
    pub(crate) fn set_target_for_test(&mut self, target: DriveCoord) {
        self.target = target;
    }

    /// A fixture's transit.
    #[cfg(test)]
    pub(crate) fn for_test(tube_id: TubeId, cursor: u8, target: DriveCoord) -> Self {
        Self {
            tube_id,
            cursor,
            target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TubeBeginError {
    ZeroLengthTube,
    UnsupportedCategory,
    MissingTerrain,
}

/// Walk 0x0075B302..0x0075B356: a direction-8 Foot+5E0 head enters the tube
/// of the current cell when its index (Cell+0x116) is a live tube. Automatic
/// zero-length records stay predicate metadata (module doc). This is a producer
/// admission check only; state and substrate mutation happen atomically in
/// [`begin_path_tube_step`].
///
/// RESIDUAL: a direction-8 head without a live tube takes native's invalid-tube
/// arm (0x0075B551, which clears Foot+5E0); here it falls through to admission,
/// which reads the word's low three bits. Trigger: a tube route whose tube
/// record was removed before the head was consumed. Frequency: never observed;
/// tube records are map-static. Risk: one stray step north.
pub fn pending_path_tube_id(
    path_replay: &crate::sim::components::FootPathQueue,
    position: &Position,
    current_layer: MovementLayer,
    terrain: Option<&ResolvedTerrainGrid>,
) -> Option<TubeId> {
    if current_layer != MovementLayer::Ground
        || path_replay.remaining_directions().first()
            != Some(&crate::util::direction::TUBE_STEP_DIRECTION)
    {
        return None;
    }
    let terrain = terrain?;
    let tube_id = terrain.cell(position.rx(), position.ry())?.tube_index?;
    if tube_id.0 > i8::MAX as u16 {
        return None;
    }
    let tube = terrain.tube(tube_id)?;
    (tube.source == TubeSource::ExplicitMap && tube.path_len() > 0).then_some(tube_id)
}

/// Begin a verified explicit tube and perform native Mark(REMOVE) substrate
/// teardown. The direction-8 head is consumed immediately (Walk 0x0075B3E0
/// shifts Foot+5E0; Drive 0x004B1362), preserving the route tail for the
/// post-tube object turn.
pub(crate) fn begin_path_tube_step(
    foot_occupation_enabled: &mut bool,
    path_replay: &mut crate::sim::components::FootPathQueue,
    entity_id: u64,
    category: EntityCategory,
    position: &mut Position,
    locomotor: &mut Option<LocomotorState>,
    low_bridge_tube_state: &mut Option<LowBridgeTubeMovementState>,
    cell_marked: &mut bool,
    tube_id: TubeId,
    terrain: &ResolvedTerrainGrid,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    raw_cell_occupation: &mut RawCellOccupationGrid,
) -> Result<(), TubeBeginError> {
    if !matches!(category, EntityCategory::Unit | EntityCategory::Infantry) {
        return Err(TubeBeginError::UnsupportedCategory);
    }
    if category == EntityCategory::Unit
        && locomotor
            .as_ref()
            .and_then(|loco| loco.track_progress(TrackFamily::Drive))
            .is_some_and(|track| track.turn_index() != -1)
    {
        return Err(TubeBeginError::MissingTerrain);
    }
    let tube = terrain
        .tube(tube_id)
        .ok_or(TubeBeginError::MissingTerrain)?;
    let state = initial_state(category, position, tube_id, tube, terrain)?;

    detach_for_tube(
        foot_occupation_enabled,
        entity_id,
        category,
        position,
        locomotor,
        cell_marked,
        occupancy,
        cell_occupation,
        raw_cell_occupation,
    );
    if category == EntityCategory::Unit
        && let Some(loco) = locomotor.as_mut()
        && let Some(mut progress) = loco.track_progress(TrackFamily::Drive)
    {
        // Original4B1352/1357/135A stores the signed exit center in Head_To
        // with Z=0. Destination, cursor, short selection and residual survive.
        // The queue shift4B1362..136E does not rewrite Foot+558's reference.
        loco.store_track_head(
            TrackFamily::Drive,
            Some(DriveCoord {
                x: i32::from(tube.exit.0 as i16)
                    .wrapping_mul(256)
                    .wrapping_add(128),
                y: i32::from(tube.exit.1 as i16)
                    .wrapping_mul(256)
                    .wrapping_add(128),
                z: 0,
            }),
        );
        loco.store_track_valid(TrackFamily::Drive, true); // Original4B1480.
        progress.set_turn_index(-1); // Original4B1484; cursor is untouched.
        loco.store_track_progress(TrackFamily::Drive, progress);
    }
    super::path_markers::consume_path_replay(path_replay, 1);
    *low_bridge_tube_state = Some(state);
    Ok(())
}

fn initial_state(
    category: EntityCategory,
    position: &mut Position,
    tube_id: TubeId,
    tube: &TubeFact,
    terrain: &ResolvedTerrainGrid,
) -> Result<LowBridgeTubeMovementState, TubeBeginError> {
    let path_len = tube.path_len();
    if path_len == 0 {
        return Err(TubeBeginError::ZeroLengthTube);
    }
    let raw_step = tube.path_steps[0];
    let (dx, dy) = direction_delta(raw_step);
    let [current_x, current_y] = ground_pose::position_world_xy(position);
    let current_ground =
        ground_pose::ground_surface_z_at([current_x, current_y], false, Some(terrain), None)
            .ok_or(TubeBeginError::MissingTerrain)?;
    let exit_ground =
        floor_at_cell_center(terrain, tube.exit).ok_or(TubeBeginError::MissingTerrain)?;
    let z_step = exit_ground.wrapping_sub(current_ground) / path_len as i32;
    let (target_x, target_y) = if category == EntityCategory::Infantry {
        (
            current_x.wrapping_add(dx.wrapping_mul(256)),
            current_y.wrapping_add(dy.wrapping_mul(256)),
        )
    } else {
        let next_x = i32::from(tube.entry.0).wrapping_add(dx);
        let next_y = i32::from(tube.entry.1).wrapping_add(dy);
        (
            next_x.wrapping_mul(256).wrapping_add(128),
            next_y.wrapping_mul(256).wrapping_add(128),
        )
    };
    if position.exact_z_leptons().is_none() {
        position.put_z(current_ground);
    }
    Ok(LowBridgeTubeMovementState {
        tube_id,
        cursor: 0,
        target: DriveCoord {
            x: target_x,
            y: target_y,
            z: current_ground.wrapping_add(z_step),
        },
    })
}

fn detach_for_tube(
    foot_occupation_enabled: &mut bool,
    entity_id: u64,
    category: EntityCategory,
    position: &Position,
    locomotor: &mut Option<LocomotorState>,
    cell_marked: &mut bool,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    raw_cell_occupation: &mut RawCellOccupationGrid,
) {
    let rx = position.rx();
    let ry = position.ry();
    occupancy.remove_on_layer(rx, ry, entity_id, MovementLayer::Ground);
    match category {
        EntityCategory::Unit => {
            if let Some(loco) = locomotor
                .as_mut()
                .filter(|l| l.has_track_state(TrackFamily::Drive))
            {
                crate::sim::occupancy::clear_drive_head_to_occupation_for_remove(
                    loco,
                    cell_occupation,
                    entity_id,
                );
                *foot_occupation_enabled = false;
            }
            cell_occupation.clear_vehicle_on_layer(rx, ry, entity_id, MovementLayer::Ground);
            raw_cell_occupation.clear_ground(rx, ry, VEHICLE_OCCUPATION_BIT);
        }
        EntityCategory::Infantry => raw_cell_occupation.clear_ground(
            rx,
            ry,
            infantry_raw_occupation_mask(position.sub_x(), position.sub_y()),
        ),
        _ => {}
    }
    *cell_marked = false;
}

/// What one tube step did ([`tick_active_tube_object`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TubeTurn {
    /// The object holds no tube.
    Inactive,
    /// The object moved along its tube, or a Unit left it.
    Moved,
    /// A Unit at its tube's end found the exit Cell taken
    /// ([`Simulation::tube_exit_blocked`]).
    Blocked,
    /// An infantryman stands at its tube's end
    /// ([`Simulation::infantry_tube_exit`]).
    InfantryExit,
}

/// One step of an active Unit/Infantry TubeMovement. The exits that read the
/// whole Simulation are left to [`Simulation::tube_object_turn`].
fn tick_active_tube_object(
    entities: &mut EntityStore,
    entity_id: u64,
    terrain: &ResolvedTerrainGrid,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    raw_cell_occupation: &mut RawCellOccupationGrid,
    rules: Option<&RuleSet>,
    interner: &StringInterner,
    native_frame: u32,
) -> TubeTurn {
    let Some(entity) = entities.get(entity_id) else {
        return TubeTurn::Inactive;
    };
    let Some(mut state) = entity.low_bridge_tube_state else {
        return TubeTurn::Inactive;
    };
    let Some(tube) = terrain.tube(state.tube_id) else {
        return TubeTurn::Moved;
    };
    let category = entity.category();
    let speed = rules
        .and_then(|rules| rules.object(interner.resolve(entity.type_ref())))
        .map_or(0, |object| {
            crate::util::fixed_math::ra2_speed_to_leptons_per_frame(object.speed)
        });
    let budget = if category == EntityCategory::Unit {
        speed.wrapping_mul(3) / 2
    } else {
        speed
    };
    let z_step = live_z_step(terrain, tube).unwrap_or(0);

    if usize::from(state.cursor) >= tube.path_len() {
        return finalize_tube_object(
            entities,
            entity_id,
            state,
            terrain,
            occupancy,
            cell_occupation,
            raw_cell_occupation,
            rules,
            interner,
            native_frame,
        );
    }

    // 0x00735A2F..0x00735A47: GetCoords (vt+0x48) less the target (+0x568),
    // through Distance3D.
    let current = ground_pose::object_get_coords(entity, Some(terrain));
    let distance = distance_3d_leptons(
        [current.x, current.y, current.z],
        [state.target.x, state.target.y, state.target.z],
    );
    if distance > budget {
        let raw_step = tube.path_steps[usize::from(state.cursor)];
        let trig = TrigTable::embedded();
        let next = advance_amount(current, state.target, budget, raw_step, z_step, trig);
        ground_pose::foot_set_location(entities, entity_id, next, rules, interner);
        return TubeTurn::Moved;
    }

    state.cursor = state.cursor.saturating_add(1);
    if let Some(entity) = entities.get_mut(entity_id) {
        entity.low_bridge_tube_state = Some(state);
    }
    ground_pose::foot_set_location(entities, entity_id, state.target, rules, interner);
    if usize::from(state.cursor) >= tube.path_len() {
        return finalize_tube_object(
            entities,
            entity_id,
            state,
            terrain,
            occupancy,
            cell_occupation,
            raw_cell_occupation,
            rules,
            interner,
            native_frame,
        );
    }

    let next_raw = tube.path_steps[usize::from(state.cursor)];
    let (dx, dy) = direction_delta(next_raw);
    let old_target = state.target;
    state.target.x = state.target.x.wrapping_add(dx.wrapping_mul(256));
    state.target.y = state.target.y.wrapping_add(dy.wrapping_mul(256));
    state.target.z = state.target.z.wrapping_add(z_step);
    let leftover = budget.wrapping_sub(distance);
    let trig = TrigTable::embedded();
    let next = advance_amount(old_target, state.target, leftover, next_raw, z_step, trig);
    if let Some(entity) = entities.get_mut(entity_id) {
        entity.low_bridge_tube_state = Some(state);
    }
    ground_pose::foot_set_location(entities, entity_id, next, rules, interner);
    TubeTurn::Moved
}

/// The tube's end. An infantryman's exit is
/// [`Simulation::infantry_tube_exit`]; this is `UnitClass::TubeMovement`'s
/// (`0x00735E36..0x007360AA`). Any object on the ground list (`+0xE4`) of
/// the Cell holding the Location blocks it (`0x00735E66..0x00735E6E`,
/// [`Simulation::tube_exit_blocked`]). Otherwise SetLocation (vt+0x1B4) puts
/// the unit at the exit cell's centre at the target's Z (`+0x570`,
/// `0x00735FA1..0x00735FEC`).
///
/// RESIDUAL: the unblocked arm does not follow the native order or owners.
/// Native runs Mark(DOWN) (`0x00735FFF`), `Scatter(null, 1, 1)` on itself
/// without a NavCom (`0x00736005..0x00736028`, #893), the `+0x68B` latch
/// (`0x00736038`), Per_Cell_Process(2) (`0x0073603F`), then speed 1
/// (`0x0073604F`) and the facing. Here [`put_unit_after_tube`] stands in for
/// Mark(DOWN), the self-Scatter and the latch are missing, and the speed and
/// facing come before the object turn's Per_Cell_Process.
/// - Trigger: every Unit leaving a map tube.
/// - Effect: a Unit without a NavCom stays on the exit cell instead of
///   scattering; Mark(DOWN)'s AddContent discovery and Recalc are skipped.
/// - Frequency: retail maps with tubes only.
/// - Risk: Mark(DOWN) writes the raw vehicle bit only while `+0x6B6` holds,
///   and the tube entry here clears that flag (`detach_for_tube`), so moving
///   to [`Simulation::foot_mark_put`] needs the entry's Mark(UP)
///   (`0x004B12BA`) ported first.
fn finalize_tube_object(
    entities: &mut EntityStore,
    entity_id: u64,
    state: LowBridgeTubeMovementState,
    terrain: &ResolvedTerrainGrid,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    raw_cell_occupation: &mut RawCellOccupationGrid,
    rules: Option<&RuleSet>,
    interner: &StringInterner,
    native_frame: u32,
) -> TubeTurn {
    let Some(tube) = terrain.tube(state.tube_id) else {
        return TubeTurn::Moved;
    };
    let Some(entity) = entities.get(entity_id) else {
        return TubeTurn::Moved;
    };
    if entity.category() == EntityCategory::Infantry {
        return TubeTurn::InfantryExit;
    }
    let reached_cell = (entity.position.rx(), entity.position.ry());
    if occupancy
        .get(reached_cell.0, reached_cell.1)
        .is_some_and(|cell| cell.iter_layer(MovementLayer::Ground).next().is_some())
    {
        return TubeTurn::Blocked;
    }

    let exit = DriveCoord::cell(tube.exit.0, tube.exit.1, state.target.z);
    ground_pose::foot_set_location(entities, entity_id, exit, rules, interner);
    if let Some(entity) = entities.get_mut(entity_id) {
        if let Some(cell) = terrain.cell(tube.exit.0, tube.exit.1) {
            entity.position.store_level(cell.level);
        }
        if let Some(loco) = entity.locomotor.as_mut() {
            loco.store_track_target_fraction(TrackFamily::Drive, SIM_ONE);
        }
        // Unit73604F writes the live Foot owner even if PerCell replaced
        // Drive. Exact post-callback timing remains part of the Process host.
        entity.foot_speed.set_speed_fraction(SIM_ONE);
        entity.low_bridge_tube_state = None;
    }
    put_unit_after_tube(
        entities,
        entity_id,
        occupancy,
        cell_occupation,
        raw_cell_occupation,
    );
    update_unit_final_facing(entities, entity_id, terrain, native_frame);
    TubeTurn::Moved
}

/// The Unit exit's stand-in for Mark(DOWN) (see [`finalize_tube_object`]).
fn put_unit_after_tube(
    entities: &mut EntityStore,
    entity_id: u64,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    raw_cell_occupation: &mut RawCellOccupationGrid,
) {
    let Some(entity) = entities.get_mut(entity_id) else {
        return;
    };
    let rx = entity.position.rx();
    let ry = entity.position.ry();
    occupancy.add(
        rx,
        ry,
        entity_id,
        MovementLayer::Ground,
        None,
        CellListInsertion::from_category(entity.category()),
    );
    raw_cell_occupation.mark_ground(rx, ry, VEHICLE_OCCUPATION_BIT);
    cell_occupation.mark_vehicle_on_layer(rx, ry, entity_id, MovementLayer::Ground);
    entity.set_foot_occupation_enabled(true);
    entity.lifecycle.set_cell_marked(true);
}

/// What one active tube object turn did ([`Simulation::tube_object_turn`]).
#[derive(Debug, Default)]
pub(crate) struct TubeObjectTurn {
    pub(crate) movement: MovementTickStats,
    pub(crate) bridge_state_changed: bool,
    /// The exit ran `Per_Cell_Process(2)`.
    pub(crate) per_cell_ran: bool,
}

impl Simulation {
    /// The TubeMovement the class AI runs in place of the rest of its turn
    /// while the object holds a tube (`+0x684`): `UnitClass` `0x007359F0`
    /// (called at `0x007363B0`) and `InfantryClass` `0x0051B350` (called at
    /// `0x0051BAC2`). It takes a step along the tube, or at its end the exit,
    /// which ends with Per_Cell_Process(2) (Unit `0x0073603F`, Infantry
    /// `0x0051BA9B`). No locomotor Process runs: one the exit calls (the
    /// Infantry self-Scatter's, `0x0051D478`) is the locomotor's alone and
    /// reads no tube.
    ///
    /// The owed destination setter ([`Simulation::complete_pending_order`])
    /// and the vehicle-plane reconcile run first, as at a Process entry.
    pub(crate) fn tube_object_turn(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> Result<TubeObjectTurn, FrameAdvanceError> {
        let mut turn = TubeObjectTurn::default();
        self.complete_pending_order(id, rules, registry, effects);
        let Some(entity) = self.substrate.entities.get(id) else {
            return Ok(turn);
        };
        self.substrate
            .cell_occupation
            .reconcile_entity(entity, &self.substrate.occupancy);
        let Some(terrain) = self.resolved_terrain.as_ref() else {
            return Ok(turn);
        };
        let step = tick_active_tube_object(
            &mut self.substrate.entities,
            id,
            terrain,
            &mut self.substrate.occupancy,
            &mut self.substrate.cell_occupation,
            &mut self.substrate.raw_cell_occupation,
            rules,
            &self.interner,
            self.session.binary_frame,
        );
        let exit = match step {
            TubeTurn::Inactive => return Ok(turn),
            TubeTurn::Moved => Ok(false),
            TubeTurn::Blocked => self.tube_exit_blocked(id, rules, registry, effects),
            TubeTurn::InfantryExit => self.infantry_tube_exit(id, rules, registry, effects),
        };
        turn.movement.movers_total = 1;
        turn.bridge_state_changed = exit.map_err(|cause| {
            FrameAdvanceError::new(self.session.tick, self.session.binary_frame, id, cause)
        })?;
        if self
            .substrate
            .entities
            .get(id)
            .is_some_and(|entity| entity.low_bridge_tube_state.is_none())
        {
            turn.bridge_state_changed |=
                self.per_cell_process(id, PerCellReason::Arrival, rules, registry, effects)?;
            turn.per_cell_ran = true;
        }
        Ok(turn)
    }

    /// A tube exit its Cell refuses. Each Unit or Infantry on the ground
    /// list (`+0xE4`) of the Cell holding the Location whose locomotor is not
    /// moving ([`tube_exit_scatters`]) takes `Scatter(null, 1, 1)`: the list
    /// is copied first and each copy tested as the loop reaches it (Unit
    /// `0x00735E74..0x00735F62`, Infantry `0x0051B7E6..0x0051B8F4`). Then the
    /// object's speed becomes 0 (Unit `0x00735F6A`, Infantry `0x0051B8FC`)
    /// and it stays in the tube, to try again on its next turn. Without rules
    /// no Scatter admits, so none runs.
    ///
    /// Answers whether an Infantry Scatter's immediate Process changed bridge
    /// state.
    fn tube_exit_blocked(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> Result<bool, String> {
        let occupants = self
            .substrate
            .entities
            .get(id)
            .and_then(|actor| {
                self.substrate
                    .occupancy
                    .get(actor.position.rx(), actor.position.ry())
            })
            .map_or_else(Vec::new, |cell| cell.snapshot_layer(MovementLayer::Ground));
        let mut bridge_state_changed = false;
        if let Some(rules) = rules {
            for blocker in occupants {
                if self
                    .substrate
                    .entities
                    .get(blocker)
                    .is_some_and(tube_exit_scatters)
                {
                    bridge_state_changed |= self.scatter(
                        blocker,
                        None,
                        ScatterFlags::new(true, true),
                        rules,
                        registry,
                        effects,
                    )?;
                }
            }
        }
        stop_blocked_mover(&mut self.substrate.entities, id);
        Ok(bridge_state_changed)
    }

    /// `InfantryClass` TubeMovement's exit (`0x0051B798..0x0051BAA8`), once
    /// the last step put the infantryman at its target. It asks its own
    /// `Can_Enter_Cell` (`+0x1AC`) at the Cell holding its Location
    /// (`0x0051B7BE`), with direction -1, height 0, no previous cell and
    /// mode 1 (`0x0051B7C3..0x0051B7D8`).
    ///
    /// - Refused, it scatters the stationary Units and Infantry there, sets
    ///   its own speed to 0 and stays in the tube
    ///   ([`Self::tube_exit_blocked`]).
    /// - Admitted, PlaceInfantryInCell (`0x00481180`, `0x0051B967`) picks a
    ///   spot on the Cell's ground plane, and SetLocation (`0x0051B99C`)
    ///   puts it there at the floor (`0x00578080`, `0x0051B98A`). Then come
    ///   the locomotor's Force_Immediate_Destination(null) (`0x0051B9F4`)
    ///   and Stop_Movement_Animation (`0x0051BA16`), Mark(DOWN)
    ///   (`0x0051BA22`) and the raw receiver (vt+0xF0, `0x0051BA45`). A
    ///   NavCom naming the Cell scatters the infantryman itself
    ///   (`0x0051BA4B..0x0051BA6F`); otherwise its speed becomes 1
    ///   (`0x0051BA81`). Last, it leaves the tube (`+0x684 = -1`,
    ///   `0x0051BA8D`) and latches `+0x68B` (`0x0051BA94`). Its
    ///   Per_Cell_Process(2) (`0x0051BA9B`) follows in
    ///   [`Self::tube_object_turn`].
    ///
    /// The exit does not test PlaceInfantryInCell's answer. Without a spot
    /// it answers the zeroed coordinate at `0x0089E778` (`0x004812EC`,
    /// `0x004813CC`; zeroed by `0x0047B300`), so the infantryman goes to the
    /// floor at (0, 0). That needs an occupant the `+0x1AC` admits but the
    /// placement refuses, such as a vehicle the infantryman is entering.
    ///
    /// RESIDUAL: the Walk tube entry stores the exit Cell's centre (Z 0) as
    /// the Walk head (`0x0075B3CA..0x0075B3D2`); [`begin_path_tube_step`]
    /// stores none, so Force_Immediate_Destination(null) here releases the
    /// raw bit of another spot.
    /// - Trigger: every infantryman leaving a map tube.
    /// - Effect: native clears raw spot bit 0 (the centre) of the exit Cell;
    ///   here the release falls on the infantryman's new spot, still
    ///   unmarked.
    /// - Frequency: retail maps with tubes; it matters only with an occupant
    ///   on that Cell's centre spot.
    /// - Risk: that occupant keeps a raw bit native clears. The head write
    ///   belongs to the tube entry.
    ///
    /// Answers whether an Infantry Scatter's immediate Process changed
    /// bridge state.
    fn infantry_tube_exit(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> Result<bool, String> {
        let rules = rules.ok_or("Infantry tube exit requires rules")?;
        let terrain = self
            .resolved_terrain
            .as_ref()
            .ok_or("Infantry tube exit requires map cells")?;
        let actor = self
            .substrate
            .entities
            .get(id)
            .ok_or("retired tube mover")?;
        let location = ground_pose::position_world_coord(&actor.position);
        let at = (actor.position.rx(), actor.position.ry());
        let cell = NativeCellQuery::canonical(terrain).lookup_world(location.x, location.y);
        let args = InfantryEntryArgs {
            direction: -1,
            height: 0,
            previous_cell: None,
        };
        if self.foot_can_enter(id, cell, args, rules, registry)? != 0 {
            return self.tube_exit_blocked(id, Some(rules), registry, effects);
        }

        let out = self
            .place_infantry_in_ground_cell(rules, location)
            .map_or(DriveCoord { x: 0, y: 0, z: 0 }, |spot| {
                walk_head::selected_head(location, spot, 0, false)
            });
        let (exit, level) = {
            let cells = NativeCellQuery::canonical(
                self.resolved_terrain
                    .as_ref()
                    .ok_or("Infantry tube exit requires map cells")?,
            );
            let exit = DriveCoord {
                z: ground_pose::query_ground_height(&cells, out)?,
                ..out
            };
            (
                exit,
                cells.ground_fields(cells.lookup_world(exit.x, exit.y)).0,
            )
        };
        ground_pose::foot_set_location(
            &mut self.substrate.entities,
            id,
            exit,
            Some(rules),
            &self.interner,
        );
        if let Some(actor) = self.substrate.entities.get_mut(id) {
            // VERA's spot and level caches follow the Location: the spot is
            // the one the raw receiver reads from it.
            actor.sub_cell.hold(cell_kernel::infantry_preferred_spot(
                cell_kernel::CellQueryPoint {
                    x: exit.x,
                    y: exit.y,
                },
            ));
            let on_bridge = actor.on_bridge();
            actor.position.store_cell_level(level, on_bridge);
        }
        self.foot_force_immediate_destination(id, None, Some(rules), registry, effects)?;
        if let Some(locomotor) = self
            .substrate
            .entities
            .get_mut(id)
            .and_then(|actor| actor.locomotor.as_mut())
        {
            locomotor.stop_movement_animation();
        }
        self.foot_mark_put(id, Some(rules), registry, effects);
        if let Some(actor) = self.substrate.entities.get(id) {
            let location = ground_pose::position_world_coord(&actor.position);
            self.object_raw_receiver_at(id, location, true);
        }

        let navcom_names_cell = self.substrate.entities.get(id).is_some_and(|actor| {
            matches!(actor.navigation.nav_com(), Some(NavTargetRef::Cell { rx, ry }) if (rx, ry) == at)
        });
        let mut bridge_state_changed = false;
        if navcom_names_cell {
            bridge_state_changed = self.scatter(
                id,
                None,
                ScatterFlags::new(true, true),
                rules,
                registry,
                effects,
            )?;
        } else if let Some(actor) = self.substrate.entities.get_mut(id) {
            actor.foot_speed.set_speed_fraction(SIM_ONE);
        }
        if let Some(actor) = self.substrate.entities.get_mut(id) {
            actor.low_bridge_tube_state = None;
            actor.runtime_bridge_transition.latch();
        }
        Ok(bridge_state_changed)
    }
}

/// Whether a blocked tube exit scatters `blocker`: a Unit or Infantry
/// (`WhatAmI` 1 or 0xF) whose locomotor is not moving (ILocomotion +0x10).
/// Unit `0x00735EF9..0x00735F31`, Infantry `0x0051B88E..0x0051B8C6`. A Foot
/// without a locomotor raises a COM error natively (`0x00735F19`); here it is
/// skipped.
fn tube_exit_scatters(blocker: &GameEntity) -> bool {
    matches!(
        blocker.category(),
        EntityCategory::Unit | EntityCategory::Infantry
    ) && blocker.locomotor.is_some()
        && super::motion_query::is_moving(blocker) != Some(true)
}

fn stop_blocked_mover(entities: &mut EntityStore, entity_id: u64) {
    if let Some(entity) = entities.get_mut(entity_id) {
        if let Some(loco) = entity.locomotor.as_mut() {
            loco.store_track_target_fraction(TrackFamily::Drive, SIM_ZERO);
        }
        // Unit735F6A / Infantry51B8FC apply zero on the live Foot owner.
        entity.foot_speed.set_speed_fraction(SIM_ZERO);
    }
}

fn update_unit_final_facing(
    entities: &mut EntityStore,
    entity_id: u64,
    terrain: &ResolvedTerrainGrid,
    native_frame: u32,
) {
    let Some(entity) = entities.get_mut(entity_id) else {
        return;
    };
    let Some(tube) = terrain
        .cell(entity.position.rx(), entity.position.ry())
        .and_then(|cell| cell.tube_index)
        .and_then(|tube_id| terrain.tube(tube_id))
    else {
        return;
    };
    let q32 = tube.direction.wrapping_shl(13).wrapping_sub(0x6001) & 0xffff_e000_u32 as i32;
    entity.body_facing.snap(q32 as u16, native_frame);
}

fn live_z_step(terrain: &ResolvedTerrainGrid, tube: &TubeFact) -> Option<i32> {
    let count = i32::try_from(tube.path_len()).ok()?;
    if count == 0 {
        return None;
    }
    let entry = floor_at_cell_center(terrain, tube.entry)?;
    let exit = floor_at_cell_center(terrain, tube.exit)?;
    Some(exit.wrapping_sub(entry) / count)
}

/// The floor height (`0x00578080`) at a cell's centre, as the live Z step
/// reads it at the tube's entry and exit (`0x00735B05`, `0x00735B16`).
fn floor_at_cell_center(terrain: &ResolvedTerrainGrid, cell: (u16, u16)) -> Option<i32> {
    let center = DriveCoord::cell(cell.0, cell.1, 0);
    ground_pose::ground_surface_z_at([center.x, center.y], false, Some(terrain), None)
}

fn direction_delta(raw: i32) -> (i32, i32) {
    crate::util::direction::DIRECTION_DELTAS[(raw & 7) as usize]
}

fn advance_amount(
    base: DriveCoord,
    target: DriveCoord,
    amount: i32,
    raw_step: i32,
    z_step: i32,
    trig: &TrigTable,
) -> DriveCoord {
    let facing = crate::util::direction_tables::facing16_from_delta(
        target.x.wrapping_sub(base.x),
        target.y.wrapping_sub(base.y),
    );
    let angle_scale = f64::from_bits(0xbf19_222d_989f_5e57);
    let angle = f64::from(i32::from(facing as i16).wrapping_sub(0x3fff)) * angle_scale;
    let cosine = trig.cos_radians(angle);
    let sine = trig.sin_radians(angle);
    let amount_x87 = X87Chop53::load_i32(amount);
    let x = ftol_add_f32_product(base.x, cosine, amount_x87);
    let y = ftol_add_f32_product(base.y, -sine, amount_x87);

    let nominal_bits = if raw_step & 1 == 0 {
        256.0_f64.to_bits()
    } else {
        0x4076_a09e_667f_3bcd
    };
    let nominal = X87Chop53::load_f64(NativeF64Bits::from_bits(nominal_bits))
        .expect("nominal tube segment length is finite");
    let ratio = X87Chop53::div(amount_x87, nominal).expect("nominal length is nonzero");
    let z_delta = X87Chop53::mul(ratio, X87Chop53::load_i32(z_step));
    let z = X87Chop53::ftol_i64(X87Chop53::add(X87Chop53::load_i32(base.z), z_delta))
        .expect("tube Z fits i32") as i32;
    DriveCoord { x, y, z }
}

fn ftol_add_f32_product(base: i32, factor: f32, amount: crate::util::native_x87::X87Value) -> i32 {
    let factor = X87Chop53::load_f32(NativeF32Bits::from_bits(factor.to_bits()))
        .expect("retail trig entry is finite");
    X87Chop53::ftol_i64(X87Chop53::add(
        X87Chop53::load_i32(base),
        X87Chop53::mul(factor, amount),
    ))
    .expect("tube XY fits i32") as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::resolved_terrain::{ResolvedTerrainCell, ResolvedTerrainGrid};
    use crate::map::tube_facts::TubeSource;
    use crate::sim::combat::Health;
    use crate::sim::game_entity::GameEntity;
    use crate::sim::occupancy::CellListInsertion;

    #[test]
    fn tube_exit_scatter_skips_retained_motion_without_an_order() {
        use crate::rules::locomotor_type::LocomotorKind;
        use crate::sim::movement::locomotor::LocomotorState;
        for kind in [LocomotorKind::Walk, LocomotorKind::Jumpjet] {
            let mut actor = GameEntity::test_default(2, "E1", "Americans", 5, 5);
            actor.set_category_for_test(EntityCategory::Infantry);
            let mut loco = LocomotorState::for_test_kind(kind);
            if let Some(state) = loco.jumpjet_runtime_mut() {
                *state = state
                    .clone()
                    .with_moving_for_test(true)
                    .with_phase_for_test(2);
            } else {
                loco.set_walk_destination(Some(DriveCoord::cell(6, 5, 0)));
            }
            actor.locomotor = Some(loco);
            assert!(!tube_exit_scatters(&actor), "{kind:?} is moving");
        }
    }

    fn flat_cell(rx: u16, ry: u16, tube_index: Option<TubeId>) -> ResolvedTerrainCell {
        ResolvedTerrainCell {
            tube_index,
            ..crate::map::resolved_terrain::test_loader_clear_cell(rx, ry)
        }
    }

    fn explicit_terrain(path: Vec<i32>) -> ResolvedTerrainGrid {
        let mut cells = Vec::new();
        for x in 0..=3 {
            cells.push(flat_cell(x, 0, (x == 0).then_some(TubeId(0))));
        }
        ResolvedTerrainGrid::from_cells_with_tubes(
            4,
            1,
            cells,
            vec![TubeFact {
                entry: (0, 0),
                exit: (path.len() as u16, 0),
                direction: 2,
                path_steps: path,
                source: TubeSource::ExplicitMap,
            }],
        )
    }

    fn unit(id: u64) -> GameEntity {
        unit_of(id, "MTNK")
    }

    fn unit_of(id: u64, kind: &str) -> GameEntity {
        let mut entity = GameEntity::new_at_frame_zero_for_test(
            id,
            0,
            0,
            0,
            0,
            crate::sim::intern::test_intern("Americans"),
            Health::new(100),
            crate::sim::intern::test_intern(kind),
            EntityCategory::Unit,
            0,
            5,
            true,
        );
        entity.lifecycle.set_in_limbo_for_test(false);
        entity.lifecycle.set_cell_marked(true);
        entity.locomotor = Some(LocomotorState::for_test_kind(
            crate::rules::locomotor_type::LocomotorKind::Drive,
        ));
        entity
            .locomotor
            .as_mut()
            .unwrap()
            .ensure_installed_track_state();
        entity
    }

    #[test]
    fn gsi_04_15_zero_step_auto_shell_is_never_a_path_producer() {
        let terrain = ResolvedTerrainGrid::from_cells_with_tubes(
            1,
            1,
            vec![flat_cell(0, 0, Some(TubeId(0)))],
            vec![TubeFact::auto_low_bridge((0, 0), 2)],
        );
        let queue = {
            let mut value = crate::sim::components::FootPathQueue::default();
            value.set_directions_for_test(vec![crate::util::direction::TUBE_STEP_DIRECTION]);
            value
        };
        let entity = unit(1);
        assert_eq!(
            pending_path_tube_id(
                &queue,
                &entity.position,
                MovementLayer::Ground,
                Some(&terrain)
            ),
            None
        );
    }

    /// Supplied post-Tube state exercises the real finalizer's owner writes.
    /// It does not emulate the preceding native PerCell callback or its timing.
    #[test]
    fn tube_finalizer_updates_live_owner_speed_without_drive_payload() {
        use crate::rules::locomotor_type::LocomotorKind;
        use crate::sim::movement::locomotor::LocomotorState;
        use crate::util::fixed_math::SimFixed;
        for blocked in [false, true] {
            let terrain = explicit_terrain(vec![2, 2]);
            let mut entity = unit(1);
            entity.locomotor = Some(LocomotorState::for_test_kind(LocomotorKind::Teleport));
            entity.lifecycle.set_cell_marked(false);
            entity.position.set_rx_for_test(2);
            entity.foot_speed.set_speed_fraction(SimFixed::lit("0.625"));
            let state = LowBridgeTubeMovementState {
                tube_id: TubeId(0),
                cursor: 2,
                target: DriveCoord::cell(2, 0, 0),
            };
            entity.low_bridge_tube_state = Some(state);
            let mut sim = Simulation::new();
            sim.substrate.entities.insert(entity);
            if blocked {
                let mut blocker = unit(2);
                blocker.position.set_rx_for_test(2);
                sim.substrate.entities.insert(blocker);
                sim.substrate.occupancy.add(
                    2,
                    0,
                    2,
                    MovementLayer::Ground,
                    None,
                    CellListInsertion::PrependNonBuilding,
                );
            }
            sim.resolved_terrain = Some(terrain);
            sim.interner = crate::sim::intern::test_interner();
            let turn = sim
                .tube_object_turn(1, None, None, FrameEffects::default())
                .expect("tube turn");
            assert_eq!(turn.movement.movers_total, 1);
            assert_eq!(turn.per_cell_ran, !blocked);
            let owner = sim.substrate.entities.get(1).unwrap();
            assert!(owner.locomotor.as_ref().is_none_or(|loco| {
                !loco.has_track_state(crate::sim::movement::track_process::TrackFamily::Drive)
            }));
            assert_eq!(
                owner.locomotor.as_ref().unwrap().active_kind(),
                LocomotorKind::Teleport
            );
            assert_eq!(
                owner.foot_speed.applied_fraction(),
                if blocked { SIM_ZERO } else { SIM_ONE }
            );
            // GetCurrentSpeed follows the fraction; the rules-less getter
            // reads the order's stamped 150 leptons/s.
            assert_eq!(
                crate::sim::movement::owner_current_speed(
                    owner,
                    None,
                    1.0,
                    &std::collections::BTreeMap::new(),
                ),
                if blocked { 0 } else { 10 }
            );
            assert_eq!(owner.low_bridge_tube_state.is_some(), blocked);
            if !blocked {
                assert!(owner.lifecycle.cell_marked());
                assert_eq!(
                    sim.substrate
                        .occupancy
                        .count_on_layer(2, 0, MovementLayer::Ground),
                    1
                );
            }
        }
    }

    #[test]
    fn gsi_04_15_tube_admission_requires_ground_object_list_layer() {
        let terrain = explicit_terrain(vec![2, 2]);
        let entity = unit(1);
        let mut queue = {
            let mut value = crate::sim::components::FootPathQueue::default();
            value.set_directions_for_test(vec![crate::util::direction::TUBE_STEP_DIRECTION]);
            value
        };

        assert_eq!(
            pending_path_tube_id(
                &queue,
                &entity.position,
                MovementLayer::Bridge,
                Some(&terrain)
            ),
            None
        );
        assert_eq!(
            pending_path_tube_id(
                &queue,
                &entity.position,
                MovementLayer::Ground,
                Some(&terrain)
            ),
            Some(TubeId(0))
        );
        // Walk75B302 enters only on a direction-8 head word.
        queue.set_directions_for_test(vec![2]);
        assert_eq!(
            pending_path_tube_id(
                &queue,
                &entity.position,
                MovementLayer::Ground,
                Some(&terrain)
            ),
            None
        );
    }

    #[test]
    fn gsi_04_15_begin_detaches_ground_list_owner_mark_and_raw_byte() {
        let terrain = explicit_terrain(vec![2, 2]);
        let mut entity = unit(1);
        let destination = DriveCoord {
            x: 901,
            y: -17,
            z: 731,
        };
        let retained = crate::sim::components::TrackProgress::for_test(-1, -1, true, 6);
        let loco = entity.locomotor.as_mut().unwrap();
        loco.store_track_destination(TrackFamily::Drive, Some(destination));
        loco.store_track_head(
            TrackFamily::Drive,
            Some(DriveCoord {
                x: 43,
                y: 81,
                z: 512,
            }),
        );
        loco.store_track_progress(TrackFamily::Drive, retained);
        entity.navigation.path_replay =
            crate::sim::components::FootPathQueue::for_test(vec![8, 2], 0, Some((-3, 7)));
        let mut occupancy = OccupancyGrid::new();
        occupancy.add(
            0,
            0,
            1,
            MovementLayer::Ground,
            None,
            CellListInsertion::PrependNonBuilding,
        );
        let mut cell_occupation = CellOccupationGrid::new();
        cell_occupation.mark_vehicle_on_layer(0, 0, 1, MovementLayer::Ground);
        let mut raw = RawCellOccupationGrid::new();
        raw.mark_ground(0, 0, VEHICLE_OCCUPATION_BIT);

        let (stable_id, category) = (entity.stable_id(), entity.category());
        let mut cell_marked = entity.lifecycle.cell_marked();
        begin_path_tube_step(
            &mut true,
            &mut entity.navigation.path_replay,
            stable_id,
            category,
            &mut entity.position,
            &mut entity.locomotor,
            &mut entity.low_bridge_tube_state,
            &mut cell_marked,
            TubeId(0),
            &terrain,
            &mut occupancy,
            &mut cell_occupation,
            &mut raw,
        )
        .unwrap();
        entity.lifecycle.set_cell_marked(cell_marked);

        assert!(!entity.lifecycle.cell_marked());
        assert_eq!(occupancy.count_on_layer(0, 0, MovementLayer::Ground), 0);
        assert_eq!(raw.ground_bits(0, 0), 0);
        assert_eq!(cell_occupation.vehicle_bits(0, 0, MovementLayer::Ground), 0);
        assert_eq!(entity.low_bridge_tube_state.unwrap().target.x, 384);
        let drive = entity
            .locomotor
            .as_ref()
            .and_then(|loco| loco.selected_drive_runtime())
            .and_then(|runtime| runtime.retained())
            .unwrap();
        assert_eq!(drive.destination(), Some(destination));
        assert_eq!(
            drive.head_to(),
            Some(DriveCoord {
                x: 640,
                y: 128,
                z: 0
            })
        );
        assert!(drive.track_valid());
        assert_eq!(drive.track(), retained);
        assert_eq!(entity.navigation.path_replay.cursor(), 1);
        assert_eq!(
            entity.navigation.path_replay.reference_cell(),
            Some((-3, 7))
        );
    }

    #[test]
    fn gsi_04_15_cardinal_partial_uses_full_3d_distance_but_nominal_z_denominator() {
        let trig = TrigTable::embedded();
        let current = DriveCoord { x: 0, y: 0, z: 0 };
        let target = DriveCoord {
            x: 256,
            y: 0,
            z: 100,
        };
        assert!(distance_3d_leptons([0, 0, 0], [target.x, target.y, target.z]) > 256);
        assert_eq!(
            advance_amount(current, target, 100, 2, 100, trig),
            DriveCoord {
                x: 100,
                y: 0,
                z: 39
            }
        );
    }

    #[test]
    fn gsi_04_15_type_speed_budget_uses_native_scaled_field() {
        use crate::util::fixed_math::ra2_speed_to_leptons_per_frame;
        assert_eq!(ra2_speed_to_leptons_per_frame(-1), 0);
        assert_eq!(ra2_speed_to_leptons_per_frame(4), 10);
        assert_eq!(ra2_speed_to_leptons_per_frame(100), 255);
        assert_eq!(ra2_speed_to_leptons_per_frame(200), 255);
        assert_eq!(ra2_speed_to_leptons_per_frame(11) * 3 / 2, 42);
    }

    #[test]
    fn gsi_04_15_unit_final_facing_uses_final_cells_raw_tube_direction() {
        let terrain = explicit_terrain(vec![2, 2]);
        let mut entities = EntityStore::new();
        let mut entity = unit(1);
        entity.position.set_rx_for_test(2);
        entity.low_bridge_tube_state = Some(LowBridgeTubeMovementState {
            tube_id: TubeId(0),
            cursor: 2,
            target: DriveCoord {
                x: 640,
                y: 128,
                z: 0,
            },
        });
        entities.insert(entity);
        update_unit_final_facing(&mut entities, 1, &terrain, 0);
        // Exit has no tube index in this fixture: facing is preserved.
        assert_eq!(entities.get(1).unwrap().body_facing_current(0), 0);
    }

    /// One walking infantryman (id 1, `Americans` `E1`) at the end of
    /// [`explicit_terrain`]'s two-step tube: at in-cell (64, 192) of the
    /// exit (2, 0), on Clear ground with a Foot speed row, its movement
    /// animation running.
    fn infantry_at_tube_end() -> (Simulation, RuleSet) {
        use crate::rules::ini_parser::IniFile;
        let rules = RuleSet::from_ini(&IniFile::from_str(
            "[InfantryTypes]\n0=E1\n[VehicleTypes]\n0=APC\n[AircraftTypes]\n[BuildingTypes]\n\
             [E1]\nStrength=100\nSpeed=4\n[APC]\nStrength=100\nSpeed=4\n",
        ))
        .expect("rules");
        let mut terrain = explicit_terrain(vec![2, 2]);
        for x in 0..=3 {
            terrain.cell_mut(x, 0).unwrap().speed_costs.foot = Some(100);
        }
        let mut sim = Simulation::new();
        let owner = sim.interner.intern("Americans");
        let kind = sim.interner.intern("E1");
        let mut entity = GameEntity::new_at_frame_zero_for_test(
            1,
            2,
            0,
            0,
            0,
            owner,
            Health::new(100),
            kind,
            EntityCategory::Infantry,
            0,
            5,
            true,
        );
        entity.lifecycle.set_in_limbo_for_test(false);
        entity.lifecycle.set_cell_marked(false);
        let spot = DriveCoord {
            x: 2 * 256 + 64,
            y: 192,
            z: 0,
        };
        entity.position.put_location(spot);
        let mut walk = crate::sim::movement::locomotor::LocomotorState::for_test_kind(
            crate::rules::locomotor_type::LocomotorKind::Walk,
        );
        walk.set_step_head(Some(spot));
        walk.begin_walk_motion();
        walk.set_step_head(None);
        entity.locomotor = Some(walk);
        entity.low_bridge_tube_state = Some(LowBridgeTubeMovementState {
            tube_id: TubeId(0),
            cursor: 2,
            target: spot,
        });
        sim.substrate.entities.insert(entity);
        sim.resolved_terrain = Some(terrain);
        (sim, rules)
    }

    /// Infantry leaves a tube at the spot PlaceInfantryInCell picks
    /// (`0x0051B967`), at the floor there (`0x00578080` at `0x0051B98A`),
    /// through SetLocation (`0x0051B99C`). Its movement animation stops
    /// (`0x0051BA16`), Mark(DOWN) lists it (`0x0051BA22`), the raw receiver
    /// marks its spot (`0x0051BA45`), its speed becomes 1 (`0x0051BA81`),
    /// it latches `+0x68B` (`0x0051BA94`) and runs Per_Cell_Process(2)
    /// (`0x0051BA9B`).
    /// The floor is native's: `tools/ramp_height_vectors.json`
    /// `ramp_1_sub_64_192` (level 0, ramp 1, sub-cell (64, 192)) is 26.
    #[test]
    fn infantry_leaves_a_tube_at_the_floor_of_its_spot() {
        let (mut sim, rules) = infantry_at_tube_end();
        sim.resolved_terrain
            .as_mut()
            .unwrap()
            .cell_mut(2, 0)
            .unwrap()
            .slope_type = 1;
        let turn = sim
            .tube_object_turn(1, Some(&rules), None, FrameEffects::default())
            .expect("tube turn");
        assert!(turn.per_cell_ran);
        assert_eq!(turn.movement.movers_total, 1);
        let owner = sim.substrate.entities.get(1).unwrap();
        assert!(owner.low_bridge_tube_state.is_none());
        assert_eq!(
            owner.locomotor.as_ref().unwrap().walk_animation_moving(),
            Some(false)
        );
        assert_eq!(owner.sub_cell(), Some(3));
        assert_eq!(
            ground_pose::position_world_coord(&owner.position),
            DriveCoord {
                x: 2 * 256 + 64,
                y: 192,
                z: 26
            }
        );
        assert!(owner.lifecycle.cell_marked());
        assert!(sim.substrate.occupancy.contains_entity(2, 0, 1));
        assert_eq!(
            sim.substrate.raw_cell_occupation.bits_at(
                crate::sim::occupancy::RawCellKey::Real(2, 0),
                MovementLayer::Ground
            ),
            1 << 3
        );
        assert_eq!(owner.foot_speed.applied_fraction(), SIM_ONE);
        assert!(owner.runtime_bridge_transition.pending_mismatch());
    }

    /// The exit asks the infantryman's own `Can_Enter_Cell` (`0x0051B7D8`).
    /// An unarmed one refuses a cell holding an enemy: the standing enemy
    /// takes `Scatter(null, 1, 1)` (`0x0051B8F4`), and the infantryman stays
    /// in the tube at speed 0 (`0x0051B8FC`), to try again on its next turn.
    #[test]
    fn an_enemy_on_the_exit_keeps_the_infantryman_in_the_tube() {
        let (mut sim, rules) = infantry_at_tube_end();
        crate::sim::arena_fixture::supply_native_map(&mut sim);
        let enemy_owner = sim.interner.intern("Russians");
        let kind = sim.interner.intern("E1");
        let mut enemy = GameEntity::new_at_frame_zero_for_test(
            2,
            2,
            0,
            0,
            0,
            enemy_owner,
            Health::new(100),
            kind,
            EntityCategory::Infantry,
            0,
            5,
            true,
        );
        enemy.position.put_location(DriveCoord {
            x: 2 * 256 + 192,
            y: 64,
            z: 0,
        });
        enemy.set_sub_cell(Some(2));
        enemy.locomotor = Some(
            crate::sim::movement::locomotor::LocomotorState::for_test_kind(
                crate::rules::locomotor_type::LocomotorKind::Walk,
            ),
        );
        sim.substrate.entities.insert(enemy);
        sim.add_entity_occupancy(2);

        crate::sim::movement::answered_process::install();
        let turn = sim
            .tube_object_turn(1, Some(&rules), None, FrameEffects::default())
            .expect("tube turn");
        let processed = crate::sim::movement::answered_process::finish();
        assert!(!turn.per_cell_ran);
        let owner = sim.substrate.entities.get(1).unwrap();
        assert!(owner.low_bridge_tube_state.is_some());
        assert!(!owner.lifecycle.cell_marked());
        assert_eq!(owner.foot_speed.applied_fraction(), SIM_ZERO);
        assert!(!sim.substrate.occupancy.contains_entity(2, 0, 1));
        let enemy = sim.substrate.entities.get(2).unwrap();
        assert!(
            matches!(enemy.navigation.nav_com(), Some(NavTargetRef::Cell { .. })),
            "the enemy scattered"
        );
        assert_eq!(processed, [2], "its Scatter ran its locomotor's Process");
    }

    /// PlaceInfantryInCell skips a spot another infantryman's raw bit holds:
    /// with a friend on the spot its Location names, the infantryman leaves
    /// the tube at the cell's next free spot.
    #[test]
    fn a_friend_on_the_spot_sends_the_infantryman_to_the_next() {
        let (mut sim, rules) = infantry_at_tube_end();
        let owner = sim.interner.intern("Americans");
        let kind = sim.interner.intern("E1");
        let mut friend = GameEntity::new_at_frame_zero_for_test(
            2,
            2,
            0,
            0,
            0,
            owner,
            Health::new(100),
            kind,
            EntityCategory::Infantry,
            0,
            5,
            true,
        );
        let spot = DriveCoord {
            x: 2 * 256 + 64,
            y: 192,
            z: 0,
        };
        friend.position.put_location(spot);
        friend.set_sub_cell(Some(3));
        friend.locomotor = Some(
            crate::sim::movement::locomotor::LocomotorState::for_test_kind(
                crate::rules::locomotor_type::LocomotorKind::Walk,
            ),
        );
        sim.substrate.entities.insert(friend);
        // Mark lists the friend; its raw receiver marks the spot.
        sim.add_entity_occupancy(2);
        sim.object_raw_receiver_at(2, spot, true);

        assert_eq!(
            sim.infantry_tube_exit(1, Some(&rules), None, FrameEffects::default()),
            Ok(false)
        );
        let infantry = sim.substrate.entities.get(1).unwrap();
        assert_eq!(
            ground_pose::position_world_coord(&infantry.position),
            DriveCoord {
                x: 2 * 256 + 192,
                y: 192,
                z: 0
            }
        );
        assert_eq!(infantry.sub_cell(), Some(4));
        assert_eq!(
            sim.substrate.raw_cell_occupation.bits_at(
                crate::sim::occupancy::RawCellKey::Real(2, 0),
                MovementLayer::Ground
            ),
            1 << 3 | 1 << 4
        );
    }

    /// A NavCom naming the exit cell scatters the infantryman itself
    /// (`0x0051BA4B..0x0051BA6F`). Its Scatter runs the Walk Process at once
    /// (`0x0051D478`), which reads no tube (`+0x684`): only
    /// `InfantryClass::AI` runs TubeMovement (`0x0051BAC2`), so the exit
    /// places the infantryman once and marks one spot.
    #[test]
    fn the_self_scatter_does_not_run_the_exit_again() {
        let (mut sim, rules) = infantry_at_tube_end();
        crate::sim::arena_fixture::supply_native_map(&mut sim);
        sim.substrate
            .entities
            .get_mut(1)
            .unwrap()
            .navigation
            .set_nav_com_for_test(Some(NavTargetRef::Cell { rx: 2, ry: 0 }));

        let turn = sim
            .tube_object_turn(1, Some(&rules), None, FrameEffects::default())
            .expect("tube turn");
        assert!(turn.per_cell_ran);
        let infantry = sim.substrate.entities.get(1).unwrap();
        assert!(infantry.low_bridge_tube_state.is_none());
        assert_ne!(
            infantry.navigation.nav_com(),
            Some(NavTargetRef::Cell { rx: 2, ry: 0 }),
            "the Scatter gave it a new destination"
        );
        let listed: Vec<u16> = (0..=3)
            .filter(|&x| sim.substrate.occupancy.contains_entity(x, 0, 1))
            .collect();
        assert_eq!(listed, [2]);
        assert_eq!(
            sim.substrate.raw_cell_occupation.bits_at(
                crate::sim::occupancy::RawCellKey::Real(2, 0),
                MovementLayer::Ground
            ),
            1 << 3,
            "the exit marked the spot PlaceInfantryInCell picked, once"
        );
    }

    /// The exit does not test PlaceInfantryInCell's answer. An infantryman
    /// entering a vehicle that stands on the exit passes its `+0x1AC` (its
    /// direct target), but the vehicle's raw bit refuses every spot
    /// (`0x0048126B..0x0048128A`). The zeroed coordinate at `0x0089E778`
    /// then puts it at the floor at (0, 0), where Mark(DOWN) lists it.
    #[test]
    fn an_exit_without_a_spot_goes_to_the_map_origin() {
        let (mut sim, rules) = infantry_at_tube_end();
        let owner = sim.interner.intern("Americans");
        let kind = sim.interner.intern("APC");
        let mut vehicle = unit(3);
        vehicle.set_owner_for_test(owner);
        vehicle.set_type_ref_for_test(kind);
        vehicle.lifecycle.set_cell_marked(false);
        vehicle.position.put_location(DriveCoord::cell(2, 0, 0));
        sim.substrate.entities.insert(vehicle);
        sim.add_entity_occupancy(3);
        assert_ne!(
            sim.substrate.raw_cell_occupation.bits_at(
                crate::sim::occupancy::RawCellKey::Real(2, 0),
                MovementLayer::Ground
            ) & VEHICLE_OCCUPATION_BIT,
            0
        );
        let infantry = sim.substrate.entities.get_mut(1).unwrap();
        infantry.mission.set_current_for_test(
            crate::sim::mission::MissionId::from_known(crate::sim::mission::MissionType::Enter),
            0,
        );
        infantry
            .navigation
            .set_nav_com_for_test(Some(NavTargetRef::Entity { id: 3 }));

        assert_eq!(
            sim.infantry_tube_exit(1, Some(&rules), None, FrameEffects::default()),
            Ok(false)
        );
        let infantry = sim.substrate.entities.get(1).unwrap();
        assert_eq!(
            ground_pose::position_world_coord(&infantry.position),
            DriveCoord { x: 0, y: 0, z: 0 }
        );
        assert!(infantry.low_bridge_tube_state.is_none());
        assert!(sim.substrate.occupancy.contains_entity(0, 0, 1));
    }

    /// A step that reaches its target moves the unit through
    /// `SetLocation(target)` (vt+0x1B4 = `FootClass::SetLocation 0x004DB810`,
    /// `0x00735B58`), and the leftover step through `0x00735D27`; the
    /// `OpenTopped=` tail carries the riders along (`0x004DB88A` ->
    /// `0x007104F0`).
    #[test]
    fn an_open_topped_transport_carries_its_riders_through_a_tube() {
        use crate::rules::ini_parser::IniFile;
        use crate::sim::passenger::{PassengerCargo, PassengerRole};
        let rules = RuleSet::from_ini(&IniFile::from_str(
            "[VehicleTypes]\n0=BFRT\n[BFRT]\nSpeed=4\nOpenTopped=yes\nPassengers=5\n",
        ))
        .expect("rules");
        let terrain = explicit_terrain(vec![2, 2]);
        let mut transport = unit_of(1, "BFRT");
        // Ten leptons short of the first step's target, inside the budget.
        transport.position.set_rx_for_test(1);
        transport
            .position
            .set_sub_x_for_test(crate::util::fixed_math::SimFixed::from_num(118));
        transport.low_bridge_tube_state = Some(LowBridgeTubeMovementState {
            tube_id: TubeId(0),
            cursor: 0,
            target: DriveCoord::cell(1, 0, 0),
        });
        let mut cargo = PassengerCargo::new(5, 2);
        assert!(cargo.board(2, 1));
        transport.passenger_role = PassengerRole::transport(cargo);
        let mut rider = unit_of(2, "E1");
        rider.set_category_for_test(EntityCategory::Infantry);
        rider.position.set_rx_for_test(3);
        rider.passenger_role = PassengerRole::inside_for_test(1, true);
        let mut entities = EntityStore::new();
        entities.insert(transport);
        entities.insert(rider);
        assert_eq!(
            tick_active_tube_object(
                &mut entities,
                1,
                &terrain,
                &mut OccupancyGrid::new(),
                &mut CellOccupationGrid::new(),
                &mut RawCellOccupationGrid::new(),
                Some(&rules),
                &crate::sim::intern::test_interner(),
                21,
            ),
            TubeTurn::Moved
        );
        let [transport, rider] = [1, 2].map(|id| &entities.get(id).unwrap().position);
        let coord = |p: &Position| (ground_pose::position_world_xy(p), p.exact_z_leptons());
        // The leftover budget then steps on towards the next target as well.
        assert!(
            ground_pose::position_world_xy(transport)[0] >= 384,
            "the step reached its target"
        );
        assert_eq!(coord(rider), coord(transport), "the rider came along");
    }
}
