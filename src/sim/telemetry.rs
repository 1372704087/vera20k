//! Dashboard telemetry: the read-only snapshot schema consumed by the remote
//! debugger frontend (the browser visualization client).
//!
//! `TelemetrySnapshot` is the wire contract between the engine and the
//! dashboard. It is deliberately *derived data*: `Simulation::telemetry_snapshot`
//! walks live state on demand and returns a plain, versioned,
//! serde-serializable struct. Nothing in this module is stored on
//! `Simulation`, serialized into authoritative state, or hashed — the same
//! discipline `debug_event_log` follows, so telemetry can never perturb
//! determinism, saves or replay.
//!
//! ## Design notes
//!
//! - **Pull model.** The capture is one pass over the entity store, sampled
//!   by the app at dashboard rate (5–10 Hz), never per tick. No hot path is
//!   instrumented and no counter is maintained while nobody is watching.
//! - **Wire format.** The app serializes with `serde_json` (text WebSocket
//!   frames) and the browser parses directly; the payload is aggregate data
//!   plus capped spots, not the 20k-unit worst case per frame. `bincode`
//!   remains a drop-in alternative through the same derives when a future
//!   payload size demands it.
//! - **Spots.** `spots` is the dashboard's map view: one entry per entity,
//!   flattened to f32 world cells (cell + sub-cell). `owner_index` addresses
//!   `houses` (u16::MAX = no house row). Capped at `TELEMETRY_MAX_SPOTS` in
//!   store order; viewport culling and LOD are a later, camera-aware step.
//!   Constructor-held (limbo) entities sit at (0,0) by native semantics —
//!   `store_spawned_limbo` resets the CoordStruct to the off-map default —
//!   so the map view shows them piled at the origin.
//! - **Deferred: pathfinding health counters.** `DebugEventKind`'s locomotor
//!   variants (Repath / Blocked / StuckAbort / PhaseChange) have no emission
//!   sites in sim today: the only `push_debug_event` callers are the miner
//!   and special-movement systems, and they hold a `GameEntity`, not the
//!   `Simulation`, so a global counter has no honest home yet. Global
//!   pathfinding counters land with the locomotor instrumentation that
//!   constructs those events; until then this schema carries no zeros that
//!   pretend to be measurements.
//! - **Wire stability.** Houses and spots are `Vec`s in store/interned-id
//!   order — deterministic across peers and runs. New fields arrive as
//!   `Option` or new sections under a bumped `TELEMETRY_SCHEMA_VERSION`.
//! - **Debug panel port.** `debug` carries the former egui Terrain Debug /
//!   Event History panel data: cursor cell and walkability, entities at the
//!   cursor, selected movers' paths, miner state and per-entity event logs.
//!   The app layer fills it from cursor/selection/inspector state; the
//!   simulation itself never does.
//!
//! ## Dependency rules
//! - Part of sim/; no render/ui/audio/net dependencies.

use std::collections::BTreeMap;

use crate::map::entities::EntityCategory;
use crate::sim::intern::InternedId;
use crate::sim::world::Simulation;

/// Wire schema version. Bump on any breaking change to `TelemetrySnapshot`.
///
/// v13: `profile` gained `phases_ms`, the per-subsystem frame ledger the report
/// table lists. The four pie buckets are derived from it, not added alongside.
/// v14: `render` — the per-phase breakdown of one in-game render.
/// v15: `logs` — incremental log records for the dashboard's log view.
pub const TELEMETRY_SCHEMA_VERSION: u32 = 15;

/// Spot ceiling per snapshot: the project's headline unit scale. Beyond it
/// the capture keeps the first N in store order (deterministic) and the
/// dashboard renders a truncated world rather than an unbounded payload.
pub const TELEMETRY_MAX_SPOTS: usize = 20_000;

/// Per-house roster ceiling. The secondary screen lists what a house actually
/// fields; a long tail of one-off types belongs on a drill-down, not the wall.
pub const TELEMETRY_MAX_ROSTER: usize = 16;

/// Live population of one entity category.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CategoryCounts {
    pub unit: u32,
    pub infantry: u32,
    pub aircraft: u32,
    pub structure: u32,
}

impl CategoryCounts {
    /// All four categories summed.
    pub fn total(&self) -> u32 {
        self.unit + self.infantry + self.aircraft + self.structure
    }

