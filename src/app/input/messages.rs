//! Chat/system message surface driver (study §3.1): anchors the
//! `ui::messages::MessageList` to the tactical viewport (x+3 / y / w−14),
//! posts system messages (insert sound = [AudioVisual] IncomingMessage),
//! expires rows on Main_Tick against the advancing process wall clock,
//! and builds the text instances drawn between sidebar text and tooltip.
//!
//! ## Dependency rules
//! - Part of the app layer — may depend on everything.

use crate::app::AppState;
use crate::render::batch::SpriteInstance;
use crate::ui::game_screen::GameScreen;
use crate::ui::shell::geom::RectPx;

/// Native Init anchors: x = tactical_x + 3, y = tactical_y, w = tactical_w − 14.
const MESSAGE_X_INSET: i32 = 3;
const MESSAGE_WIDTH_INSET: i32 = 14;
/// Fallback while match color schemes are unavailable.
const MESSAGE_RGB_SYSTEM: [f32; 3] = [1.0, 1.0, 1.0];
/// Names the local player is mentioned by are repainted in this colour.
const MESSAGE_MENTION_RGB: [u8; 3] = [255, 220, 60];
/// Opacity of the chat box's backing strips (history rows and input bar).
const CHATBOX_ROW_BACKING_ALPHA: f32 = 0.6;
/// The chat box caret blinks: 500 ms on, 500 ms off.
const CHATBOX_CARET_BLINK_MS: u64 = 500;
/// `0xF0` native 16 ms timer buckets.
const TYPE_SELECT_MESSAGE_TIMEOUT_MS: u64 = 3_840;
/// The Supers' lines' `0x96` native 16 ms timer buckets.
const SUPER_WEAPON_MESSAGE_TIMEOUT_MS: u64 = 0x96 * 16;
/// PlaceBeacon's help lines' `0xE1` buckets (`0x00430D01`, `0x00430D38`).
const BEACON_HELP_TIMEOUT_MS: u64 = 0xE1 * 16;
/// Native falls back to runtime color-scheme 3. Rust stores the undoubled
/// `[Colors]` entry index, so that scheme is entry 1.
const TYPE_SELECT_FALLBACK_SCHEME_ENTRY: crate::rules::house_colors::HouseColorIndex =
    crate::rules::house_colors::HouseColorIndex(3 / 2);

/// Add_Message5D3DD5 and Manage5D4449 read process timer887338/887340.
/// Menu/Scenario pause stops polling, not this clock. EnterPause683EB0 and
/// ExitPause683FB0 only stop/rearm the separate Scenario614 score clock.
/// Source: original gamemd.exe instructions and message-pause native controls.
pub(crate) fn message_now_ms(state: &AppState) -> u64 {
    crate::app::input::tooltips::now_ms(state)
}

/// Original TAction11 6DE0B0..6DE122 resolves its CSF label, selects the
/// scenario-side priority through69A310 and adds a non-silent4046 row.
/// MessageDelay's ftol result is in WallClock16ms buckets, not sim frames.
pub(crate) fn post_trigger_message(state: &mut AppState, csf_key: &str) {
    let Some(runtime) = state.match_state.sim_runtime.as_ref() else {
        return;
    };
    // The selected campaign opening keeps its startup player/country. The
    // existing House owner supplies that side; a later scenario-side change
    // independent of the current House remains a separate retained-field gap.
    // Ares `TActionClass_Execute_MessageColor @ 0x1002FA80` reads the saved
    // Scenario.PlayerSideIndex instead.
    #[cfg(feature = "ares")]
    let side = runtime.simulation.session().scenario_side();
    #[cfg(not(feature = "ares"))]
    let side = runtime
        .simulation
        .session()
        .current_house()
        .and_then(|owner| runtime.simulation.houses().get(&owner))
        .map_or(0, |house| i32::from(house.side_index()));
    let rules = &runtime.resources.rules;
    #[cfg(feature = "ares")]
    let rgb = ares_message_text_rgb(rules, side).unwrap_or_else(|| {
        trigger_message_rgb(&rules.color_schemes, side, rules.skirmish_colors())
    });
    #[cfg(not(feature = "ares"))]
    let rgb = trigger_message_rgb(&rules.color_schemes, side, rules.skirmish_colors());
    let timeout = message_delay_timeout_ms(rules.general.message_delay_minutes);
    post_localized_message(
        state,
        &crate::ui::messages::MessagePost {
            prefix: None,
            text: csf_key,
            rgb,
            visible_timeout_ms: timeout,
            retain_timeout_ms: timeout,
            silent: false,
        },
    );
}

fn message_delay_buckets(delay: crate::util::native_x87::NativeF64Bits) -> i32 {
    use crate::util::native_x87::{NativeF64Bits, X87Chop53};
    X87Chop53::load_f64(delay)
        .map(|delay| {
            X87Chop53::mul(
                delay,
                X87Chop53::load_f64(NativeF64Bits::from_bits(0x408C_2000_0000_0000))
                    .expect("native900 constant is finite"),
            )
        })
        .map_or(0, X87Chop53::ftol_i32_low_masked)
}

/// Ares `[Side] MessageTextColor=`: the stored runtime ColorScheme index (two
/// schemes per `[Colors]` entry) resolves to that entry. `-1`/an
/// out-of-range entry leaves the stock side-priority path unchanged.
#[cfg(feature = "ares")]
fn ares_message_text_rgb(rules: &crate::rules::ruleset::RuleSet, side: i32) -> Option<[f32; 3]> {
    let index = rules.side_message_text_color_index(side)?;
    if index < 0 {
        return None;
    }
    let entry = usize::try_from(index).ok()? / 2;
    crate::rules::color_scheme::scheme_hsv_by_entry(&rules.color_schemes, entry)
        .map(crate::rules::color_scheme::hsv_to_rgb)
        .map(|rgb| rgb.map(|channel| f32::from(channel) / 255.0))
}

fn trigger_message_rgb(
    schemes: &[crate::rules::color_scheme::ColorSchemeEntry],
    side: i32,
    skirmish_colors: crate::rules::skirmish_colors::SkirmishColorMode,
) -> [f32; 3] {
    let priority = match side {
        0 => 2,
        1 => 1,
        _ => 5,
    };
    let entry = skirmish_colors.entry_for_priority(priority);
    crate::rules::color_scheme::scheme_hsv_by_entry(schemes, entry)
        .map(crate::rules::color_scheme::hsv_to_rgb)
        .unwrap_or([255; 3])
        .map(|channel| f32::from(channel) / 255.0)
}

/// Post the localized, silent HUD result of one executed TypeSelect tap.
pub(crate) fn post_type_select_feedback(state: &mut AppState, csf_key: &str) {
    post_silent_house_feedback(state, csf_key, TYPE_SELECT_MESSAGE_TIMEOUT_MS);
}

///63A0D0/639FD0/639130 use the same silent730A90 HUD owner with duration1E0.
/// Their separate ScoldSound belongs to the planning input owner.
pub(crate) fn post_planning_refusal(state: &mut AppState, csf_key: &str) {
    post_silent_house_feedback(state, csf_key, 0x1E0 * 16);
}

fn post_silent_house_feedback(state: &mut AppState, csf_key: &str, timeout_ms: u64) {
    sync_view(state);
    let now = message_now_ms(state);
    let rgb = house_message_rgb(
        crate::app::input::commands::preferred_local_owner_name(state).as_deref(),
        &state.match_state.match_presentation.house_color_map,
        state.rules().map(|rules| rules.color_schemes.as_slice()),
    );
    let font = &state.renderer.bit_font;
    let measure = |s: &str| font.text_width(s) as i32;
    let outcome = add_localized_message(
        &mut state.match_state.match_presentation.message_list,
        state.process_assets.csf.as_ref(),
        &crate::ui::messages::MessagePost {
            prefix: None,
            text: csf_key,
            rgb,
            visible_timeout_ms: Some(timeout_ms),
            retain_timeout_ms: Some(timeout_ms),
            silent: true,
        },
        now,
        &measure,
    );
    debug_assert!(
        !outcome.play_sound,
        "TypeSelect feedback is a silent message add"
    );
}

#[cfg(test)]
fn add_type_select_feedback(
    list: &mut crate::ui::messages::MessageList,
    csf: Option<&crate::assets::csf_file::CsfFile>,
    csf_key: &str,
    rgb: [f32; 3],
    now_ms: u64,
    measure: &dyn Fn(&str) -> i32,
) -> crate::ui::messages::AddOutcome {
    add_localized_message(
        list,
        csf,
        &crate::ui::messages::MessagePost {
            prefix: None,
            text: csf_key,
            rgb,
            visible_timeout_ms: Some(TYPE_SELECT_MESSAGE_TIMEOUT_MS),
            retain_timeout_ms: Some(TYPE_SELECT_MESSAGE_TIMEOUT_MS),
            silent: true,
        },
        now_ms,
        measure,
    )
}

fn add_localized_message(
    list: &mut crate::ui::messages::MessageList,
    csf: Option<&crate::assets::csf_file::CsfFile>,
    post: &crate::ui::messages::MessagePost<'_>,
    now_ms: u64,
    measure: &dyn Fn(&str) -> i32,
) -> crate::ui::messages::AddOutcome {
    let text = csf
        .map(|table| table.text(post.text))
        .unwrap_or(std::borrow::Cow::Borrowed(post.text));
    list.add_message(
        &crate::ui::messages::MessagePost {
            text: text.as_ref(),
            ..*post
        },
        now_ms,
        measure,
    )
}

/// HealthNav uses the same silent, house-colored 0xF0-bucket message add as
/// TypeSelect, after formatting MSG:UnitsWorth/NoUnitsSel/NavEmpty (733569).
pub(crate) fn post_selection_navigation_text(state: &mut AppState, text: &str) {
    sync_view(state);
    let now = message_now_ms(state);
    let rgb = house_message_rgb(
        crate::app::input::commands::preferred_local_owner_name(state).as_deref(),
        &state.match_state.match_presentation.house_color_map,
        state.rules().map(|rules| rules.color_schemes.as_slice()),
    );
    let font = &state.renderer.bit_font;
    state
        .match_state
        .match_presentation
        .message_list
        .add_message(
            &crate::ui::messages::MessagePost {
                prefix: None,
                text,
                rgb,
                visible_timeout_ms: Some(TYPE_SELECT_MESSAGE_TIMEOUT_MS),
                retain_timeout_ms: Some(TYPE_SELECT_MESSAGE_TIMEOUT_MS),
                silent: true,
            },
            now,
            &|s| font.text_width(s) as i32,
        );
}

