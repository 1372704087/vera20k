//! Unified entity struct replacing hecs ECS components.
//!
//! All 31 former ECS components are fields on `GameEntity`. Always-present
//! data is stored directly; optional/conditional components use `Option<T>`.
//! Zero-size markers (Selected, Repairing, VoxelModel/SpriteModel) become bools.
//!
//! ## Why plain structs?
//! - Deterministic iteration (sorted by stable_id) without per-query sorting
//! - Direct field access (`entity.position`) instead of `world.get::<&Position>(e)`
//! - No two-phase snapshot patterns needed for simple mutations
//! - Simpler borrow checker interactions than ECS archetype queries
//!
//! ## Dependency rules
//! - Part of sim/ — depends on map/ (EntityCategory), sim/components, sim/locomotor,
//!   sim/combat (TarCom), sim/animation, sim/miner, and special movement modules.
//! - sim/ NEVER depends on render/, ui/, audio/, net/.

use crate::map::entities::EntityCategory;
use crate::sim::animation::Animation;
use crate::sim::cloak_disguise::{CloakRuntime, DisguiseRuntime};
use crate::sim::combat::combat_weapon::WeaponSlot;
use crate::sim::combat::{Health, TarCom, TargetKind};
use crate::sim::components::{
    HarvestOverlay, NavigationState, PendingC4Detonation, Position, RockingState, VoxelAnimation,
};
use crate::sim::debug_event_log::{DebugEventKind, DebugEventLog};
use crate::sim::docking::aircraft_dock::AircraftAmmo;
use crate::sim::intern::InternedId;
use crate::sim::miner::Miner;
use crate::sim::mission::{MissionCom, MissionLeafState, MissionTimer, MissionType};
use crate::sim::movement::locomotor::LocomotorState;
use crate::sim::movement::tube_movement::LowBridgeTubeMovementState;
use crate::sim::passenger::PassengerRole;
use crate::sim::radio::Contacts;
use crate::sim::superweapon::invulnerability::InvulnerabilityState;
use crate::sim::timer::CdTimer;
use crate::sim::world::ObjectLifecycle;

mod foot_air;

/// ObjectClass's retained fall words: +2C, +84, +8D and +8F.
/// DropAsBomb5F4160 preserves the rate and parachute byte; landing5F3F86
/// clears only falling. Constructor5F3900 initializes all four to zero.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct ObjectFallState {
    rate: i32,
    parachuted: bool,
    falling: bool,
    dropped_as_bomb: bool,
}

/// Infantry-only runtime fear/prone state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InfantryRuntime {
    pub fear_level: u16,
    pub is_prone: bool,
    /// Countdown to this man's next idle fidget.
    ///
    /// Re-armed to a fresh random wait every time the idle action fires, and
    /// only then — the same one-shot timer gamemd keeps on the infantry object.
    /// Its default is the unarmed sentinel, which reads as already due, so a
    /// freshly built infantryman is eligible on his first idle turn.
    #[serde(default)]
    pub idle_action_timer: MissionTimer,
    /// Infantry+6DC. `InfantryClass` failed-path receiver `0x0051DAF0`
    /// (Infantry vtable +0x500, called from `FootClass::Find_Path` failure
    /// `0x004D4044` and `Do_Action` `0x0051D6F0` at zero health) writes the
    /// current-cell `Can_Enter_Cell` answer here: 1 when the answer is nonzero
    /// (`0x51DBBE`), otherwise the zero answer byte (`0x51DBAC`). Readers:
    /// Infantry Mission_Move restart `0x00520FF1` and save/load `0x00521D0F`.
    #[serde(default)]
    pub cell_entry_blocked: bool,
}

impl Default for InfantryRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl InfantryRuntime {
    pub fn new() -> Self {
        Self {
            fear_level: 0,
            is_prone: false,
            idle_action_timer: MissionTimer::default(),
            cell_entry_blocked: false,
        }
    }
}

/// The barrel elevation's rate: the TechnoClass constructor passes 3
/// (`PUSH 3` at `0x006F2EDE`) to `FacingClass(int) @ 0x004C91E0`.
const BARREL_ELEVATION_ROT: i32 = 3;

/// A level barrel: the draw pitches by `d32 - 8`, which is zero at `0x4000`.
pub(crate) const BARREL_LEVEL: u16 = 0x4000;

/// The elevation Unlimbo turns a barrel toward: level minus the low byte of
/// `FireAngle=` shifted up a byte (`MOV CH, byte [Type+0x3D0]` at
/// `0x006F6DD9`, `SUB` at `0x006F6DDF`), kept to a word.
pub(crate) fn unlimbo_barrel_target(fire_angle: i32) -> u16 {
    BARREL_LEVEL.wrapping_sub(u16::from(fire_angle as u8) << 8)
}

/// Persistent TechnoClass state used by the active House base-defence
/// responder. The two admission bytes are constructor-true; cooldown writes
/// occur only after a responder assignment or strict budget overshoot. The
/// ArchiveTarget (`+0x218`) stored here is the general Techno field, which
/// responders, miners, the slave manager and the rally click all write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct BaseDefenseResponseState {
    pub(crate) recruitable_a: bool,
    pub(crate) recruitable_b: bool,
    archive_target: Option<TargetKind>,
    /// `TechnoClass+0x650`/`+0x658`: the attacker's call-for-help cooldown
    /// (`TechnoClass::RespondToBaseAttack @ 0x00708171..0x00708192`, written
    /// at `0x0070879E..0x007087A9`); the constructor leaves it at `-1`, 0.
    pub(crate) cooldown: CdTimer,
}

impl Default for BaseDefenseResponseState {
    fn default() -> Self {
        Self {
            recruitable_a: true,
            recruitable_b: true,
            archive_target: None,
            cooldown: CdTimer::from_raw(-1, 0),
        }
    }
}

/// The unit side of the tank-bunker reciprocal link (the pre-install approach
/// state plus the installed link, folded into one hashed field). Distinct from
/// `PassengerRole` cargo: a bunker is a single reciprocal link, never cargo.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum BunkerLink {
    /// Not heading to or inside any bunker.
    #[default]
    None,
    /// Ordered into bunker `id`, still approaching (pre-install); the install
    /// machine resets once this marker no longer names its bunker. Cleared by
    /// the install and by either side's despawn
    /// (`bunker_link::break_links_on_despawn`); a retask leaves it
    /// (`sim::building_entry` residual).
    Approaching(u64),
    /// Installed inside bunker `id` (reciprocal of `building.bunker_occupant`).
    Installed(u64),
}

impl BunkerLink {
    /// The bunker this unit is installed in, if any.
    pub fn installed_in(self) -> Option<u64> {
        match self {
            BunkerLink::Installed(id) => Some(id),
            _ => None,
        }
    }
    /// The bunker this unit is approaching, if any.
    pub fn approaching(self) -> Option<u64> {
        match self {
            BunkerLink::Approaching(id) => Some(id),
            _ => None,
        }
    }
}

pub(crate) use crate::sim::gate_runtime::BuildingGateRuntime;

/// Accepted Psychedelic-warhead state owned by TechnoClass.
///
/// `active` mirrors the byte at native `Techno+0x298`; `timer` mirrors the
/// signed dword at `+0x29C`. The latter is the exact distance-zero damage
/// kernel result, so it deliberately remains signed rather than becoming a
/// Rust duration type.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct BerserkState {
    pub active: bool,
    pub timer: i32,
}

impl BerserkState {
    /// The Psychedelic arm's writes (`0x00701D6B..0x00701D7D`): the time,
    /// then the flag. Answers whether this hit turned the object berserk; a
    /// berserk object only has its time refreshed.
    pub fn start(&mut self, time: i32) -> bool {
        self.timer = time;
        !std::mem::replace(&mut self.active, true)
    }

    /// `TechnoClass::AI_Update`'s countdown (`0x006F9F0D..0x006F9F31`): a
    /// berserk object's time drops by one each frame. At zero or below the
    /// flag and the time clear, and the answer is true.
    pub fn count_down(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.timer = self.timer.wrapping_sub(1);
        if self.timer > 0 {
            return false;
        }
        *self = Self::default();
        true
    }
}

/// A building's delayed fire (`BuildingClass+0x704` mode, `+0x714`
/// countdown, `+0x708..+0x710` payload), armed by Mission_Attack and served by
/// ProcessDelayedFire (`0x004503F0`) when the countdown ends.
///
/// A shot captures no target: expiry reads the building's live
/// `attack_target`, with the weapon Mission_Attack saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PendingBuildingFire {
    /// Signed native timer value, clamped to zero by ProcessDelayedFire after
    /// its pre-decrement.
    pub remaining_ticks: i32,
    pub fire: DelayedFire,
}

/// What a delayed fire does when its countdown ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DelayedFire {
    /// Mode 1: FireAt the live target with the saved weapon (`+0x708`).
    Weapon(WeaponSlot),
    /// Mode 2: a Prism support beam (`0x0044ABD0`) to the master's weapon-0
    /// FLH as it stood at recruitment (`+0x708..+0x710`, world leptons).
    SupportBeam {
        to: crate::sim::projectile::ProjectileCoord,
    },
}

impl std::hash::Hash for PendingBuildingFire {
    /// A weapon shot folds as the pre-support latch did (countdown, then the
    /// slot); a support beam folds a third tag and its point.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.remaining_ticks.hash(state);
        match self.fire {
            DelayedFire::Weapon(slot) => slot.hash(state),
            DelayedFire::SupportBeam { to } => {
                2isize.hash(state);
                to.hash(state);
            }
        }
    }
}

/// Constructor state captured while a generated-map Techno still owns the
/// launch-time Scenario cursor. Projection validates all identity fields
/// before installing the already-consumed word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedTechnoInit {
    pub entity_index: usize,
    pub techno_type: String,
    pub cell: (u16, u16),
    pub techno_ctor_random_word: u16,
    pub native_unique_id: i32,
}

/// The two evidence-backed ways a live Techno obtains its persistent
/// constructor word. Only `FreshScenario` is allowed to advance Scenario RNG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TechnoConstructorInit {
    FreshScenario,
    PreconsumedGenerated(GeneratedTechnoInit),
}

/// Authored Building upgrades construct as distinct Technos, then Unlimbo at
/// their host location. The host/slot association is persistent identity; the
/// upgrade does not own a competing building footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StructureUpgradeLink {
    pub parent_stable_id: u64,
    pub slot: u8,
}

/// Client-relative Techno discovery history, native +41A/+41B/+41C.
/// Constructor6F2FB5..6F2FC1 clears all three; InitManagers6F3F40 and
/// ChangeOwner70173B classify only the current owner. Discovery6F4960 and
/// Conceal6F4A40 own the two retained observations. Infantry raw Save/Load
/// preserves the bytes: never reconstruct them from the restored owner.
/// Native peer CRC64DAB0 omits them; this is also omitted from the shared
/// Rust peer hash. The separate diagnostic CRC70C270 is not that checksum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TechnoDiscoveryHistory {
    pub owned_by_current_house: bool,
    pub discovered_by_current_house: bool,
    /// A single historical byte for all other receiver houses, not a house set.
    pub discovered_by_other_house: bool,
}

