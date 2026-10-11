//! The one place the shell family picks its layout: gamemd's centered,
//! unscaled 800x600 dialog, or the same dialog scaled to fit the live window
//! at its own shape (`[graphics] menu_fill_window`).
//!
//! Every shell route that both paints and hit-tests must take its rects from
//! here; a render path computing one layout and an input handler the other
//! misaligns clicks by the scale factor.

use crate::app::AppState;
use crate::ui::main_menu_shell::{MainMenuShellLayout, compute_layout, compute_responsive_layout};
use crate::ui::shell::menu_page::{self, MenuPageLayout, MenuPageSpec};
use crate::ui::skirmish_shell::{
    ChooseMapModalLayout, RandomMapSetupLayout, SavedSeedLayout, SavedSeedMode, SkirmishShellLayout,
};

/// The window pixel space the shell paints and hit-tests in. The GPU surface
/// size, not the monitor's: a resized window repaints at the new size.
pub(crate) fn shell_screen_size(state: &AppState) -> (u32, u32) {
    (
        state.renderer.gpu.config.width,
        state.renderer.gpu.config.height,
    )
}

/// Main menu dialog `0xE2`.
pub(crate) fn main_menu_layout(state: &AppState) -> MainMenuShellLayout {
    let (width, height) = shell_screen_size(state);
    if state.platform.menu_fill_window {
        compute_responsive_layout(width, height)
    } else {
        compute_layout(width, height)
    }
}

/// The shared right-panel menu pages (`0x100` Single Player, `0x101` Movies &
/// Credits) and any other [`MenuPageSpec`]-driven page.
pub(crate) fn menu_page_layout(state: &AppState, spec: &MenuPageSpec) -> MenuPageLayout {
    let (width, height) = shell_screen_size(state);
    if state.platform.menu_fill_window {
        menu_page::compute_responsive_layout(spec, width, height)
    } else {
        menu_page::compute_layout(spec, width, height)
    }
}

/// The skirmish dialog `0x102`, the page behind its modals.
pub(crate) fn skirmish_layout(state: &AppState) -> SkirmishShellLayout {
    let (width, height) = (state.render_width(), state.render_height());
    if state.platform.menu_fill_window {
        crate::ui::skirmish_shell::compute_responsive_layout(width, height)
    } else {
        crate::ui::skirmish_shell::compute_layout(width, height)
    }
}

/// Choose Map `0x6B`, which replaces `0x102` while it is open.
pub(crate) fn choose_map_layout(state: &AppState) -> ChooseMapModalLayout {
    let (width, height) = (state.render_width(), state.render_height());
    if state.platform.menu_fill_window {
        crate::ui::skirmish_shell::compute_responsive_choose_map_modal_layout(width, height)
    } else {
        crate::ui::skirmish_shell::compute_choose_map_modal_layout(width, height)
    }
}

/// Random-map setup `0x105`, which replaces `0x6B` while it is open.
pub(crate) fn random_map_setup_layout(state: &AppState) -> RandomMapSetupLayout {
    let (width, height) = (state.render_width(), state.render_height());
    if state.platform.menu_fill_window {
        crate::ui::skirmish_shell::compute_responsive_random_map_setup_layout(width, height)
    } else {
        crate::ui::skirmish_shell::compute_random_map_setup_layout(width, height)
    }
}

/// The saved-seed browser (Load / Save / Delete), which covers `0x105`.
pub(crate) fn saved_seed_layout(state: &AppState, mode: SavedSeedMode) -> SavedSeedLayout {
    let (width, height) = (state.render_width(), state.render_height());
    if state.platform.menu_fill_window {
        crate::ui::skirmish_shell::compute_responsive_saved_seed_layout(mode, width, height)
    } else {
        crate::ui::skirmish_shell::compute_saved_seed_layout(mode, width, height)
    }
}