/// Local message facts share one ordered projection with the Supers' lines.
/// Admission/shortfall guard belongs to House, not to this presentation enum.
/// Native caller arguments: tools/spatial_oracle/house_power_consumers.py;
/// shared MessageDelay controls: trigger_action_values.json/localized_message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HudMessage {
    LowPower,
    SuperWeapon(&'static str),
    /// The configured label/style is resolved from its type at presentation.
    #[cfg(feature = "ares")]
    AresSuperWeaponRefused(crate::sim::intern::InternedId),
    /// MPlayer_Defeated's line for a non-observer House (`0x004FC271`,
    /// `0x004FC34F`).
    PlayerDefeated(crate::sim::intern::InternedId),
    /// MakeAlly's `TXT_HAS_ALLIED` or MakeEnemy's `TXT_AT_WAR` (owner,
    /// other) in the owner's HouseType colour (`0x004F9D4E..0x004F9DFC`,
    /// `0x004FA0E4..0x004FA175`).
    Alliance {
        owner: crate::sim::intern::InternedId,
        other: crate::sim::intern::InternedId,
        formed: bool,
    },
    /// PlaceBeacon's `TXT_BEACON_HELP1`/`2` for the local House's beacon
    /// (`0x00430CF9..0x00430D69`): PlayerPtr's scheme `+0x16054`, style
    /// 4046, `0xE1` buckets, silent 0.
    BeaconHelp(&'static str),
}

impl HudMessage {
    fn csf_key(self) -> Option<&'static str> {
        Some(match self {
            #[cfg(feature = "ares")]
            Self::AresSuperWeaponRefused(_) => return None,
            Self::LowPower => "TXT_LOW_POWER", // original House4F8D4A
            Self::SuperWeapon(key) | Self::BeaconHelp(key) => key,
            Self::PlayerDefeated(_) => "TXT_PLAYER_DEFEATED",
            Self::Alliance { formed: true, .. } => "TXT_HAS_ALLIED",
            Self::Alliance { formed: false, .. } => "TXT_AT_WAR",
        })
    }

    fn timeout_ms(self, rules: &crate::rules::ruleset::RuleSet) -> Option<u64> {
        match self {
            // House4F8D20..31 uses the same MessageDelay*900/ftol as
            // TAction11; these are 16 ms clock buckets, never speed normalized.
            Self::LowPower | Self::PlayerDefeated(_) | Self::Alliance { .. } => {
                message_delay_timeout_ms(rules.general.message_delay_minutes)
            }
            Self::SuperWeapon(_) => Some(SUPER_WEAPON_MESSAGE_TIMEOUT_MS),
            #[cfg(feature = "ares")]
            Self::AresSuperWeaponRefused(_) => {
                ares_message_delay_timeout_ms(rules.general.message_delay_minutes)
            }
            Self::BeaconHelp(_) => Some(BEACON_HELP_TIMEOUT_MS),
        }
    }

    fn post(
        self,
        rules: &crate::rules::ruleset::RuleSet,
        rgb: [f32; 3],
    ) -> Option<crate::ui::messages::MessagePost<'static>> {
        Some(crate::ui::messages::MessagePost {
            prefix: None,
            text: self.csf_key()?,
            rgb,
            timeout_ms: self.timeout_ms(rules),
            silent: false,
        })
    }
}

fn message_delay_timeout_ms(delay: crate::util::native_x87::NativeF64Bits) -> Option<u64> {
    let buckets = message_delay_buckets(delay);
    message_timeout_ms_from_buckets(buckets)
}

fn message_timeout_ms_from_buckets(buckets: i32) -> Option<u64> {
    (buckets != -1).then(|| u64::try_from(buckets).unwrap_or(0) * 16)
}

/// Ares PrintMessage10033BD0 uses CRT100772B0's signed32 conversion,
/// unlike the stock callers' Math7C5F00 low dword. Reuse the numeric owner.
#[cfg(feature = "ares")]
fn ares_message_delay_timeout_ms(delay: crate::util::native_x87::NativeF64Bits) -> Option<u64> {
    use crate::util::native_x87::{MaskedX87Chop53 as X, NativeF64Bits};
    let scaled = X::mul(
        X::load_f64(delay),
        X::load_f64(NativeF64Bits::from_bits(0x408c_2000_0000_0000)),
    );
    message_timeout_ms_from_buckets(X::fistp_i32_masked(scaled))
}

/// Installed Ares PrintMessage10033BD0: retain the caller's label, choose
/// firer/configured/player colour (Gold only for a null house), MessageDelay
/// in900 wall-clock buckets, no prefix and silent0. The existing message list
/// owns CSF lookup, empty-text admission, repeated rows, expiry and insert audio.
#[cfg(feature = "ares")]
fn ares_refusal_post<'a>(
    settings: &'a crate::rules::superweapon_type::AresSuperWeaponMessages,
    refusal: crate::sim::superweapon::AresCameoRefusal,
    schemes: &[crate::rules::color_scheme::ColorSchemeEntry],
    colors: &crate::map::houses::HouseColorMap,
    firer: Option<&str>,
    player: Option<&str>,
    delay: crate::util::native_x87::NativeF64Bits,
) -> Option<crate::ui::messages::MessagePost<'a>> {
    use crate::sim::superweapon::AresCameoRefusal;
    let text = match refusal {
        AresCameoRefusal::CannotFire => settings.cannot_fire(),
        AresCameoRefusal::InsufficientFunds => settings.insufficient_funds(),
        AresCameoRefusal::ManualFireDisabled => return None,
    };
    if text.is_empty() {
        return None;
    }
    let house = if settings.firer_color() {
        firer
    } else {
        player
    };
    let rgb = if !settings.firer_color() && settings.color_scheme() >= 0 {
        country_message_rgb(settings.color_scheme(), schemes)
    } else if let Some(house) = house {
        house_message_rgb(Some(house), colors, Some(schemes))
    } else {
        // PrintMessage10033BD0 calls ColorScheme::Find("Gold", 1): the even member.
        let gold = crate::rules::color_scheme::scheme_entry_by_name(schemes, "Gold")
            .map_or(-1, |entry| (entry as i32).wrapping_mul(2));
        country_message_rgb(gold, schemes)
    };
    Some(crate::ui::messages::MessagePost {
        prefix: None,
        text,
        rgb,
        timeout_ms: ares_message_delay_timeout_ms(delay),
        silent: false,
    })
}

#[cfg(feature = "ares")]
pub(crate) fn post_ares_superweapon_refusal(
    state: &mut AppState,
    section: &str,
    refusal: crate::sim::superweapon::AresCameoRefusal,
) {
    let Some(settings) = state
        .rules()
        .and_then(|rules| rules.super_weapon(section))
        .map(|sw| sw.ares_messages().clone())
    else {
        return;
    };
    let owner = crate::app::input::commands::preferred_local_owner_name(state);
    let Some(rules) = state.rules() else {
        return;
    };
    let Some(post) = ares_refusal_post(
        &settings,
        refusal,
        &rules.color_schemes,
        &state.match_state.match_presentation.house_color_map,
        owner.as_deref(),
        owner.as_deref(),
        rules.general.message_delay_minutes,
    ) else {
        return;
    };
    post_localized_message(state, &post);
}

/// House4F8D5C and the Super print callers use MessageList5D3BA0 with
/// PlayerPtr's color, no sender, and silent0. Keep the existing list/clock,
/// localization and IncomingMessage owners; no warning timer/guard lives here.
/// The callers' text flags still share the existing bitmap-text lane; this
/// transport does not establish native glyph/style raster equivalence.
pub(crate) fn post_hud_message(state: &mut AppState, message: HudMessage) {
    #[cfg(feature = "ares")]
    if let HudMessage::AresSuperWeaponRefused(sw_type) = message {
        let section = state.match_state.sim_runtime.as_ref().and_then(|runtime| {
            runtime
                .simulation
                .interner()
                .try_resolve(sw_type)
                .map(str::to_owned)
        });
        if let Some(section) = section {
            post_ares_superweapon_refusal(
                state,
                &section,
                crate::sim::superweapon::AresCameoRefusal::CannotFire,
            );
        }
        return;
    }
    let Some(key) = message.csf_key() else {
        return;
    };
    if let HudMessage::PlayerDefeated(house) = message {
        post_house_line(state, key, house, HouseLineColor::Player);
        return;
    }
    if let HudMessage::Alliance { owner, other, .. } = message {
        post_house_names_line(state, key, &[owner, other], HouseLineColor::Country);
        return;
    }
    let Some(rules) = state.rules() else {
        return;
    };
    let rgb = house_message_rgb(
        crate::app::input::commands::preferred_local_owner_name(state).as_deref(),
        &state.match_state.match_presentation.house_color_map,
        Some(&rules.color_schemes),
    );
    if let Some(post) = message.post(rules, rgb) {
        post_localized_message(state, &post);
    }
}

/// The colour of a House-named line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HouseLineColor {
    /// The player's colour scheme: MPlayer_Defeated (House+0x16054 remote,
    /// Session+0x15C local, which VERA reads as the House's own scheme).
    Player,
    /// The HouseType's `Color=` scheme `[[House+0x34]+0xC0]`:
    /// DestroyConnection (`0x005DA827..0x005DA834`).
    Country,
}

/// A House-named line: the CSF template formatted with House+0x1602A, for
/// MessageDelay*900 buckets, silent 0. DestroyConnection5DA750 posts
/// TXT_LEFT_GAME/TXT_CONNECTION_LOST (name read at 5DA772); MPlayer_Defeated
/// posts TXT_PLAYER_DEFEATED (4FC271..4FC2DB local, 4FC34F..4FC3AD remote).
pub(crate) fn post_house_line(
    state: &mut AppState,
    key: &str,
    house: crate::sim::intern::InternedId,
    color: HouseLineColor,
) {
    post_house_names_line(state, key, &[house], color);
}

/// [`post_house_line`] with one `%s` per House, coloured by the first.
fn post_house_names_line(
    state: &mut AppState,
    key: &str,
    houses: &[crate::sim::intern::InternedId],
    color: HouseLineColor,
) {
    let Some(names) = houses
        .iter()
        .map(|&house| crate::app::presentation::house_ui::house_ui_name(state, house))
        .collect::<Option<Vec<String>>>()
    else {
        return;
    };
    let Some(&house) = houses.first() else {
        return;
    };
    let Some(sim) = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|runtime| &runtime.simulation)
    else {
        return;
    };
    let owner = sim.interner().resolve(house).to_string();
    let country = sim
        .houses()
        .get(&house)
        .and_then(|state| state.country())
        .map(|country| sim.interner().resolve(country).to_string());
    let Some(rules) = state.rules() else {
        return;
    };
    let rgb = match color {
        HouseLineColor::Player => house_message_rgb(
            Some(&owner),
            &state.match_state.match_presentation.house_color_map,
            Some(&rules.color_schemes),
        ),
        HouseLineColor::Country => {
            // HouseType+0xC0 holds 2*entry+1 (ReadColor 0x474A90).
            let scheme = country.map_or(0, |country| rules.country_color_scheme(&country));
            country_message_rgb(scheme, &rules.color_schemes)
        }
    };
    let timeout_ms = message_delay_timeout_ms(rules.general.message_delay_minutes);
    let template = crate::app::frontend::shell_pass::resolve_csf(state, key).into_owned();
    let args: Vec<crate::assets::csf_file::CsfArg<'_>> = names
        .iter()
        .map(|name| crate::assets::csf_file::CsfArg::Str(name))
        .collect();
    let text = crate::assets::csf_file::format_csf(&template, &args);
    post_resolved_message(
        state,
        &crate::ui::messages::MessagePost {
            prefix: None,
            text: &text,
            rgb,
            visible_timeout_ms: timeout_ms,
            retain_timeout_ms: timeout_ms,
            silent: false,
        },
    );
}

/// MapPlayerColorToScheme `0x0069A310` of a player's Session colour: VERA
/// reads it as the House's own scheme, as MPlayer_Defeated's lines do.
pub(crate) fn player_line_rgb(state: &AppState, owner: crate::sim::intern::InternedId) -> [f32; 3] {
    let owner = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|runtime| runtime.simulation.interner().resolve(owner).to_string());
    house_message_rgb(
        owner.as_deref(),
        &state.match_state.match_presentation.house_color_map,
        state.rules().map(|rules| rules.color_schemes.as_slice()),
    )
}

/// Network packet colour is a Session priority, converted by69A310 before
/// TextLabel72A4A0. Keep this in the same message-colour owner as local echoes.
pub(crate) fn packet_player_line_rgb(state: &AppState, priority: i32) -> [f32; 3] {
    let Some(rules) = state.rules() else {
        return MESSAGE_RGB_SYSTEM;
    };
    country_message_rgb(
        rules.skirmish_colors().scheme_index_for_priority(priority) as i32,
        &rules.color_schemes,
    )
}