/// Every game object (unit, infantry, building, aircraft) is one `GameEntity`.
/// Core fields are always present; optional subsystems use `Option<T>`. A
/// mechanism's own state is a record its module defines and alone writes
/// (for example `building_art`, `veterancy`); other code reads it through
/// getters and changes it through that module's functions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameEntity {
    // --- Always present (every entity has these) ---
    // The object's identity (stable id, native UniqueID, the constructor's
    // Scenario word, owner, type and class) is private to this module:
    // readers use the getters, construction and decoding establish it, and
    // only the owner-change path below moves the owner.
    /// Deterministic stable ID — primary key, used for cross-entity references,
    /// replay logs, state hashing, and networking. Never reused.
    stable_id: u64,
    /// AbstractClass+10, assigned by the concrete constructor after the base
    /// Techno Scenario word (Unit735454 / Building43BA15). May wrap or duplicate;
    /// stable handles remain the reference and storage authority.
    native_unique_id: i32,
    /// Native Techno+24C float bits, initialized to zero by6F2D62 and
    /// preserved by native persistence (70C32A). Only visual-character4 reads
    /// this displacement through70BE50; no simulation/RNG/timer consumer.
    /// Retain the raw datum, not a renderer-owned reconstructed animation.
    native_cloak_displacement: crate::util::native_x87::NativeF32Bits,
    /// Low word of the one raw Scenario RNG draw performed by the active-retail
    /// `TechnoClass` constructor (`0x006F3254`, stored at native `+0x3C8`).
    /// Later report-selection consumers read this persistent value; placement
    /// failure never refunds the draw.
    techno_ctor_random_word: u16,
    /// One native Techno stage clock. Class receivers mutate through this
    /// owner; animation, miner and buildup components carry no competing copy.
    stage: crate::sim::stage::StageClass,
    pub discovery: TechnoDiscoveryHistory,
    /// Authored structure-upgrade identity. `None` for ordinary Technos.
    pub structure_upgrade_link: Option<StructureUpgradeLink>,
    /// World position in isometric cell coordinates + cached screen position.
    pub position: Position,
    /// The body heading, `TechnoClass+0x388` (PrimaryFacing), and its only
    /// copy: every class has one (the TechnoClass constructor builds it at
    /// `0x006F2EEB`), a building included — its `+0x388` is what its turret
    /// or its body aims. 16-bit DirStruct, 0 = north on screen, 0x4000 = east.
    ///
    /// Readers sample `current(frame)` (`FacingClass::Current @ 0x004C93D0`),
    /// writers turn it (`Set @ 0x004C9220`) or snap it (`Set_Current @
    /// 0x004C9300`). Its rate is written once, by the class constructor
    /// ([`Self::set_body_facing_rot`]). The image's only other `+0x39C` write
    /// is AircraftClass::AI's Carryall copy (`0x00415146..0x00415184`, the
    /// carrier's `+0x3A0` over its cargo's `+0x388` and `+0x3A0`), gated on
    /// AircraftType `+0xDFC` (`Carryall=`, read at `0x0041CCA2`). Retail sets
    /// that key only on `[HIND]`, a VehicleType, so the copy is dormant and not
    /// ported.
    pub body_facing: crate::sim::movement::FacingClass,
    /// The barrel elevation, `TechnoClass+0x370`, the first of the three
    /// FacingClasses (the body is `+0x388`, the turret `+0x3A0`,
    /// [`Self::barrel_facing`]). The TechnoClass constructor builds it at 0
    /// turning at rate 3 (`0x006F2EDE..0x006F2EE6`, `0x004C91E0`), and
    /// [`Self::unlimbo_barrel_elevation`] levels and aims it. `UnitClass::DrawVoxelBody`
    /// pitches the barrel voxel by it (`0x0073BB77..0x0073BBAC`); nothing in
    /// a unit's simulation reads it.
    ///
    /// The building writers (the constructor's `Set`s at `0x0043BA5E` and
    /// `0x0043BACB`, Sell's `Set_Current` at `0x0044A08C`, Mission_Missile's
    /// `Set`s at `0x0044CEE2` and `0x0044CF7A`, an MCV deploy's `Set_Current`
    /// at `0x00739827`) and their readers are not ported.
    barrel_elevation: crate::sim::movement::FacingClass,
    /// The body-animation counter (`FootClass+0x538`), a
    /// [`BodyFrameCounter`] that `sim::animation` owns.
    ///
    /// [`BodyFrameCounter`]: crate::sim::animation::BodyFrameCounter
    #[serde(default)]
    pub(crate) body_frame_counter: crate::sim::animation::BodyFrameCounter,
    /// The owning house (`TechnoClass+0x21C`), interned.
    /// `TechnoClass::ChangeOwner @ 0x007014A0` moves it; here only the store's
    /// owner-change path does ([`Self::set_owner_from_store`]).
    owner: InternedId,
    /// The object's health (`ObjectClass+0x6C`), a [`Health`] that
    /// `combat::object_health` owns.
    pub(crate) health: Health,
    /// Object+70: signed damage reservations/recovery, not an actual-HP cache.
    /// Native constructors and map admission seed this from admitted health.
    pub(crate) estimated_health: crate::sim::estimated_health::EstimatedHealth,
    /// rules.ini section name (e.g., "HTNK", "E1", "GAPOWR") — interned for zero-cost clones.
    type_ref: InternedId,
    /// The concrete class (`AbstractClass::What_Am_I`): Unit, Infantry,
    /// Aircraft or Structure. Fixed at construction.
    category: EntityCategory,
    /// Facts the object's type fixes (its foundation and a building's base
    /// and cell-list inputs), a [`TypeFacts`] that construction stamps.
    ///
    /// [`TypeFacts`]: crate::sim::world::TypeFacts
    pub(crate) type_facts: crate::sim::world::TypeFacts,
    /// The object's veterancy: the running accumulator every rank is sampled
    /// from, and the rank last announced. `combat::veterancy` owns it.
    #[serde(default)]
    pub(crate) veterancy: crate::sim::combat::veterancy::Veterancy,
    /// The flash countdown (`TechnoClass+0xF0`) the elite promotion, a
    /// building's Flash and team script action 50 seed. `sim::flashing` owns
    /// it.
    #[serde(default)]
    pub(crate) flashing: crate::sim::flashing::Flashing,
    /// The armor and firepower crate multipliers (`Techno+0x158`, `+0x160`),
    /// a [`CrateMultipliers`] that the crate arms own.
    ///
    /// [`CrateMultipliers`]: crate::sim::crates::CrateMultipliers
    #[serde(default)]
    pub(crate) crate_multipliers: crate::sim::crates::CrateMultipliers,
    /// The type facts the house counts and the score test (`DontScore=` and
    /// the rest), a [`TrackingFacts`](crate::sim::house_tracking::TrackingFacts)
    /// that construction stamps.
    #[serde(default)]
    pub(crate) tracking_facts: crate::sim::house_tracking::TrackingFacts,
    /// Native infantry total-count latches (`Techno+0x438/+0x439`), owned by
    /// `sim::house_tracking`. The constructor starts both clear; Add_Tracking
    /// sets the once-counted latch when it adds House+0x2F4.
    #[serde(default)]
    house_tracking_latches: crate::sim::house_tracking::InfantryTrackingLatches,
    /// The fog sight range and the zero-`Sight=` fact, a [`SightFacts`] that
    /// `sim::vision` owns.
    ///
    /// [`SightFacts`]: crate::sim::vision::SightFacts
    pub(crate) sight: crate::sim::vision::SightFacts,
    /// Foot65C/664 high-flying sight refresh timer, virtualized per viewer.
    /// Independent of the retained sight-admission latch and stored footprint.
    pub(crate) sight_refresh_timers: crate::sim::vision::SightRefreshTimers,
    /// Building43FB20 operational edge and Techno6FB170/6FB470 deposit latch.
    pub(crate) gap_generator: crate::sim::vision::GapGeneratorRuntime,

    // --- Render model (mutually exclusive) ---
    /// True = VXL/HVA model, false = SHP sprite; effective art metadata is
    /// authoritative. Only the constructor writes it ([`Self::is_voxel`]).
    is_voxel: bool,

    // --- Bool markers (were zero-size ECS components) ---
    /// Committed ObjectClass selection membership in simulation state.
    /// `Command::Select` updates it through `apply_selection_snapshot`; lifecycle,
    /// transport and combat paths also clear or restore it. Combat reads it for
    /// selected-unit effects and parasite reselect. The app keeps input order and
    /// pending commands separately, then reconciles that ledger after the frame.
    pub selected: bool,
    /// Offline for want of a powering building (`TechnoClass+0x1C8`), a
    /// [`Deactivation`] that `sim::powered_unit` owns.
    ///
    /// [`Deactivation`]: crate::sim::powered_unit::Deactivation
    #[serde(default)]
    pub(crate) deactivated: crate::sim::powered_unit::Deactivation,
    /// A building's repair byte (`BuildingClass+0x6E8`), a [`RepairByte`]
    /// that `production::production_repair` owns.
    ///
    /// [`RepairByte`]: crate::sim::production::RepairByte
    pub(crate) repairing: crate::sim::production::RepairByte,
    /// LogicClass active-vector membership — mirrors gamemd ObjectClass+0x98.
    /// True iff this entity is currently in `Simulation::logic`. Not serialized:
    /// Rust snapshots rebuild it from the serialized LogicVector order. Exact
    /// native save/load reconstruction remains unverified.
    #[serde(skip)]
    pub in_logic_vector: bool,
    /// Independent, serialized ObjectClass lifecycle facts, an
    /// [`ObjectLifecycle`] that `world::lifecycle` owns.
    #[serde(default)]
    pub(crate) lifecycle: ObjectLifecycle,
    /// Canonical TechnoClass playfield-membership byte (`TechnoClass+0x3D5`).
    ///
    /// gamemd-derived: the constructor clears it at `0x006F2F5B`, Unlimbo
    /// establishes an exact mode-one result at `0x006F6CFE`, ordinary cell
    /// movement promotes false to true without normally demoting it at
    /// `0x006F511A..0x006F5139` (the Teleport warp's PerCell(2) included), the
    /// Chronosphere warp's arrival state clears it outside the playfield at
    /// `0x00719A99` (`movement::teleport_chrono`), and
    /// `MapClass::Set_Clipped_LocalSize @ 0x00567230` recomputes every
    /// Techno exactly after a LocalSize writer. Consumers must read this stored
    /// fact; a fresh bounds query would erase the native movement hysteresis.
    #[serde(default)]
    pub in_playfield: bool,
    /// Retained Techno+3D4 (VERA name; historical native name unconfirmed).
    /// Constructor6F2F55 clears it; special Aircraft Unlimbo4143EB and
    /// reinforcement65E6BE promote it. Never infer it from current type,
    /// mission, cargo or +3D5. Ordinary click action692766 reads this history;
    /// forced ObjectSelect5F4578 has a separate virtual+A0 prerequisite.
    #[serde(default)]
    mission_only: bool,
    /// The type facts the removal paths read (Limbo's dirty-rect branch and
    /// `Occupier=`), a [`RemovalFacts`] that `world::lifecycle` owns.
    ///
    /// [`RemovalFacts`]: crate::sim::world::RemovalFacts
    #[serde(default)]
    pub(crate) removal_facts: crate::sim::world::RemovalFacts,
    /// The kill credit and the UnInit loss guard, a [`KillRecord`] that
    /// `combat::record_kill` owns.
    ///
    /// [`KillRecord`]: crate::sim::combat::KillRecord
    #[serde(default)]
    pub(crate) kill_record: crate::sim::combat::KillRecord,
    /// Foot's independent tracker Cell, slot-notification Cell and retained
    /// airborne-vector membership. Only the shared Foot air owner writes them.
    #[serde(default)]
    foot_air: foot_air::FootAirState,

    // --- Optional subsystem components ---
    /// Locomotor state — present on moving types and on zero-speed Foot
    /// Drive/Ship types whose native class-local payload still exists.
    pub locomotor: Option<LocomotorState>,
    /// FootClass-style owner navigation destination state.
    ///
    /// Native `NavCom` is distinct from the active execution path, so this can
    /// remain visible after a DriveTrack segment has cleared.
    #[serde(default)]
    pub navigation: NavigationState,
    /// Live Foot-owned applied speed; survives active locomotor replacement.
    #[serde(default)]
    pub foot_speed: crate::sim::components::FootSpeedState,
    /// Techno+2E8, ctor6F2E00 zero, saved70C354. Fly approach writes it;
    /// landing and Aircraft unload consume it. Independent of body rocking
    /// and of the active/suspended locomotor. Deterministic fixed radians.
    #[serde(default)]
    pub(crate) flight_attitude: crate::sim::movement::fly_height::FlightAttitude,
    /// Foot+6B6 raw occupation enable; shared by Drive/Ship and world Mark.
    /// Foot ctor4D344A initializes1; Unit7353CE invokes that base ctor.
    pub(crate) foot_occupation_enabled: bool,
    /// Foot+6AD, constructor `0x004D3414` zero: a Magnetron lifted this
    /// object. `TechnoClass::ImbueLocomotor @ 0x00710000` raises it
    /// (`0x00710352`) and only Jumpjet `End_Piggyback @ 0x0054DA50` clears it
    /// ([`crate::sim::locomotor_lift`]). Independent of MCV Unit+68C and
    /// infantry deploy animation.
    #[serde(default)]
    pub(crate) foot_locomotor_swap_active: bool,
    /// Foot+6AE, constructor `0x004D341A` zero: the lift may end. A release
    /// on the ground (`0x0070FFBB`), Foot's null destination with a holder
    /// (`0x004D9538`) and every Jumpjet touchdown (`0x0054C9FE`) raise it;
    /// Jumpjet `Is_Ok_To_End @ 0x0054DB00` reads it and `End_Piggyback`
    /// clears it.
    #[serde(default)]
    pub(crate) lift_may_end: bool,
    /// Techno+2AC LocomotorTarget: the object this Magnetron holds aloft
    /// (`0x00710307`); its +2B0 names this object back.
    #[serde(default)]
    pub(crate) locomotor_target: Option<u64>,
    /// Techno+2B0 LocomotorSource: the Magnetron holding this object
    /// (`0x00710318`).
    #[serde(default)]
    pub(crate) locomotor_source: Option<u64>,
    /// Techno+427 IsBeingManipulated: a Magnetron let this object go in the
    /// air (`0x0070FF32`), so its Jumpjet State 5 lands it on whatever is
    /// below (`0x0054CC39`). Constructor `0x006F3005` zero; every Jumpjet
    /// touchdown clears it (`0x0054CA08`).
    #[serde(default)]
    pub(crate) being_manipulated: bool,
    /// Techno+428 BeingManipulatedBy: the Magnetron that dropped this object
    /// (`0x0070FF3F`), credited with what its fall kills. Cleared by the
    /// fall's end, `End_Piggyback` (Jumpjet and Teleport) and the Chrono
    /// Warp's arrival; pointer expiry clears it (`0x0070783B`).
    #[serde(default)]
    pub(crate) manipulated_by: Option<u64>,
    /// Techno+42C ChronoWarpedByHouse: the house a Chronosphere warp
    /// (`0x006CCC67`) or a Magnetron drop (`0x0070FF51`) credits with this
    /// object's death on landing; cleared with +428.
    #[serde(default)]
    pub(crate) warped_by_house: Option<InternedId>,
    /// Techno+0x1F8, constructor zero (`0x006F2CE1`). The Unit setter's
    /// Teleporter arm raises it when a Drive cannot end yet (`0x007425C6`), so
    /// the next setter call runs for an unchanged NavCom (`0x00741A88`); every
    /// setter call that passes that check clears it (`0x00741A9C`). Unlimbo's
    /// own raise and clear (`0x006F6E1B`/`0x006F6E34`) bracket one call and
    /// leave nothing behind.
    #[serde(default)]
    pub(crate) setter_force_reassign: bool,
    /// Techno+0x284, constructor zero (`0x006F2DDE`): the warp-in the
    /// Chronosphere's Teleport states time (`0x00719B23`). A blocked landing
    /// writes it (`[General] ChronoDelay=`, `0x00719983`) and it stays for
    /// every later warp of the object; its other writer, the chrono
    /// reinforcement (`0x0065F212`, `ChronoReinfDelay=`), is not ported.
    #[serde(default)]
    chrono_warp_delay: i32,
    /// The object's target (`TechnoClass+0x2B4`), a [`TarCom`] that the class
    /// target setter (`mission::concrete_effects`) writes.
    pub(crate) attack_target: TarCom,
    /// A building's delayed fire: a delayed shot, or a Prism support beam.
    #[serde(default)]
    pub pending_building_fire: Option<PendingBuildingFire>,
    /// The Prism master's recruited supporters (`BuildingClass+0x664`), a
    /// [`PrismSupport`] that `techno_ai::building_missions` owns.
    ///
    /// [`PrismSupport`]: crate::sim::world::PrismSupport
    #[serde(default)]
    pub(crate) prism_support_count: crate::sim::world::PrismSupport,
    /// Techno+138, constructor6F2B99..6F2BCA=0; SetGunnerWeapon70DC70 owns writes.
    /// Combat selection and turret facing read the same signed weapon number.
    current_weapon_number: i32,
    /// Techno+124, constructor6F2B99..6F2BCA=-1. SetGunnerWeapon70DC70 and
    /// charge AI6FA4FB own writes; indexed voxel drawing is its consumer.
    current_turret_index: i32,
    /// Techno+2F8, constructor6F2E98=0. FireAt saves the final rearm duration
    /// independently of later timer writers; charge AI reads it for drawing.
    charge_turret_delay: i32,
    /// RadioClass-style live contacts for this entity, stored as stable IDs.
    /// Used by runtime building-entry/pathing exceptions such as contacted
    /// war factory exits and refinery dock entry. Kept per mover; a building
    /// being contacted does not globally relax passability for unrelated units.
    /// `sim::radio` owns the slots (`RadioClass+0xE4`/`+0xE8`).
    #[serde(default)]
    pub(crate) radio_contacts: Contacts,
    /// The tether latch (`TechnoClass+0x418`), a [`Tether`] that `sim::radio`
    /// owns.
    ///
    /// [`Tether`]: crate::sim::radio::Tether
    #[serde(default)]
    pub(crate) tether: crate::sim::radio::Tether,
    /// The computer's sale and repair inputs (`TechnoClass+0x3D1`,
    /// `BuildingClass+0x6DC`, `+0x6CB`), an [`AiSellRepair`] that
    /// `production::production_repair` owns.
    ///
    /// [`AiSellRepair`]: crate::sim::production::AiSellRepair
    #[serde(default)]
    pub(crate) ai_sell_repair: crate::sim::production::AiSellRepair,
    /// The turret heading, `TechnoClass+0x3A0` (SecondaryFacing), a
    /// [`FacingClass`] that moves only through its own methods. VERA keeps
    /// one for a `Turret=yes` unit and every aircraft: construction installs
    /// it (`world_spawn::construction`) and a Fly order installs it on an
    /// aircraft built without one (`air_movement::ensure_fly_secondary_facing`).
    /// A building aims its [`Self::body_facing`] instead.
    ///
    /// [`FacingClass`]: crate::sim::movement::FacingClass
    pub(crate) barrel_facing: Option<crate::sim::movement::FacingClass>,
    /// The turret rotation latch (`UnitClass+0x6AF`), a
    /// [`TurretRotationLatch`] that `world::unit_post` owns.
    ///
    /// [`TurretRotationLatch`]: crate::sim::world::unit_post::TurretRotationLatch
    #[serde(default)]
    pub(crate) turret_rotation_latch: crate::sim::world::unit_post::TurretRotationLatch,
    /// The frame of the object's own last shot (`TechnoClass+0x120`), a
    /// [`LastFireFrame`] that FireAt (`combat::world_fire`) writes.
    ///
    /// [`LastFireFrame`]: crate::sim::combat::world_fire::LastFireFrame
    #[serde(default)]
    pub(crate) last_fire_frame: crate::sim::combat::world_fire::LastFireFrame,
    /// The turret rotation-sound latch (`TechnoClass+0x49C`), a
    /// [`TurretRotateLatch`] that `world::turret_rotate_sound` owns.
    ///
    /// [`TurretRotateLatch`]: crate::sim::world::TurretRotateLatch
    #[serde(default)]
    pub(crate) turret_rotate_latch: crate::sim::world::TurretRotateLatch,
    /// Techno+4F8: one ordinary VoiceDeploy attempt is suppressed after a
    /// SlaveManager relocation (6B01C4). Constructor6F30FA starts false.
    /// The Building resource-Archive voice459C20 bypasses both the test and
    /// clear; common VoiceDeploy708E00 clears it even when the slot is absent.
    /// This retained control history is distinct from the audio pending line.
    #[serde(default)]
    suppress_deploy_voice: bool,
    /// The rearm countdown (`TechnoClass+0x2EC`), a [`RearmTimer`] whose
    /// writers `combat::rof` documents.
    ///
    /// [`RearmTimer`]: crate::sim::combat::rof::RearmTimer
    #[serde(default)]
    pub(crate) rearm_timer: crate::sim::combat::rof::RearmTimer,
    /// Gattling stage, value and report latch (`TechnoClass+0x140`,
    /// `+0x144`, `+0x4B8`), owned by `combat::gattling`.
    #[serde(default)]
    pub(crate) gattling: crate::sim::combat::gattling::GattlingState,
    /// The voxel turret's animation counter (`TechnoClass+0x148`), a
    /// [`TurretAnimCounter`] that `combat::gattling` owns.
    ///
    /// [`TurretAnimCounter`]: crate::sim::combat::gattling::TurretAnimCounter
    #[serde(default)]
    pub(crate) turret_anim_frame: crate::sim::combat::gattling::TurretAnimCounter,
    /// Optional presentation-only native recoil, owned by voxel_recoil.
    voxel_recoil: Option<Box<voxel_recoil::VoxelRecoil>>,
    /// Techno+3B8, the burst index, owned by `combat::burst`; it survives
    /// target replacement and mission changes.
    #[serde(default)]
    pub(crate) weapon_burst: crate::sim::combat::burst::WeaponBurst,
    /// Sole native Building+534/+538 body state; construction/sale use the
    /// existing private StageClass and MissionLeaf ready byte.
    building_body: Option<crate::sim::building_construction::BuildingBody>,
    /// Building+544: actual health sampled by440042..440074. Constructor
    /// 43B78F and successful ordinary Unlimbo440D2D reset it to zero.
    /// A repair changes actual health without invalidating its House. The
    /// next sample does invalidate it; any other native dirty writer can
    /// expose that live health through an earlier House assessment.
    building_power_health_sample: i32,
    /// Sale route metadata, never a second mission, ready byte or timer.
    building_sale: Option<crate::sim::building_construction::BuildingDown>,
    /// `BuildingClass+0x550..+0x558`, the wait before the building's
    /// computer factory tries its finished object again. The constructor
    /// starts it with no duration (`0x0043B7A7`, `0x0043B7AD`); a placement
    /// that must wait restarts it for `[General] PlacementDelay=`
    /// (`0x004501CB..0x004501F7`); only `production::factory_ai` reads it.
    #[serde(default)]
    pub(crate) ai_placement_timer: crate::sim::timer::CdTimer,
    /// The Building's animation art: its AnimClass slots, their replay
    /// marks, the damaged-art flag, its StorageClass art, the Stuff flag and
    /// the Grand_Opening guard. `sim::building_art` owns it.
    #[serde(default)]
    pub(crate) building_art: crate::sim::building_art::BuildingArt,
    /// The last operational sample (`BuildingClass+0x6C8`), a
    /// [`LastOperational`] that `world::gap_generator` owns.
    ///
    /// [`LastOperational`]: crate::sim::world::LastOperational
    #[serde(default)]
    pub(crate) building_last_operational: crate::sim::world::LastOperational,
    /// A `PowersUnit=` building's share in its house's powered centers
    /// (`BuildingClass+0x662`), a [`PoweredCenter`] that `sim::powered_unit`
    /// owns.
    ///
    /// [`PoweredCenter`]: crate::sim::powered_unit::PoweredCenter
    #[serde(default)]
    pub(crate) building_powered_center: crate::sim::powered_unit::PoweredCenter,
    /// HasEngineer (`BuildingClass+0x6CC`), a [`HasEngineer`] that
    /// `sim::building_art` owns.
    ///
    /// [`HasEngineer`]: crate::sim::building_art::HasEngineer
    #[serde(default)]
    pub(crate) building_has_engineer: crate::sim::building_art::HasEngineer,
    /// The `HasSpotlight=` fact and the owned BuildingLight, a
    /// [`BuildingLight`] that `world::lifecycle` owns.
    ///
    /// [`BuildingLight`]: crate::sim::world::BuildingLight
    #[serde(default)]
    pub(crate) building_light: crate::sim::world::BuildingLight,
    /// The damage-fire transition cache and its eight AnimClass slots
    /// (`BuildingClass+0x5E8`, `+0x5C8..+0x5E4`), a [`DamageFires`] that
    /// `sim::anim_class` owns.
    ///
    /// [`DamageFires`]: crate::sim::anim_class::DamageFires
    #[serde(default)]
    pub(crate) damage_fires: crate::sim::anim_class::DamageFires,
    /// Persistent bridge layer flag — authoritative source for "is this entity on a bridge?"
    /// Mirrors original engine's FootClass+0x8C. Survives repath operations that reset
    /// locomotor.layer. Set during spawn, updated at cell-crossing bridge transitions.
    #[serde(default)]
    pub on_bridge: bool,
    /// Runtime-only Foot bridge mismatch latch (`FootClass+0x68b` in YR).
    #[serde(default)]
    pub(crate) runtime_bridge_transition:
        crate::sim::movement::movement_bridge::RuntimeBridgeTransitionState,
    /// Infantry sprite animation state (sequence + frame + timing).
    pub animation: Option<Animation>,
    /// Voxel HVA animation state (frame cycling for multi-frame models).
    pub voxel_animation: Option<VoxelAnimation>,
    /// Harvest overlay animation (oregath.shp ore-gathering visual).
    pub harvest_overlay: Option<HarvestOverlay>,
    /// Harvester state machine (ore collection, refinery docking, cargo).
    pub miner: Option<Miner>,
    /// `SlaveManagerClass` of an `Enslaves=` master (`TechnoClass+0x2D8`).
    #[serde(default)]
    pub(crate) slave_manager: Option<crate::sim::slave_manager::SlaveManager>,
    /// The slave side of `SlaveManagerClass`: the master whose manager holds
    /// this slave (`TechnoClass+0x2DC`) and its Storage (`+0x33C`), written
    /// only by `sim::slave_manager`.
    #[serde(default)]
    pub(crate) slave: crate::sim::slave_manager::SlaveLink,
    /// The Foot's MegaMission order, the attack-move (`FootClass +0x5C4..+0x5D1`),
    /// owned by `sim::mission::mega_mission`.
    #[serde(default)]
    pub(crate) mega_mission: crate::sim::mission::mega_mission::MegaMission,
    /// Evidence-bounded native cloak transition state and visual producer values.
    #[serde(default)]
    pub cloak: Option<CloakRuntime>,
    /// Last sensor coverage actually deposited into CellClass counters.
    /// Removal must use these cached owner/location/radius values after the
    /// entity's current facts have changed.
    #[serde(default)]
    pub sensor_deposit: Option<crate::sim::sensor_lifecycle::SensorDeposit>,
    /// Core disguise identity, timestamp, and reveal tuple, owned by
    /// `cloak_disguise`.
    #[serde(default)]
    pub(crate) disguise: Option<DisguiseRuntime>,
    /// Active low-bridge TubeClass movement. Active YR behaviour — not to be
    /// confused with the subterranean tunnel locomotor, which is Tiberian Sun
    /// legacy and was removed as unreachable in stock YR.
    #[serde(default)]
    pub low_bridge_tube_state: Option<LowBridgeTubeMovementState>,
    /// Controller-owned reversible mind-control manager (`TechnoClass+0x2BC`),
    /// owned by `capture_manager`. Capacity and ordered victim links are
    /// authoritative runtime state; they cannot be reconstructed from the
    /// victims' back-links.
    #[serde(default)]
    pub(crate) capture_manager: Option<crate::sim::capture_manager::CaptureManagerState>,
    /// Spawn-manager pool carried by a `Spawns=` parent (V3 Launcher,
    /// Dreadnought, Boomer, Aircraft Carrier, Destroyer). Mirrors the native
    /// `TechnoClass+0x2D0` manager pointer: present iff `Spawns=` resolved.
    #[serde(default)]
    pub spawn_manager: Option<crate::sim::spawn_manager::SpawnManagerState>,
    /// A spawned child's SpawnOwner (`TechnoClass+0x2D4`), a [`SpawnOwner`]
    /// that `spawn_manager` owns.
    ///
    /// [`SpawnOwner`]: crate::sim::spawn_manager::SpawnOwner
    #[serde(default)]
    pub(crate) spawn_owner_id: crate::sim::spawn_manager::SpawnOwner,
    /// Object-level fall state, shared by Paradrop and DropAsBomb.
    #[serde(default)]
    fall: ObjectFallState,
    /// The IronCurtain or ForceShield state: the curtain's timer and its tint
    /// stage. `None` until the first curtain; `Some` after it, nullifying all
    /// damage (except healing) while the timer runs ([`is_invulnerable`]).
    /// Applied by superweapon launch handlers.
    ///
    /// [`is_invulnerable`]: crate::sim::superweapon::invulnerability::is_invulnerable
    #[serde(default)]
    pub invulnerability: Option<InvulnerabilityState>,
    /// The victim side of mind control (`TechnoClass+0x2C0` MindControlledBy,
    /// `+0x2C8` the ring anim), written only by `capture_manager`.
    #[serde(default)]
    pub(crate) mind_control: crate::sim::capture_manager::MindControlLink,
    /// TemporalClass state (`TechnoClass+0x274` TemporalImUsing and `+0x278`
    /// TemporalTargetingMe), written only by `temporal`.
    #[serde(default)]
    pub(crate) temporal: crate::sim::temporal::TemporalState,
    /// The Crazy Ivan bomb it carries (`ObjectClass+0x38` and its
    /// `BombClass`), written only by `bomb`.
    #[serde(default)]
    pub(crate) bomb: Option<crate::sim::bomb::Bomb>,
    /// Psychedelic/chaos runtime, separate from reversible mind control.
    #[serde(default)]
    pub berserk: BerserkState,
    /// Native Foot/Unit+500 entry target, independent of Radio contacts.
    /// Unit741D9F installs it; Foot70D84F/70D889 and expiry clear it.
    pending_entry: Option<u64>,
    /// Foot `+0x68F..+0x691`, cleared by the Foot constructor
    /// (`0x004D33D2..0x004D33DE`) and written only by `sim::building_entry`.
    #[serde(default)]
    pub(crate) building_entry: crate::sim::building_entry::BuildingEntryLatches,
    /// An aircraft's Ammo (`TechnoClass+0x2FC`), pending release and dock,
    /// an [`AircraftAmmo`] that `docking::aircraft_dock` owns. Present on all
    /// Aircraft, including signed negative native Ammo counts.
    pub(crate) aircraft_ammo: Option<AircraftAmmo>,
    /// Infantry sub-cell position (0–4). Only meaningful for infantry.
    pub sub_cell: Option<u8>,
    /// `Crushable=` and `DeployedCrushable=`, a [`CrushableFacts`] that
    /// construction stamps.
    ///
    /// [`CrushableFacts`]: crate::sim::world::CrushableFacts
    pub(crate) crushable_facts: crate::sim::world::CrushableFacts,
    /// Techno+2A4, cleared by construction and retained independently of
    /// Doing. Infantry520B4E/520BAD update it after their Do_Action request
    /// even when the requested Deployed/Ready sequence refuses.
    #[serde(default)]
    native_crush_immunity: u8,
    /// `OmniCrusher=`, `Crusher=`, `Accelerates=`, `OmniCrushResistant=` and
    /// `ImmuneToRadiation=`, a [`MoverFacts`] that construction stamps.
    ///
    /// [`MoverFacts`]: crate::sim::world::MoverFacts
    pub(crate) mover_facts: crate::sim::world::MoverFacts,
    /// The terminal lifetime (the dying mark and the Infantry terminal
    /// policy), a [`TerminalLifetime`] that `world::infantry_terminal` owns.
    /// Products derived from the entities (the movement pass's blocker plane
    /// and block sets) follow it through the store's touch log, like any
    /// other field.
    ///
    /// [`TerminalLifetime`]: crate::sim::world::TerminalLifetime
    pub(crate) death: crate::sim::world::TerminalLifetime,
    /// Foot+53C/+540 latch and countdown. The shared Foot sound owner alone
    /// writes these; the process-local audio handle is not simulation state.
    #[serde(default)]
    pub(crate) move_sound: crate::sim::world::MoveSoundState,
    /// The crash latch (`FootClass+0x425`/`+0x426`), a [`CrashLatch`] that
    /// `world::crash` owns.
    #[serde(default)]
    pub(crate) crash: crate::sim::world::CrashLatch,
    /// Techno+3CD/+3CE retained sinking lifetime and prior Foot AI sound edge.
    /// `world::sinking` owns mutations; this is independent of crash +425/+426.
    #[serde(default)]
    pub(crate) sinking: crate::sim::world::SinkingState,

    // --- Passenger/transport system ---
    /// The passenger relation and cargo hold (`TechnoClass+0x114` CargoClass,
    /// `+0x11C` Transporter, `+0x82` InOpenToppedTransport), a
    /// [`PassengerRole`] that `sim::passenger` owns.
    pub(crate) passenger_role: PassengerRole,
    /// Temporary VXL model override for visual-only state changes.
    /// When Some, the renderer should use this type's VXL model instead of `type_ref`.
    /// Set during refinery unloading (UnloadingClass= from rules.ini).
    pub display_type_override: Option<InternedId>,
    /// Techno+338, constructor6F2EC8=-1. Infantry capture519F94 stores its
    /// native InfantryType array index. Building consumers are diagnostics;
    /// the Unit hijacker lifecycle is a separate mechanism.
    capture_infantry_type_index: i32,
    /// Shared Building C4/PostMortem detonation latch: the marker `+0x6DF`
    /// with its C4Timer (`+0x528`) and C4AppliedBy (`+0x540`). The C4 plant
    /// (`sim::c4_plant`) owns one producer; qualifying delayed-death receiver
    /// results own the other. The Building LogicVector visit consumes expiry
    /// synchronously (`Simulation::tick_pending_building_detonation`) and
    /// keeps the marker, except a bridge repair hut's, which clears it.
    /// IronCurtain/ForceShield entry cancels it; `None` means no shared latch.
    #[serde(default)]
    pub pending_c4_detonation: Option<PendingC4Detonation>,
    /// A building's HasBeenCaptured byte (`BuildingClass+0x6E3`), a
    /// [`HasBeenCaptured`] that `crew_survival` owns.
    ///
    /// [`HasBeenCaptured`]: crate::sim::crew_survival::HasBeenCaptured
    #[serde(default)]
    pub(crate) has_been_captured: crate::sim::crew_survival::HasBeenCaptured,
    /// Stable ID of the unit installed in a `Bunker=yes` building.
    ///
    /// Mirrors the live `BuildingClass+0x2E4` role for tank bunkers: an empty
    /// bunker can be skipped by the NumberImpassableRows helper, while an
    /// occupied bunker remains a normal building blocker.
    #[serde(default)]
    pub bunker_occupant: Option<u64>,
    /// Unit side of the bunker reciprocal link (approach + installed states).
    #[serde(default)]
    pub bunker_link: BunkerLink,
    /// Runtime state for `Gate=yes` building passability.
    ///
    /// Native gate passability4525F0 accepts only mission `0x18` plus stable-open helper
    /// state. Opening and closing gates are still blockers for the same check.
    #[serde(default)]
    pub(crate) building_gate: Option<BuildingGateRuntime>,
    /// Shared Techno+350 door. Gate/factory/Unit consumers mutate it only
    /// through this entity's DoorClass operations.
    #[serde(default)]
    door: crate::sim::door::DoorClass,
    /// Tank-bunker install state machine. `Some` on `Bunker=yes` buildings from
    /// spawn (state `Idle` when empty); its presence marks the entity as a tank
    /// bunker. Drives entry admission → install.
    #[serde(default)]
    pub bunker_runtime: Option<crate::sim::docking::bunker_install::BunkerRuntime>,
    /// Techno+130/+134, both cleared by the Techno constructor. Animation ownership
    /// and the Jumpjet landing request survive independently of Unit6E0..6E2.
    deploy_anim: Option<crate::sim::anim_class::AnimId>,
    landing_for_deploy: bool,
    /// Unit+0x68C: runtime Deploy continuation, not the type's DeployToFire.
    /// Writers/readers are owned by sim::mcv_deploy (gamemd 0x007393C0).
    #[serde(default)]
    pub(crate) mcv_deploy_pending: bool,
    /// Infantry fear/prone runtime. `None` for non-infantry entities.
    #[serde(default)]
    pub infantry: Option<InfantryRuntime>,
    /// Optional body-rocking state only. Drive/Ship slope transitions belong
    /// to their typed locomotor payloads. This defaults to `None` and becomes
    /// present only when an explicit body-rocking producer activates it.
    #[serde(default)]
    pub rocking: Option<RockingState>,
    /// Exact native-width Mission state, owned by `sim::mission`: all writes
    /// pass through a named legacy compatibility adapter or the dormant
    /// exact-authority surface.
    pub(crate) mission: MissionCom,
    /// The passive target scan's timer, last frame and acquisition mark
    /// (`TechnoClass+0x180`/`+0x188`, `+0x4FC`, `+0x50C`), a [`PassiveScan`]
    /// that `techno_ai::target_scan` owns.
    ///
    /// [`PassiveScan`]: crate::sim::world::PassiveScan
    #[serde(default)]
    pub(crate) passive_scan: crate::sim::world::PassiveScan,
    /// Foot+688, initialized false at4D33A8. A stopped object unable to fire
    /// narrows its next scans through Foot::Greatest_Threat4D9920. This is
    /// independent of Techno's passive-target byte+50C.
    #[serde(default)]
    foot_retarget_after_stop: bool,
    /// Category-specific bytes read by Mission readiness and Aircraft policy.
    pub(crate) mission_leaf: MissionLeafState,
    /// Target identity archived by the Techno Override wrapper.
    pub(crate) suspended_attack_target: Option<TargetKind>,
    /// Active House/base-defence recruitment, archive and attacker-cooldown
    /// bytes. Snapshot migration defaults reproduce Techno construction.
    #[serde(default)]
    pub(crate) base_defense_response: BaseDefenseResponseState,
    /// The damage-Spark system handle (`TechnoClass+0x308`) as its expiry
    /// tick, a [`DamageSpark`] that `techno_ai` owns.
    ///
    /// [`DamageSpark`]: crate::sim::world::DamageSpark
    #[serde(default)]
    pub(crate) damage_particle_live_until: crate::sim::world::DamageSpark,
    /// The damage-Smoke system handle (`TechnoClass+0x310`), a
    /// [`DamageSmoke`] that `particles::spawn` owns.
    ///
    /// [`DamageSmoke`]: crate::sim::particles::spawn::DamageSmoke
    #[serde(default)]
    pub(crate) damage_smoke_system_id: crate::sim::particles::spawn::DamageSmoke,
    /// The passengers a transport's Unload keeps aboard (`UnitClass+0x6E4`),
    /// an [`UnloadKeep`] that `transport_unload` owns.
    ///
    /// [`UnloadKeep`]: crate::sim::transport_unload::UnloadKeep
    #[serde(default)]
    pub(crate) transport_unload_keep_count: crate::sim::transport_unload::UnloadKeep,
    /// Oil derricks' ProduceCash timer ([`credit_income`](crate::sim::credit_income)).
    #[serde(default)]
    pub(crate) produce_cash: crate::sim::credit_income::ProduceCash,
    /// The Floating Disc drain link pair ([`credit_income`](crate::sim::credit_income)).
    #[serde(default)]
    pub(crate) drain: crate::sim::credit_income::Drain,
    /// The parasite fields: the ParasiteClass a dog or drone owns, the eater
    /// attached to a victim, paralysis ([`parasite`](crate::sim::combat::parasite)).
    #[serde(default)]
    pub(crate) parasite: crate::sim::combat::parasite::FootParasite,
    /// Debug event log — records movement/state transitions for the inspector panel.
    /// Only allocated when debug inspector is active (X hotkey). Not included in state hashing.
    #[serde(skip)]
    debug_log: Option<DebugEventLog>,
    /// Techno+508, the contribution last published to the House spatial maps.
    /// Constructor6F310C..6F311E leaves this dword unwritten; None preserves
    /// that domain until admitted alive Unlimbo6F6ED2 or Add70F689 writes it.
    /// It is independent of current type/garrison threat and survives saves.
    #[serde(default)]
    cached_spatial_threat: Option<i32>,
}

