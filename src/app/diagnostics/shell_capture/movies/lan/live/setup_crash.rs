//! Real editable-LAN process-loss proof. Only the external owned-process
//! runner kills a victim. Diagnostic control files acknowledge that boundary;
//! they never inject a service event or alter the real peer lease.

use super::*;
use crate::app::diagnostics::shell_capture::LanSetupCrash;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const FAULT_BOUNDARY_LIMIT: Duration = Duration::from_secs(5);

struct FaultBoundary {
    id: String,
    ready: Value,
    ready_at: Instant,
    killed: Option<Value>,
}

pub(super) struct SetupCrashCapture {
    victim: LanSetupCrash,
    prefix: Option<Value>,
    armed_at: Option<Instant>,
    boundary: Option<FaultBoundary>,
    recovered: Option<Value>,
    returned: Option<Value>,
}

fn sidecar(prefix: &Path, kind: &str) -> PathBuf {
    let mut name = prefix.as_os_str().to_owned();
    name.push(format!(".{kind}.json"));
    PathBuf::from(name)
}

fn write_atomic(path: &Path, value: &Value) -> Result<()> {
    ensure!(!path.try_exists()?, "stale fault receipt exists");
    let pending = sidecar(path, "pending");
    write_new_file(&pending, &serde_json::to_vec_pretty(value)?)?;
    // One writer owns each named sidecar. Publish only its complete immutable
    // bytes; a reader never sees an incomplete Ready/kill-boundary receipt.
    fs::rename(pending, path)?;
    Ok(())
}

fn read_control(path: &Path) -> Result<Option<Value>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= 4096,
        "invalid fault control file"
    );
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(4097).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 4096,
        "fault control file grew beyond its bound"
    );
    Ok(Some(serde_json::from_slice(&bytes)?))
}

fn control_id<'a>(control: &'a Value, phase: &str) -> Result<&'a str> {
    let id = control["fault_id"]
        .as_str()
        .context("fault control identity")?;
    ensure!(
        control["schema"] == "vera20k.lan-setup-crash-control.v1"
            && control["phase"] == phase
            && id.len() == 32
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "stale or out-of-order fault control"
    );
    Ok(id)
}

fn same_identity(first: &Value, next: &Value) -> bool {
    [
        "scope",
        "members",
        "local_endpoint",
        "peer_endpoint",
        "peer_name",
        "peer_source",
        "peer_raw_row",
        "map_admission",
        "lease_period",
    ]
    .iter()
    .all(|field| first[*field] == next[*field])
}

/// One read-only projection is reused for initial, requested-Ready and
/// post-kill observations. Admission/map/timer decisions remain app-owned.
fn snapshot(
    victim: LanSetupCrash,
    core: &LiveLanCapture,
    state: &AppState,
    rendered: PresentedShell,
    frame: u32,
    now: u32,
) -> Result<Option<Value>> {
    if rendered != PresentedShell::NetworkSetup || !setup_settled(state, core.host()) {
        return Ok(None);
    }
    let session = &state.match_state.session;
    let Some(launch) = session.network_setup() else {
        return Ok(None);
    };
    let Some(service) = state.match_state.lan_service.as_ref() else {
        return Ok(None);
    };
    let protocol = state
        .frontend
        .network_setup_protocol
        .as_ref()
        .context("editable setup")?;
    if launch.humans.len() != 2
        || !service.setup_roster_complete(session)
        || !protocol.editable()
        || protocol.lan_go_send_pending()
        || protocol.blocking_map_transfer()
        || protocol.transfer_progress().is_some()
        || service.queued_setup_count() != 0
        || session.network_start_seed().is_none()
        || core.map_admission.is_none()
        || launch.humans.iter().enumerate().any(|(source, human)| {
            human.raw_slot_index.is_none() || session.network_map_status(source) != Some(1)
        })
    {
        return Ok(None);
    }
    let config = core.config.as_ref().context("real LAN sockets")?;
    let Some(source) = session.network_endpoint_source(config.peer) else {
        return Ok(None);
    };
    let Some((remaining, period)) = service
        .setup_peer_lease(config.peer, now)
        .filter(|(remaining, _)| *remaining > 0)
    else {
        return Ok(None);
    };
    let peer = &launch.humans[source];
    ensure!(
        source != launch.local_player_index
            && peer.name == if core.host() { GUEST_NAME } else { HOST_NAME },
        "wrong admitted crash peer"
    );
    let scope = service.setup_scope().context("admitted setup scope")?;
    let members = launch
        .humans
        .iter()
        .map(|human| json!({"name":human.name,"raw_row":human.raw_slot_index}))
        .collect::<Vec<_>>();
    Ok(Some(
        json!({"schema":"vera20k.lan-setup-crash-prefix.v1", "victim":victim.as_str(),
        "account":if core.host() {HOST_NAME} else {GUEST_NAME},"host":HOST_NAME,"external_fault_pending":true,
        "setup":{"scope":scope,"members":members,"local_endpoint":config.bind.to_string(),
            "peer_endpoint":config.peer.to_string(),"peer_name":peer.name,"peer_source":source,
            "peer_raw_row":peer.raw_slot_index,"map_admission":core.map_admission,
            "editable":true,"go_pending":false,"loading":false,"gameplay_started":false,
            "observed_bucket":now,"lease_remaining":remaining,"lease_period":period,"render_frame":frame}}),
    ))
}