    /// Count one entity into its category bucket.
    pub(crate) fn add(&mut self, category: EntityCategory) {
        match category {
            EntityCategory::Unit => self.unit += 1,
            EntityCategory::Infantry => self.infantry += 1,
            EntityCategory::Aircraft => self.aircraft += 1,
            EntityCategory::Structure => self.structure += 1,
        }
    }
}

/// One house's identity and live population, as the dashboard sees it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HousePopulation {
    /// House name resolved through the interner (e.g. "Americans").
    pub name: String,
    /// Rules-owned side index: the position in the loaded `[Sides]` section.
    /// Not a fixed enum — retail order differs from test fixtures, so the app
    /// also sends `side_name`, the authoritative spelling.
    pub side_index: u8,
    /// The `[Sides]` key this house belongs to ("Allied", "ThirdSide",
    /// "Civilian", "Mutant", …), resolved from the loaded rules. `None` when
    /// no rule set is available.
    pub side_name: Option<String>,
    /// The house's AI difficulty ("hard" / "normal" / "easy"). Human houses
    /// keep Normal; the dashboard only shows this for computer houses.
    pub difficulty: String,
    pub is_human: bool,
    /// True for the non-contending houses the engine keeps for map objects:
    /// `Neutral` and `Special` own scenery and civilians but never take part
    /// in the match, so the dashboard filters them out of any lineup. This is
    /// the same flag `Simulation::contending_house_count` counts.
    pub passive: bool,
    pub counts: CategoryCounts,
    /// The house's own team colour, RGB, from the rules `[Colors]` scheme the
    /// player picked — the colour the game draws its units in. `None` for
    /// houses without a player colour (Neutral, Special) and until the app
    /// fills it: the simulation does not own colour schemes, so the app
    /// resolves this from the launch session's house-colour map.
    pub color: Option<[u8; 3]>,
    /// Live cash balance (native house `+0x30C`).
    pub credits: i32,
    /// Running total taken by `Spend_Money` (native house `+0x2DC`).
    pub spent: i32,
    /// House score (native house `+0x54E8`): deposits, kills and captures.
    pub score: i32,
    /// Whether this house has been eliminated.
    pub defeated: bool,
    /// Historical primary base cell (native `HouseClass+0x5490`); `None` until
    /// a building lifecycle boundary publishes it.
    pub base_center: Option<(u16, u16)>,
    /// Live unit-type roster for this house, largest first. Capped at
    /// `TELEMETRY_MAX_ROSTER`: the secondary screen lists the bulk, not the
    /// long tail.
    pub roster: Vec<TypeCount>,
}

/// One object type held by a house, by live count. The secondary screen's
/// roster line ("GRIZZLY ×6"); derived from the entity store, never stored.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeCount {
    /// Object type as the rules spell it ("GRIZZLY", "GAPOWR").
    pub type_name: String,
    pub category: EntityCategory,
    pub count: u32,
}

/// One entity's spot for the dashboard's map view. Presentation-derived:
/// sub-cell precision flattened to f32 world cells.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntitySpot {
    /// World cell X = cell rx + sub-cell offset (0..256) / 256.
    pub x: f32,
    /// World cell Y = cell ry + sub-cell offset (0..256) / 256.
    pub y: f32,
    /// Elevation level (0 = ground).
    pub z: u8,
    pub category: EntityCategory,
    /// Index into `TelemetrySnapshot::houses`; u16::MAX when the entity's
    /// owner has no house row.
    pub owner_index: u16,
}

/// Frame-timing sample, filled by the app layer (the simulation never
/// measures wall time). `budget_utilization` is `tick_ms` over `budget_ms`;
/// values above 1.0 mean the tick cost exceeded its budget and the pacer can
/// no longer hold the tick rate.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TimingSample {
    /// The most recent tick's cost, in milliseconds.
    pub tick_ms: f32,
    /// Median tick cost over the app's sampling window.
    pub p50_ms: f32,
    /// 99th-percentile tick cost over the app's sampling window.
    pub p99_ms: f32,
    /// The frame's tick budget in milliseconds (the pacer's tick interval).
    pub budget_ms: f32,
    /// `tick_ms` divided by `budget_ms`.
    pub budget_utilization: f32,
    /// Presentation frames per second, from the app's render-frame timer.
    /// 0 while no sample has been taken (menu, or before the first redraw).
    pub fps: f32,
    /// Committed simulation ticks per second, measured over the app's
    /// wall-clock window. 0 while the runtime is paused or frozen.
    pub tick_hz: f32,
}