mod construction_stage;
mod gunner;
mod simple_deploy;
mod voxel_recoil;

impl GameEntity {
    /// The infantry house-total latches (`Techno+0x438/+0x439`). Only
    /// `sim::house_tracking` can change them; the mutable borrow exists so
    /// that owner can reach the latches stored on the object.
    pub(crate) fn house_tracking_latches(
        &self,
    ) -> crate::sim::house_tracking::InfantryTrackingLatches {
        self.house_tracking_latches
    }

    pub(crate) fn house_tracking_latches_mut(
        &mut self,
    ) -> &mut crate::sim::house_tracking::InfantryTrackingLatches {
        &mut self.house_tracking_latches
    }

    /// Techno+0x284, the Chronosphere warp-in's frames.
    pub(crate) fn chrono_warp_delay(&self) -> i32 {
        self.chrono_warp_delay
    }

    pub(crate) fn set_chrono_warp_delay(&mut self, frames: i32) {
        self.chrono_warp_delay = frames;
    }
}

impl GameEntity {
    /// Native703860 base decision through the shared cloak owner. Accepted
    /// Technos have one real owner; retained +41A belongs to discovery, not cloak.
    /// Building4544A0 delegates here only with its +6ED override stage zero.
    /// The nonzero building override has no Rust producer and remains separate.
    pub fn visual_character(
        &self,
        cloaking_stages: i32,
        invisible: bool,
        query: crate::sim::cloak_disguise::VisualCharacterQuery,
    ) -> u8 {
        let (state, progress) = self
            .cloak
            .as_ref()
            .map_or((0, 0), |c| (c.state, c.depth as i32));
        crate::sim::cloak_disguise::visual_character(
            state,
            progress,
            cloaking_stages,
            invisible,
            self.category == EntityCategory::Structure,
            self.discovery.owned_by_current_house,
            true,
            query,
        )
    }

