//! Production WOL Custom Match checkpoints: Main Menu -> Internet -> Custom
//! Match through the real `0x10E`/`0x10F`/`0xC4`/box handlers, with a
//! controlled WOL service that answers the visit's own requests. Nothing
//! here builds a page; the service events are decoded results, not a
//! transport or backend. The accepted login WRITES the MultiPlayer handle:
//! use an isolated RA2_DIR.

use super::*;
use crate::app::shell_wol::{
    WolApiStatus, WolLoginAccepted, WolProvider, WolServer, WolServerList, WolServiceEvent,
    WolServiceRequest, service_event,
};
use crate::ui::shell::geom::RectPx;
use crate::ui::wol_login::{BACK, LOGIN};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WolTarget {
    /// `0x10F` settled after a controlled server list, empty fields.
    LoginSteady,
    /// A typed nickname and an eight-character password: Login enabled.
    LoginTyped,
    /// Back on `0x10F`: a fresh `0x10E` settled.
    LoginBack,
    /// The server-list wait box `0xC4` once its 7000 ms timer showed it.
    ServerListWait,
    /// Disconnect on that box: WOL left, a new `0xE2` settled.
    ServerListDisconnect,
    /// A failed server list: WOL left without a box, a new `0xE2` settled.
    ServerListFailure,
    /// Login rejected with `TXT_BADPASS`: the box `0xD0`.
    LoginRejected,
    /// OK on that box: a fresh `0x10E` settled.
    LoginRejectedReturn,
    /// Login accepted: the custom lobby route settled.
    LoginAccepted,
}

pub(crate) const WOL_CHECKPOINTS: &[(&str, WolTarget)] = &[
    ("wol-0x10f-steady", WolTarget::LoginSteady),
    ("wol-0x10f-typed", WolTarget::LoginTyped),
    ("wol-0x10f-back", WolTarget::LoginBack),
    ("wol-0xc4-server-list-wait", WolTarget::ServerListWait),
    ("wol-0xc4-disconnect", WolTarget::ServerListDisconnect),
    ("wol-server-list-failure", WolTarget::ServerListFailure),
    ("wol-0xd0-login-rejected", WolTarget::LoginRejected),
    ("wol-login-rejected-return", WolTarget::LoginRejectedReturn),
    ("wol-0x113-login-accepted", WolTarget::LoginAccepted),
];

const NICKNAME: &str = "Tester";
const PASSWORD: &str = "secret12";
const BAD_PASSWORD: u32 = 0x8004_0065;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WolPhase {
    Main,
    Welcome,
    /// Custom Match pressed: its teardown, then the API request.
    Custom,
    /// The server list is outstanding.
    Servers,
    Login,
    /// Login or Back pressed on `0x10F`.
    LoginClosing,
    Rejected,
    /// A new `0x10E` after the login flow returned 0.
    WelcomeAgain,
    /// WOL left: a new `0xE2`.
    MainReturn,
    Lobby,
    Settle(u32),
}

pub(super) struct WolCapture {
    target: WolTarget,
    phase: WolPhase,
    route: Vec<Value>,
}

impl WolCapture {
    pub(super) fn new(target: WolTarget) -> Self {
        Self {
            target,
            phase: WolPhase::Main,
            route: Vec::new(),
        }
    }

    pub(super) fn timeout(&self) -> Duration {
        Duration::from_secs(60)
    }

    fn event(&mut self, frame: u32, action: impl Into<Value>) {
        self.route
            .push(json!({"frame": frame, "action": action.into()}));
    }

    fn settle(&mut self) {
        self.phase = WolPhase::Settle(SETTLE_FRAMES);
    }

    fn requests(&mut self, state: &mut AppState, frame: u32) -> Result<Vec<WolServiceRequest>> {
        let requests = state
            .frontend
            .wol
            .as_mut()
            .context("WOL visit")?
            .take_service_requests();
        self.route.push(json!({"frame": frame,
            "requests": requests.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>()}));
        Ok(requests)
    }

    fn page_settled(state: &AppState, kind: ShellSlideKind) -> bool {
        MoviesCapture::slide_settled(state, kind)
            && state.frontend.shell_exit.is_none()
            && state.frontend.shell_page_title.is_terminal()
    }