/// A chat line: Add_Message with the sender's name as prefix, style 4046,
/// MessageDelay*900 buckets and silent 0, so IncomingMessage plays once
/// (`0x0055F0A8..0x0055F104` echo, `0x0048D93A..0x0048D98D` receive).
///
/// Layout-aware deadlines:
/// - Native: both visible and retain = MessageDelay (gamemd hard cutoff).
/// - ChatBox: visible = MessageDelay (row fades off screen), retain = None
///   (kept in history so a reopened chat still shows it).
pub(crate) fn post_chat_line(state: &mut AppState, name: &str, text: &str, rgb: [f32; 3]) {
    let Some(rules) = state.rules() else {
        return;
    };
    let layout = state.match_state.match_presentation.message_list.layout();
    let visible_timeout_ms = message_delay_timeout_ms(rules.general.message_delay_minutes);
    let retain_timeout_ms = match layout {
        crate::ui::messages::MessageLayout::Native => visible_timeout_ms,
        crate::ui::messages::MessageLayout::ChatBox => None,
    };
    // VeraX chat box: stamp the row with the local clock and append the line
    // to the on-disk chat log. The native layout keeps gamemd's plain text and
    // writes nothing.
    // The log keeps the clock so a session can be reviewed; the chat box shows
    // the plain line, as the Minecraft reference does.
    if layout == crate::ui::messages::MessageLayout::ChatBox {
        let stamp = chat_time_stamp().unwrap_or_default();
        crate::app::input::chat_log::append(state, &format!("[{stamp}] {name}: {text}"));
    }
    // Every chat line carries its sender, as the Minecraft reference does.
    post_resolved_message(
        state,
        &crate::ui::messages::MessagePost {
            prefix: Some(name),
            text,
            rgb,
            visible_timeout_ms,
            retain_timeout_ms,
            silent: false,
        },
    );
}

/// A VeraX-only client string: the language pack's entry for `key` when it
/// carries one, else `fallback`. Retail CSF has no keys for the chat box's
/// own chrome, so the fallback is what most languages see.
fn verax_ui_text(state: &AppState, key: &str, fallback: &str) -> String {
    ui_text_or(
        crate::app::frontend::shell_pass::resolve_csf(state, key).as_ref(),
        key,
        fallback,
    )
}

/// A language pack without the key yields the CSF's own missing marker
/// (`CsfFile::text`, `MISSING:'<key>'`), not the key itself, so both mean
/// "no translation here".
fn ui_text_or(text: &str, key: &str, fallback: &str) -> String {
    if text == key || text == format!("MISSING:'{key}'") {
        fallback.to_owned()
    } else {
        text.to_owned()
    }
}
/// The local clock as the OS locale's short time (e.g. `14:32`). Uses the
/// existing file-time formatter, which delegates to `GetTimeFormatA` on
/// Windows and `strftime` with the user's locale on Unix, so the stamp reads
/// the way the player's own clock does.
pub(crate) fn chat_time_stamp() -> Option<String> {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    // FILETIME counts 100 ns ticks from 1601-01-01; the Unix epoch is
    // 11_644_473_600 s into that epoch.
    let ticks =
        (unix.as_secs() + 11_644_473_600) * 10_000_000 + u64::from(unix.subsec_nanos()) / 100;
    crate::util::native_file_time::format_file_time_parts(ticks).map(|(_, time)| time)
}

/// Radar subtitle callback `0x005BF360`: literal text, scheme 0 and native
/// MessageList duration buckets; cue Position/RGB belong to full-screen movies.
pub(crate) fn post_movie_subtitle(state: &mut AppState, text: &str, duration: u32) {
    let rgb = state.rules().map_or(MESSAGE_RGB_SYSTEM, |rules| {
        country_message_rgb(0, &rules.color_schemes)
    });
    post_resolved_message(state, &movie_subtitle_post(text, duration, rgb));
}

fn movie_subtitle_post(
    text: &str,
    duration: u32,
    rgb: [f32; 3],
) -> crate::ui::messages::MessagePost<'_> {
    crate::ui::messages::MessagePost {
        prefix: None,
        text,
        rgb,
        // 0x005BF360 passes the cue duration as Add_Message's signed timeout.
        timeout_ms: message_timeout_ms_from_buckets(duration as i32),
        silent: false,
    }
}

/// The observer gate's silent `TXT_OBSERVER_CANT_SEND` row in the "Grey"
/// scheme (`0x004E12D0` with `0x00822728`), flags 0x16, MessageDelay*900
/// (`0x0055EED0..0x0055EF2C`).
pub(crate) fn post_observer_refusal(state: &mut AppState) {
    post_grey_row(state, "TXT_OBSERVER_CANT_SEND", true, true);
}

/// The network command queue was full when a local order tried to enqueue.
pub(super) fn post_command_backpressure(state: &mut AppState) {
    post_grey_row(
        state,
        "Commands are still being sent. Try again shortly.",
        false,
        true,
    );
}

/// The WOL page sinks in a match (`0x007A92F9..0x007A93B4`): the page line
/// and, from the ANSI sink, `TXT_HOW_TO_REPLY` under it, each a Grey row
/// with flags 0x16, `MessageDelay * 900` and silent 0.
pub(crate) fn post_wol_page_rows(state: &mut AppState, line: &str, reply_hint: bool) {
    post_grey_row(state, line, false, false);
    if reply_hint {
        post_grey_row(state, "TXT_HOW_TO_REPLY", true, false);
    }
}

/// A WOL find or page-result row in a match with no window to take it
/// (`0x007AA5D5..`, `0x007A9800`): the same Grey row, silent 0.
pub(crate) fn post_wol_result_row(state: &mut AppState, text: &str) {
    post_grey_row(state, text, false, false);
}

/// The WOL NetStatus sink's lost connection in a match (`0x007AA7C0..`):
/// one `TXT_WOL_CONNLOST` row in the same scheme, flags and timeout, with
/// its sound.
pub(crate) fn post_wol_connection_lost(state: &mut AppState) {
    post_grey_row(state, "TXT_WOL_CONNLOST", true, false);
}

/// Cooperative `+0xA0`'s save result (`0x005C37B7..0x005C381F`):
/// `TXT_COOP_SAVE_SUCCESS` or `TXT_COOP_SAVE_FAILED`, a Grey row with its
/// sound.
pub(crate) fn post_cooperative_save_result(state: &mut AppState, saved: bool) {
    let key = if saved {
        "TXT_COOP_SAVE_SUCCESS"
    } else {
        "TXT_COOP_SAVE_FAILED"
    };
    post_grey_row(state, key, true, false);
}

/// A row in `GadgetClass::GetColorScheme` (`0x004E12D0`, "Grey"), flags
/// 0x16, `MessageDelay * 900`; `text` is a CSF key when `localize`.
fn post_grey_row(state: &mut AppState, text: &str, localize: bool, silent: bool) {
    let Some(rules) = state.rules() else {
        return;
    };
    let timeout_ms = message_delay_timeout_ms(rules.general.message_delay_minutes);
    let rgb = crate::rules::color_scheme::scheme_entry_by_name(&rules.color_schemes, "Grey")
        .and_then(|entry| u8::try_from(entry).ok())
        .map_or(MESSAGE_RGB_SYSTEM, |entry| {
            crate::rules::house_colors::house_text_rgb(
                &rules.color_schemes,
                crate::rules::house_colors::HouseColorIndex(entry),
            )
            .map(|channel| f32::from(channel) / 255.0)
        });
    post_message(
        state,
        &crate::ui::messages::MessagePost {
            prefix: None,
            text,
            rgb,
            visible_timeout_ms: timeout_ms,
            retain_timeout_ms: timeout_ms,
            silent,
        },
        localize,
    );
}

/// Post a row whose `text` is a CSF key.
fn post_localized_message(state: &mut AppState, post: &crate::ui::messages::MessagePost<'_>) {
    post_message(state, post, true);
}

/// Post a row whose `text` is already formatted from its CSF template.
fn post_resolved_message(state: &mut AppState, post: &crate::ui::messages::MessagePost<'_>) {
    post_message(state, post, false);
}

fn post_message(state: &mut AppState, post: &crate::ui::messages::MessagePost<'_>, localize: bool) {
    sync_view(state);
    let now = message_now_ms(state);
    let font = &state.renderer.bit_font;
    let outcome = add_localized_message(
        &mut state.match_state.match_presentation.message_list,
        state.process_assets.csf.as_ref().filter(|_| localize),
        post,
        now,
        &|s| font.text_width(s) as i32,
    );
    if outcome.play_sound {
        let sound = state
            .rules()
            .and_then(|r| r.general.incoming_message_sound().map(str::to_owned));
        crate::app::App::play_shell_ui_sound_by_id(state, sound.as_deref());
    }
}

/// TextLabel72A4A0 reads scheme+308's original HSV, unlike the unit
/// palette ramp. Shared by TypeSelect732BF0, HUD730AC3, HealthNav7335A7,
/// Super prints53AE42/53B452 and House4F8D5C. Native caller-null fallback3
/// is [Colors] entry1; TextLabel72A4D6's invalid-index fallback is scheme0.
/// Source: tools/procedural_drawing_oracle/house_color.md, message consumer.
/// A runtime scheme index through TextLabel's invalid-index fallback to
/// scheme 0 (`0x0072A4D6`).
fn country_message_rgb(
    scheme: i32,
    schemes: &[crate::rules::color_scheme::ColorSchemeEntry],
) -> [f32; 3] {
    let entry = u8::try_from(scheme / 2)
        .ok()
        .filter(|entry| usize::from(*entry) < schemes.len())
        .unwrap_or(0);
    crate::rules::house_colors::house_text_rgb(
        schemes,
        crate::rules::house_colors::HouseColorIndex(entry),
    )
    .map(|channel| f32::from(channel) / 255.0)
}

fn house_message_rgb(
    local_owner: Option<&str>,
    house_colors: &crate::map::houses::HouseColorMap,
    schemes: Option<&[crate::rules::color_scheme::ColorSchemeEntry]>,
) -> [f32; 3] {
    let Some(schemes) = schemes else {
        return MESSAGE_RGB_SYSTEM;
    };
    let mut scheme = local_owner
        .and_then(|owner| house_colors.get(owner).copied())
        .unwrap_or(TYPE_SELECT_FALLBACK_SCHEME_ENTRY);
    if usize::from(scheme.0) >= schemes.len() {
        scheme = crate::rules::house_colors::HouseColorIndex(0);
    }
    crate::rules::house_colors::house_text_rgb(schemes, scheme)
        .map(|channel| f32::from(channel) / 255.0)
}

/// Main_Tick55DDA5 polls Manage only when its offline menu/Scenario holds
/// permit it. On the first resumed pass, deadlines include the entire hold.
pub(crate) fn update(state: &mut AppState, main_tick_admitted: bool) -> Option<u64> {
    if state.frontend.screen != GameScreen::InGame
        || !poll_is_admitted(main_tick_admitted, state.match_state.main_tick_frozen())
    {
        return None;
    }
    sync_view(state);
    let now = message_now_ms(state);
    state
        .match_state
        .match_presentation
        .message_list
        .manage(now);
    Some(now)
}

/// A hold created by an already-admitted Logic cannot revoke this pass's
/// Manage call. An expired hold is released before runtime admission.
fn poll_is_admitted(main_tick_admitted: bool, currently_frozen: bool) -> bool {
    main_tick_admitted || !currently_frozen
}

fn sync_view(state: &mut AppState) {
    let typing = state
        .frontend
        .multiplayer_shell_runtime
        .scroll_text(state.match_state.session_mode());
    // Read the viewport size before taking the mutable list borrow.
    let (tactical_w, tactical_h) = crate::app::input::camera::tactical_viewport_size(state);
    let tactical_w = tactical_w as i32;
    let tactical_h = tactical_h as i32;
    let list = &mut state.match_state.match_presentation.message_list;
    list.set_typing(typing);
    // VeraX builds use the Minecraft chat box layout with its 100-row history
    // cap; native builds keep the stock TextLabel layout and its 6-row retail
    // cap. sync_view runs before every message add, so the layout and cap stay
    // correct even if the list was reconstructed between frames.
    if crate::rules::VERAX_ENABLED {
        list.set_layout(crate::ui::messages::MessageLayout::ChatBox);
        list.set_max_visible(crate::ui::messages::MESSAGE_MAX_VISIBLE_CHATBOX);
    } else {
        list.set_layout(crate::ui::messages::MessageLayout::Native);
        list.set_max_visible(crate::ui::messages::MESSAGE_MAX_VISIBLE_RETAIL);
    }
    // The list spans the tactical viewport: the frame left of the sidebar.
    list.set_view(
        MESSAGE_X_INSET,
        0,
        (tactical_w - MESSAGE_WIDTH_INSET).max(0),
        tactical_h,
    );
}

