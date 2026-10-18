/**
 * Schema mirror of `src/sim/telemetry.rs` (v12); keep both in step.
 */

export const SCHEMA_VERSION = 17;

export type EntityCategory = "Unit" | "Infantry" | "Aircraft" | "Structure";

export interface CategoryCounts {
  unit: number;
  infantry: number;
  aircraft: number;
  structure: number;
}

export interface HousePopulation {
  name: string;
  /** Position in the loaded [Sides] section — not a fixed enum. */
  side_index: number;
  /** The [Sides] key this house belongs to ("Allied", "ThirdSide", …). */
  side_name: string | null;
  /** AI difficulty ladder: "hard" (brutal) | "normal" | "easy". */
  difficulty: string;
  is_human: boolean;
  /** Non-contending house (`Neutral`, `Special`): owns map scenery and
   *  civilians but never plays. Lineups filter it out. */
  passive: boolean;
  counts: CategoryCounts;
  /** The house's own team colour from the rules [Colors] scheme (RGB 0-255);
   *  null for houses without a player colour. */
  color: [number, number, number] | null;
  /** Live cash balance. */
  credits: number;
  power_output: number;
  power_drain: number;
  low_power: boolean;
  /** Running total spent. */
  spent: number;
  /** House score: deposits, kills and captures. */
  score: number;
  /** Whether this house has been eliminated. */
  defeated: boolean;
  /** Historical primary base cell; null until a building publishes it. */
  base_center: [number, number] | null;
  /** Live roster, largest first, capped at 16 types. */
  roster: TypeCount[];
}

/** One object type held by a house, by live count. */
export interface TypeCount {
  type_name: string;
  category: EntityCategory;
  count: number;
}

export interface EntitySpot {
  x: number;
  y: number;
  z: number;
  category: EntityCategory;
  owner_index: number;
}

export interface TimingSample {
  tick_ms: number;
  p50_ms: number;
  p99_ms: number;
  budget_ms: number;
  budget_utilization: number;
  /** Presentation frames per second (0 when no sample has been taken). */
  fps: number;
  /** Committed simulation ticks per second (0 while paused or frozen). */
  tick_hz: number;
}

export interface EntityDesc {
  type_name: string;
  category: string;
  owner: string;
  hp: number;
  max_hp: number;
}

export interface LandRow {
  speed_type: string;
  cost: number;
}

export interface PathStep {
  cell: [number, number];
  walkable: boolean;
  first: boolean;
}

export interface MinerDebug {
  kind: string;
  state: string;
  cargo: number;
  capacity: number;
  stage: number;
  rate: number;
  harvesting: boolean;
  archived_ore: [number, number] | null;
  teleport_active: boolean;
}

export interface DebugEventEntry {
  tick: number;
  text: string;
}

export interface SelectedDebug {
  stable_id: number;
  type_name: string;
  cell: [number, number];
  sub: [number, number];
  moving: boolean;
  path_steps_left: number;
  path_blocked: boolean;
  goal: [number, number] | null;
  path_preview: PathStep[];
  miner: MinerDebug | null;
  inspector_active: boolean;
  events: DebugEventEntry[];
}

export interface DebugPanel {
  cursor_cell: [number, number] | null;
  cursor_elevation: number;
  grid: [number, number] | null;
  cursor_walkable: boolean | null;
  land_rows: LandRow[];
  active_speed_type: string;
  vxl_sprites: number | null;
  entities_at_cursor: EntityDesc[];
  footprint: string | null;
  selected: SelectedDebug[];
  overlays: OverlayFlags;
}

/** Debug-overlay switch states, mirrored from the engine's real state. */
export interface OverlayFlags {
  terrain_costs: boolean;
  heightmap: boolean;
  cell_grid: boolean;
  fog: boolean;
  unit_inspector: boolean;
  foot_path: boolean;
  paused: boolean;
}

/** Process-level resources, filled by the app layer. */
export interface ProcessStats {
  /** Share of the whole machine's CPU capacity, 0..100 across all cores. */
  cpu_percent: number;
  cpu_cores: number;
  /** Resident working set, MiB. */
  working_set_mb: number;
  /** High-water working set, MiB. */
  peak_working_set_mb: number;
  threads: number;
  gpu_adapter: string | null;
  /** Our own resident atlas texture bytes, MiB — not the card's total usage. */
  gpu_texture_mb: number;
}

/** Running frame-phase split, straight from the engine's phase stamps. */
export interface FrameProfileReport {
  frames: number;
  mean_frame_ms: number;
  peak_frame_ms: number;
  logic_ms: number;
  objects_ms: number;
  vision_ms: number;
  tail_ms: number;
  unmarked_ms: number;
  /** Per-subsystem ledger, in the engine's `PHASE_KEYS` order (schema v13).
   *  The four buckets above are sums over this array. */
  phases_ms: number[];
}

/** Per-phase means of one in-game render, in milliseconds (schema v14). */
export interface RenderTimingReport {
  frames: number;
  world_ms: number;
  debug_ms: number;
  shroud_ms: number;
  ui_ms: number;
  sidebar_ms: number;
  upload_ms: number;
  other_ms: number;
}

/** The dashboard trainer's switches, as the engine reports them (schema v16). */
export interface CheatReport {
  infinite_money: boolean;
  money_target: number;
  infinite_power: boolean;
  instant_build: boolean;
  invincible: boolean;
  damage_scale: number;
  reveal_map: boolean;
  /** The house the switches act on, resolved; null before they are used. */
  owner: string | null;
  /** True while the power switch is overriding a real deficit. */
  power_forced: boolean;
}

/** One log record on the wire (schema v15). */
export interface LogRecord {
  seq: number;
  /** 1 = error, 2 = warn, 3 = info, 4 = debug, 5 = trace. */
  level: number;
  target: string;
  message: string;
  t_ms: number;
}

/** Records captured since the previous snapshot. */
export interface LogBatch {
  /** Records the ring evicted before they could be sent. */
  dropped: number;
  lines: LogRecord[];
}

export interface TelemetrySnapshot {
  version: number;
  tick: number;
  binary_frame: number;
  map_width: number;
  map_height: number;
  entities_total: number;
  houses: HousePopulation[];
  spots: EntitySpot[];
  timing: TimingSample | null;
  debug: DebugPanel | null;
  process: ProcessStats | null;
  profile: FrameProfileReport | null;
  render: RenderTimingReport | null;
  logs: LogBatch | null;
  cheats: CheatReport;
}