/// Campaign selection `0x94`, the page behind Single Player's New Campaign.
pub(crate) fn campaign_layout(state: &AppState) -> crate::ui::campaign_shell::CampaignLayout {
    let (width, height) = shell_screen_size(state);
    if state.platform.menu_fill_window {
        crate::ui::campaign_shell::compute_responsive_layout(width, height)
    } else {
        crate::ui::campaign_shell::compute_layout(width, height)
    }
}

/// Single Player's Load Saved Game `0xB7`.
pub(crate) fn load_saved_game_layout(
    state: &AppState,
) -> crate::ui::shell::saved_games::MainMenuSavedGameLayout {
    let (width, height) = shell_screen_size(state);
    if state.platform.menu_fill_window {
        crate::ui::shell::saved_games::main_menu_saved_game_responsive_layout(width, height)
    } else {
        crate::ui::shell::saved_games::main_menu_saved_game_layout(width, height)
    }
}

/// Movie list `0x129`, the page behind Movies & Credits' Play Movies.
pub(crate) fn movie_list_layout(
    state: &AppState,
) -> crate::ui::movies_credits_shell::MovieListLayout {
    let (width, height) = shell_screen_size(state);
    if state.platform.menu_fill_window {
        crate::ui::movies_credits_shell::compute_responsive_movie_list_layout(width, height)
    } else {
        crate::ui::movies_credits_shell::compute_movie_list_layout(width, height)
    }
}

/// End-of-match score screen `0x108`.
pub(crate) fn score_layout(state: &AppState) -> crate::ui::score_shell::ScoreLayout {
    let (width, height) = shell_screen_size(state);
    if state.platform.menu_fill_window {
        crate::ui::score_shell::compute_responsive_layout(width, height)
    } else {
        crate::ui::score_shell::compute_layout(width, height)
    }
}

/// Launcher Options `0xD5`.
pub(crate) fn launcher_options_layout(
    state: &AppState,
) -> crate::ui::main_menu_dialogs::options::shell::LauncherOptionsLayout {
    let (width, height) = (state.render_width() as i32, state.render_height() as i32);
    if state.platform.menu_fill_window {
        crate::ui::main_menu_dialogs::options::shell::LauncherOptionsLayout::responsive(
            width, height,
        )
    } else {
        crate::ui::main_menu_dialogs::options::shell::LauncherOptionsLayout::new(width, height)
    }
}

/// The width a page's art set is keyed on. With the fill-window switch the
/// authored 800x600 is the space the layout maps from, so the page takes the
/// art it would show there: the large score SHP (gamemd swaps to the small one
/// only at exactly 640) and the Westwood Online background (loaded only at 800
/// wide by `0x0072C7E0`).
pub(crate) fn art_key_screen_width(state: &AppState) -> u32 {
    if state.platform.menu_fill_window {
        crate::ui::shell::geom::SHELL_BASE_W as u32
    } else {
        state.renderer.gpu.config.width
    }
}

/// Westwood Online welcome `0x10E`.
pub(crate) fn wol_welcome_layout(state: &AppState) -> crate::ui::wol_shell::WolWelcomeLayout {
    let (width, height) = shell_screen_size(state);
    if state.platform.menu_fill_window {
        crate::ui::wol_shell::compute_responsive_layout(width, height)
    } else {
        crate::ui::wol_shell::compute_layout(width, height)
    }
}

/// Keyboard `0xA3`. Only the front-end family page stretches: the same dialog
/// opened from the in-game menu sits inside the tactical shell's own frame, so it
/// keeps the centered layout and the pixel space its frame was measured in.
pub(crate) fn keyboard_layout(
    state: &AppState,
    width: i32,
    height: i32,
    shell: Option<(crate::ui::shell::in_game_shell::InGameShellLayout, [i32; 2])>,
) -> crate::ui::shell::keyboard::KeyboardLayout {
    match (state.platform.menu_fill_window, shell) {
        (true, None) => crate::ui::shell::keyboard::KeyboardLayout::responsive(width, height),
        _ => crate::ui::shell::keyboard::KeyboardLayout::new(width, height, shell),
    }
}
