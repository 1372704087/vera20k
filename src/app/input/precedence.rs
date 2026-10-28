//! Which layer takes window input, and which modal page render draws over the
//! current screen: one ordered decision shared by render and by the key,
//! cursor, button and wheel paths, so a new layer is ordered in one place.
//!
//! Two blocking layers come first and draw nothing of their own: the house
//! outcome wait (in game) and a shell transition or the quit cascade (in the
//! shell). Then the modal pages, top first. The in-game connection wait 0xEA
//! sits over the Keyboard page A3: A3's pump (`0x005FBF2E`) runs
//! `ProcessModalServicePump 0x00623120`, which in network modes enters
//! `Main_Tick 0x0055D360` and from it `Wait_For_Players 0x00648710`, which
//! opens 0xEA. Below the pages each screen routes its own input; the status
//! cards (`frontend::status_screen`) are decided before all of this.

use crate::app::{App, AppState, GameScreen};

/// A modal page drawn over the current screen, top first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalPage {
    NetworkFailure,
    /// 0xEA, the in-game connection wait.
    NetworkWait,
    /// A3, from the launcher or in-game Options.
    Keyboard,
    NetworkOptions,
}

/// The layer that takes a window input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputLayer {
    /// Takes every event; a release still drops held input state.
    HouseOutcomeWait,
    /// A shell slide or the quit cascade: no page takes input.
    ShellTransition,
    Page(ModalPage),
    /// The current screen routes the event.
    Screen,
}

#[derive(Debug, Default, Clone, Copy)]
struct Layers {
    house_outcome_wait: bool,
    shell_transition: bool,
    network_failure: bool,
    network_wait: bool,
    keyboard: bool,
    network_options: bool,
}

impl Layers {
    fn of(state: &AppState) -> Self {
        Self {
            house_outcome_wait: App::house_outcome_wait_pending(state),
            shell_transition: crate::app::frontend::shell_transition::blocks_shell_input(state),
            network_failure: crate::app::match_runtime::network_failure::pending(state),
            network_wait: state.frontend.screen == GameScreen::InGame
                && state.match_state.network_wait.dialog_open(),
            keyboard: state.frontend.keyboard_dialog.is_some(),
            network_options: state.frontend.network_options.is_some(),
        }
    }

    fn page(self) -> Option<ModalPage> {
        [
            (self.network_failure, ModalPage::NetworkFailure),
            (self.network_wait, ModalPage::NetworkWait),
            (self.keyboard, ModalPage::Keyboard),
            (self.network_options, ModalPage::NetworkOptions),
        ]
        .into_iter()
        .find_map(|(open, page)| open.then_some(page))
    }

    fn input(self) -> InputLayer {
        if self.house_outcome_wait {
            InputLayer::HouseOutcomeWait
        } else if self.shell_transition {
            InputLayer::ShellTransition
        } else {
            self.page().map_or(InputLayer::Screen, InputLayer::Page)
        }
    }
}

/// The modal page render draws over the current screen.
pub(crate) fn modal_page(state: &AppState) -> Option<ModalPage> {
    Layers::of(state).page()
}

/// The layer that takes a window input event now.
pub(crate) fn input_layer(state: &AppState) -> InputLayer {
    Layers::of(state).input()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The launcher A3 slides out with its dialog still retained: the slide
    /// takes a key as it takes every other input, while render still draws A3.
    #[test]
    fn a_shell_transition_takes_input_from_a_retained_keyboard_page() {
        let layers = Layers {
            shell_transition: true,
            keyboard: true,
            ..Default::default()
        };
        assert_eq!(layers.input(), InputLayer::ShellTransition);
        assert_eq!(layers.page(), Some(ModalPage::Keyboard));
    }

    /// 0xEA opens over in-game A3; it is drawn and takes input there.
    #[test]
    fn the_connection_wait_covers_the_in_game_keyboard_page() {
        let layers = Layers {
            network_wait: true,
            keyboard: true,
            ..Default::default()
        };
        assert_eq!(layers.input(), InputLayer::Page(ModalPage::NetworkWait));
        assert_eq!(layers.page(), Some(ModalPage::NetworkWait));
    }
}