    /// `Techno70BE50`: actual COM410220 reads Abstract+10, then native7C5F00
    /// truncates retained+24C. Wrapping signed addition precedes IDIV400.
    /// Offset is RGB565 words, not world coordinates or a per-frame RNG draw.
    /// Original executed controls: translucent_blitter_a.json/ cloak_offsets.
    pub fn native_cloak_offset_words(&self) -> i32 {
        use crate::util::native_x87::{NativeF32Bits, X87Chop53};
        let displacement = if self.native_cloak_displacement == NativeF32Bits::POSITIVE_ZERO {
            0
        } else {
            X87Chop53::load_f32(self.native_cloak_displacement)
                .map(X87Chop53::ftol_i32_low_masked)
                .unwrap_or(0)
        };
        self.native_unique_id.wrapping_add(displacement) % 400
    }

    pub(crate) fn door_phase(&self) -> crate::sim::door::DoorPhase {
        self.door.phase()
    }

    #[cfg(test)]
    pub(crate) fn door_timer_fields(&self) -> (i32, i32, i32) {
        self.door.timer_fields()
    }

    pub(crate) fn open_door(&mut self, ticks: u32, frame: u32) {
        self.door.open(ticks, frame);
    }

    pub(crate) fn close_door(&mut self, ticks: u32, frame: u32) {
        self.door.close(ticks, frame);
    }

