//! The in-match side of a LAN/WOL connection: a peer's departure
//! (`Execute_DoList`'s remote EXIT branch `0x0064C91B`) and
//! `Network__DestroyConnection 0x005DA750`, which the loading timeout and
//! the departure share.
//!
//! The transport is outside VERA: a controlled service delivers what a
//! peer's packets would have decided ([`NetworkMatchEvent`]), and the
//! session's request queue carries what the transport is asked to do.

use crate::app::AppState;
use crate::app::match_runtime::session::MatchSession;
use crate::sim::intern::InternedId;
use crate::sim::world::Simulation;

/// DestroyConnection's error argument (`EDX`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionError {
    /// 0: the peer sent EXIT. `TXT_LEFT_GAME`, nothing queued.
    Left,
    /// 1: the connection was lost. `TXT_CONNECTION_LOST`; Flag_To_Die at
    /// frame 0, REMOVEPLAYER otherwise.
    Lost,
}

impl ConnectionError {
    fn code(self) -> i32 {
        match self {
            Self::Left => 0,
            Self::Lost => 1,
        }
    }

    fn csf_key(self) -> &'static str {
        match self {
            Self::Left => "TXT_LEFT_GAME",
            Self::Lost => "TXT_CONNECTION_LOST",
        }
    }
}

/// The line DestroyConnection posts for a destroyed connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConnectionDestroyed {
    pub(crate) owner: InternedId,
    pub(crate) key: &'static str,
}

/// What a controlled peer did during the match, decoded from its packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NetworkMatchEvent {
    /// Participant `source_index` sent EXIT: its event executes at the
    /// current frame as if received.
    PeerExit { source_index: usize },
}

/// `Network__DestroyConnection 0x005DA750` for participant `source_index`.
/// Returns early, with no line, when the House is not human (`0x005DA77E`);
/// a participant already disconnected has no connection record, so its
/// caller never reaches here twice. The line itself is posted by
/// [`post_connection_line`] once the simulation is installed.
pub(crate) fn destroy_connection(
    session: &mut MatchSession,
    sim: &mut Simulation,
    source_index: usize,
    error: ConnectionError,
) -> Option<ConnectionDestroyed> {
    let owner = sim
        .interner()
        .get(&session.network_source_owner(source_index)?)?;
    if !sim
        .houses()
        .get(&owner)
        .is_some_and(|house| house.is_human())
    {
        return None;
    }
    if !session.disconnect_network_participant(source_index, error.code()) {
        return None;
    }
    if error == ConnectionError::Lost {
        if sim.session().binary_frame() == 0 {
            sim.flag_house_to_die(owner);
        } else {
            // EVENT_REMOVEPLAYER from PlayerPtr with the House's index
            // (0x005DA898..0x005DA8F0); it kills the House on every peer.
            let local = sim.session().current_house();
            let index = sim.houses().order().iter().position(|id| *id == owner);
            if let (Some(local), Some(index)) = (local, index) {
                let local = sim.interner().resolve(local).to_string();
                crate::app::input::commands::schedule_command_in_sim(
                    sim,
                    &local,
                    crate::sim::command::Command::RemovePlayer {
                        house_index: index as i32,
                    },
                );
            }
        }
    }
    // Error 0 outside WOL changes nothing more (0x005DA8F7).
    Some(ConnectionDestroyed {
        owner,
        key: error.csf_key(),
    })
}

/// DestroyConnection's line (`0x005DA7A9..0x005DA83E`): the template with the
/// House's UI name, in its HouseType's `Color=` scheme (`[[House+0x34]+0xC0]`).
pub(crate) fn post_connection_line(state: &mut AppState, destroyed: ConnectionDestroyed) {
    crate::app::input::messages::post_house_line(
        state,
        destroyed.key,
        destroyed.owner,
        crate::app::input::messages::HouseLineColor::Country,
    );
}

/// The executed EXIT events of remote senders this frame: each runs
/// DestroyConnection(house, 0) (`0x0064C9B1`).
pub(crate) fn drain_departures(state: &mut AppState) {
    let Some(runtime) = state.match_state.sim_runtime.as_mut() else {
        return;
    };
    let departed = runtime.simulation.take_departed_owners();
    for owner in departed {
        let Some(runtime) = state.match_state.sim_runtime.as_mut() else {
            return;
        };
        let sim = &mut runtime.simulation;
        let name = sim.interner().resolve(owner).to_string();
        let Some(source) = state.match_state.session.network_owner_source(&name) else {
            continue;
        };
        if let Some(destroyed) = destroy_connection(
            &mut state.match_state.session,
            sim,
            source,
            ConnectionError::Left,
        ) {
            post_connection_line(state, destroyed);
        }
    }
}

/// Deliver one controlled in-match event.
pub(crate) fn service_event(state: &mut AppState, event: NetworkMatchEvent) -> anyhow::Result<()> {
    match event {
        NetworkMatchEvent::PeerExit { source_index } => {
            let owner = state
                .match_state
                .session
                .network_source_owner(source_index)
                .ok_or_else(|| anyhow::anyhow!("no participant {source_index}"))?;
            anyhow::ensure!(
                state
                    .match_state
                    .session
                    .network_connected_source_indices()
                    .contains(&source_index),
                "participant {source_index} is not connected"
            );
            let runtime = state
                .match_state
                .sim_runtime
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("no running match"))?;
            crate::app::input::commands::schedule_command_in_sim(
                &mut runtime.simulation,
                &owner,
                crate::sim::command::Command::ExitMatch,
            )
            .ok_or_else(|| anyhow::anyhow!("EXIT for {owner} was not queued"))?;
        }
    }
    Ok(())
}
