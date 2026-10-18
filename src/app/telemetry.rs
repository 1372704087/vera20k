//! Public telemetry façade for out-of-crate tooling.
//!
//! The engine's own wiring reaches these types through their crate-internal
//! paths; side binaries (the headless `telemetry-serve` demo) import them
//! here so the internal module layout can stay private.

pub use crate::sim::cheats::CheatFlags;
pub use crate::app::match_runtime::telemetry::TickTimingRing;
pub use crate::app::telemetry_server::{
    DEFAULT_PORT, DEFAULT_WEB_ROOT, ENV_ADDR, ENV_ALLOW_REMOTE, ENV_WEB_ROOT, TelemetryServer,
    start_from_env,
};

/// One browser-originated debug command — the dashboard's switches.
///
/// Presentation-level only: none of these touch simulation state that tests
/// or replays hash. `FrameStep` advances one tick and is honoured only while
/// debug-paused, mirroring the `Ctrl+Shift+.` hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum DebugCommand {
    /// Flip one boolean overlay (`{"cmd":"toggle","switch":"heightmap"}`).
    Toggle { switch: DebugSwitch },
    /// Step the terrain-cost SpeedType one notch (`{"cmd":"cycle_speed_type","forward":true}`).
    CycleSpeedType { forward: bool },
    /// Advance one sim tick while debug-paused (`{"cmd":"frame_step"}`).
    FrameStep,
    /// Replace the trainer switchboard (`{"cmd":"set_cheats","flags":{…}}`).
    /// The panel always sends the whole block, so the message is idempotent
    /// and two windows cannot half-apply a change.
    SetCheats { flags: CheatFlags },
    /// Fire a one-shot trainer action (`{"cmd":"cheat_action","action":"…"}`).
    CheatAction { action: CheatAction },
}

/// The trainer buttons that act once instead of staying on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheatAction {
    /// Restore every owned object to its `Strength=`.
    HealAll,
    /// Promote every owned combat object to elite.
    VeteranAll,
    /// Charge every granted Super the house owns.
    SuperweaponsReady,
}

/// The overlays the dashboard may switch (the Ctrl+Shift chord's targets).
///
/// Fog of war is deliberately absent: the trainer's `reveal_map` switch owns
/// the shroud (`sandbox_full_visibility`), and the `^+V` / F10 chord routes
/// through the trainer so both controls read the same fact. A second web
/// toggle would only be able to disagree with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugSwitch {
    /// Terrain-cost / PathGrid wash (`^+P` / F9).
    TerrainCosts,
    /// Heightmap overlay, blue = bridge deck (`^+K`).
    Heightmap,
    /// Cell grid, blue = terrain / yellow = overlay (`^+L`).
    CellGrid,
    /// Per-entity debug event logging (`^+X`).
    UnitInspector,
    /// Debug pause (`^+J`).
    DebugPause,
}