    pub(crate) fn reverse_door(&mut self, frame: u32) {
        self.door.reverse(frame);
    }

    pub(crate) fn advance_door(&mut self, frame: u32) {
        self.door.advance(frame);
    }

    pub(crate) fn hash_door_state(&self, hasher: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        self.door.hash(hasher);
    }

    ///Building4525F0: MissionClass owns Open24, DoorClass owns stable-open.
    pub(crate) fn is_open_gate(&self) -> bool {
        self.mission.effective().known() == Some(crate::sim::mission::MissionType::Open)
            && self.door_phase() == crate::sim::door::DoorPhase::OpenStable
    }

    pub(crate) const fn pending_entry(&self) -> Option<u64> {
        self.pending_entry
    }

    /// Native Unit741D9F / Foot70D84F / Techno707AE7 entry-pointer writer.
    pub(crate) fn set_pending_entry(&mut self, target: Option<u64>) {
        self.pending_entry = target;
    }

    pub(crate) const fn cached_spatial_threat(&self) -> Option<i32> {
        self.cached_spatial_threat
    }

    pub(crate) fn retain_spatial_threat(&mut self, value: i32) {
        self.cached_spatial_threat = Some(value);
    }
    pub(crate) fn native_stage(&self) -> &crate::sim::stage::StageClass {
        &self.stage
    }

    /// Read native Doing and shared +F8 for SHP drawing, including actions
    /// absent from the generic sequence vocabulary. Presentation never advances
    /// this clock or owns a writable copy of its progress. Doing-1 is retained;
    /// the frame-selector caller owns its native default-action selection.
    pub fn infantry_sprite_pose(&self) -> Option<(i32, i32)> {
        if !crate::sim::movement::infantry_action::doing_owns_sequence(self) {
            return None;
        }
        Some((self.mission_leaf.as_infantry()?.doing(), self.stage.value()))
    }

    /// Advance the common clock at this class's native scheduling point.
    pub(crate) fn tick_native_stage(&mut self, now: i32) -> bool {
        self.stage.advance(now)
    }

    /// Start a class action/control without altering independent FC/110 state.
    pub(crate) fn restart_native_stage(&mut self, value: i32, now: i32, rate: i32) {
        self.stage.restart(value, now, rate);
    }

    pub(crate) fn set_native_stage_value(&mut self, value: i32) {
        self.stage.set_value(value);
    }

    #[cfg(test)]
    pub(crate) fn install_native_stage_fixture(&mut self, stage: crate::sim::stage::StageClass) {
        self.stage = stage;
    }

    /// Invalidate the one live Foot path head. Native callers write
    /// Foot+5E0=-1; retained suffix/reference words and the destination
    /// remain intact.
    pub(crate) fn clear_live_path_head(&mut self) {
        self.navigation.path_replay.clear_live_head();
    }

    /// `TechnoClass::ArchiveTarget` (`Techno+0x218`): the base-defence
    /// responder's post, a harvester's archived ore cell and a factory's
    /// rally point share this one field, stored in
    /// [`BaseDefenseResponseState`].
    pub(crate) fn archive_target(&self) -> Option<crate::sim::combat::TargetKind> {
        self.base_defense_response.archive_target
    }

    /// `TechnoClass::Set_ArchiveTarget @ 0x0070C610`, a plain store.
    pub(crate) fn set_archive_target(&mut self, target: Option<crate::sim::combat::TargetKind>) {
        self.base_defense_response.archive_target = target;
    }

    pub(crate) fn deploy_voice_suppressed(&self) -> bool {
        self.suppress_deploy_voice
    }

    /// SlaveManager6B01C4's write, before it archives the refinery's own cell.
    pub(crate) fn suppress_next_deploy_voice(&mut self) {
        self.suppress_deploy_voice = true;
    }

    /// Common VoiceDeploy708E6A/708EA0 clears after its attempted request.
    pub(crate) fn clear_deploy_voice_suppression(&mut self) {
        self.suppress_deploy_voice = false;
    }

    /// A factory's rally point is its ArchiveTarget: `BuildingClass::
    /// SetRallyPoint @ 0x00443860` archives the clicked cell through event
    /// 0x1E (`Set_ArchiveTarget`), and `BuildingClass::ExitObject_Main @
    /// 0x00443C60` reads `+0x218` for the object leaving it. The shared
    /// Building destination setter (`0x00455D50`) can also replace this
    /// archive; Stop reaches its null arm and removes the rally.
    pub(crate) fn rally_cell(&self) -> Option<(u16, u16)> {
        match self.archive_target() {
            Some(crate::sim::combat::TargetKind::Cell(rx, ry)) => Some((rx, ry)),
            _ => None,
        }
    }

    pub(crate) const fn is_mission_only(&self) -> bool {
        self.mission_only
    }

    /// Native SET writers are monotonic for this object's lifetime.
    pub(crate) fn mark_mission_only(&mut self) {
        self.mission_only = true;
    }

    /// AircraftUnlimbo4143A8..4143F2, only after Foot placement succeeds.
    /// GetWeapon(0) uses the same tier/slot authority as production combat.
    pub(crate) fn retain_aircraft_unlimbo_control(
        &mut self,
        rules: &crate::rules::ruleset::RuleSet,
        type_id: &str,
    ) {
        if self.category != EntityCategory::Aircraft {
            return;
        }
        let Some(object) = rules.object(type_id) else {
            return;
        };
        let camera =
            crate::sim::combat::combat_weapon::primary_for_tier(object, self.veterancy.rank())
                .and_then(|id| rules.weapon(id))
                .is_some_and(|weapon| weapon.camera);
        if !object.selectable || !object.landable || camera {
            self.mark_mission_only();
        }
    }

    /// The rest of AircraftClass::Unlimbo's tail, after the retained +3D4
    /// ([`Self::retain_aircraft_unlimbo_control`]): the Stage restarts at 0
    /// with rate 1 (`0x0041441C..0x00414444`), then SetSpeedFraction
    /// (vt+0x544, Foot `0x004D3710`) takes 1.0 when GetHeight (vt+0x1C8) is
    /// the type's FlightLevel (vt+0xBC), else 0 (`0x0041444E..0x0041448A`).
    /// Native comparison: tools/spatial_oracle/aircraft_unlimbo_height.json.
    pub(crate) fn finish_aircraft_unlimbo(&mut self, height: i32, flight_level: i32, now: i32) {
        self.stage.restart(0, now, 1);
        let fraction = if height == flight_level {
            crate::util::fixed_math::SIM_ONE
        } else {
            crate::util::fixed_math::SIM_ZERO
        };
        self.foot_speed.set_speed_fraction(fraction);
    }

    /// Immutable storage key. Construction and snapshot decoding establish it.
    pub fn stable_id(&self) -> u64 {
        self.stable_id
    }

    /// Indexed ownership. Live transfers go through Simulation's owner lifecycle
    /// and EntityStore's index update; ordinary payload mutation cannot assign it.
    pub fn owner(&self) -> InternedId {
        self.owner
    }

    /// Immutable indexed type identity, established by construction/decoding.
    pub fn type_ref(&self) -> InternedId {
        self.type_ref
    }

    /// A VXL/HVA model (true) or an SHP sprite.
    pub(crate) const fn is_voxel(&self) -> bool {
        self.is_voxel
    }

    /// Install a fixture's model kind.
    #[cfg(test)]
    pub(crate) fn set_is_voxel_for_test(&mut self, is_voxel: bool) {
        self.is_voxel = is_voxel;
    }

    /// The concrete class, fixed at construction.
    pub const fn category(&self) -> EntityCategory {
        self.category
    }

    /// AbstractClass+10, the native UniqueID.
    pub(crate) const fn native_unique_id(&self) -> i32 {
        self.native_unique_id
    }

    /// The TechnoClass constructor's retained Scenario word (`+0x3C8`).
    pub(crate) const fn techno_ctor_random_word(&self) -> u16 {
        self.techno_ctor_random_word
    }
}

/// Fixture setup for the identity a constructor or decoder establishes.
#[cfg(test)]
impl GameEntity {
    pub(crate) fn set_stable_id_for_test(&mut self, stable_id: u64) {
        self.stable_id = stable_id;
    }

    pub(crate) fn set_native_unique_id_for_test(&mut self, native_unique_id: i32) {
        self.native_unique_id = native_unique_id;
    }

    pub(crate) fn set_techno_ctor_random_word_for_test(&mut self, word: u16) {
        self.techno_ctor_random_word = word;
    }

    pub(crate) fn set_owner_for_test(&mut self, owner: InternedId) {
        self.owner = owner;
    }

    pub(crate) fn set_type_ref_for_test(&mut self, type_ref: InternedId) {
        self.type_ref = type_ref;
    }

    /// Changes only the class tag. A fixture that needs another class's
    /// private state (a Building's body, an Infantry's runtime) constructs it
    /// with [`Self::test_default_of_category`] instead.
    pub(crate) fn set_category_for_test(&mut self, category: EntityCategory) {
        self.category = category;
    }
}

