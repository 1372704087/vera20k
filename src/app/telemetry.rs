//! Public telemetry façade for out-of-crate tooling.
//!
//! The engine's own wiring reaches these types through their crate-internal
//! paths; side binaries (the headless `telemetry-serve` demo) import them
//! here so the internal module layout can stay private.

pub use crate::app::match_runtime::telemetry::TickTimingRing;
pub use crate::app::telemetry_server::{
    DEFAULT_PORT, DEFAULT_WEB_ROOT, ENV_ADDR, ENV_WEB_ROOT, TelemetryServer, start_from_env,
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

/// The trainer switches as they cross the wire.
///
/// Every field is `#[serde(default)]`: a panel that has not been updated yet
/// sends only the switches it knows, and the ones it omits read as "off"
/// rather than failing the whole command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct CheatFlags {
    pub infinite_money: bool,
    /// Wallet floor the money switch holds. Clamped at zero on arrival.
    pub money_target: i32,
    pub infinite_power: bool,
    pub instant_build: bool,
    pub invincible: bool,
    /// Damage multiplier; 0 and 1 both mean "no change".
    pub damage_scale: u16,
    pub reveal_map: bool,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugSwitch {
    /// Terrain-cost / PathGrid wash (`^+P` / F9).
    TerrainCosts,
    /// Heightmap overlay, blue = bridge deck (`^+K`).
    Heightmap,
    /// Cell grid, blue = terrain / yellow = overlay (`^+L`).
    CellGrid,
    /// Fog of war (`^+V` / F10). The flag is the fog itself, not visibility.
    Fog,
    /// Per-entity debug event logging (`^+X`).
    UnitInspector,
    /// Debug pause (`^+J`).
    DebugPause,
}