/// The open edit line in window pixels, where the input method shows its
/// candidates; `None` while no edit is open.
pub(crate) fn ime_edit_rect(state: &AppState) -> Option<RectPx> {
    if state.frontend.screen != GameScreen::InGame {
        return None;
    }
    let list = &state.match_state.match_presentation.message_list;
    let edit = list.edit_line()?;
    let font = &state.renderer.bit_font;
    // The message rows are tactical pixels; the zoom pass stretches them over
    // the window.
    let sx = state.renderer.gpu.config.width as f32 / state.render_width().max(1) as f32;
    let sy = state.renderer.gpu.config.height as f32 / state.render_height().max(1) as f32;
    let (width, height) = if list.layout() == crate::ui::messages::MessageLayout::ChatBox {
        // The chat box's input bar spans the view, so the candidates sit
        // under its whole line rather than behind the typed text.
        let tactical_w = crate::app::input::camera::tactical_viewport_size(state).0 as i32;
        (
            (tactical_w - MESSAGE_WIDTH_INSET).max(0) as f32,
            crate::ui::messages::MESSAGE_LINE_HEIGHT_PX as f32,
        )
    } else {
        (font.text_width(&edit.text) as f32, font.cell_height())
    };
    Some(RectPx::new(
        (list.x() as f32 * sx) as i32,
        (edit.y as f32 * sy) as i32,
        (width * sx) as i32,
        (height * sy) as i32,
    ))
}

/// Prepare one successful offline TextLabel paint, using raw wall time for
/// typing and the existing sound owner for every MessageCharTyped request.
/// Expiry uses the same wall clock, but is polled only by admitted Main_Tick passes.
pub(crate) fn build_message_instances(
    state: &mut AppState,
) -> (Vec<SpriteInstance>, Vec<SpriteInstance>) {
    if state.frontend.screen != GameScreen::InGame {
        return (Vec::new(), Vec::new());
    }
    let wall_ms = crate::app::input::tooltips::now_ms(state) as u64;
    let requests = state
        .match_state
        .match_presentation
        .message_list
        .advance_reveal(wall_ms as u32);
    if requests > 0 {
        let sound = state
            .rules()
            .and_then(|rules| rules.general.message_char_typed_sound().map(str::to_owned));
        for _ in 0..requests {
            crate::app::App::play_shell_ui_sound_by_id(state, sound.as_deref());
        }
    }
    let mention = state
        .match_state
        .session
        .local_player_name()
        .filter(|name| !name.is_empty())
        .map(str::to_owned);
    let composition = state
        .platform
        .ime
        .composition(crate::app::input::ime::ImeTarget::InGameChat)
        .map(|(units, _)| units);
    // Menu labels come from the language pack when it carries them; the
    // English fallback keeps the menu usable otherwise.
    let copy = verax_ui_text(state, "TXT_VERAX_COPY", "Copy");
    let open_link = verax_ui_text(state, "TXT_VERAX_OPEN_LINK", "Open link");
    message_instances(
        &state.renderer.bit_font,
        &state.match_state.match_presentation.message_list,
        [
            state.match_state.input.camera_x,
            state.match_state.input.camera_y,
        ],
        wall_ms,
        &ChatChrome {
            mention: mention.as_deref(),
            composition,
            menu_labels: Some([copy.as_str(), open_link.as_str()]),
        },
    )
}

/// VeraX-only chat chrome the render pass needs but the model does not hold:
/// the local player's mention name, the live IME composition, and the two
/// right-click menu labels (already resolved through the language pack).
#[derive(Default)]
struct ChatChrome<'a> {
    mention: Option<&'a str>,
    composition: Option<&'a [u16]>,
    menu_labels: Option<[&'a str; 2]>,
}

fn message_instances(
    font: &crate::render::bit_font::BitFont,
    list: &crate::ui::messages::MessageList,
    camera_offset: [f32; 2],
    wall_ms: u64,
    chrome: &ChatChrome<'_>,
) -> (Vec<SpriteInstance>, Vec<SpriteInstance>) {
    let x = list.x() as f32;
    let layout = list.layout();
    let editing = list.editing();
    let messages_slice: &[crate::ui::messages::Message] = list.messages();
    let mut fills = Vec::new();
    let mut texts = Vec::new();

    // Draw 0x005D49A0 paints the edit label first, whole: its reveal flag
    // is never set and the chat cursor character 0 draws no caret.
    if let Some(edit_line) = list.edit_line() {
        let m = crate::ui::messages::Message::edit_row(&edit_line);
        let alpha = 1.0_f32;
        build_row_instances(
            font,
            &m,
            x,
            camera_offset,
            alpha,
            chatbox_input_backing(list),
            &mut fills,
            &mut texts,
        );
        // The VeraX chat box paints the open composition in the NewEdit
        // composition colour (`0x00777EA0`, `[0xAC4618]`) after the text the
        // input method is editing; the native layout leaves it to the system
        // IME window, as OwnerDraw__ServiceIMEMessage `0x00778030` does.
        let composing = (layout == crate::ui::messages::MessageLayout::ChatBox)
            .then_some(chrome.composition)
            .flatten()
            .map(String::from_utf16_lossy)
            .filter(|text| !text.is_empty());
        let mut caret_x = font.text_width(&edit_line.text) as f32;
        // An open search paints its own bar over the message line.
        if let Some(query) = list.search_query() {
            push_message_fill(
                camera_offset,
                [
                    x - 1.0,
                    (edit_line.y - crate::ui::messages::MESSAGE_INPUT_TEXT_INSET_PX) as f32,
                ],
                [
                    list.view_width() as f32 + 2.0,
                    crate::ui::messages::MESSAGE_INPUT_BAR_HEIGHT_PX as f32,
                ],
                [0.0, 0.0, 0.0],
                0.85,
                0.00023,
                &mut fills,
            );
            let shown = format!("? {query}");
            texts.extend(font.build_text_label(
                &shown,
                x,
                edit_line.y as f32,
                0.00021,
                camera_offset,
                MESSAGE_MENTION_RGB,
                0,
            ));
            // The search bar owns the keys, so the composition goes here.
            caret_x = font.text_width(&shown) as f32;
            if let Some(composing) = composing.as_deref() {
                texts.extend(font.build_text_label(
                    composing,
                    x + caret_x,
                    edit_line.y as f32,
                    0.00022,
                    camera_offset,
                    crate::app::input::ime::COMPOSITION_RGB,
                    0,
                ));
                caret_x += font.text_width(composing) as f32;
            }
        } else if let Some(composing) = composing.as_deref() {
            texts.extend(font.build_text_label(
                composing,
                x + caret_x,
                edit_line.y as f32,
                0.00022,
                camera_offset,
                crate::app::input::ime::COMPOSITION_RGB,
                0,
            ));
            caret_x += font.text_width(composing) as f32;
        }
        // The chat box shows the text caret the native label never draws:
        // an underscore just past the typed text, blinking half a second
        // on and half a second off.
        let caret_on = wall_ms % (CHATBOX_CARET_BLINK_MS * 2) < CHATBOX_CARET_BLINK_MS;
        if layout == crate::ui::messages::MessageLayout::ChatBox && caret_on {
            push_message_fill(
                camera_offset,
                [
                    x + caret_x + 1.0,
                    (edit_line.y + font.cell_height() as i32 - 3) as f32,
                ],
                [4.0, 2.0],
                [1.0, 1.0, 1.0],
                0.9,
                0.00021,
                &mut fills,
            );
        }
    }

    // Draw history rows, branch by layout/editing state.
    match (layout, editing) {
        (crate::ui::messages::MessageLayout::ChatBox, false) => {
            // ChatBox closed: filter by is_visible(), apply alpha_at() fade.
            for m in messages_slice {
                if !m.is_visible(wall_ms) {
                    continue;
                }
                let alpha = m.alpha_at(wall_ms);
                build_row_instances(
                    font,
                    m,
                    x,
                    camera_offset,
                    alpha,
                    chatbox_history_backing(list),
                    &mut fills,
                    &mut texts,
                );
            }
        }
        (crate::ui::messages::MessageLayout::ChatBox, true) => {
            // ChatBox open: show the visible window of history rows at
            // alpha=1.0. Scroll offset selects which rows are in view.
            for i in list.display_rows() {
                let m = &messages_slice[i];
                build_row_instances(
                    font,
                    m,
                    x,
                    camera_offset,
                    1.0_f32,
                    chatbox_history_backing(list),
                    &mut fills,
                    &mut texts,
                );
                // Mention highlight: repaint the local player's name in the
                // highlight colour, one sprite run per occurrence.
                if let Some(name) = chrome.mention {
                    push_mention_runs(
                        font,
                        &m.text,
                        m.y,
                        name,
                        x,
                        camera_offset,
                        1.0_f32,
                        m.prefix_bytes,
                        &mut texts,
                    );
                }
            }
            // Selection band: the painted history rows that fall inside the
            // drag, clipped to the visible window. Drawn between the row box
            // and its glyphs so the text stays readable.
            if !list.search_active()
                && let Some((sel_start, sel_end)) = list.selection()
            {
                let (win_start, win_end) = list.visible_message_range();
                let band_height = font.cell_height() as f32 + 2.0;
                for (i, m) in messages_slice.iter().enumerate() {
                    if i < win_start || i >= win_end || i < sel_start.row || i > sel_end.row {
                        continue;
                    }
                    let len = m.text.chars().count();
                    let from = if i == sel_start.row {
                        sel_start.char_index.min(len)
                    } else {
                        0
                    };
                    let to = if i == sel_end.row {
                        sel_end.char_index.min(len)
                    } else {
                        len
                    };
                    if to <= from {
                        continue;
                    }
                    let x0 = x + font.text_width(&m.text[..char_byte_offset(&m.text, from)]) as f32;
                    let x1 = x + font.text_width(&m.text[..char_byte_offset(&m.text, to)]) as f32;
                    push_message_fill(
                        camera_offset,
                        [x0, m.y as f32 - 1.0],
                        [x1 - x0, band_height],
                        [0.25, 0.5, 0.95],
                        0.55,
                        0.000225,
                        &mut fills,
                    );
                }
            }
            // Scroll indicator: a dim track plus a thumb whose position and
            // length show where the window sits in the retained history.
            // VeraX-only chrome (gamemd has no scrollable history).
            if let Some(bar) = list.scrollbar_geometry() {
                push_message_fill(
                    camera_offset,
                    [bar.x as f32, bar.track_y as f32],
                    [
                        crate::ui::messages::MESSAGE_SCROLLBAR_WIDTH_PX as f32,
                        bar.track_height as f32,
                    ],
                    [0.0, 0.0, 0.0],
                    0.35,
                    0.00023,
                    &mut fills,
                );
                let thumb_tint = if list.has_unread() {
                    [1.0, 0.85, 0.3]
                } else {
                    [0.75, 0.75, 0.75]
                };
                push_message_fill(
                    camera_offset,
                    [bar.x as f32, bar.thumb_y as f32],
                    [
                        crate::ui::messages::MESSAGE_SCROLLBAR_WIDTH_PX as f32,
                        bar.thumb_height as f32,
                    ],
                    thumb_tint,
                    0.9,
                    0.00023,
                    &mut fills,
                );
            }
            // Unread accent: a thin line above the input bar says rows
            // arrived below the window (End or the wheel jumps to them).
            if let Some(marker_y) = list.unread_marker_y() {
                push_message_fill(
                    camera_offset,
                    [x, marker_y as f32],
                    [
                        (list.view_width() as f32),
                        crate::ui::messages::MESSAGE_UNREAD_MARKER_PX as f32,
                    ],
                    [1.0, 0.85, 0.3],
                    0.8,
                    0.00023,
                    &mut fills,
                );
            }
        }
        (crate::ui::messages::MessageLayout::Native, _) => {
            // Native: all rows, alpha=1.0 (gamemd has no fade).
            for m in messages_slice {
                build_row_instances(
                    font,
                    m,
                    x,
                    camera_offset,
                    1.0_f32,
                    chatbox_history_backing(list),
                    &mut fills,
                    &mut texts,
                );
            }
        }
    }

    // Right-click menu: box, hover band, and one label per row.
    if let Some(menu) = list.menu() {
        let labels = chrome.menu_labels.unwrap_or(["Copy", "Open link"]);
        push_message_fill(
            camera_offset,
            [menu.x as f32, menu.y as f32],
            [
                crate::ui::messages::MESSAGE_MENU_WIDTH_PX as f32,
                menu.height() as f32,
            ],
            [0.05, 0.05, 0.08],
            0.94,
            0.00019,
            &mut fills,
        );
        for (row, label) in labels.iter().enumerate() {
            let row_y = menu.y
                + crate::ui::messages::MESSAGE_MENU_PADDING_PX
                + row as i32 * crate::ui::messages::MESSAGE_MENU_ROW_HEIGHT_PX;
            let enabled = menu.row_enabled(row);
            if enabled && menu.hover == Some(row) {
                push_message_fill(
                    camera_offset,
                    [menu.x as f32 + 1.0, row_y as f32],
                    [
                        crate::ui::messages::MESSAGE_MENU_WIDTH_PX as f32 - 2.0,
                        crate::ui::messages::MESSAGE_MENU_ROW_HEIGHT_PX as f32,
                    ],
                    [0.25, 0.45, 0.85],
                    0.7,
                    0.000185,
                    &mut fills,
                );
            }
            let tint = if enabled {
                [220u8, 220, 220]
            } else {
                [110, 110, 110]
            };
            texts.extend(font.build_text_label(
                label,
                (menu.x + 6) as f32,
                (row_y + 5) as f32,
                0.00018,
                camera_offset,
                tint,
                0,
            ));
        }
    }

    (fills, texts)
}