impl GameEntity {
    pub(crate) const fn foot_retarget_after_stop(&self) -> bool {
        self.foot_retarget_after_stop
    }

    /// Drive4B2E9F / Ship6A24F2 / Hover51684D: the held target cannot
    /// be fired at after stopping. Set before the team Scan_Limit/target clear;
    /// Team6EC3BD also sets it after each member's class target clear.
    pub(crate) fn mark_stopped_cannot_fire(&mut self) {
        self.foot_retarget_after_stop = true;
    }

    /// FootGreatestThreat4D9951..55 clears only an empty completed base scan.
    /// Class overrides which return before Foot leave the byte untouched.
    pub(crate) fn finish_foot_threat_scan(&mut self, found: bool) {
        if !found {
            self.foot_retarget_after_stop = false;
        }
    }

    /// Mission_Rescue4DE03E resets the latch before its direct mask0 scan.
    pub(crate) fn clear_rescue_retarget_latch(&mut self) {
        self.foot_retarget_after_stop = false;
    }

    /// Foot4D9931..33 changes AL only; all upper mask bits survive.
    pub(crate) const fn coerce_foot_threat_mask(&self, mask: u32) -> u32 {
        if self.foot_retarget_after_stop {
            (mask & !2) | 1
        } else {
            mask
        }
    }

    /// The body heading at `frame`: `FacingClass::Current @ 0x004C93D0` on
    /// `+0x388`.
    pub(crate) fn body_facing_current(&self, frame: u32) -> u16 {
        self.body_facing.current(frame)
    }

    /// The body heading's high byte at `frame`, the 8-bit direction a reader
    /// that drops the DirStruct's low byte sees.
    pub(crate) fn body_facing_byte(&self, frame: u32) -> u8 {
        (self.body_facing.current(frame) >> 8) as u8
    }

    /// The body heading at `frame` as a rounded DirType byte,
    /// `((raw >> 7) + 1) >> 1`: the conversion gamemd applies when it passes
    /// `Current()` on as a direction argument (an Unlimbo's, a Do_Turn
    /// comparison's), wrapping 0xFF80.. to 0.
    pub(crate) fn body_facing_dir(&self, frame: u32) -> u8 {
        (((u32::from(self.body_facing.current(frame)) >> 7) + 1) >> 1) as u8
    }

    /// The barrel elevation (`+0x370`); its readers sample `current(frame)`
    /// (`FacingClass::Current @ 0x004C93D0`).
    pub(crate) fn barrel_elevation(&self) -> &crate::sim::movement::FacingClass {
        &self.barrel_elevation
    }

    /// Whether the barrel elevation still holds its constructor value, which
    /// an object that never unlimboed keeps.
    pub(crate) fn barrel_elevation_is_constructed(&self) -> bool {
        self.barrel_elevation == crate::sim::movement::FacingClass::new(0, BARREL_ELEVATION_ROT)
    }

    /// `TechnoClass::Unlimbo`'s barrel writes, right after its body snap
    /// (`0x006F6DAA`): the elevation snaps level (`Set_Current(0x4000)` at
    /// `0x006F6DC3`), then turns toward the type's `FireAngle=` (`Set` at
    /// `0x006F6DF5`, [`unlimbo_barrel_target`]).
    pub(crate) fn unlimbo_barrel_elevation(&mut self, fire_angle: i32, frame: u32) {
        self.barrel_elevation.snap(BARREL_LEVEL, frame);
        self.barrel_elevation
            .set(unlimbo_barrel_target(fire_angle), frame);
    }

    /// `BuildingClass::Flash` (vt+0x148, `0x00456E00`) for `frames`. Its
    /// Techno body (`0x006F9DD0`) starts the flash countdown (`+0xF0`) unless
    /// the object is disguised to the player: a building's IsDisguisedTo
    /// (vt+0xC8, `0x0041C020`) answers its own disguise flag (`+0x1D8`) and
    /// its GetDisguiseHouse (vt+0xD0, `0x0041BE70`) none. The redraw flag
    /// (`+0x80`) and the relit slot anims are presentation, which the draws
    /// derive from the countdown (`app::presentation::lighting`).
    pub(crate) fn flash_building(&mut self, frames: i32) {
        if !self
            .disguise
            .as_ref()
            .is_some_and(|disguise| disguise.is_disguised())
        {
            self.flashing.start(frames);
        }
    }

    /// The class constructor's one rate write on `+0x388` (`Set_ROT @
    /// 0x004C9680`): the constant 127 for infantry (`0x00517BC5`), `ROT=`
    /// (`Type+0x71C`) for a unit (`0x00735579`), an aircraft (`0x00413FE7`)
    /// and a building (`BuildingClass::Init` at `0x00442CA5`).
    pub(crate) fn set_body_facing_rot(&mut self, type_rot: i32) {
        self.body_facing
            .set_rot(if self.category == EntityCategory::Infantry {
                127
            } else {
                type_rot
            });
    }

    pub(crate) fn set_owner_from_store(
        &mut self,
        owner: InternedId,
        _: crate::sim::entity_store::OwnerChangeAuthority,
    ) {
        self.owner = owner;
    }

    /// The Harvest FSM cursor of record, decoded from
    /// `MissionCom::handler_state`. `None` when the entity has no Miner
    /// component. An out-of-vocabulary cursor decodes as `SearchOre` (the
    /// zeroed handler state every mission transition writes) — the only
    /// writers are the FSM commit and the zeroing transitions, so anything
    /// else is a logic error surfaced by the dispatch-time debug assert.
    pub fn miner_state(&self) -> Option<crate::sim::miner::MinerState> {
        self.miner.as_ref().map(|_| {
            crate::sim::miner::MinerState::from_cursor(self.mission.handler_state())
                .unwrap_or(crate::sim::miner::MinerState::SearchOre)
        })
    }

    /// The type's `Harvester=` (`+0xE0E`): a War or Chrono Miner, which runs
    /// the harvest and refinery dock missions. The Slave Miner carries a Miner
    /// component only as an order marker (`sim::slave_manager`), so it is not
    /// one.
    pub fn is_harvester(&self) -> bool {
        self.miner
            .as_ref()
            .is_some_and(|miner| miner.kind != crate::sim::miner::MinerKind::Slave)
    }

    /// The committed mission read at TechnoAI6FA697, before its class's
    /// subsequent Ready/Commence checkpoint. Clearing a target or destination
    /// does not change this selector; only the mission owner can commence a
    /// queued Guard. Native FV bridge-collapse execution at Logic45 rejects
    /// Attack here and consumes no scan draw (`fv_cell_attack` continuation).
    pub fn passive_acquire_mission(&self) -> MissionType {
        self.mission.current().known().unwrap_or(MissionType::None)
    }

    /// Construct after the owning world funnel has resolved the explicit
    /// `TechnoConstructorInit` capability. Kept inside `sim` so ordinary app,
    /// render, and diagnostic code cannot silently invent the native word.
    pub(in crate::sim) fn new_at_frame_from_constructor_word(
        stable_id: u64,
        native_unique_id: i32,
        rx: u16,
        ry: u16,
        z: u8,
        facing: u8,
        owner: InternedId,
        health: Health,
        type_ref: InternedId,
        category: EntityCategory,
        veterancy: u16,
        vision_range: u16,
        is_voxel: bool,
        construction_frame: u32,
        techno_ctor_random_word: u16,
    ) -> Self {
        // Infantry spawn at sub-cell 2 (top of diamond) instead of cell center
        // so they don't overlap with other units at the same position.
        let (init_sub_x, init_sub_y) = if category == EntityCategory::Infantry {
            crate::util::lepton::subcell_lepton_offset(Some(2))
        } else {
            (
                crate::util::lepton::CELL_CENTER_LEPTON,
                crate::util::lepton::CELL_CENTER_LEPTON,
            )
        };
        Self {
            cached_spatial_threat: None,
            tracking_facts: Default::default(),
            house_tracking_latches: Default::default(),
            stable_id,
            native_unique_id,
            native_cloak_displacement: crate::util::native_x87::NativeF32Bits::POSITIVE_ZERO,
            techno_ctor_random_word,
            stage: crate::sim::stage::StageClass::constructed(construction_frame as i32),
            discovery: TechnoDiscoveryHistory::default(),
            structure_upgrade_link: None,
            position: Position {
                rx,
                ry,
                z,
                exact_z_leptons: None,
                sub_x: init_sub_x,
                sub_y: init_sub_y,
            },
            // The Unlimbo direction; the class constructor supplies the rate.
            body_facing: crate::sim::movement::FacingClass::new(u16::from(facing) << 8, 0),
            // Unlimbo's type-derived initialization arms the turret loop latch.
            turret_rotate_latch: Default::default(),
            suppress_deploy_voice: false,
            barrel_elevation: crate::sim::movement::FacingClass::new(0, BARREL_ELEVATION_ROT),
            body_frame_counter: Default::default(),
            owner,
            health,
            estimated_health: crate::sim::estimated_health::EstimatedHealth::from_raw(
                health.current(),
            ),
            type_ref,
            category,
            type_facts: crate::sim::world::TypeFacts::new(category),
            veterancy: crate::sim::combat::veterancy::Veterancy::new(veterancy),
            flashing: Default::default(),
            crate_multipliers: Default::default(),
            sight: crate::sim::vision::SightFacts::new(vision_range),
            sight_refresh_timers: crate::sim::vision::SightRefreshTimers::at_construction(
                construction_frame,
            ),
            gap_generator: crate::sim::vision::GapGeneratorRuntime::default(),
            is_voxel,
            selected: false,
            deactivated: Default::default(),
            repairing: Default::default(),
            in_logic_vector: false,
            lifecycle: ObjectLifecycle::default(),
            in_playfield: false,
            mission_only: false,
            removal_facts: Default::default(),
            kill_record: Default::default(),
            foot_air: foot_air::FootAirState::new(stable_id),
            locomotor: None,
            navigation: NavigationState::at_frame(construction_frame),
            foot_speed: crate::sim::components::FootSpeedState::default(),
            flight_attitude: Default::default(),
            foot_occupation_enabled: true,
            foot_locomotor_swap_active: false,
            lift_may_end: false,
            locomotor_target: None,
            locomotor_source: None,
            being_manipulated: false,
            manipulated_by: None,
            warped_by_house: None,
            setter_force_reassign: false,
            chrono_warp_delay: 0,
            attack_target: TarCom::default(),
            pending_building_fire: None,
            prism_support_count: Default::default(),
            current_weapon_number: 0,
            current_turret_index: -1,
            charge_turret_delay: 0,
            radio_contacts: Contacts::default(),
            tether: crate::sim::radio::Tether::default(),
            ai_sell_repair: Default::default(),
            barrel_facing: None,
            turret_rotation_latch: Default::default(),
            last_fire_frame: Default::default(),
            // `TechnoClass` constructor `0x006F2E81..0x006F2E8C`: started
            // at the construction frame with no duration.
            rearm_timer: crate::sim::combat::rof::RearmTimer::new(construction_frame as i32),
            gattling: Default::default(),
            turret_anim_frame: Default::default(),
            voxel_recoil: None,
            weapon_burst: Default::default(),
            building_body: (category == EntityCategory::Structure).then(Default::default),
            building_power_health_sample: 0,
            building_sale: None,
            ai_placement_timer: crate::sim::timer::CdTimer::default(),
            building_art: Default::default(),
            building_last_operational: Default::default(),
            building_powered_center: Default::default(),
            building_has_engineer: Default::default(),
            building_light: Default::default(),
            damage_fires: Default::default(),
            on_bridge: false,
            runtime_bridge_transition: Default::default(),
            animation: None,
            voxel_animation: None,
            harvest_overlay: None,
            miner: None,
            slave_manager: None,
            slave: Default::default(),
            mega_mission: Default::default(),
            cloak: None,
            sensor_deposit: None,
            disguise: Some(DisguiseRuntime::new(construction_frame)),
            low_bridge_tube_state: None,
            capture_manager: None,
            spawn_manager: None,
            spawn_owner_id: Default::default(),
            fall: ObjectFallState::default(),
            invulnerability: None,
            mind_control: Default::default(),
            temporal: Default::default(),
            bomb: None,
            berserk: BerserkState::default(),
            pending_entry: None,
            building_entry: Default::default(),
            aircraft_ammo: None,
            // Infantry get sub-cell 2 (first distinct position) at spawn so
            // they don't all pile up at cell center when multiple are created.
            sub_cell: if category == EntityCategory::Infantry {
                Some(2)
            } else {
                None
            },
            crushable_facts: Default::default(),
            native_crush_immunity: 0,
            mover_facts: Default::default(),
            death: crate::sim::world::TerminalLifetime::default(),
            move_sound: crate::sim::world::MoveSoundState::default(),
            crash: crate::sim::world::CrashLatch::default(),
            sinking: crate::sim::world::SinkingState::default(),
            passenger_role: PassengerRole::default(),
            display_type_override: None,
            capture_infantry_type_index: -1,
            pending_c4_detonation: None,
            has_been_captured: Default::default(),
            bunker_occupant: None,
            bunker_link: BunkerLink::None,
            building_gate: None,
            door: crate::sim::door::DoorClass::at_frame(construction_frame),
            bunker_runtime: None,
            deploy_anim: None,
            landing_for_deploy: false,
            mcv_deploy_pending: false,
            infantry: if category == EntityCategory::Infantry {
                Some(InfantryRuntime::new())
            } else {
                None
            },
            rocking: None,
            mission: MissionCom::at_frame(construction_frame),
            passive_scan: crate::sim::world::PassiveScan::new(construction_frame),
            foot_retarget_after_stop: false,
            mission_leaf: MissionLeafState::constructed(category, construction_frame as i32),
            suspended_attack_target: None,
            base_defense_response: BaseDefenseResponseState::default(),
            damage_particle_live_until: Default::default(),
            damage_smoke_system_id: Default::default(),
            transport_unload_keep_count: Default::default(),
            produce_cash: crate::sim::credit_income::ProduceCash::new(construction_frame),
            drain: Default::default(),
            parasite: crate::sim::combat::parasite::FootParasite::new(construction_frame),
            debug_log: None,
        }
    }