    pub(super) fn after_present(
        &mut self,
        state: &mut AppState,
        rendered: PresentedShell,
        frame: u32,
    ) -> Result<()> {
        ensure!(
            state.frontend.main_menu_shell_error.is_none(),
            "WOL capture encountered the shell fallback"
        );
        match self.phase {
            WolPhase::Main
                if rendered == PresentedShell::MainMenu
                    && steady_main_menu_capture_ready(MainMenuCaptureSnapshot::from_state(
                        state,
                    ))? =>
            {
                App::leave_shell_dialog(
                    state,
                    crate::app::frontend::shell_transition::ShellExitThen::MainMenu(
                        crate::ui::main_menu_shell::MainMenuShellAction::WwOnline,
                    ),
                );
                self.event(frame, "0xE2 Internet");
                self.phase = WolPhase::Welcome;
            }
            WolPhase::Welcome
                if rendered == PresentedShell::WolWelcome
                    && Self::page_settled(state, ShellSlideKind::WolWelcome) =>
            {
                state
                    .frontend
                    .wol
                    .as_mut()
                    .context("WOL visit")?
                    .set_provider(WolProvider::Controlled);
                let rect = App::wol_welcome_layout(state)
                    .page
                    .button_rect(crate::ui::wol_shell::CUSTOM_MATCH)
                    .context("Custom Match")?;
                welcome_click(state, rect);
                ensure!(
                    state.frontend.shell_exit.is_some(),
                    "Custom Match did not tear 0x10E down"
                );
                self.event(frame, "0x10E Custom Match (controlled service)");
                self.phase = WolPhase::Custom;
            }
            WolPhase::Custom if state.frontend.shell_exit.is_none() => {
                let requests = self.requests(state, frame)?;
                ensure!(
                    requests == [WolServiceRequest::EnsureApi],
                    "the flow did not start with the WOLAPI object"
                );
                ensure!(
                    state.frontend.wol.as_ref().and_then(|visit| visit.mode())
                        == Some(crate::ui::wol_shell::WolMode::CustomMatch),
                    "Custom Match did not write mode 4"
                );
                service_event(state, WolServiceEvent::Api(WolApiStatus::Available));
                let requests = self.requests(state, frame)?;
                ensure!(
                    requests
                        == [
                            WolServiceRequest::ListCredentials,
                            WolServiceRequest::FetchServers { flags: 0x2900 }
                        ],
                    "the server list was not requested"
                );
                let wait = state
                    .frontend
                    .wol
                    .as_ref()
                    .and_then(|visit| visit.wait_box())
                    .context("no 0xC4 wait box")?;
                ensure!(!wait.shown, "0xC4 must be created hidden");
                self.event(frame, "controlled WOLAPI available");
                match self.target {
                    WolTarget::ServerListWait | WolTarget::ServerListDisconnect => {
                        self.phase = WolPhase::Servers;
                    }
                    WolTarget::ServerListFailure => {
                        service_event(state, WolServiceEvent::ServerList(Err(())));
                        self.event(frame, "controlled server list failure");
                        self.phase = WolPhase::MainReturn;
                    }
                    _ => {
                        service_event(state, WolServiceEvent::Credentials(Vec::new()));
                        service_event(state, WolServiceEvent::ServerStatus(0x0004_012F));
                        service_event(state, WolServiceEvent::ServerList(Ok(server_list())));
                        ensure!(
                            state
                                .frontend
                                .wol
                                .as_ref()
                                .is_some_and(|visit| visit.login().is_some()),
                            "the server list did not open 0x10F"
                        );
                        self.event(frame, "controlled server list: 3 records, selected 1");
                        self.phase = WolPhase::Login;
                    }
                }
            }
            WolPhase::Servers => {
                let wait = state
                    .frontend
                    .wol
                    .as_ref()
                    .and_then(|visit| visit.wait_box())
                    .context("the wait box closed")?;
                if !wait.shown {
                    return Ok(());
                }
                let shown_after = wait.opened_at.elapsed().as_millis() as u64;
                self.route
                    .push(json!({"frame": frame, "action": "0xC4 shown",
                    "after_ms": shown_after}));
                if self.target == WolTarget::ServerListWait {
                    MoviesCapture::restore_neutral_pointer(state);
                    self.settle();
                    return Ok(());
                }
                let layout = crate::ui::shell::modal::wait_box_layout(
                    state.renderer.gpu.config.width as i32,
                    state.renderer.gpu.config.height as i32,
                );
                welcome_click(state, layout.disconnect);
                ensure!(state.frontend.wol.is_none(), "Disconnect did not leave WOL");
                self.event(frame, "0xC4 Disconnect");
                self.phase = WolPhase::MainReturn;
            }
            WolPhase::Login
                if rendered == PresentedShell::WolLogin
                    && Self::page_settled(state, ShellSlideKind::WolLogin) =>
            {
                let layout = crate::app::shell_wol::login_layout(state);
                match self.target {
                    WolTarget::LoginSteady => {
                        MoviesCapture::restore_neutral_pointer(state);
                        App::handle_wol_mouse_move(state);
                        self.settle();
                        return Ok(());
                    }
                    WolTarget::LoginBack => {
                        login_click(state, button(&layout, BACK)?);
                        self.event(frame, "0x10F Back");
                        self.phase = WolPhase::LoginClosing;
                        return Ok(());
                    }
                    _ => {}
                }
                login_click(state, layout.nickname_face);
                App::handle_wol_key(state, None, true, Some(NICKNAME));
                login_click(state, layout.password);
                App::handle_wol_key(state, None, true, Some(PASSWORD));
                let login = state
                    .frontend
                    .wol
                    .as_ref()
                    .and_then(|visit| visit.login())
                    .context("0x10F")?;
                ensure!(
                    login.login_enabled && login.remember_enabled && !login.forget_enabled,
                    "typed account did not enable Login and Remember"
                );
                self.event(frame, "typed nickname and password");
                if self.target == WolTarget::LoginTyped {
                    MoviesCapture::restore_neutral_pointer(state);
                    App::handle_wol_mouse_move(state);
                    self.settle();
                    return Ok(());
                }
                login_click(state, button(&layout, LOGIN)?);
                self.event(frame, "0x10F Login");
                self.phase = WolPhase::LoginClosing;
            }
            WolPhase::LoginClosing if state.frontend.shell_exit.is_none() => {
                if self.target == WolTarget::LoginBack {
                    self.phase = WolPhase::WelcomeAgain;
                    return Ok(());
                }
                let requests = self.requests(state, frame)?;
                ensure!(
                    requests
                        == [WolServiceRequest::Connect {
                            server: server_list().servers[1].clone(),
                            nickname: NICKNAME.into(),
                            password: PASSWORD.into(),
                            changed: true,
                        }],
                    "Login did not connect to the selected record"
                );
                if self.target == WolTarget::LoginAccepted {
                    service_event(
                        state,
                        WolServiceEvent::Login(Ok(WolLoginAccepted {
                            motd: "Controlled message of the day".into(),
                        })),
                    );
                    self.event(frame, "controlled login accepted");
                    self.phase = WolPhase::Lobby;
                } else {
                    service_event(state, WolServiceEvent::Login(Err(BAD_PASSWORD)));
                    self.event(frame, "controlled login rejected 0x80040065");
                    self.phase = WolPhase::Rejected;
                }
            }
            WolPhase::Rejected => {
                let expected = App::csf_label(state, "TXT_BADPASS", "TXT_BADPASS");
                ensure!(
                    state
                        .frontend
                        .wol
                        .as_ref()
                        .and_then(|visit| visit.message())
                        .is_some_and(|message| message.body == expected),
                    "TXT_BADPASS did not show"
                );
                MoviesCapture::restore_neutral_pointer(state);
                if self.target == WolTarget::LoginRejected {
                    App::handle_wol_mouse_move(state);
                    self.settle();
                    return Ok(());
                }
                let ok = crate::ui::shell::modal::body_ok_layout(
                    state.renderer.gpu.config.width as i32,
                    state.renderer.gpu.config.height as i32,
                )
                .ok;
                welcome_click(state, ok);
                self.event(frame, "0xD0 OK");
                self.phase = WolPhase::WelcomeAgain;
            }
            WolPhase::WelcomeAgain
                if rendered == PresentedShell::WolWelcome
                    && Self::page_settled(state, ShellSlideKind::WolWelcome) =>
            {
                ensure!(
                    state
                        .frontend
                        .wol
                        .as_ref()
                        .and_then(|visit| visit.mode())
                        .is_none(),
                    "the login flow did not return mode 0"
                );
                MoviesCapture::restore_neutral_pointer(state);
                App::handle_wol_mouse_move(state);
                self.event(frame, "fresh 0x10E settled");
                self.settle();
            }
            WolPhase::MainReturn
                if rendered == PresentedShell::MainMenu
                    && steady_main_menu_capture_ready(MainMenuCaptureSnapshot::from_state(
                        state,
                    ))? =>
            {
                ensure!(state.frontend.wol.is_none(), "the WOL visit survived");
                self.event(frame, "new 0xE2 settled");
                self.settle();
            }
            WolPhase::Lobby
                if rendered == PresentedShell::WolLobby
                    && Self::page_settled(state, ShellSlideKind::WolLobby) =>
            {
                let visit = state.frontend.wol.as_ref().context("WOL visit")?;
                ensure!(
                    visit.authenticated()
                        && visit.motd() == Some("Controlled message of the day")
                        && state.frontend.shell_route.wol_lobby(),
                    "the accepted login did not enter the custom lobby"
                );
                ensure!(
                    state
                        .frontend
                        .multiplayer_shell_runtime
                        .local_participant()
                        .name
                        == NICKNAME,
                    "the nickname did not become the MultiPlayer handle"
                );
                self.event(frame, "0x113 settled");
                self.settle();
            }
            WolPhase::Settle(remaining) if remaining > 0 => {
                self.phase = WolPhase::Settle(remaining - 1);
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn ready(&self, state: &AppState) -> Result<bool> {
        // Behind a box `0x10E`/`0x10F` is gone, and with it the status line.
        let backdrop = state.frontend.wol.as_ref().is_some_and(|visit| {
            matches!(
                visit.page,
                crate::app::shell_wol::WolPage::Backdrop { .. }
                    | crate::app::shell_wol::WolPage::Message { .. }
            )
        });
        Ok(self.phase == WolPhase::Settle(0)
            && state.frontend.shell_exit.is_none()
            && (backdrop || state.frontend.shell_status_line.is_terminal())
            && state.frontend.shell_first_paint_slide.is_none())
    }

    pub(super) fn receipt(&self, state: &AppState) -> Value {
        let visit = state.frontend.wol.as_ref();
        json!({
            "target": format!("{:?}", self.target),
            "route": self.route,
            "service_boundary": "controlled decoded WOL results; no WOLAPI, transport or backend claim",
            "shell_route": format!("{:?}", state.frontend.shell_route),
            "mode": visit.and_then(|visit| visit.mode()).map(|mode| mode.native()),
            "authenticated": visit.is_some_and(|visit| visit.authenticated()),
            "login": visit.and_then(|visit| visit.login()).map(|login| json!({
                "nickname": login.nickname.display_text(),
                "password_units": login.password.units.len(),
                "login_enabled": login.login_enabled,
                "forget_enabled": login.forget_enabled,
                "remember_enabled": login.remember_enabled,
                "remember_checked": login.remember_checked,
                "focused": login.focused_control,
            })),
        })
    }
}

/// Three IRC records; the preferred-server draw left record 1 selected.
fn server_list() -> WolServerList {
    WolServerList {
        servers: ["Pacific", "Westwood", "Europe"]
            .into_iter()
            .map(|name| WolServer {
                name: format!("irc.westwood.com:{name}"),
            })
            .collect(),
        selected: 1,
    }
}

fn button(layout: &crate::ui::wol_login::WolLoginLayout, id: u16) -> Result<RectPx> {
    layout.page.button_rect(id).context("0x10F button")
}

/// A press and release through the window handlers' WOL routing; like the
/// native helper, the pointer then returns to the neutral point.
fn welcome_click(state: &mut AppState, rect: RectPx) {
    set_window_pointer_to_rect_center(state, rect);
    App::handle_wol_mouse_move(state);
    App::handle_wol_mouse_down(state);
    App::handle_wol_mouse_up(state);
    MoviesCapture::restore_neutral_pointer(state);
}

fn login_click(state: &mut AppState, rect: RectPx) {
    // The combo face's left part is its NewEdit, clear of the arrow.
    let rect = RectPx::new(rect.x, rect.y, rect.w / 2, rect.h);
    welcome_click(state, rect);
}
