//! LAN Cooperative across two App processes: the host uses the Soviet
//! campaign through the real chooser, both players take B6 Observe, the
//! lost mission opens BC6 on both, both press Continue, and the retried
//! mission loads over the same admission. Only user input is automated; the
//! campaign text, BC6's records and the next loading cross real UDP.

use super::*;
use crate::app::diagnostics::shell_capture::movies::abort_return::AbortReturnInput;

/// Both players open the menu at this committed frame, so each B6 still
/// counts two live humans and offers Observe.
const OBSERVE_FRAME: u32 = 150;
/// Native frames the retried mission must run.
const RETRY_FRAMES: u32 = 90;
/// Render frames BC6 stays open before Continue, as a player's reaction:
/// a peer still finishing the match reads round records as in-match
/// packets and drops them, as `0x0048D1E0` does.
const CONTINUE_DELAY: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoopPhase {
    OpenChooser,
    Chooser,
    ChooserReturn,
    Playing,
    Observing,
    AwaitResult,
    AwaitNext,
    Retry,
    Done,
}

pub(super) struct CooperativeRun {
    phase: CoopPhase,
    /// The host takes Use Saved Campaign instead of the Soviet row.
    saved: bool,
    abort: Option<AbortReturnInput>,
    first_map: Option<String>,
    result: Option<Value>,
    retry_startup: Option<Value>,
    retry_first_frame: u32,
    result_frame: Option<u32>,
}

impl CooperativeRun {
    pub(super) fn new(saved: bool) -> Self {
        Self {
            phase: CoopPhase::OpenChooser,
            saved,
            abort: None,
            first_map: None,
            result: None,
            retry_startup: None,
            retry_first_frame: 0,
            result_frame: None,
        }
    }

    pub(super) fn done(&self) -> bool {
        self.phase == CoopPhase::Done
    }