/// One entity's compact description for the dashboard's cursor list.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EntityDesc {
    pub type_name: String,
    /// "Unit" / "Inf" / "Bld" / "Air" — the egui panel's short labels.
    pub category: String,
    pub owner: String,
    pub hp: i32,
    pub max_hp: i32,
}

/// One SpeedType's land row at the cursor cell; cost 0 means impassable.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LandRow {
    pub speed_type: String,
    pub cost: u8,
}

/// One step of a selected mover's remaining path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PathStep {
    pub cell: (u16, u16),
    pub walkable: bool,
    /// The mover's next cell (the egui panel's `>` marker).
    pub first: bool,
}

/// Miner state for a selected harvester, mirroring the egui panel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MinerDebug {
    pub kind: String,
    pub state: String,
    pub cargo: u32,
    pub capacity: u32,
    pub stage: i32,
    pub rate: i32,
    pub harvesting: bool,
    pub archived_ore: Option<(u16, u16)>,
    pub teleport_active: bool,
}

/// One formatted debug event; the text mirrors the egui Event History
/// panel's formatting (without its colours).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DebugEventEntry {
    pub tick: u32,
    pub text: String,
}

/// One selected entity's debug state: identity, position, path and (when
/// the inspector was active) its event log.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelectedDebug {
    pub stable_id: u64,
    pub type_name: String,
    pub cell: (u16, u16),
    /// Sub-cell offsets, 0..256.
    pub sub: (u16, u16),
    pub moving: bool,
    pub path_steps_left: u32,
    pub path_blocked: bool,
    pub goal: Option<(u16, u16)>,
    /// The first steps of the remaining path, the mover's next cell first.
    pub path_preview: Vec<PathStep>,
    pub miner: Option<MinerDebug>,
    /// Whether the per-entity event log was active for this entity.
    pub inspector_active: bool,
    /// The most recent events, newest first (capped).
    pub events: Vec<DebugEventEntry>,
}

/// The dashboard's port of the egui Terrain Debug / Event History panels.
/// Filled by the app layer from live cursor, selection, inspector and
/// overlay-switch state; `None` when no match is running.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DebugPanel {
    /// Cell under the cursor, when the cursor maps to the map.
    pub cursor_cell: Option<(u16, u16)>,
    /// Elevation level at the cursor cell.
    pub cursor_elevation: u8,
    /// PathGrid dimensions, when one exists.
    pub grid: Option<(u16, u16)>,
    /// Whether the cursor cell is walkable.
    pub cursor_walkable: Option<bool>,
    /// Land rows for the five locomotor SpeedTypes at the cursor.
    pub land_rows: Vec<LandRow>,
    /// The terrain-cost overlay's active SpeedType (`[` `]` cycle it).
    pub active_speed_type: String,
    /// VXL sprites rendered by the last atlas build.
    pub vxl_sprites: Option<u32>,
    /// Entities standing on the cursor cell.
    pub entities_at_cursor: Vec<EntityDesc>,
    /// The structure whose footprint covers the cursor, if any.
    pub footprint: Option<String>,
    /// Selected entities, in store order.
    pub selected: Vec<SelectedDebug>,
    /// Debug-overlay switch states, so the dashboard's switches show the
    /// engine's real state (they may also be toggled by the game hotkeys).
    pub overlays: OverlayFlags,
}

/// Debug-overlay switch states — the dashboard's readback for its switches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OverlayFlags {
    /// Terrain-cost / PathGrid wash (`^+P` / F9).
    pub terrain_costs: bool,
    /// Heightmap overlay (`^+K`).
    pub heightmap: bool,
    /// Cell grid overlay (`^+L`).
    pub cell_grid: bool,
    /// Fog of war (the engine stores the inverse: `sandbox_full_visibility`).
    pub fog: bool,
    /// Per-entity debug event logging (`^+X`).
    pub unit_inspector: bool,
    /// Foot-path step overlay (`^+O`).
    pub foot_path: bool,
    /// Debug pause (`^+J`).
    pub paused: bool,
}