/// The VeraX chat box history row's backing strip: a fixed two-thirds of the
/// view, whatever the row's text length; None for the native layout, whose
/// rows keep gamemd's label-sized box.
fn chatbox_history_backing(list: &crate::ui::messages::MessageList) -> Option<RowBacking> {
    (list.layout() == crate::ui::messages::MessageLayout::ChatBox).then_some(RowBacking {
        width: list.view_width() as f32 * crate::ui::messages::CHATBOX_ROW_WIDTH_FRACTION,
        alpha: CHATBOX_ROW_BACKING_ALPHA,
    })
}
/// The open input bar's backing keeps the full-view strip it already had.
fn chatbox_input_backing(list: &crate::ui::messages::MessageList) -> Option<RowBacking> {
    (list.layout() == crate::ui::messages::MessageLayout::ChatBox).then_some(RowBacking {
        width: list.view_width() as f32 + 2.0,
        alpha: CHATBOX_ROW_BACKING_ALPHA,
    })
}
/// A flat screen-space fill for the chat box chrome (scrollbar track, thumb,
/// unread accent, selection band). Same camera compensation as a message row's
/// backing box; `depth` picks the layer (smaller is nearer the viewer, so the
/// row box sits behind its glyphs at 0.00023 and the text at 0.00022).
fn push_message_fill(
    camera_offset: [f32; 2],
    position: [f32; 2],
    size: [f32; 2],
    tint: [f32; 3],
    alpha: f32,
    depth: f32,
    fills: &mut Vec<SpriteInstance>,
) {
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return;
    }
    fills.push(SpriteInstance {
        position: [
            position[0] + camera_offset[0],
            position[1] + camera_offset[1],
        ],
        size,
        uv_origin: [0.0, 0.0],
        uv_size: [1.0, 1.0],
        depth,
        tint,
        alpha,
        ..Default::default()
    });
}
/// Repaint every occurrence of `name` inside a row at the mention colour.
/// The extra labels sit above the row's own glyphs (depth 0.00021), so they
/// recolour just those characters.
fn push_mention_runs(
    font: &crate::render::bit_font::BitFont,
    text: &str,
    y: i32,
    name: &str,
    x: f32,
    camera_offset: [f32; 2],
    alpha: f32,
    // Matches before this byte are the row's own sender tag, not a mention.
    from_byte: usize,
    texts: &mut Vec<SpriteInstance>,
) {
    let mut from = from_byte.min(text.len());
    while let Some(found) = text[from..].find(name) {
        let start = from + found;
        let end = start + name.len();
        let run_x = x + font.text_width(&text[..start]) as f32;
        let mut sprites = font.build_text_label(
            &text[start..end],
            run_x,
            y as f32,
            0.00021,
            camera_offset,
            MESSAGE_MENTION_RGB,
            0,
        );
        for sprite in &mut sprites {
            sprite.alpha *= alpha;
        }
        texts.extend(sprites);
        from = end;
    }
}
/// How a row's backing band is drawn. Native keeps gamemd's per-label box;
/// the VeraX chat box paints a fixed strip on its history rows and a
/// full-view strip on its input bar.
#[derive(Clone, Copy)]
struct RowBacking {
    /// Painted band width in pixels.
    width: f32,
    alpha: f32,
}

fn build_row_instances(
    font: &crate::render::bit_font::BitFont,
    m: &crate::ui::messages::Message,
    x: f32,
    camera_offset: [f32; 2],
    alpha: f32,
    backing: Option<RowBacking>,
    fills: &mut Vec<SpriteInstance>,
    texts: &mut Vec<SpriteInstance>,
) {
    // Message rows are screen-space, but `draw_pooled_ui` binds the
    // live world camera and the shader subtracts it. Add that camera
    // back here so scrolling cannot displace the HUD text.
    let tint = m.rgb.map(|channel| (channel * 255.0).round() as u8);
    let reveal = m.reveal_count();
    // The chat box paints the sender tag in the sender's own colour and
    // the body in white, as the Minecraft reference does. The native
    // layout keeps one label in one colour, and a type-on reveal keeps
    // the single label so its character count spans the whole row.
    let split = (m.prefix_bytes > 0 && m.prefix_bytes < m.text.len() && reveal == 0)
        .then_some(m.prefix_bytes);
    let mut text_sprites = match split {
        Some(bytes) => {
            let tag = &m.text[..bytes];
            let mut sprites =
                font.build_text_label(tag, x, m.y as f32, 0.00022, camera_offset, tint, 0);
            sprites.extend(font.build_text_label(
                &m.text[bytes..],
                x + font.text_width(tag) as f32,
                m.y as f32,
                0.00022,
                camera_offset,
                [255, 255, 255],
                0,
            ));
            sprites
        }
        None => font.build_text_label(&m.text, x, m.y as f32, 0.00022, camera_offset, tint, reveal),
    };
    for inst in &mut text_sprites {
        inst.alpha *= alpha;
    }
    texts.extend(text_sprites);
    // Native's label box hugs the text by 1 px; the chat box's strips use
    // their own widths.
    let (box_width, box_alpha) = match backing {
        Some(backing) => (backing.width, backing.alpha * alpha),
        None => (font.text_width(&m.text) as f32 + 2.0, alpha),
    };
    let height = font.cell_height() as f32;
    fills.push(SpriteInstance {
        position: [
            x - 1.0 + camera_offset[0],
            m.y as f32 - 1.0 + camera_offset[1],
        ],
        size: [box_width, height + 2.0],
        uv_origin: [0.0, 0.0],
        uv_size: [1.0, 1.0],
        // The UI pass depth-tests: the box must sit BEHIND its own glyphs
        // (text depth 0.00022), mirroring the tooltip lane's ordering
        // (fill 0.00021 > border 0.000205 > text).
        depth: 0.00023,
        tint: [0.0; 3],
        alpha: box_alpha,
        ..Default::default()
    });
}

/// Map a screen point to a position in the retained chat history. None when
/// the point is outside the history area (the input bar, the sidebar, the
/// battlefield) or the layout is not the VeraX chat box.
pub(crate) fn chat_point_at(
    state: &AppState,
    x: f32,
    y: f32,
) -> Option<crate::ui::messages::ChatPoint> {
    let list = &state.match_state.match_presentation.message_list;
    let row = list.row_at_y(y as i32)?;
    let message = list.messages().get(row)?;
    let font = &state.renderer.bit_font;
    Some(crate::ui::messages::ChatPoint {
        row,
        char_index: char_index_at(font, &message.text, x - list.x() as f32),
    })
}

/// The char index whose left edge is at or before `x` (the char boundary
/// nearest the pointer, measured the same way the row is drawn).
fn char_index_at(font: &crate::render::bit_font::BitFont, text: &str, x: f32) -> usize {
    if x <= 0.0 {
        return 0;
    }
    let mut byte = 0usize;
    let mut index = 0usize;
    for (i, ch) in text.chars().enumerate() {
        byte += ch.len_utf8();
        if font.text_width(&text[..byte]) as f32 <= x {
            index = i + 1;
        } else {
            break;
        }
    }
    index
}

/// Byte offset of `char_index` in `text` (clamped to the end).
fn char_byte_offset(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

/// Every http(s) span in `text`, as `(start, end)` char indices.
fn url_spans(text: &str) -> Vec<(usize, usize)> {
    fn begins_with(chars: &[char], prefix: &str) -> bool {
        let prefix: Vec<char> = prefix.chars().collect();
        chars.len() >= prefix.len() && chars[..prefix.len()] == prefix[..]
    }
    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if begins_with(&chars[i..], "http://") || begins_with(&chars[i..], "https://") {
            let mut end = i;
            while end < chars.len()
                && !chars[end].is_whitespace()
                && !matches!(chars[end], '<' | '>' | '"' | '\'')
            {
                end += 1;
            }
            spans.push((i, end));
            i = end;
        } else {
            i += 1;
        }
    }
    spans
}

/// The link under a chat point, if that point fell inside an http(s) span.
fn chat_url_at(state: &AppState, point: crate::ui::messages::ChatPoint) -> Option<String> {
    let list = &state.match_state.match_presentation.message_list;
    let message = list.messages().get(point.row)?;
    let chars: Vec<char> = message.text.chars().collect();
    url_spans(&message.text)
        .into_iter()
        .find(|(start, end)| point.char_index >= *start && point.char_index < *end)
        .map(|(start, end)| chars[start..end.min(chars.len())].iter().collect())
}

