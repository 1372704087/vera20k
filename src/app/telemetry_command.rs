//! Browser debug commands → app-state switches.
//!
//! The dashboard's switches send [`DebugCommand`]s over the telemetry socket;
//! [`drain_and_apply`] runs once per frame on the game thread and routes them
//! through the same toggles the Ctrl+Shift chord uses, so both input paths
//! share one implementation. Presentation-level only: nothing here writes
//! simulation state that tests or replays hash — the frame step requests the
//! same one-shot advance the `.` hotkey requests.

use crate::app::state::AppState;
use crate::app::telemetry::{CheatAction, CheatFlags, DebugCommand, DebugSwitch};

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
        // The trainer is not a dev overlay: it writes simulation state the
        // shipping build also has, so it is handled before the feature gate
        // and works in every build that serves the dashboard.
        if handle_trainer(state, command) {
            continue;
        }
        apply(state, command);
    }
    // Echo the new switch state immediately. The tick-driven publish would
    // otherwise leave the dashboard's switches stale whenever the sim is not
    // advancing — exactly the case where the user is clicking them from
    // another window (an unfocused game freezes the runtime).
    crate::app::telemetry_capture::publish_now(state);
}

/// Apply the trainer switches, returning `false` for commands that are not
/// trainer commands.
fn handle_trainer(state: &mut AppState, command: DebugCommand) -> bool {
    match command {
        DebugCommand::SetCheats { flags } => {
            set_cheats(state, flags);
            true
        }
        DebugCommand::CheatAction { action } => {
            run_cheat_action(state, action);
            true
        }
        _ => false,
    }
}

/// Replace the switchboard. Sent whole every time, so this is a pure
/// assignment plus the one side effect the simulation cannot see on its own:
/// the shroud override is an app-level flag.
fn set_cheats(state: &mut AppState, flags: CheatFlags) {
    let reveal_changed = {
        let Some(runtime) = state.match_state.sim_runtime.as_mut() else {
            return;
        };
        let sim = &mut runtime.simulation;
        let was_revealed = sim.cheats.reveal_map;
        sim.set_cheats(crate::sim::cheats::Cheats::switches(
            flags.infinite_money,
            flags.money_target.max(0),
            flags.infinite_power,
            flags.instant_build,
            flags.invincible,
            flags.damage_scale,
            flags.reveal_map,
        ));
        log::info!(
            "Trainer: money {} target {} · power {} · instant build {} · \
             invincible {} · damage x{} · shroud {}",
            flags.infinite_money,
            flags.money_target,
            flags.infinite_power,
            flags.instant_build,
            flags.invincible,
            flags.damage_scale,
            if flags.reveal_map { "off" } else { "on" },
        );
        was_revealed != flags.reveal_map
    };
    if reveal_changed {
        state.match_state.sandbox_full_visibility = flags.reveal_map;
    }
}

/// Run a one-shot trainer button now, not on the next tick: a paused match
/// still has to answer "heal my units".
fn run_cheat_action(state: &mut AppState, action: CheatAction) {
    use crate::sim::cheats::action as cheat_action;
    // `rules` and `simulation` are disjoint fields of the same runtime, so the
    // borrow checker lets the action read one and write the other.
    let Some(runtime) = state.match_state.sim_runtime.as_mut() else {
        return;
    };
    let bit = match action {
        CheatAction::HealAll => cheat_action::HEAL_ALL,
        CheatAction::VeteranAll => cheat_action::VETERAN_ALL,
        CheatAction::SuperweaponsReady => cheat_action::SUPERWEAPONS_READY,
    };
    let sim = &mut runtime.simulation;
    sim.cheats.request(bit);
    let rules = &runtime.resources.rules;
    sim.apply_cheats(Some(rules));
    log::info!("Trainer: ran {:?}", action);
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
        // Handled by `handle_trainer` before the feature gate; listed so the
        // match stays exhaustive.
        DebugCommand::SetCheats { .. } | DebugCommand::CheatAction { .. } => {}
    }
}

#[cfg(not(feature = "dev-overlays"))]
fn apply(_state: &mut AppState, _command: DebugCommand) {
    // Release builds carry no debug overlays; telemetry still runs, but the
    // dashboard's switches have nothing to drive.
}