/// Process-level resources, filled by the app layer: the simulation has no
/// business knowing how much CPU or memory the host spends on it. `None` on
/// platforms the app cannot sample.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessStats {
    /// Share of the whole machine's CPU capacity, 0..100 across all cores.
    pub cpu_percent: f32,
    /// Logical cores the percentage is measured against.
    pub cpu_cores: u32,
    /// Resident working set in MiB.
    pub working_set_mb: f64,
    /// High-water working set in MiB.
    pub peak_working_set_mb: f64,
    /// Live thread count.
    pub threads: u32,
    /// The adapter selected at startup (`None` before the GPU is up).
    pub gpu_adapter: Option<String>,
    /// Palette-index texture bytes this process keeps resident (sprite and
    /// unit atlas pages) in MiB. This is **our own** texture footprint, not the
    /// card's total usage: swap-chain, driver and other allocations are not
    /// visible from inside the process, and hardware utilization percentages
    /// need vendor driver counters that this build does not read.
    pub gpu_texture_mb: f64,
}

/// Rolling means of one in-game render, in milliseconds.
///
/// App-filled: the simulation owns no render, so this rides the snapshot the
/// same way `timing` and `process` do. Order matches `RenderPhases::spans`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RenderTimingReport {
    /// Renders averaged.
    pub frames: u32,
    /// Phase 1 — terrain, overlays, entities.
    pub world_ms: f32,
    /// Phase 2 — pathgrid / cell grid / heightmap overlays.
    pub debug_ms: f32,
    /// Phase 3 — shroud ABuffer rebuild.
    pub shroud_ms: f32,
    /// Phase 4 — minimap update + UI instances.
    pub ui_ms: f32,
    /// Phase 5 — sidebar instances.
    pub sidebar_ms: f32,
    /// Phase 6 — instance upload to the GPU buffer pool.
    pub upload_ms: f32,
    /// Prep before phase 1 and everything after phase 6.
    pub other_ms: f32,
}

/// One log record on the wire.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LogRecord {
    /// Monotonic record number; the dashboard uses it to drop duplicates.
    pub seq: u64,
    /// 1 = error, 2 = warn, 3 = info, 4 = debug, 5 = trace.
    pub level: u8,
    pub target: String,
    pub message: String,
    /// Milliseconds since logging started.
    pub t_ms: u64,
}

/// Log records captured since the previous snapshot.
///
/// Incremental on purpose: the file log runs to hundreds of megabytes during
/// map load, so the wire carries only what the reader has not seen.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LogBatch {
    /// Records the ring evicted before they could be sent.
    pub dropped: u64,
    pub lines: Vec<LogRecord>,
}

/// One dashboard sample: the whole wire payload per snapshot.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TelemetrySnapshot {
    pub version: u32,
    /// Authoritative tick the sample was taken on.
    pub tick: u64,
    /// Wrapping frame counter committed with that tick.
    pub binary_frame: u32,
    /// Map dimensions in cells; frames the dashboard's ground plane.
    pub map_width: u16,
    pub map_height: u16,
    /// Every entity stored in the entity store, limbo (cargo, paradrop)
    /// included. The sum of house counts is lower only when an entity names
    /// an owner with no house row — a data anomaly the dashboard should
    /// surface, not hide.
    pub entities_total: u32,
    /// In `BTreeMap` (interned-id) order: deterministic across peers.
    pub houses: Vec<HousePopulation>,
    /// One spot per entity in store order, capped at `TELEMETRY_MAX_SPOTS`.
    pub spots: Vec<EntitySpot>,
    /// App-filled frame timing; `None` until the app has sampled a frame.
    pub timing: Option<TimingSample>,
    /// App-filled egui-panel port (cursor, selection, event log); `None`
    /// until the app layer fills it.
    pub debug: Option<DebugPanel>,
    /// App-filled process resources (CPU, memory, threads, GPU texture).
    pub process: Option<ProcessStats>,
    /// Live frame-phase profile since this match started. The same accumulator
    /// the end-of-match report reads, so the wall shows the running split and
    /// the report freezes it.
    pub profile: Option<crate::sim::frame_profile::FrameProfileReport>,
    /// App-filled per-phase render breakdown; `None` until the app has rendered
    /// a game frame.
    pub render: Option<RenderTimingReport>,
    /// App-filled log records since the previous snapshot.
    pub logs: Option<LogBatch>,
}