/// VeraX chat box left-button handling: the press starts a history selection,
/// the release ends it and opens a link when the press did not drag. True
/// when the click landed inside the history area and must not reach the
/// battlefield.
pub(crate) fn chat_mouse(
    state: &mut AppState,
    button: winit::event::MouseButton,
    pressed: bool,
) -> bool {
    if button != winit::event::MouseButton::Left {
        return false;
    }
    {
        let list = &state.match_state.match_presentation.message_list;
        if list.layout() != crate::ui::messages::MessageLayout::ChatBox || !list.editing() {
            return false;
        }
    }
    let (cx, cy) = state.cursor_position();
    if pressed {
        // An open right-click menu takes the click: run the row or dismiss it.
        if state
            .match_state
            .match_presentation
            .message_list
            .menu()
            .is_some()
        {
            let action = state
                .match_state
                .match_presentation
                .message_list
                .menu_action_at(cx as i32, cy as i32);
            let link = state
                .match_state
                .match_presentation
                .message_list
                .menu()
                .and_then(|menu| menu.link.clone());
            state
                .match_state
                .match_presentation
                .message_list
                .close_menu();
            match action {
                Some(crate::ui::messages::ChatMenuAction::Copy) => {
                    copy_chat_selection(state);
                }
                Some(crate::ui::messages::ChatMenuAction::OpenLink) => {
                    if let Some(url) = link {
                        crate::app::input::clipboard::open_url(&url);
                    }
                }
                None => {}
            }
            return true;
        }
        // The scrollbar strip owns presses before the text does.
        if state
            .match_state
            .match_presentation
            .message_list
            .scrollbar_hit(cx as i32, cy as i32)
        {
            state
                .match_state
                .match_presentation
                .message_list
                .begin_scroll_drag(cy as i32);
            state
                .match_state
                .match_presentation
                .message_list
                .drag_scroll_to(cy as i32);
            return true;
        }
        let Some(point) = chat_point_at(state, cx, cy) else {
            return false;
        };
        let now = crate::app::input::tooltips::now_ms(state);
        state
            .match_state
            .match_presentation
            .message_list
            .press_at(point, now);
        return true;
    }
    if state
        .match_state
        .match_presentation
        .message_list
        .is_dragging_scrollbar()
    {
        state
            .match_state
            .match_presentation
            .message_list
            .end_scroll_drag();
        return true;
    }
    if !state
        .match_state
        .match_presentation
        .message_list
        .is_selecting()
    {
        return false;
    }
    state
        .match_state
        .match_presentation
        .message_list
        .end_selection();
    // A press with no drag is a click: open the link under the pointer.
    let clicked_only = state
        .match_state
        .match_presentation
        .message_list
        .selection()
        .is_some_and(|(anchor, focus)| anchor == focus);
    if clicked_only {
        if let Some(point) = chat_point_at(state, cx, cy) {
            if let Some(url) = chat_url_at(state, point) {
                crate::app::input::clipboard::open_url(&url);
            }
        }
    }
    true
}

/// Extend an in-progress selection as the pointer moves (left button held).
pub(crate) fn chat_extend_selection(state: &mut AppState) {
    let (cx, cy) = state.cursor_position();
    // Track the hovered menu row while the menu is open.
    if state
        .match_state
        .match_presentation
        .message_list
        .menu()
        .is_some()
    {
        state
            .match_state
            .match_presentation
            .message_list
            .set_menu_hover(cx as i32, cy as i32);
        return;
    }
    if state
        .match_state
        .match_presentation
        .message_list
        .is_dragging_scrollbar()
    {
        state
            .match_state
            .match_presentation
            .message_list
            .drag_scroll_to(cy as i32);
        return;
    }
    if !state
        .match_state
        .match_presentation
        .message_list
        .is_selecting()
    {
        return;
    }
    if let Some(point) = chat_point_at(state, cx, cy) {
        state
            .match_state
            .match_presentation
            .message_list
            .extend_selection(point);
    }
}

/// Copy the selected chat text to the OS clipboard. True when it copied.
pub(crate) fn copy_chat_selection(state: &mut AppState) -> bool {
    let Some(text) = state
        .match_state
        .match_presentation
        .message_list
        .selected_text()
    else {
        return false;
    };
    if !crate::app::input::clipboard::set_text(&text) {
        return false;
    }
    // Confirm the copy with a short grey row. The label goes through the
    // language pack first; retail CSF has no chat-box key, so English is the
    // fallback.
    let notice = verax_ui_text(state, "TXT_VERAX_COPIED", "Copied to clipboard");
    post_grey_row(state, &notice, false, true);
    true
}