impl SetupCrashCapture {
    pub(super) fn new(victim: LanSetupCrash) -> Self {
        Self {
            victim,
            prefix: None,
            armed_at: None,
            boundary: None,
            recovered: None,
            returned: None,
        }
    }

    pub(super) fn ready(&self) -> bool {
        self.returned.is_some()
    }

    /// Return false on every pre-kill frame, including the frame accepting
    /// the kill acknowledgement. Cleanup can only be observed on later frames.
    fn observe_boundary(
        &mut self,
        core: &LiveLanCapture,
        state: &AppState,
        rendered: PresentedShell,
        frame: u32,
        now: u32,
    ) -> Result<bool> {
        if self
            .boundary
            .as_ref()
            .is_some_and(|boundary| boundary.killed.is_some())
        {
            return Ok(true);
        }
        let mut current = snapshot(self.victim, core, state, rendered, frame, now)?
            .context("setup changed or peer cleanup began before owned kill acknowledgement")?;
        ensure!(
            same_identity(
                &self.prefix.as_ref().expect("armed prefix")["setup"],
                &current["setup"]
            ),
            "admitted setup changed before fault boundary"
        );
        let prefix_path = core
            .config
            .as_ref()
            .and_then(|config| config.failure_prefix_output.as_ref())
            .context("fault boundary path")?;
        if self.boundary.is_none() {
            ensure!(
                !sidecar(prefix_path, "killed").try_exists()?,
                "kill arrived before requested Ready"
            );
            let Some(control) = read_control(&sidecar(prefix_path, "prepare"))? else {
                return Ok(false);
            };
            let id = control_id(&control, "prepare")?.to_owned();
            ensure!(
                current["setup"]["lease_remaining"]
                    .as_u64()
                    .context("fresh lease")?
                    >= current["setup"]["lease_period"]
                        .as_u64()
                        .context("lease period")?
                        / 2,
                "fresh survivor lease is too near expiry for a bounded fault"
            );
            let ready =
                json!({"schema":"vera20k.lan-setup-crash-ready.v1","fault_id":id,"prefix":current});
            write_atomic(&sidecar(prefix_path, "ready"), &ready)?;
            self.boundary = Some(FaultBoundary {
                id,
                ready,
                ready_at: Instant::now(),
                killed: None,
            });
            return Ok(false);
        }
        let boundary = self.boundary.as_mut().expect("prepared boundary");
        ensure!(
            boundary.ready_at.elapsed() <= FAULT_BOUNDARY_LIMIT,
            "owned kill acknowledgement missed the fresh survivor boundary"
        );
        let original_prepare = read_control(&sidecar(prefix_path, "prepare"))?
            .context("fault preparation disappeared")?;
        ensure!(
            control_id(&original_prepare, "prepare")? == boundary.id,
            "fault preparation identity changed"
        );
        let Some(control) = read_control(&sidecar(prefix_path, "killed"))? else {
            return Ok(false);
        };
        ensure!(
            control_id(&control, "killed")? == boundary.id
                && control["owned_kill_succeeded"] == true
                && control["ready_render_frame"]
                    == boundary.ready["prefix"]["setup"]["render_frame"]
                && control["ready_observed_bucket"]
                    == boundary.ready["prefix"]["setup"]["observed_bucket"],
            "owned kill does not acknowledge this fresh Ready"
        );
        let request = control["request_after_launch_ms"]
            .as_u64()
            .context("fault request time")?;
        let received = control["ready_after_launch_ms"]
            .as_u64()
            .context("fresh Ready time")?;
        let started = control["kill_started_after_launch_ms"]
            .as_u64()
            .context("owned kill start")?;
        let completed = control["kill_succeeded_after_launch_ms"]
            .as_u64()
            .context("owned kill result")?;
        ensure!(
            request <= received
                && received <= started
                && started <= completed
                && completed - request <= FAULT_BOUNDARY_LIMIT.as_millis() as u64,
            "owned kill was outside the requested fresh boundary"
        );
        ensure!(
            u64::from(frame)
                > boundary.ready["prefix"]["setup"]["render_frame"]
                    .as_u64()
                    .context("Ready frame")?,
            "kill acknowledgement reused the Ready frame"
        );
        // The runner publishes this file only after the exact owned SIGKILL
        // succeeds. Actual admission and positive lease are still present NOW.
        current["external_fault_pending"] = json!(false);
        boundary.killed = Some(json!({"schema":"vera20k.lan-setup-crash-observed.v1",
            "fault_id":boundary.id,"control":control,"prefix":current,
            "elapsed_since_ready_ms":boundary.ready_at.elapsed().as_millis() as u64}));
        Ok(false)
    }