impl Simulation {
    /// Capture the live dashboard snapshot. One pass over the entity store;
    /// derived data only — never stored, serialized or hashed.
    pub fn telemetry_snapshot(&self) -> TelemetrySnapshot {
        // House rows first, so entity spots can address them by index.
        let mut owner_index: BTreeMap<InternedId, u16> = BTreeMap::new();
        let mut houses: Vec<HousePopulation> = Vec::with_capacity(self.houses.len());
        for (index, (owner, house)) in self.houses.iter().enumerate() {
            owner_index.insert(*owner, index as u16);
            houses.push(HousePopulation {
                name: self.interner.resolve(house.name).to_string(),
                side_index: house.side_index,
                side_name: None,
                difficulty: house.difficulty.telemetry_name().to_string(),
                is_human: house.is_human,
                passive: house.multiplay_passive,
                counts: CategoryCounts::default(),
                // The app resolves the launch session's [Colors] scheme.
                color: None,
                credits: house.economy.credits(),
                spent: house.economy.spent_credits(),
                score: house.economy.score(),
                defeated: house.is_defeated,
                base_center: house.base_center,
                roster: Vec::new(),
            });
        }

        // One entity pass: category tallies per owner plus the capped spots.
        // The same pass counts live types per house, so the secondary screen's
        // roster costs no extra traversal.
        let mut per_owner: BTreeMap<InternedId, CategoryCounts> = BTreeMap::new();
        let mut per_owner_types: BTreeMap<(InternedId, InternedId), (EntityCategory, u32)> =
            BTreeMap::new();
        let mut spots: Vec<EntitySpot> = Vec::new();
        let mut entities_total = 0u32;
        for entity in self.entities().values() {
            entities_total += 1;
            let owner = entity.owner();
            let category = entity.category();
            per_owner.entry(owner).or_default().add(category);
            per_owner_types
                .entry((owner, entity.type_ref()))
                .or_insert((category, 0))
                .1 += 1;
            if spots.len() < TELEMETRY_MAX_SPOTS {
                let position = &entity.position;
                spots.push(EntitySpot {
                    x: f32::from(position.rx()) + position.sub_x().to_num::<f32>() / SUB_CELL_UNITS,
                    y: f32::from(position.ry()) + position.sub_y().to_num::<f32>() / SUB_CELL_UNITS,
                    z: position.z(),
                    category,
                    owner_index: owner_index.get(&owner).copied().unwrap_or(u16::MAX),
                });
            }
        }

        // Houses and their owners share the BTreeMap iteration order.
        for (house, owner) in houses.iter_mut().zip(self.houses.keys()) {
            house.counts = per_owner.get(owner).copied().unwrap_or_default();
            let mut roster: Vec<TypeCount> = per_owner_types
                .iter()
                .filter(|((entity_owner, _), _)| entity_owner == owner)
                .map(|((_, type_ref), (category, count))| TypeCount {
                    type_name: self.interner.resolve(*type_ref).to_string(),
                    category: *category,
                    count: *count,
                })
                .collect();
            // Largest first, name as the tie-break so the order never depends
            // on interning order.
            roster.sort_by(|a, b| {
                b.count
                    .cmp(&a.count)
                    .then_with(|| a.type_name.cmp(&b.type_name))
            });
            roster.truncate(TELEMETRY_MAX_ROSTER);
            house.roster = roster;
        }

        TelemetrySnapshot {
            version: TELEMETRY_SCHEMA_VERSION,
            tick: self.session.tick,
            binary_frame: self.session.binary_frame,
            map_width: self.session.map_width,
            map_height: self.session.map_height,
            entities_total,
            houses,
            spots,
            timing: None,
            debug: None,
            process: None,
            profile: None,
            render: None,
            logs: None,
        }
    }
}

/// Sub-cell units per cell: the `Position` sub-cell range is 0..256 with 128
/// at the cell centre.
const SUB_CELL_UNITS: f32 = 256.0;