    /// Explicit zero-word constructor for tests that exercise construction
    /// frame anchoring without participating in gameplay RNG ownership.
    #[cfg(test)]
    pub fn new_at_frame_for_test(
        stable_id: u64,
        rx: u16,
        ry: u16,
        z: u8,
        facing: u8,
        owner: InternedId,
        health: Health,
        type_ref: InternedId,
        category: EntityCategory,
        veterancy: u16,
        vision_range: u16,
        is_voxel: bool,
        construction_frame: u32,
    ) -> Self {
        Self::new_at_frame_from_constructor_word(
            stable_id,
            0,
            rx,
            ry,
            z,
            facing,
            owner,
            health,
            type_ref,
            category,
            veterancy,
            vision_range,
            is_voxel,
            construction_frame,
            0,
        )
    }

    /// Explicit frame-zero constructor for tests that do not exercise
    /// construction-time Mission timer anchoring.
    #[cfg(test)]
    pub fn new_at_frame_zero_for_test(
        stable_id: u64,
        rx: u16,
        ry: u16,
        z: u8,
        facing: u8,
        owner: InternedId,
        health: Health,
        type_ref: InternedId,
        category: EntityCategory,
        veterancy: u16,
        vision_range: u16,
        is_voxel: bool,
    ) -> Self {
        Self::new_at_frame_for_test(
            stable_id,
            rx,
            ry,
            z,
            facing,
            owner,
            health,
            type_ref,
            category,
            veterancy,
            vision_range,
            is_voxel,
            0,
        )
    }

    /// ObjectClass `+0x8D` IsFallingDown. `ObjectClass::Paradrop`
    /// (`0x005F5940`) raises it at `0x005F5965`, and `ObjectClass::AI`
    /// clears it when the fall grounds (`0x005F3F86`).
    pub(crate) fn is_falling_down(&self) -> bool {
        self.fall.falling
    }

    pub(crate) fn was_dropped_as_bomb(&self) -> bool {
        self.fall.dropped_as_bomb
    }

    pub(crate) fn fall_rate(&self) -> i32 {
        self.fall.rate
    }

    pub(crate) fn is_parachuted(&self) -> bool {
        self.fall.parachuted
    }

    /// Paradrop5F5940 raises +84/+8D and resets +2C before placement.
    pub(crate) fn begin_parachute_fall(&mut self, z: i32) {
        self.position.exact_z_leptons = Some(z);
        self.fall.parachuted = true;
        self.fall.falling = true;
        self.fall.rate = 0;
    }

    /// DropAsBomb5F416A/71 writes neither Location nor FallRate nor +84.
    pub(crate) fn begin_bomb_fall(&mut self) {
        self.fall.falling = true;
        self.fall.dropped_as_bomb = true;
    }

    pub(crate) fn finish_fall(&mut self) {
        self.fall.falling = false;
    }

    /// ObjectAI5F3FBC..3FFA updates even the grounding frame's rate.
    pub(crate) fn advance_fall_rate(&mut self, parachute_cap: i32, free_fall_cap: i32) {
        self.fall.rate = if self.fall.parachuted {
            self.fall.rate.wrapping_sub(1).max(parachute_cap)
        } else {
            use crate::util::native_x87::{NativeF64Bits, X87Chop53};
            let gravity = X87Chop53::load_f64(NativeF64Bits::from_bits(0x3ff6_6666_6666_6666))
                .expect("finite ObjectAI free-fall gravity 7EF248");
            X87Chop53::ftol_i32_low_masked(X87Chop53::sub(
                X87Chop53::load_i32(self.fall.rate),
                gravity,
            ))
            .max(free_fall_cap)
        };
    }

    /// Put a fixture into a fall where it is, with a FallRate of 0.
    #[cfg(test)]
    pub(crate) fn set_falling_down_for_test(&mut self, falling: bool) {
        self.fall.falling = falling;
        self.fall.parachuted = falling;
        self.fall.rate = 0;
    }

    #[cfg(test)]
    pub(crate) fn set_fall_rate_for_test(&mut self, rate: i32) {
        self.fall.rate = rate;
    }

    /// The debug event log, while debug event logging is on. The `dev-ui`
    /// debug panel reads it.
    #[cfg(any(test, feature = "dev-ui"))]
    pub(crate) fn debug_log(&self) -> Option<&DebugEventLog> {
        self.debug_log.as_ref()
    }

    /// Turn the debug event log on (keeping one the object has) or off
    /// (dropping it).
    pub(crate) fn set_debug_logging(&mut self, enabled: bool) {
        if !enabled {
            self.debug_log = None;
        } else if self.debug_log.is_none() {
            self.debug_log = Some(DebugEventLog::new());
        }
    }

    /// Record a debug event if the event log is active. No-op when `debug_log` is `None`.
    pub fn push_debug_event(&mut self, tick: u32, kind: DebugEventKind) {
        if let Some(log) = &mut self.debug_log {
            log.push(tick, kind);
        }
    }

    /// Mark a live RadioClass-style contact with another entity.
    ///
    /// First-null slot insert: idempotent, and full slots deny without evicting
    /// (the receiver dock idiom). Slot order is hash-relevant and deterministic.
    pub fn mark_live_contact_with(&mut self, other_stable_id: u64) {
        self.radio_contacts.insert(other_stable_id);
    }

    /// Whether this entity has a live RadioClass-style contact with another entity.
    pub fn has_live_contact_with(&self, other_stable_id: u64) -> bool {
        self.radio_contacts.contains(other_stable_id)
    }

    /// Clear a live RadioClass-style contact with another entity (BREAK: nulls
    /// the slot in place, no compaction).
    pub fn clear_live_contact_with(&mut self, other_stable_id: u64) {
        self.radio_contacts.remove(other_stable_id);
    }

    /// Runtime movement/path layer with Ground as the fallback.
    ///
    /// This is not the object-list selector, which is
    /// `occupancy::cell_list_layer_for_entity`.
    pub fn movement_layer_or_ground(&self) -> crate::sim::movement::locomotor::MovementLayer {
        self.locomotor.as_ref().map_or(
            crate::sim::movement::locomotor::MovementLayer::Ground,
            |l| l.layer,
        )
    }

    /// Whether this entity is currently on a bridge deck.
    pub fn is_on_bridge_layer(&self) -> bool {
        self.on_bridge
    }

    /// Create a minimal entity for testing. Fills sensible defaults for most fields.
    #[cfg(test)]
    /// Create a minimal test entity with the given owner and type_ref strings.
    /// Uses a shared test interner via `test_intern()` for consistent IDs.
    pub fn test_default(stable_id: u64, type_ref: &str, owner: &str, rx: u16, ry: u16) -> Self {
        Self::test_default_of_category(stable_id, type_ref, owner, rx, ry, EntityCategory::Unit)
    }

    /// Construct the requested class before its private class state is installed.
    /// Tests must not turn a Unit into a Building by changing only `category`.
    #[cfg(test)]
    pub fn test_default_of_category(
        stable_id: u64,
        type_ref: &str,
        owner: &str,
        rx: u16,
        ry: u16,
        category: EntityCategory,
    ) -> Self {
        Self::new_at_frame_zero_for_test(
            stable_id,
            rx,
            ry,
            0, // z = ground level
            0, // facing = north
            crate::sim::intern::test_intern(owner),
            Health::new(100),
            crate::sim::intern::test_intern(type_ref),
            category,
            0, // veterancy = rookie
            5, // vision_range = 5 cells
            true,
        )
    }

    /// Whether this entity is alive (health > 0).
    pub fn is_alive(&self) -> bool {
        self.health.current() > 0
    }

    /// Whether ObjectClass native-alive state is set. This is intentionally
    /// independent from health and the Rust death-sequence state.
    pub fn is_object_alive(&self) -> bool {
        self.lifecycle.object_alive()
    }

    /// Transitional Rust system gate until ordinary Infantry lifecycle authority
    /// migrates. Distinct from both health-based `is_alive()` and native-alive
    /// `is_object_alive()`; death-sequence state still suppresses current raw-store
    /// consumers even while ObjectClass native-alive remains set.
    pub fn is_active(&self) -> bool {
        self.lifecycle.object_alive() && !self.death.is_dying()
    }

    /// ObjectClass IsAlive (`+0x90`) as the per-object AI and a Unit's fire
    /// update read it. Native clears it when an ordinary death UnInits the
    /// object inside ReceiveDamage; VERA defers that UnInit, so Health 0 counts
    /// as dead here unless the object is crashing, and a wreck keeps IsAlive
    /// until its impact. A death sequence (`dying`) runs through its own path.
    pub fn is_ai_alive(&self) -> bool {
        self.is_active() && (self.health.current() > 0 || self.crash.is_crashing())
    }

    /// Infantry522510's Doing27..30 deployment predicate. Undeploy31 already
    /// accepts destinations; Units retain their distinct deployment controller.
    pub fn is_deployed(&self) -> bool {
        if self.category == EntityCategory::Infantry {
            self.infantry_deploy_doing()
        } else {
            self.mission_leaf.as_unit().is_some_and(|leaf| {
                leaf.deployed() != 0
                    || leaf.deploy_begin_active() != 0
                    || leaf.deploy_reverse_active() != 0
            })
        }
    }

    /// Native deployed state. Unit6E0 remains set during reverse animation;
    /// consumers needing the transition exclusion also read unit_deploying().
    pub fn is_fully_deployed(&self) -> bool {
        if self.category == EntityCategory::Infantry {
            self.mission_leaf
                .as_infantry()
                .is_some_and(|leaf| (28..=30).contains(&leaf.doing()))
        } else {
            self.mission_leaf
                .as_unit()
                .is_some_and(|leaf| leaf.deployed() != 0)
        }
    }

    /// Infantry522510's deployment family, used by its weapon/mission and
    /// destination consumers. Undeploy31 is outside this native predicate.
    pub(crate) fn infantry_deploy_doing(&self) -> bool {
        self.category == EntityCategory::Infantry
            && self
                .mission_leaf
                .as_infantry()
                .is_some_and(|leaf| (27..=30).contains(&leaf.doing()))
    }

    pub(crate) const fn native_crush_immunity(&self) -> u8 {
        self.native_crush_immunity
    }

    pub(crate) fn set_infantry_deploy_crush_immunity(&mut self, raw: u8) {
        assert_eq!(self.category, EntityCategory::Infantry);
        self.native_crush_immunity = raw;
    }

    /// Native Get_Mission(vt184) reads current, otherwise queued. The
    /// Construction and Selling producers publish that same authority.
    pub(crate) fn constructing_or_selling(&self) -> bool {
        matches!(
            self.mission.effective().known(),
            Some(MissionType::Construction | MissionType::Selling)
        )
    }

    /// Native Building+534 is0, independent of mission status and ready byte.
    pub(crate) fn in_construction_bstate(&self) -> bool {
        self.building_body
            .as_ref()
            .is_some_and(|body| body.state() == 0)
    }

    /// Building440042..440074: compare actual health, publish the new sample
    /// and tell the host to invalidate the owning House's derived state.
    pub(crate) fn sample_building_health_for_power(&mut self) -> bool {
        if self.category != EntityCategory::Structure
            || self.building_power_health_sample == self.health.current()
        {
            return false;
        }
        self.building_power_health_sample = self.health.current();
        true
    }

    pub(crate) fn reset_building_health_sample_at_unlimbo(&mut self) {
        if self.category == EntityCategory::Structure {
            self.building_power_health_sample = 0;
        }
    }

    pub(crate) fn building_power_health_sample(&self) -> Option<i32> {
        (self.category == EntityCategory::Structure).then_some(self.building_power_health_sample)
    }

    pub(crate) fn hash_building_health_sample(&self, hasher: &mut impl std::hash::Hasher) {
        if let Some(sample) = self.building_power_health_sample() {
            use std::hash::Hash;
            sample.hash(hasher);
        }
    }

