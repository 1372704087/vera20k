//! Browser debug commands → app-state switches.
//!
//! The dashboard's switches send [`DebugCommand`]s over the telemetry socket;
//! [`drain_and_apply`] runs once per frame on the game thread and routes them
//! through the same toggles the Ctrl+Shift chord uses, so both input paths
//! share one implementation. Presentation-level only: nothing here writes
//! simulation state that tests or replays hash — the frame step requests the
//! same one-shot advance the `.` hotkey requests.

use crate::app::state::AppState;
use crate::app::telemetry::{DebugCommand, DebugSwitch};

/// Drain pending browser commands and apply them (once per frame).
pub(crate) fn drain_and_apply(state: &mut AppState) {
    // Take the borrow only long enough to drain; the toggles below need
    // `&mut state`.
    let commands = {
        let Some(server) = state.diag.telemetry_server.as_ref() else {
            return;
        };
        server.drain_commands()
    };
    if commands.is_empty() {
        return;
    }
    for command in commands {
        apply(state, command);
    }
    // Echo the new switch state immediately. The tick-driven publish would
    // otherwise leave the dashboard's switches stale whenever the sim is not
    // advancing — exactly the case where the user is clicking them from
    // another window (an unfocused game freezes the runtime).
    crate::app::telemetry_capture::publish_now(state);
}

#[cfg(feature = "dev-overlays")]
fn apply(state: &mut AppState, command: DebugCommand) {
    use crate::app::input::dispatch;

    match command {
        DebugCommand::Toggle { switch } => match switch {
            DebugSwitch::TerrainCosts => dispatch::toggle_pathgrid_overlay(state),
            DebugSwitch::Heightmap => {
                state.diag.debug_show_heightmap = !state.diag.debug_show_heightmap;
                log::info!(
                    "Debug height map overlay (web): {}",
                    if state.diag.debug_show_heightmap {
                        "ON"
                    } else {
                        "OFF"
                    }
                );
            }
            DebugSwitch::CellGrid => {
                state.diag.debug_show_cell_grid = !state.diag.debug_show_cell_grid;
                log::info!(
                    "Debug cell grid overlay (web): {}",
                    if state.diag.debug_show_cell_grid {
                        "ON"
                    } else {
                        "OFF"
                    }
                );
            }
            DebugSwitch::Fog => {
                state.match_state.sandbox_full_visibility =
                    !state.match_state.sandbox_full_visibility;
                log::info!(
                    "Fog of war (web): {}",
                    if state.match_state.sandbox_full_visibility {
                        "OFF (full visibility)"
                    } else {
                        "ON"
                    }
                );
            }
            DebugSwitch::UnitInspector => dispatch::toggle_unit_inspector(state),
            DebugSwitch::DebugPause => dispatch::toggle_debug_pause(state),
        },
        DebugCommand::CycleSpeedType { forward } => {
            if state.diag.debug_show_pathgrid {
                let current =
                    crate::app::diagnostics::debug_overlays::resolve_debug_speed_type(state);
                let next = if forward {
                    current.cycle_next()
                } else {
                    current.cycle_prev()
                };
                state.diag.debug_terrain_cost_speed_type = Some(next);
                log::info!("Terrain cost overlay (web): {}", next.name());
            }
        }
        DebugCommand::FrameStep => {
            // One-shot and only while paused — exactly the `.` hotkey.
            if state.match_state.paused() {
                state.diag.debug_frame_step_requested = true;
            }
        }
    }
}

#[cfg(not(feature = "dev-overlays"))]
fn apply(_state: &mut AppState, _command: DebugCommand) {
    // Release builds carry no debug overlays; telemetry still runs, but the
    // dashboard's switches have nothing to drive.
}