/// VeraX chat box right-button actions over the history: copy the selection,
/// else open the link under the pointer. False when the click was not over
/// the history, so the caller keeps gamemd's "right button ends the edit".
pub(crate) fn chat_right_mouse(state: &mut AppState) -> bool {
    {
        let list = &state.match_state.match_presentation.message_list;
        if list.layout() != crate::ui::messages::MessageLayout::ChatBox || !list.editing() {
            return false;
        }
    }
    let (cx, cy) = state.cursor_position();
    if chat_point_at(state, cx, cy).is_none() {
        return false;
    }
    let has_selection = state
        .match_state
        .match_presentation
        .message_list
        .selection()
        .is_some_and(|(anchor, focus)| anchor != focus);
    let link = chat_point_at(state, cx, cy).and_then(|point| chat_url_at(state, point));
    state.match_state.match_presentation.message_list.open_menu(
        cx as i32,
        cy as i32,
        has_selection,
        link,
    );
    true
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_spans_find_http_links_and_stop_at_whitespace() {
        assert_eq!(url_spans("see https://a.b/c now"), vec![(4, 17)]);
        assert_eq!(url_spans("https://x.test"), vec![(0, 14)]);
        assert_eq!(url_spans("plain text, no link"), Vec::new());
        // Two links in one row, and a quote terminates the span.
        assert_eq!(
            url_spans("http://a.io x https://b.io"),
            vec![(0, 11), (14, 26)]
        );
        assert_eq!(url_spans("watch <https://c.io>"), vec![(7, 19)]);
    }

    #[test]
    fn movie_subtitle_rows_keep_native_duration_and_literal_text() {
        let rgb = [0.2, 0.3, 0.4];
        let post = movie_subtitle_post("test subtitle", 180, rgb);
        assert_eq!(post.text, "test subtitle");
        assert_eq!(post.rgb, rgb);
        assert!(post.prefix.is_none());
        let mut list = crate::ui::messages::MessageList::new(3, 0, 6, 600);
        let result = list.add_message(&post, 1_000, &|s| s.len() as i32);
        assert!(result.play_sound, "native callback uses silent=0");
        assert_eq!(list.messages()[0].deadline_ms, Some(3_880));
        list.manage(3_879);
        assert_eq!(list.messages().len(), 1);
        list.manage(3_880);
        assert_eq!(
            list.messages().len(),
            1,
            "MessageList expires strictly after its deadline"
        );
        list.manage(3_881);
        assert!(list.messages().is_empty());
        assert!(
            movie_subtitle_post("persistent", u32::MAX, rgb)
                .timeout_ms
                .is_none()
        );
        assert_eq!(movie_subtitle_post("zero", 0, rgb).timeout_ms, Some(0));
    }

    #[cfg(feature = "ares")]
    #[test]
    fn ares_refusal_post_uses_localized_message_owner_colour_and_expiry() {
        use crate::rules::ini_parser::IniFile;
        use crate::rules::native_processing::RulesLayerStack;
        use crate::sim::superweapon::AresCameoRefusal;
        let ini = IniFile::from_str(
            "[AudioVisual]\nMessageDelay=1\n[Colors]\nGold=85,255,255\nOwner=0,255,255\nPlayer=170,255,255\n\
             [SuperWeaponTypes]\n0=Strike\n1=Configured\n2=Firer\n\
             [Strike]\nType=GenericWarhead\nMessage.CannotFire=MSG:Denied\nMessage.InsufficientFunds=MSG:Funds\n\
             [Configured]\nType=GenericWarhead\nMessage.CannotFire=MSG:Denied\nMessage.Color=Owner\n\
             [Firer]\nType=GenericWarhead\nMessage.CannotFire=MSG:Denied\nMessage.FirerColor=yes\nMessage.Color=Player\n",
        );
        let processed = RulesLayerStack::new(ini)
            .process_with_fixed_art(&IniFile::empty())
            .unwrap();
        let rules = crate::rules::ruleset::RuleSet::from_processed_rules(&processed).unwrap();
        let mut colors = crate::map::houses::HouseColorMap::new();
        colors.insert(
            "FirerHouse".into(),
            crate::rules::house_colors::HouseColorIndex(1),
        );
        colors.insert(
            "PlayerHouse".into(),
            crate::rules::house_colors::HouseColorIndex(2),
        );
        let build = |id: &str, firer| {
            ares_refusal_post(
                rules.super_weapon(id).unwrap().ares_messages(),
                AresCameoRefusal::CannotFire,
                &rules.color_schemes,
                &colors,
                firer,
                Some("PlayerHouse"),
                rules.general.message_delay_minutes,
            )
            .unwrap()
        };
        let post = build("Strike", Some("FirerHouse"));
        assert_eq!(post.rgb, [0.0, 0.0, 1.0]);
        assert_eq!(post.timeout_ms, Some(14_400));
        assert!(post.prefix.is_none() && !post.silent);
        assert_eq!(build("Configured", Some("FirerHouse")).rgb, [1.0, 0.0, 0.0]);
        assert_eq!(build("Firer", Some("FirerHouse")).rgb, [1.0, 0.0, 0.0]);
        assert_eq!(
            build("Firer", None).rgb,
            [0.0, 1.0, 0.0],
            "null firer uses Gold"
        );
        let csf = test_csf("MSG:Denied", "No eligible target");
        let mut list = crate::ui::messages::MessageList::new(3, 0, 6, 600);
        assert!(
            add_localized_message(&mut list, Some(&csf), &post, 100, &|s| s.len() as i32)
                .play_sound
        );
        assert_eq!(list.messages()[0].text, "No eligible target");
        assert_eq!(list.messages()[0].deadline_ms, Some(14_500));
        assert!(
            add_localized_message(&mut list, Some(&csf), &post, 101, &|s| s.len() as i32)
                .play_sound
        );
        assert_eq!(
            list.messages().len(),
            2,
            "retain native repeated rows; no new debounce"
        );
        let empty = test_csf("MSG:Denied", "");
        assert!(
            !add_localized_message(&mut list, Some(&empty), &post, 102, &|s| s.len() as i32)
                .play_sound
        );
        assert_eq!(list.messages().len(), 2);
        list.manage(14_502);
        assert!(list.messages().is_empty());
    }

    #[test]
    fn timed_release_admission_survives_a_new_pause_in_the_same_logic_pass() {
        use crate::app::match_runtime::session::SessionMode;
        use crate::app::match_runtime::sim_tick::{RuntimePassInputs, decide_runtime_pass};
        let mut sim = crate::sim::world::Simulation::with_seed(0);
        let decide = |paused| {
            decide_runtime_pass(RuntimePassInputs {
                exact_step: false,
                window_active: true,
                focus_frozen: false,
                startup_admitted: true,
                frame_stepping: false,
                paused: false,
                scenario_paused: paused,
                menu_open: false,
                session_mode: SessionMode::Campaign,
                pacer_timing_admits: true,
                network_wait: false,
                network_dialog_open: false,
            })
        };
        let blocked = |sim: &crate::sim::world::Simulation| {
            crate::app::match_runtime::scenario_pause::blocks_main_tick(
                SessionMode::Campaign,
                sim.session().scenario_paused(),
            )
        };
        sim.pause_scenario_for_test(1);
        assert!(!poll_is_admitted(decide(true).run_sim, blocked(&sim)));
        // Main_Tick releases the timed credit before its admission test.
        sim.expire_scenario_pause_for_test();
        let admitted = decide(sim.session().scenario_paused()).run_sim;
        assert!(admitted);
        // Action110 can acquire another credit during this admitted Logic.
        sim.pause_scenario_for_test(1);
        assert!(blocked(&sim));
        assert!(poll_is_admitted(admitted, blocked(&sim)));
        let mut list = crate::ui::messages::MessageList::new(0, 0, 6, 600);
        list.add_message(
            &crate::ui::messages::MessagePost {
                prefix: None,
                text: "objective",
                rgb: [1.0; 3],
                visible_timeout_ms: Some(1_000),
                retain_timeout_ms: Some(1_000),
                silent: true,
            },
            0,
            &|text| text.len() as i32,
        );
        if poll_is_admitted(admitted, blocked(&sim)) {
            assert!(list.manage(1_001));
        }
        assert!(list.messages().is_empty());
    }

    fn original_trigger_messages() -> serde_json::Value {
        let rows: Vec<serde_json::Value> = serde_json::from_str(crate::test_fixture::text(
            "tools/spatial_oracle/trigger_action_values.json",
        ))
        .unwrap();
        rows.into_iter()
            .find(|row| row["case_kind"] == "localized_message")
            .expect("original action11 MessageDelay/color/CSF controls")
    }

    fn original_delay_bits(row: &serde_json::Value) -> u64 {
        let hex = row["delay_f64_le_hex"].as_str().unwrap();
        assert_eq!(hex.len(), 16);
        u64::from_le_bytes(std::array::from_fn(|index| {
            u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap()
        }))
    }

    #[test]
    fn trigger_message_rules_reader_and_bucket_conversion_match_original_binary64_controls() {
        use crate::rules::ini_parser::IniFile;
        use crate::rules::ruleset::RuleSet;

        let native = original_trigger_messages();
        let rows = native["histories"].as_array().unwrap();
        assert_eq!(rows.len(), 21);
        for row in rows {
            let ini = if let Some(delay) = row["input_delay"].as_str() {
                IniFile::from_str(&format!("[AudioVisual]\nMessageDelay={delay}\n"))
            } else {
                IniFile::from_str("[AudioVisual]\n")
            };
            // GeneralRules::from_ini is private to its existing owner. Use
            // its production RuleSet entry, without exposing another reader.
            let rules = RuleSet::from_ini(&ini).unwrap();
            let delay = rules.general.message_delay_minutes;
            assert_eq!(delay.bits(), original_delay_bits(row), "{row} parsed bits");
            let message = row["calls"].as_array().unwrap().last().unwrap();
            assert_eq!(message["call"], "message_add");
            assert_eq!(
                i64::from(message_delay_buckets(delay)),
                message["duration"].as_i64().unwrap(),
                "{row} native ftol duration"
            );
            let buckets = message["duration"].as_i64().unwrap();
            assert_eq!(
                HudMessage::LowPower.timeout_ms(&rules),
                (buckets != -1).then(|| u64::try_from(buckets).unwrap_or(0) * 16),
                "House and Action11 share the native conversion"
            );
        }
        // The stock `.6` spelling and the native control's `0.6` both pass
        // through the same production ReadDouble owner. Constructor omission
        // has distinct observed bits and is covered above.
        let Some(stock) = crate::rules::retail_ini_fixture::retail_rulesmd_rules() else {
            return;
        };
        let recorded = rows.iter().find(|row| row["input_delay"] == "0.6").unwrap();
        assert_eq!(
            stock.general.message_delay_minutes.bits(),
            original_delay_bits(recorded)
        );
        assert_eq!(
            i64::from(message_delay_buckets(stock.general.message_delay_minutes)),
            recorded["calls"].as_array().unwrap().last().unwrap()["duration"]
                .as_i64()
                .unwrap()
        );
    }

    #[test]
    fn trigger_message_scenario_side_selects_original_runtime_scheme_in_stock_colors() {
        let Some(rules) = crate::rules::retail_ini_fixture::retail_rulesmd_rules() else {
            return;
        };
        let native = original_trigger_messages();
        for row in native["histories"].as_array().unwrap() {
            let color = row["calls"].as_array().unwrap().last().unwrap()["color"]
                .as_u64()
                .unwrap() as usize;
            // Original69A310's output indexes the doubled runtime scheme
            // list. The shared color owner handles HSV conversion; this
            // comparison establishes priority selection, not GPU pixels.
            let scheme = &rules.color_schemes[color / 2];
            let expected = crate::rules::color_scheme::hsv_to_rgb(scheme.hsv)
                .map(|channel| f32::from(channel) / 255.0);
            assert_eq!(
                trigger_message_rgb(
                    &rules.color_schemes,
                    row["side"].as_i64().unwrap() as i32,
                    crate::rules::skirmish_colors::SkirmishColorMode::VANILLA,
                ),
                expected,
                "{row}"
            );
        }
    }

    #[test]
    fn trigger_message_csf_labels_match_original_lookup_through_retail_archive_reader() {
        let Some((_, assets)) = crate::rules::retail_ini_fixture::retail_assets() else {
            return;
        };
        let selected = assets
            .load_file_from_mix("ra2md.csf")
            .expect("active-YR retail CSF");
        let native = original_trigger_messages();
        assert_eq!(
            crate::util::sha256::sha256_hex(&selected.bytes),
            native["csf"]["source_sha256"].as_str().unwrap(),
            "the corpus and production loader must select the same physical CSF"
        );
        let table = crate::assets::csf_file::CsfFile::from_bytes(&selected.bytes).unwrap();
        let rows = native["histories"]
            .as_array()
            .unwrap()
            .iter()
            .chain(native["label_controls"].as_array().unwrap());
        let mut compared = 0;
        for row in rows {
            let Some(message) = row["calls"].as_array().unwrap().last() else {
                continue;
            };
            assert_eq!(message["call"], "message_add");
            let label = row["label"].as_str().unwrap();
            assert_eq!(
                table.text(label).as_ref(),
                message["text"].as_str().unwrap(),
                "{label}"
            );
            compared += 1;
        }
        assert_eq!(compared, 27);
        // Archive selection, CSF parsing, case-insensitive lookup and missing
        // labels are compared. Full AppState posting, wrapping, expiration
        // and the rendered message surface remain separate validation.
    }

    fn test_csf(label: &str, value: &str) -> crate::assets::csf_file::CsfFile {
        let encoded_value: Vec<u8> = value
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .map(|byte| !byte)
            .collect();
        let mut data = Vec::new();
        data.extend_from_slice(&0x4353_4620u32.to_le_bytes());
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0x4C42_4C20u32.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&(label.len() as u32).to_le_bytes());
        data.extend_from_slice(label.as_bytes());
        data.extend_from_slice(&0x5354_5220u32.to_le_bytes());
        data.extend_from_slice(&(value.encode_utf16().count() as u32).to_le_bytes());
        data.extend_from_slice(&encoded_value);
        crate::assets::csf_file::CsfFile::from_bytes(&data).expect("item83 CSF fixture")
    }

    #[test]
    fn item83_type_select_feedback_resolves_csf_into_one_real_silent_message_row() {
        let csf = test_csf("msg:selacrossscreen", "Lokalisierte Bildschirmauswahl");
        let mut list = crate::ui::messages::MessageList::new(3, 0, 6, 1_000);
        let rgb = [0.25, 0.5, 0.75];
        let now_ms = 1_200;
        let measure = |text: &str| text.chars().count() as i32 * 8;

        let outcome = add_type_select_feedback(
            &mut list,
            Some(&csf),
            "MSG:SelAcrossScreen",
            rgb,
            now_ms,
            &measure,
        );

        assert_eq!(outcome.added, 1);
        assert!(!outcome.play_sound, "silent=1 suppresses IncomingMessage");
        assert_eq!(list.messages().len(), 1);
        let row = &list.messages()[0];
        assert_eq!(row.text, "Lokalisierte Bildschirmauswahl");
        assert_eq!(row.rgb, rgb);
        assert_eq!(row.visible_deadline_ms, Some(now_ms + 3_840));
    }

    #[test]
    fn house_low_power_native_post_reaches_retail_text_and_wall_clock_expiry() {
        let Some(retail) =
            crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")
        else {
            return;
        };
        let Some((_, assets)) = crate::rules::retail_ini_fixture::retail_assets() else {
            return;
        };
        let bytes = assets
            .load_file_from_mix("ra2md.csf")
            .expect("active-YR CSF");
        let csf = crate::assets::csf_file::CsfFile::from_bytes(&bytes.bytes).unwrap();
        let corpus: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
            "tools/spatial_oracle/house_power_consumers.json",
        ))
        .unwrap();
        let native = corpus["advice_controls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["input"]["name"] == "local_short_first")
            .unwrap();
        let presentation = native["receipt"]["presentation"].as_array().unwrap();
        let lookup = presentation
            .iter()
            .find(|call| call["kind"] == "text_lookup")
            .unwrap();
        // Original file-backed label at824714 is TXT_LOW_POWER, not the
        // synthetic UTF16 pointer the native harness supplies to MessageList.
        assert_eq!(lookup["key"], 0x824714);
        let message = presentation
            .iter()
            .find(|call| call["kind"] == "message")
            .unwrap();
        let args = message["args"].as_array().unwrap();
        assert_eq!((args[0].as_i64(), args[1].as_i64()), (Some(0), Some(0)));
        assert_eq!(args[4], 0x4046);
        let mut colors = crate::map::houses::HouseColorMap::new();
        colors.insert(
            "Americans".into(),
            crate::rules::house_colors::HouseColorIndex(0),
        );
        let rgb = house_message_rgb(
            Some("Americans"),
            &colors,
            Some(&retail.rules.color_schemes),
        );
        let post = HudMessage::LowPower.post(&retail.rules, rgb).unwrap();
        assert!(post.prefix.is_none());
        assert_eq!(post.text, "TXT_LOW_POWER");
        assert_eq!(post.silent, args[6] != 0);
        assert_eq!(
            post.visible_timeout_ms,
            Some(args[5].as_u64().unwrap() * 16)
        );
        assert_ne!(
            csf.text(post.text).as_ref(),
            post.text,
            "physical localized label exists"
        );

        let mut list = crate::ui::messages::MessageList::new(3, 0, 6, 1_000);
        let now = 1_200;
        let outcome = add_localized_message(&mut list, Some(&csf), &post, now, &|text| {
            text.chars().count() as i32 * 8
        });
        assert_eq!(outcome.added, 1);
        assert!(
            outcome.play_sound,
            "native silent0 requests IncomingMessage"
        );
        assert_eq!(list.messages()[0].text, csf.text(post.text));
        assert_eq!(list.messages()[0].rgb, rgb);
        let deadline = now + post.visible_timeout_ms.unwrap();
        assert_eq!(list.messages()[0].visible_deadline_ms, Some(deadline));
        let font = crate::render::bit_font::tests::make_test_font(
            &[(b'L' as u16, 6), (b'o' as u16, 6)],
            4,
        );
        assert_eq!(list.advance_reveal(now as u32), 1);
        assert_eq!(
            message_instances(&font, &list, [0.0; 2], now, &ChatChrome::default())
                .1
                .len(),
            1
        );
        assert_eq!(list.advance_reveal(now as u32 + 16), 1);
        assert_eq!(
            message_instances(&font, &list, [0.0; 2], now, &ChatChrome::default())
                .1
                .len(),
            2
        );
        // Offline menus skip Manage. Equality still retains the row; a
        // first resumed poll past the deadline removes it immediately.
        assert!(!list.manage(deadline));
        let resumed_wall_ms = deadline + 20_000;
        assert!(list.manage(resumed_wall_ms));
        assert!(list.messages().is_empty());

        // Admission failure cannot play the message-insertion sound.
        list.set_view(3, 0, 0, 0);
        let rejected =
            add_localized_message(&mut list, Some(&csf), &post, resumed_wall_ms + 2, &|_| 8);
        assert_eq!(rejected.added, 0);
        assert!(!rejected.play_sound);
    }

    #[test]
    fn item83_type_select_feedback_uses_local_scheme_color_zero_and_runtime_three_fallback() {
        use crate::rules::color_scheme::ColorSchemeEntry;
        use crate::rules::house_colors::HouseColorIndex;
        let schemes = [25, 43, 153].map(|hue| ColorSchemeEntry {
            name: format!("Hue{hue}"),
            hsv: [hue, 255, 255],
        });
        let native: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
            "tools/color_oracle/hsv_to_rgb.json",
        ))
        .unwrap();
        let row = native["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["saturation"] == 255 && row["value"] == 255)
            .unwrap();
        let rgb = row["rgb_by_hue"].as_str().unwrap();
        let expected = |hue: usize| {
            std::array::from_fn(|channel| {
                let at = hue * 6 + channel * 2;
                f32::from(u8::from_str_radix(&rgb[at..at + 2], 16).unwrap()) / 255.0
            })
        };
        let mut house_colors = crate::map::houses::HouseColorMap::new();
        house_colors.insert("Americans".into(), HouseColorIndex(2));
        assert_eq!(
            house_message_rgb(Some("Americans"), &house_colors, Some(&schemes)),
            expected(153)
        );
        assert_eq!(
            house_message_rgb(None, &house_colors, Some(&schemes)),
            expected(43),
            "a null House uses runtime3/Colors entry1"
        );
        house_colors.insert("Americans".into(), HouseColorIndex(255));
        assert_eq!(
            house_message_rgb(Some("Americans"), &house_colors, Some(&schemes)),
            expected(25),
            "TextLabel falls back to runtime0 for an invalid scheme"
        );
    }

    #[test]
    fn chatbox_history_strip_is_a_fixed_two_thirds_while_the_input_bar_spans_the_view() {
        use crate::render::bit_font::tests::make_test_font;

        let font = make_test_font(&[(b'x' as u16, 6)], 4);
        let mut list = crate::ui::messages::MessageList::new(3, 0, 6, 300);
        list.set_view(3, 0, 300, 100);
        list.set_layout(crate::ui::messages::MessageLayout::ChatBox);
        list.add_message(
            &crate::ui::messages::MessagePost {
                prefix: None,
                text: "xxx",
                rgb: [1.0, 1.0, 1.0],
                visible_timeout_ms: None,
                retain_timeout_ms: None,
                silent: true,
            },
            0,
            &|text| font.text_width(text) as i32,
        );
        assert!(list.add_edit(crate::ui::messages::MESSAGE_INPUT_PREFIX, [1.0; 3], false));

        let (fills, _) = message_instances(&font, &list, [0.0; 2], 0, &ChatChrome::default());
        assert_eq!(fills.len(), 3, "input bar, caret, history row");
        // The input bar paints first and keeps its full-view strip.
        assert_eq!(fills[0].size[0], list.view_width() as f32 + 2.0);
        assert!((fills[0].alpha - 0.6).abs() < 1e-6);
        // The input bar draws an underscore caret just past its text.
        assert_eq!(fills[1].size, [4.0, 2.0]);
        // It blinks off in the second half of its cycle.
        let (blink_off, _) =
            message_instances(&font, &list, [0.0; 2], 600u64, &ChatChrome::default());
        assert_eq!(blink_off.len(), 2, "the caret is hidden while off");
        // History rows paint a fixed two thirds of the view, not the whole
        // line and not a band sized to their own text.
        let history = &fills[2];
        assert!(
            (history.size[0] - list.view_width() as f32 * 2.0 / 3.0).abs() < 1e-3,
            "a history row covers two thirds of the view"
        );
        assert!(history.size[0] < fills[0].size[0]);
        assert_eq!(history.alpha, fills[0].alpha);
    }

    #[test]
    fn native_layout_leaves_the_ime_composition_to_the_system_window() {
        use crate::render::bit_font::tests::make_test_font;

        let font = make_test_font(&[(b'x' as u16, 6)], 4);
        let mut list = crate::ui::messages::MessageList::new(3, 0, 6, 300);
        list.set_view(3, 0, 300, 100);
        assert!(list.add_edit(crate::ui::messages::MESSAGE_INPUT_PREFIX, [1.0; 3], false));
        let composition = vec![b'x' as u16; 2];
        let chrome = ChatChrome {
            composition: Some(&composition),
            ..Default::default()
        };

        let (_, native) = message_instances(&font, &list, [0.0; 2], 0, &chrome);
        let (_, native_plain) =
            message_instances(&font, &list, [0.0; 2], 0, &ChatChrome::default());
        assert_eq!(
            native.len(),
            native_plain.len(),
            "the native layout paints no inline composition"
        );

        list.set_layout(crate::ui::messages::MessageLayout::ChatBox);
        let (_, chat) = message_instances(&font, &list, [0.0; 2], 0, &chrome);
        assert!(
            chat.len() > native.len(),
            "the chat box paints the composition after the typed text"
        );
    }

    #[test]
    fn chatbox_search_menu_and_mention_labels_are_painted() {
        use crate::render::bit_font::tests::make_test_font;

        let font = make_test_font(&[(b'x' as u16, 6)], 4);
        let mut list = crate::ui::messages::MessageList::new(3, 0, 6, 300);
        list.set_view(3, 0, 300, 100);
        list.set_layout(crate::ui::messages::MessageLayout::ChatBox);
        list.add_message(
            &crate::ui::messages::MessagePost {
                prefix: None,
                text: "xx",
                rgb: [1.0, 1.0, 1.0],
                visible_timeout_ms: None,
                retain_timeout_ms: None,
                silent: true,
            },
            0,
            &|text| font.text_width(text) as i32,
        );
        assert!(list.add_edit(crate::ui::messages::MESSAGE_INPUT_PREFIX, [1.0; 3], false));

        let base = message_instances(&font, &list, [0.0; 2], 0, &ChatChrome::default())
            .1
            .len();

        // A mention repaints each occurrence of the name at the highlight
        // colour, so the row's two `x` glyphs gain two more sprites.
        let mention = ChatChrome {
            mention: Some("x"),
            ..Default::default()
        };
        assert_eq!(
            message_instances(&font, &list, [0.0; 2], 0, &mention)
                .1
                .len(),
            base + 2,
            "the mention runs are painted"
        );

        // The open search paints its query text over the message line.
        list.begin_search();
        list.search_input(u16::from(b'x'), false);
        assert!(
            message_instances(&font, &list, [0.0; 2], 0, &ChatChrome::default())
                .1
                .len()
                > base,
            "the search query label is painted"
        );

        list.end_search();
        list.open_menu(10, 10, true, None);
        assert!(
            message_instances(&font, &list, [0.0; 2], 0, &ChatChrome::default())
                .1
                .len()
                > base,
            "the menu labels are painted"
        );
    }

    #[test]
    fn chatbox_paints_the_sender_tag_in_the_sender_colour_and_the_body_in_white() {
        use crate::render::bit_font::tests::make_test_font;

        let font = make_test_font(&[(b'x' as u16, 6)], 4);
        let mut list = crate::ui::messages::MessageList::new(3, 0, 6, 300);
        list.set_view(3, 0, 300, 100);
        list.set_layout(crate::ui::messages::MessageLayout::ChatBox);
        list.add_message(
            &crate::ui::messages::MessagePost {
                prefix: Some("xx"),
                text: "xx",
                rgb: [1.0, 0.0, 0.0],
                visible_timeout_ms: None,
                retain_timeout_ms: None,
                silent: true,
            },
            0,
            &|text| font.text_width(text) as i32,
        );
        assert_eq!(list.messages()[0].text, "xx xx");

        let m = &list.messages()[0];
        let mut fills = Vec::new();
        let mut texts = Vec::new();
        build_row_instances(
            &font,
            m,
            3.0,
            [0.0; 2],
            1.0,
            chatbox_history_backing(&list),
            &mut fills,
            &mut texts,
        );
        assert_eq!(texts.len(), 4, "two glyphs in each of the two labels");
        // The label path scales the tint by the font's own shade, so the
        // sender colour keeps zero green/blue and the white body keeps
        // all three channels.
        assert_eq!(
            [texts[0].tint[1], texts[0].tint[2]],
            [0.0, 0.0],
            "the tag keeps the sender colour"
        );
        assert_eq!(
            [texts[1].tint[1], texts[1].tint[2]],
            [0.0, 0.0],
            "both tag glyphs keep the sender colour"
        );
        assert!(
            texts[2].tint[0] > 0.9 && texts[2].tint[1] > 0.9 && texts[2].tint[2] > 0.9,
            "the body is white"
        );
        assert!(
            texts[2].position[0] > texts[1].position[0],
            "the body starts past the tag"
        );

        // A sender's own tag is a label, not a mention: matching starts at
        // the body, so only that occurrence is repainted.
        let mut mention = Vec::new();
        push_mention_runs(
            &font,
            &m.text,
            m.y,
            "xx",
            3.0,
            [0.0; 2],
            1.0,
            m.prefix_bytes,
            &mut mention,
        );
        assert_eq!(mention.len(), 2, "only the body occurrence counts");
    }

    #[test]
    fn chatbox_paints_the_composition_in_the_open_search_bar() {
        use crate::render::bit_font::tests::make_test_font;

        let font = make_test_font(&[(b'x' as u16, 6)], 4);
        let mut list = crate::ui::messages::MessageList::new(3, 0, 6, 300);
        list.set_view(3, 0, 300, 100);
        list.set_layout(crate::ui::messages::MessageLayout::ChatBox);
        assert!(list.add_edit(crate::ui::messages::MESSAGE_INPUT_PREFIX, [1.0; 3], false));
        list.begin_search();

        let composing = vec![b'x' as u16];
        let chrome = ChatChrome {
            composition: Some(&composing),
            ..Default::default()
        };
        let plain = message_instances(&font, &list, [0.0; 2], 0, &ChatChrome::default())
            .1
            .len();
        assert_eq!(
            message_instances(&font, &list, [0.0; 2], 0, &chrome)
                .1
                .len(),
            plain + 1,
            "the composition follows the search query instead of the message line"
        );
    }

    #[test]
    fn chat_chrome_label_falls_back_when_the_language_pack_lacks_the_key() {
        assert_eq!(
            ui_text_or("MISSING:'TXT_VERAX_COPY'", "TXT_VERAX_COPY", "Copy"),
            "Copy"
        );
        assert_eq!(
            ui_text_or("TXT_VERAX_COPY", "TXT_VERAX_COPY", "Copy"),
            "Copy"
        );
        assert_eq!(ui_text_or("复制", "TXT_VERAX_COPY", "Copy"), "复制");
    }

    #[test]
    fn gsi_02_14_message_glyphs_compensate_for_ui_camera() {
        use crate::render::bit_font::tests::make_test_font;

        let font = make_test_font(&[(b'x' as u16, 6)], 4);
        let mut list = crate::ui::messages::MessageList::new(3, 0, 6, 1_000);
        list.add_message(
            &crate::ui::messages::MessagePost {
                prefix: None,
                text: "xxx",
                rgb: [1.0, 1.0, 1.0],
                visible_timeout_ms: None,
                retain_timeout_ms: None,
                silent: true,
            },
            0,
            &|text| font.text_width(text) as i32,
        );

        let at_origin = message_instances(&font, &list, [0.0, 0.0], 0u64, &ChatChrome::default()).1;
        let ui_camera = [640.0_f32, 480.0_f32];
        let panned = message_instances(&font, &list, ui_camera, 0u64, &ChatChrome::default()).1;

        assert_eq!(at_origin.len(), 3);
        assert_eq!(panned.len(), at_origin.len());
        for (origin, shifted) in at_origin.iter().zip(&panned) {
            let after_shader_subtraction = [
                shifted.position[0] - ui_camera[0].round(),
                shifted.position[1] - ui_camera[1].round(),
            ];
            assert_eq!(after_shader_subtraction, origin.position);
        }
    }
}