    pub(super) fn after_present(
        &mut self,
        core: &mut LiveLanCapture,
        state: &mut AppState,
        rendered: PresentedShell,
        frame: u32,
    ) -> Result<bool> {
        ensure!(
            !matches!(
                state.frontend.screen,
                GameScreen::Loading | GameScreen::InGame
            ) && state.match_state.live_network.is_none()
                && state
                    .match_state
                    .sim_runtime
                    .as_ref()
                    .is_none_or(|runtime| !runtime.simulation.session().started()),
            "editable LAN crash checkpoint reached loading/gameplay"
        );
        if self.returned.is_some() {
            return Ok(true);
        }
        if self.prefix.is_none() && !matches!(core.phase, Phase::HostReady | Phase::GuestSetup) {
            return Ok(false);
        }
        let now = (crate::app::match_runtime::frame_pacer::wall_clock_ms() as u32) >> 4;
        if self.prefix.is_none() {
            let Some(prefix) = snapshot(self.victim, core, state, rendered, frame, now)? else {
                return Ok(true);
            };
            let path = core
                .config
                .as_ref()
                .and_then(|config| config.failure_prefix_output.as_ref())
                .context("setup crash prefix output")?;
            for kind in ["prepare", "ready", "killed"] {
                ensure!(
                    !sidecar(path, kind).try_exists()?,
                    "stale setup fault control exists"
                );
            }
            write_atomic(path, &prefix)?;
            self.prefix = Some(prefix);
            self.armed_at = Some(Instant::now());
            core.event(frame, "Real editable setup prefix saved; hold normal inputs for external prepare/kill boundary");
            return Ok(true);
        }
        ensure!(
            self.armed_at
                .is_some_and(|armed| armed.elapsed() < Duration::from_secs(120)),
            "editable LAN crash did not recover through its ordinary lease"
        );
        if core.host() == (self.victim == LanSetupCrash::Host) {
            return Ok(true);
        }
        if !self.observe_boundary(core, state, rendered, frame, now)? {
            return Ok(true);
        }
        let boundary = self.boundary.as_ref().expect("requested boundary");
        let killed = boundary.killed.as_ref().expect("observed owned kill");
        let setup = &killed["prefix"]["setup"];
        ensure!(
            u64::from(frame) > setup["render_frame"].as_u64().context("post-kill frame")?,
            "cleanup was not observed strictly after the owned kill"
        );
        let elapsed = now.wrapping_sub(
            setup["observed_bucket"]
                .as_u64()
                .context("kill observation bucket")? as u32,
        );
        let config = core.config.as_ref().context("real LAN sockets")?;
        let peer_retired = state
            .match_state
            .session
            .network_endpoint_source(config.peer)
            .is_none()
            && state
                .match_state
                .lan_service
                .as_ref()
                .is_some_and(|service| {
                    !service.setup_peer_cleanup_pending(config.peer)
                        && service.queued_setup_count() == 0
                });
        if self.recovered.is_none()
            && core.host()
            && rendered == PresentedShell::NetworkSetup
            && setup_settled(state, true)
            && peer_retired
            && state
                .frontend
                .network_setup_protocol
                .as_ref()
                .is_some_and(|protocol| protocol.editable())
            && state
                .match_state
                .session
                .network_setup()
                .is_some_and(|launch| {
                    launch.humans.len() == 1
                        && launch.local_player().name == HOST_NAME
                        && !launch.humans.iter().any(|human| human.name == GUEST_NAME)
                })
        {
            ensure!(
                u64::from(elapsed)
                    >= setup["lease_remaining"]
                        .as_u64()
                        .context("post-kill lease")?,
                "guest was removed before its actual post-kill lease elapsed"
            );
            let row = u8::try_from(setup["peer_raw_row"].as_u64().context("peer raw row")?)?;
            ensure!(
                state.match_state.session.network_vacancy(row)
                    == Some(crate::app::match_runtime::session::NetworkVacancy::Open),
                "expired guest retained its occupied setup row"
            );
            self.recovered = Some(
                json!({"fault_id":boundary.id,"kind":"guest removed; host remained editable","render_frame":frame,
                "observed_bucket":now,"elapsed_buckets":elapsed,"peer_endpoint_retired":true,
                "peer_admission_retired":true,"map_recipient_retired":true,"reliable_setup_queue_empty":true,
                "peer_raw_row":row,"peer_row_open":true,"remaining_names":[HOST_NAME]}),
            );
            setup_button(state, crate::ui::multiplayer_setup::BACK)?;
            core.event(
                frame,
                "Post-kill guest lease expired; actual host Back clicked from still-editable setup",
            );
            return Ok(true);
        }
        if rendered == PresentedShell::LanLobby
            && lobby_settled(state)
            && state.frontend.shell_exit.is_none()
            && peer_retired
            && state.frontend.network_setup_protocol.is_none()
            && state.frontend.multiplayer_setup.is_none()
            && state.match_state.session.network_setup().is_none()
            && state.match_state.session.network_map_metadata().is_none()
            && state
                .match_state
                .session
                .network_local_map_source()
                .is_none()
            && state
                .match_state
                .lan_service
                .as_ref()
                .is_some_and(|service| {
                    service.setup_scope().is_none() && !service.setup_map_work_active()
                })
        {
            ensure!(
                !core.host() || self.recovered.is_some(),
                "host skipped editable peer-loss recovery"
            );
            ensure!(
                u64::from(elapsed)
                    >= setup["lease_remaining"]
                        .as_u64()
                        .context("post-kill lease")?,
                "host was removed before its actual post-kill lease elapsed"
            );
            if self.recovered.is_none() {
                self.recovered = Some(
                    json!({"fault_id":boundary.id,"kind":"host lost; guest returned through HostLeft","render_frame":frame,
                    "observed_bucket":now,"elapsed_buckets":elapsed,"peer_endpoint_retired":true,
                    "peer_admission_retired":true,"map_recipient_retired":true,"reliable_setup_queue_empty":true}),
                );
            }
            self.returned = Some(
                json!({"fault_id":boundary.id,"render_frame":frame,"settled_lan_lobby":true,"setup_retired":true,
                "session_setup_retired":true,"map_source_retired":true,"map_transfer_retired":true,
                "setup_scope_retired":true,"live_owner_retired":true,"lan_service_retained":true,
                "loading":false,"gameplay_started":false}),
            );
            core.event(frame, "Settled actual BB after externally acknowledged kill; old setup/map/peer owners retired");
        }
        Ok(true)
    }

    pub(super) fn receipt(&self) -> Result<Value> {
        ensure!(
            self.ready(),
            "editable LAN crash checkpoint has not returned"
        );
        let boundary = self.boundary.as_ref().context("fault boundary")?;
        Ok(
            json!({"schema":"vera20k.lan-setup-crash.v1","victim":self.victim.as_str(),
            "scope":"real editable LAN setup; freshly requested admitted lease, exact owned process termination, strictly later ordinary lease cleanup and actual BB; no gameplay proof",
            "prefix":self.prefix,"fault_id":boundary.id,"ready":boundary.ready,
            "kill_observed":boundary.killed,"recovered":self.recovered,"returned":self.returned}),
        )
    }
}