    /// The host's BC: Choose Map, Cooperative, the Soviet row, Use Map. True
    /// once BC shows the campaign's first map.
    pub(super) fn prepare_host(
        &mut self,
        state: &mut AppState,
        rendered: PresentedShell,
        route: &mut Vec<Value>,
        frame: u32,
    ) -> Result<bool> {
        match self.phase {
            CoopPhase::OpenChooser
                if rendered == PresentedShell::NetworkSetup && setup_settled(state, true) =>
            {
                setup_button(state, crate::ui::multiplayer_setup::CHOOSE_MAP)?;
                self.phase = CoopPhase::Chooser;
            }
            CoopPhase::Chooser
                if rendered == PresentedShell::NetworkSetup
                    && MoviesCapture::slide_settled(state, ShellSlideKind::ChooseMap)
                    && state
                        .frontend
                        .skirmish_shell_state
                        .choose_map_modal
                        .as_ref()
                        .is_some_and(|modal| modal.statics.is_terminal()) =>
            {
                let labels = click_cooperative_mode(state)?;
                if self.saved {
                    let label =
                        crate::app::App::csf_label(state, "TXT_COOP_USE_SAVE", "TXT_COOP_USE_SAVE");
                    let row = labels
                        .iter()
                        .position(|row| *row == label)
                        .context("the saved-campaign row")?;
                    use_chooser_row(state, row)?;
                } else {
                    use_soviet_campaign_row(state, &labels)?;
                }
                route.push(json!({"render_frame":frame,"action":"co-op campaign row used","saved":self.saved,"rows":labels}));
                self.phase = CoopPhase::ChooserReturn;
            }
            CoopPhase::ChooserReturn
                if rendered == PresentedShell::NetworkSetup && setup_settled(state, true) =>
            {
                let launch = state.match_state.session.network_setup().context("BC")?;
                ensure!(
                    launch.mode.id == cooperative_mode(state)?.id,
                    "Use Map left Cooperative"
                );
                self.first_map = launch.selected_map_file.clone();
                route.push(
                    json!({"render_frame":frame,"action":"BC in Cooperative","map":self.first_map}),
                );
                self.phase = CoopPhase::Playing;
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    /// The guest learns the first map from the host's options.
    pub(super) fn match_loaded(&mut self, state: &AppState) {
        if self.phase == CoopPhase::OpenChooser {
            self.phase = CoopPhase::Playing;
        }
        if self.first_map.is_none() {
            self.first_map = state
                .match_state
                .session
                .network_setup()
                .and_then(|launch| launch.selected_map_file.clone());
        }
    }

    /// Everything after the first mission loaded.
    pub(super) fn step(
        &mut self,
        state: &mut AppState,
        route: &mut Vec<Value>,
        frame: u32,
    ) -> Result<()> {
        match self.phase {
            CoopPhase::Playing => {
                if committed_frame(state)? < OBSERVE_FRAME {
                    return Ok(());
                }
                ensure!(
                    state
                        .match_state
                        .session
                        .network_setup()
                        .is_some_and(|launch| launch.mode.class
                            == crate::skirmish_modes::MpModeClass::Cooperative),
                    "the live match is not Cooperative"
                );
                self.abort = Some(AbortReturnInput::begin_network_observe(
                    state, route, frame,
                )?);
                self.phase = CoopPhase::Observing;
            }
            CoopPhase::Observing => {
                if self
                    .abort
                    .as_mut()
                    .context("Observe input")?
                    .after_present(state, route, frame)?
                {
                    route.push(json!({"render_frame":frame,"action":"Observe selected through B6; real DESTRUCT queued"}));
                    self.phase = CoopPhase::AwaitResult;
                }
            }
            CoopPhase::AwaitResult => {
                if !crate::app::input::coop_score::active(state) {
                    return Ok(());
                }
                let opened = *self.result_frame.get_or_insert(frame);
                if frame < opened + CONTINUE_DELAY {
                    return Ok(());
                }
                ensure!(
                    state.match_state.live_network.is_none()
                        && state.match_state.lan_service.is_some(),
                    "BC6 kept the match's private channels"
                );
                let page = state
                    .frontend
                    .coop_score_page
                    .as_ref()
                    .context("BC6 page")?;
                ensure!(
                    !page.victory && page.model.can_continue,
                    "both humans observing loses a mission that can be retried"
                );
                let next = state
                    .match_state
                    .session
                    .network_setup()
                    .and_then(|launch| launch.selected_map_file.clone());
                ensure!(
                    next.is_some() && next == self.first_map,
                    "a defeat keeps the mission: {:?} -> {next:?}",
                    self.first_map
                );
                self.result = Some(json!({"render_frame":frame,"victory":page.victory,
                    "rows":page.model.rows.iter().map(|row| row.name.clone()).collect::<Vec<_>>(),
                    "next_map":next}));
                let layout = crate::app::input::coop_score::layout(state);
                let rect = layout
                    .control_rect(crate::ui::coop_score_shell::CONTINUE)
                    .context("BC6 Continue")?;
                set_window_pointer_to_rect_center(state, rect);
                crate::app::input::coop_score::cursor_moved(state);
                crate::app::input::coop_score::mouse(state, MouseButton::Left, true);
                crate::app::input::coop_score::mouse(state, MouseButton::Left, false);
                MoviesCapture::restore_neutral_pointer(state);
                ensure!(
                    state
                        .match_state
                        .session
                        .cooperative_round()
                        .is_some_and(|round| round.local_ready()),
                    "BC6 Continue did not mark the local player ready"
                );
                route.push(json!({"render_frame":frame,"action":"BC6 opened; Continue clicked"}));
                self.phase = CoopPhase::AwaitNext;
            }
            CoopPhase::AwaitNext => {
                if state.frontend.screen != GameScreen::InGame {
                    return Ok(());
                }
                ensure!(
                    state.match_state.live_network.is_some(),
                    "the retried mission has no real command transport"
                );
                self.retry_startup = Some(startup_receipt(state)?);
                self.retry_first_frame = committed_frame(state)?;
                route.push(json!({"render_frame":frame,"action":"BC6 round over UDP; retried mission loaded"}));
                self.phase = CoopPhase::Retry;
            }
            CoopPhase::Retry => {
                ensure!(
                    state.frontend.screen == GameScreen::InGame,
                    "the retried mission left gameplay"
                );
                if committed_frame(state)? >= self.retry_first_frame + RETRY_FRAMES {
                    route.push(json!({"render_frame":frame,"action":"retried mission ran"}));
                    self.phase = CoopPhase::Done;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn receipt(&self, state: &AppState) -> Result<Value> {
        ensure!(self.done(), "live co-op capture has not finished");
        let sim = &state
            .match_state
            .sim_runtime
            .as_ref()
            .context("retried simulation")?
            .simulation;
        Ok(json!({"first_map":self.first_map,"result":self.result,
            "retry_startup":self.retry_startup,"retry_first_native_frame":self.retry_first_frame,
            "final_native_frame":sim.session().binary_frame(),
            "checksum_failure":format!("{:?}",sim.pending_network_failure())}))
    }
}

fn committed_frame(state: &AppState) -> Result<u32> {
    Ok(state
        .match_state
        .sim_runtime
        .as_ref()
        .context("live simulation")?
        .simulation
        .session()
        .binary_frame())
}