    pub(crate) fn record_infantry_capture_type(&mut self, native_index: i32) {
        self.capture_infantry_type_index = native_index;
    }

    pub(crate) fn hash_capture_infantry_type(&self, hasher: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        self.capture_infantry_type_index.hash(hasher);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native_cloak_fixture() -> serde_json::Value {
        serde_json::from_str(crate::test_fixture::text(
            "tools/procedural_drawing_oracle/translucent_blitter_a.json",
        ))
        .unwrap()
    }

    #[test]
    fn visual_character_matches_original_executed_transition_controls() {
        let native = native_cloak_fixture();
        let rows = native["cloak_transition"].as_array().unwrap();
        assert_eq!(rows.len(), 256);
        let mut entity = GameEntity::test_default(1, "MTNK", "Americans", 10, 20);
        for row in rows {
            let mut cloak = CloakRuntime::new(0);
            cloak.state = row["state"].as_i64().unwrap() as i32;
            cloak.depth = row["depth"].as_i64().unwrap() as u32;
            entity.cloak = Some(cloak);
            entity.discovery.owned_by_current_house = row["owned"].as_bool().unwrap();
            let query = if row["force"].as_bool().unwrap() {
                crate::sim::cloak_disguise::VisualCharacterQuery::sensor(false, false, false)
            } else {
                crate::sim::cloak_disguise::VisualCharacterQuery::screen(
                    true, false, false, false, false, false,
                )
            };
            assert_eq!(
                entity.visual_character(row["stage"].as_i64().unwrap() as i32, false, query),
                row["character"].as_u64().unwrap() as u8,
                "native control {row}"
            );
        }
    }

    #[test]
    fn native_cloak_offset_matches_original_identity_float_and_wrap_controls() {
        let native = native_cloak_fixture();
        let rows = native["cloak_offsets"].as_array().unwrap();
        assert_eq!(rows.len(), 14);
        let mut entity = GameEntity::test_default(777, "MTNK", "Americans", 10, 20);
        for row in rows {
            entity.native_unique_id = row["native_unique_id"].as_i64().unwrap() as i32;
            let hex = row["raw_f32_hex"].as_str().unwrap();
            let bytes: [u8; 4] =
                std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap());
            entity.native_cloak_displacement =
                crate::util::native_x87::NativeF32Bits::from_bits(u32::from_le_bytes(bytes));
            assert_eq!(
                entity.native_cloak_offset_words(),
                row["offset_words"].as_i64().unwrap() as i32,
                "native control {row}"
            );
            let saved = bincode::serialize(&entity).unwrap();
            let restored: GameEntity = bincode::deserialize(&saved).unwrap();
            assert_eq!(
                restored.native_cloak_displacement,
                entity.native_cloak_displacement
            );
            assert_eq!(
                restored.native_cloak_offset_words(),
                entity.native_cloak_offset_words()
            );
        }
    }

    #[test]
    fn test_new_entity_defaults() {
        let e = GameEntity::test_default(1, "HTNK", "Americans", 30, 40);
        assert_eq!(e.stable_id, 1);
        assert_eq!(e.type_ref, crate::sim::intern::test_intern("HTNK"));
        assert_eq!(e.owner, crate::sim::intern::test_intern("Americans"));
        assert_eq!(e.position.rx, 30);
        assert_eq!(e.position.ry, 40);
        assert_eq!(e.position.z, 0);
        assert_eq!(e.body_facing_current(0), 0);
        assert_eq!(e.health.current(), 100);
        assert_eq!(e.category, EntityCategory::Unit);
        assert_eq!(e.veterancy.rank(), 0);
        assert_eq!(e.sight.range(), 5);
        assert!(e.is_voxel);
        assert!(!e.selected);
        assert!(!e.repairing.is_on());
        assert!(e.locomotor.is_none());
        assert!(e.attack_target.target().is_none());
        assert!(e.radio_contacts.is_empty());
        assert_eq!(e.rally_cell(), None);
        assert!(!e.ai_sell_repair.was_attacked_by_enemy());
        assert!(e.barrel_facing.is_none());
        assert!(e.miner.is_none());
        assert!(!e.mega_mission.is_attack_move());
        assert!(!e.on_bridge);
        assert!(
            !e.in_playfield,
            "TechnoClass ctor @ 0x006F2F5B initializes +0x3D5 false"
        );
    }

    #[test]
    fn gsi_05_10_pending_building_fire_serde_default_is_none() {
        let entity = GameEntity::test_default(1, "NATSLA", "Soviet", 30, 40);
        let mut value = serde_json::to_value(entity).expect("serialize entity");
        value
            .as_object_mut()
            .expect("entity object")
            .remove("pending_building_fire");

        let restored: GameEntity = serde_json::from_value(value).expect("deserialize entity");
        assert!(restored.pending_building_fire.is_none());
    }

    #[test]
    fn gsi_13_06_body_frame_counter_serde_default_is_zero() {
        let entity = GameEntity::test_default(1, "DRON", "Soviet", 30, 40);
        let mut value = serde_json::to_value(entity).expect("serialize entity");
        value
            .as_object_mut()
            .expect("entity object")
            .remove("body_frame_counter");

        let restored: GameEntity = serde_json::from_value(value).expect("deserialize entity");
        assert_eq!(restored.body_frame_counter.get(), 0);
    }

    #[test]
    fn live_contacts_are_per_entity_and_idempotent() {
        let mut contacted = GameEntity::test_default(1, "MTNK", "Americans", 30, 40);
        let unrelated = GameEntity::test_default(2, "MTNK", "Americans", 31, 40);

        contacted.mark_live_contact_with(100);
        contacted.mark_live_contact_with(100);

        assert_eq!(contacted.radio_contacts.len(), 1); // idempotent — one slot used
        assert!(contacted.has_live_contact_with(100));
        assert!(!unrelated.has_live_contact_with(100));

        contacted.clear_live_contact_with(100);
        assert!(!contacted.has_live_contact_with(100));
    }

    #[test]
    fn test_is_alive() {
        let mut e = GameEntity::test_default(1, "E1", "Soviet", 10, 10);
        assert!(e.is_alive());
        e.health.set_for_test(0);
        assert!(!e.is_alive());
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn lifecycle_authority_state_axes_are_independent() {
        let mut e = GameEntity::test_default(1, "E1", "Americans", 3, 3);
        assert_eq!(e.lifecycle, ObjectLifecycle::default());
        assert!(!e.in_logic_vector);
        assert!(!e.removal_facts.dirty_rect_eligible());
        assert!(!e.kill_record.is_recorded());

        // Exercise every combination without deriving one fact from another.
        for bits in 0u8..64 {
            e.lifecycle.set_object_alive_for_test(bits & 0b00_0001 != 0);
            e.lifecycle.set_in_limbo_for_test(bits & 0b00_0010 != 0);
            e.lifecycle.cell_marked = bits & 0b00_0100 != 0;
            e.in_logic_vector = bits & 0b00_1000 != 0;
            e.death.set_dying_for_test(bits & 0b01_0000 != 0);
            e.health
                .set_for_test(if bits & 0b10_0000 != 0 { 1 } else { 0 });

            assert_eq!(e.is_object_alive(), bits & 0b00_0001 != 0);
            assert_eq!(e.lifecycle.in_limbo(), bits & 0b00_0010 != 0);
            assert_eq!(e.lifecycle.cell_marked, bits & 0b00_0100 != 0);
            assert_eq!(e.in_logic_vector, bits & 0b00_1000 != 0);
            assert_eq!(e.death.is_dying(), bits & 0b01_0000 != 0);
            assert_eq!(e.is_alive(), bits & 0b10_0000 != 0);
            assert_eq!(
                e.is_active(),
                e.lifecycle.object_alive() && !e.death.is_dying(),
            );
        }
    }
}

#[cfg(test)]
mod mission_shadow_tests {
    use super::*;
    use crate::sim::mission::state::MissionTestFixture;
    use crate::sim::mission::{MissionDispatchTimer, MissionId, MissionType};

    #[test]
    fn mission_defaults_to_idle_none() {
        let e = GameEntity::test_default(1, "E1", "Americans", 3, 3);
        assert_eq!(e.mission.current(), MissionId::NONE);
        assert_eq!(e.mission.handler_state(), 0);
        assert_eq!(e.mission.ai_counter(), 0);
    }

    /// A map placement authored as Sleep, Sticky or Harmless means "stand still
    /// and do nothing". Those missions have no completion transition, so the
    /// derived idle-Unit Guard reading must never take one back — otherwise
    /// every neutral civilian on a stock skirmish map scans and opens fire.
    #[test]
    fn stand_still_missions_beat_the_derived_guard_reading() {
        for mission in [
            MissionType::Sleep,
            MissionType::Sticky,
            MissionType::Harmless,
        ] {
            for category in [
                crate::map::entities::EntityCategory::Unit,
                crate::map::entities::EntityCategory::Infantry,
                crate::map::entities::EntityCategory::Structure,
            ] {
                let mut e = GameEntity::test_default(1, "CIVBTM", "Neutral", 3, 3);
                e.category = category;
                // No destination, order or target may override this mission.
                e.mission.apply_test_fixture(MissionTestFixture {
                    current: MissionId::from_known(mission),
                    suspended: MissionId::NONE,
                    queued: MissionId::NONE,
                    movement_bypass_latch: 0,
                    handler_state: 0,
                    mission_start_frame: 0,
                    ai_counter: 0,
                    dispatch_timer: MissionDispatchTimer::at_frame(0),
                });
                assert_eq!(
                    e.passive_acquire_mission(),
                    mission,
                    "{category:?} committed to {mission:?}"
                );
            }
        }
    }

    /// Arrival belongs to the Move handler, not the passive consumer. Until
    /// that owner commences Guard, an empty NavCom still reads Move.
    #[test]
    fn an_empty_navigation_goal_does_not_commence_guard() {
        let mut e = GameEntity::test_default(1, "MTNK", "Americans", 3, 3); // Unit
        e.mission.apply_test_fixture(MissionTestFixture {
            current: MissionId::from_known(MissionType::Move),
            suspended: MissionId::NONE,
            queued: MissionId::NONE,
            movement_bypass_latch: 0,
            handler_state: 0,
            mission_start_frame: 0,
            ai_counter: 0,
            dispatch_timer: MissionDispatchTimer::at_frame(0),
        });
        assert_eq!(e.passive_acquire_mission(), MissionType::Move);
    }

    /// Area Guard is a job that never finishes AND has its own handler, which
    /// owns its acquisition. The finished-job bridge must not read it as Guard,
    /// or the object would be admitted to the passive-acquire block as well and
    /// scan twice per cadence. This is the state AI-slot starting units spawn
    /// in, so it is every non-human unit in every skirmish.
    #[test]
    fn committed_area_guard_is_never_bridged_to_guard() {
        for category in [
            crate::map::entities::EntityCategory::Unit,
            crate::map::entities::EntityCategory::Infantry,
        ] {
            let mut e = GameEntity::test_default(1, "MTNK", "Americans", 3, 3);
            e.category = category;
            e.mission.apply_test_fixture(MissionTestFixture {
                current: MissionId::from_known(MissionType::AreaGuard),
                suspended: MissionId::NONE,
                queued: MissionId::NONE,
                movement_bypass_latch: 0,
                handler_state: 0,
                mission_start_frame: 0,
                ai_counter: 0,
                dispatch_timer: MissionDispatchTimer::at_frame(0),
            });
            assert_eq!(
                e.passive_acquire_mission(),
                MissionType::AreaGuard,
                "{category:?} committed to Area Guard"
            );
        }
    }

    #[test]
    fn mission_round_trips_through_serde() {
        // Slice 6 un-skips the field so the queued/suspended interrupt stack +
        // timer survive a save/load. (current/substate are also reconciled from
        // the legacy machines on load; the rest persists as serialized.)
        let mut e = GameEntity::test_default(1, "E1", "Americans", 3, 3);
        e.mission.apply_test_fixture(MissionTestFixture {
            current: MissionId::from_known(MissionType::Attack),
            suspended: MissionId::NONE,
            queued: MissionId::from_known(MissionType::Guard),
            movement_bypass_latch: 0,
            handler_state: 0,
            mission_start_frame: 0,
            ai_counter: 99,
            dispatch_timer: MissionDispatchTimer::at_frame(0),
        });
        let json = serde_json::to_string(&e).expect("serialize entity");
        assert!(
            json.contains("mission"),
            "mission must round-trip — present in serialized form"
        );
        let restored: GameEntity = serde_json::from_str(&json).expect("deserialize entity");
        assert_eq!(
            restored.mission.current(),
            MissionId::from_known(MissionType::Attack)
        );
        assert_eq!(
            restored.mission.queued(),
            MissionId::from_known(MissionType::Guard)
        );
        assert_eq!(restored.mission.ai_counter(), 99);
    }
}