#[cfg(all(test, feature = "ares"))]
mod ares_tests {
    use super::*;
    use crate::rules::ini_parser::IniFile;

    #[test]
    fn fourth_side_message_text_color_resolves_to_its_colors_entry() {
        let mut colors = String::from("[Colors]\n");
        for index in 0..11 {
            colors.push_str(&format!("C{index}={},{},{}\n", index, index, index));
        }
        colors.push_str("DarkRed=0,230,255\n");
        let ini = IniFile::from_str(&format!(
            "{colors}\
             [Sides]\nGDI=x\nNod=x\nThirdSide=x\nFourth=x\n\
             [Fourth]\nMessageTextColor=DarkRed\n"
        ));
        let processed = crate::rules::native_processing::RulesLayerStack::new(ini)
            .process()
            .expect("message color rules process");
        let rules = crate::rules::ruleset::RuleSet::from_processed_rules(&processed)
            .expect("message color projection");
        let entry =
            crate::rules::color_scheme::scheme_entry_by_name(&rules.color_schemes, "DarkRed")
                .expect("DarkRed entry");
        assert_eq!(
            ares_message_text_rgb(&rules, 3),
            Some(
                crate::rules::color_scheme::hsv_to_rgb(rules.color_schemes[entry].hsv)
                    .map(|channel| f32::from(channel) / 255.0)
            )
        );
    }
}
