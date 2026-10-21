//! The LAN Cooperative continuation: Cooperative `+0xA0` at a mission's
//! end, the result dialog BC6 and the next mission over the same session.
//!
//! The progress record belongs to `MultiplayerShellRuntime`'s
//! `CooperativeProgressState`; the participants and the agreed seed to
//! `MatchSession`. This module orders their calls the way
//! `Main_Game__Route_Scenario_Termination` (`0x0055CFD0`) and
//! `GameExit__Victory`/`Defeat` (`0x00685670`/`0x00685DC0`) do.

use crate::app::AppState;
use crate::app::frontend::skirmish_session::cooperative::CooperativeMissionRow;

/// A network Cooperative match: the router calls the mode's `+0xA0`
/// before GameExit (`0x0055D0C5..0x0055D0E2`, GameMode 3 or 4).
pub(crate) fn applies(state: &AppState) -> bool {
    state.match_state.session_mode().is_network()
        && state
            .match_state
            .sim_runtime
            .as_ref()
            .and_then(|runtime| runtime.simulation.session().multiplayer_mode_class())
            == Some(crate::skirmish_modes::MpModeClass::Cooperative)
}

/// Cooperative `+0xA0` (`0x005C3710`) at the terminal edge, with the
/// mission's elapsed ticks. "Won" is read from the Houses, not the
/// outcome: some human House is not defeated. A win with an active record
/// adds the mission, writes coopsave.ini and posts the save message; then
/// the record's next stage, matched case-sensitively as `<token>.MAP`,
/// becomes the scenario (a defeat, or no match, keeps the current one).
/// RESIDUAL: the tail's writes (`[0xA8B538]`, `0x004A8BF0`, the score
/// history column) have no VERA counterpart.
pub(crate) fn mission_ended(state: &mut AppState, elapsed_ticks: i32) {
    let Some(sim) = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|runtime| &runtime.simulation)
    else {
        return;
    };
    let won = sim.cooperative_humans_won();
    let rows: Vec<CooperativeMissionRow> = sim
        .cooperative_score_rows()
        .into_iter()
        .map(|row| CooperativeMissionRow {
            name: crate::app::presentation::house_ui::house_ui_name(state, row.owner)
                .map(|name| crate::util::native_string::narrow_utf16_c_string(name.encode_utf16()))
                .unwrap_or_default(),
            kills: row.kills as i32,
            built: row.built as i32,
            lost: row.losses as i32,
            score: row.score,
        })
        .collect();
    if won {
        let house = save_houses(state);
        // 0x0049B7E0 leaves a missing row uninitialised; it reads as zero.
        let missing = CooperativeMissionRow::default();
        let rows = [
            rows.first().unwrap_or(&missing),
            rows.get(1).unwrap_or(&missing),
        ];
        if let Some(saved) = state
            .frontend
            .multiplayer_shell_runtime
            .record_cooperative_win(rows, elapsed_ticks, house)
        {
            crate::app::input::messages::post_cooperative_save_result(state, saved);
        }
    }
    let next = state
        .frontend
        .multiplayer_shell_runtime
        .cooperative_next_scenario_token()
        .map(|token| format!("{token}.MAP"));
    let index = next.and_then(|file| {
        state
            .frontend
            .scenario_catalog
            .records()
            .iter()
            .position(|record| record.file_name == file)
    });
    if let Some(index) = index {
        state
            .frontend
            .multiplayer_shell_runtime
            .set_scenario_index(index);
    }
}

/// `0x0049C5A0`'s House1/Color1 (node 0, the local player) and
/// House2/Color2 (the partner): `0x00696F10`/`0x00696F20`, -2 for a Random
/// choice.
/// RESIDUAL: VERA's resolved launch keeps no chosen-Random flag
/// (`node+0x4F`), so a player who chose Random saves the drawn country
/// instead of `<random>`; only the saved-campaign reader, not ported,
/// reads it.
fn save_houses(state: &AppState) -> [(i32, i32); 2] {
    let Some(launch) = state.match_state.session.network_setup() else {
        return [(-1, -1); 2];
    };
    let value = |slot: &crate::skirmish_launch::SkirmishLocalSlot| {
        (
            if slot.country_random {
                -2
            } else {
                i32::from(slot.country.raw_index())
            },
            if slot.color_random {
                -2
            } else {
                i32::from(slot.color_index)
            },
        )
    };
    let partner = launch
        .humans
        .iter()
        .enumerate()
        .find(|(index, human)| *index != launch.local_player_index && !human.is_observer())
        .map_or((-1, -1), |(_, human)| value(&human.slot));
    [value(launch.local_slot()), partner]
}

/// GameExit's co-op arm after the teardown: `+0xA8` runs BC6.
pub(crate) fn open_result(state: &mut AppState, victory: bool) {
    let _ = victory;
    crate::app::App::leave_mission_result_screen(state);
}
