//! Opt-in observation of an original host killed after ordinary loading100,
//! before its private startup certificate is installed. Native loading and wait
//! decisions remain with LoadingSession and LiveCommandSession respectively.
//! Receipt `uptime_ms` is the native continuous system clock. Compare it only
//! within the same game process, never with controller launch-relative times or
//! another machine's clock.

use super::fault_files::{read_control, write_atomic};
use super::*;
use crate::app::loading::pump::NetworkLoadingObservation;
use serde_json::{Value, json};

pub(crate) fn sidecar(output: &Path, kind: &str) -> PathBuf {
    let mut name = output.as_os_str().to_owned();
    name.push(format!(".startup.{kind}.json"));
    PathBuf::from(name)
}

/// Borrowed for one loading pump. The result stays in LoadingSession; this
/// observer retains only its completed diagnostic projection until install.
pub(crate) struct StartupLoadingObserver<'a> {
    config: &'a LiveOnlineConfig,
    frame: u32,
    uptime_ms: u128,
    completed: Option<Value>,
}

impl<'a> StartupLoadingObserver<'a> {
    pub(crate) fn new(config: &'a LiveOnlineConfig, frame: u32, uptime_ms: u128) -> Self {
        Self {
            config,
            frame,
            uptime_ms,
            completed: None,
        }
    }

    pub(crate) fn defer_install(
        &mut self,
        state: &AppState,
        view: NetworkLoadingObservation<'_>,
    ) -> Result<bool> {
        let config = self.config;
        let launch = view
            .startup
            .launch_session()
            .context("startup fault loading launch")?;
        let local = launch.local_player_index;
        ensure!(
            launch.humans.len() == 3
                && launch.humans[local].name == config.account
                && view.rows.local_player_index == local
                && view.rows.participants.len() == 3
                && view.stage == "ready_to_install"
                && view.timeout.is_none()
                && view.progress.current_percent() == 100.0
                && view.incomplete.is_empty()
                && view
                    .rows
                    .participants
                    .iter()
                    .enumerate()
                    .all(|(source, row)| {
                        source == local
                            || row
                                .remote_progress
                                .as_ref()
                                .is_some_and(|progress| progress.current_percent() == 100.0)
                    }),
            "startup checkpoint requires the ordinary complete original three-player loading result"
        );
        let sim = view.simulation.context("completed loading world")?;
        ensure!(
            sim.session().binary_frame() == 0
                && sim.session().tick() == 0
                && sim.session().network_command_timing().is_none(),
            "startup victim advanced or installed live timing before its handoff"
        );
        let checkpoint = config
            .startup_host_loss
            .as_deref()
            .context("startup checkpoint")?;
        let mut snapshot = super::loading_host_loss::loading_snapshot(
            config,
            state,
            &view,
            self.frame,
            self.uptime_ms,
            checkpoint,
        )?;
        let output = config.loading_output.as_ref().context("startup output")?;
        if config.account == config.host {
            let path = sidecar(output, "ready");
            if !path.try_exists()? {
                snapshot["schema"] = json!("vera20k.startup-host-ready.v1");
                write_atomic(&path, &snapshot)?;
            }
            // Diagnostic input gate only. The ordinary complete decision and
            // reliable100 publication already happened; no loader is re-run.
            return Ok(true);
        }
        self.completed = Some(snapshot);
        Ok(false)
    }

    pub(crate) fn before_install(&mut self) -> Result<()> {
        let mut snapshot = self
            .completed
            .take()
            .context("startup install missed normal complete")?;
        ensure!(
            self.config.account != self.config.host,
            "startup victim reached private installation"
        );
        snapshot["schema"] = json!("vera20k.startup-host-install.v1");
        write_atomic(
            &sidecar(
                self.config
                    .loading_output
                    .as_ref()
                    .context("startup output")?,
                "install",
            ),
            &snapshot,
        )
    }

    pub(crate) fn installed(&self, state: &AppState) -> Result<()> {
        ensure!(
            state.frontend.screen == GameScreen::InGame
                && state
                    .match_state
                    .live_network
                    .as_ref()
                    .is_some_and(|live| { live.startup_pending() && live.peers().len() == 2 }),
            "startup survivor did not install the full three-player private admission"
        );
        Ok(())
    }
}

pub(crate) fn read_killed(config: &LiveOnlineConfig, match_id: &Value) -> Result<Option<Value>> {
    let output = config.loading_output.as_ref().context("startup output")?;
    let Some(control) = read_control(&sidecar(output, "killed"), 4096)? else {
        return Ok(None);
    };
    let requested = control["request_after_launch_ms"]
        .as_u64()
        .context("startup kill request time")?;
    let started = control["kill_started_after_launch_ms"]
        .as_u64()
        .context("startup kill start time")?;
    let finished = control["kill_succeeded_after_launch_ms"]
        .as_u64()
        .context("startup kill completion time")?;
    ensure!(
        control["schema"] == "vera20k.startup-host-killed.v1"
            && control["phase"] == "startup-killed"
            && control["checkpoint_id"] == json!(config.startup_host_loss)
            && &control["match_id"] == match_id
            && !match_id.is_null()
            && control["victim_account"] == config.host
            && control["victim_process_id"]
                .as_u64()
                .is_some_and(|pid| pid > 0)
            && control["victim_ready_render_frame"].as_u64().is_some()
            && control["owned_kill_succeeded"] == true
            && requested <= started
            && started <= finished,
        "startup kill control belongs to another process/match/attempt or lacks acknowledged ownership"
    );
    Ok(Some(control))
}
