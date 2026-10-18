//! Campaign-victory score page: the reachable retail dialog `0x103`.
//!
//! Campaign victory reaches `0x6C8E80` from `0x685960`; that body selects
//! dialog `0x103` and the dialog proc `0x6C8FF0`. Its `RT_DIALOG` template is
//! the campaign score page, not the skirmish results dialog `0x108`. The
//! control ids, captions, template DLU rectangles and style bits below come
//! from retail `gamemd.exe`'s `RT_DIALOG 0x103`. The tooltip keys and kind-1
//! reveal parameters were read by executing the existing
//! `shell_help_keys.help_label` and `shell_static_timers.run_getter` helpers
//! against that retail executable.
//!
//! The app supplies already-resolved display strings. This module owns no
//! score calculation, campaign progression or asset lookup.

use std::time::{Duration, Instant};

use crate::ui::shell::descriptor::DialogId;
use crate::ui::shell::geom::{RectPx, ShellScale, dlu_rect};
use crate::ui::shell::menu_page::{self, MenuPageButtonSpec, MenuPageLayout, MenuPageSpec};
use crate::ui::shell::static_reveal::{Kind1Params, Kind1RevealWindow, PresentedKind1Static};

pub const CAMPAIGN_SCORE_DIALOG: DialogId = DialogId(0x0103);

pub const CONTINUE_BUTTON: u16 = 0x0423;
pub const TITLE_STATIC: u16 = 0x0694;
pub const STATUS_HELP_STATIC: u16 = 0x0695;
pub const PAR_TIME_LABEL_STATIC: u16 = 0x06EF;
pub const YOUR_TIME_LABEL_STATIC: u16 = 0x06F0;
pub const PAR_TIME_VALUE_STATIC: u16 = 0x06F1;
pub const YOUR_TIME_VALUE_STATIC: u16 = 0x06F2;
pub const RANK_STATIC: u16 = 0x06F3;
pub const COMMENDATION_STATIC: u16 = 0x06F4;
pub const ENEMIES_DESTROYED_LABEL_STATIC: u16 = 0x069F;
pub const ENEMIES_DESTROYED_VALUE_STATIC: u16 = 0x06F6;
pub const SCORE_ART_REGION_STATIC: u16 = 0x072B;

/// Retail `gamemd.exe` SHA-256 `3e81a61775d2745d1dabe397325ef663cd994ffc194da4e998e3bf5d2d308600`:
/// `0x006033F0` returns 100 ms for dialog `0x103` / static `0x72B`.
const CAMPAIGN_SCORE_EMBLEM_INTERVAL: Duration = Duration::from_millis(100);
/// `0x006C93C9..0x006C93E1` handles its frame-zero `0x4D8` by selecting SCRA
/// frame 29 again.
const CAMPAIGN_SCORE_EMBLEM_WRAP_FRAME: usize = 29;

/// Shared shell metadata for the right-edge heading, status help and bottom
/// Continue button. The campaign dialog's Continue control has its own id and
/// tooltip; the application reacts to that control id rather than a guessed
/// dialog return value.
pub const CAMPAIGN_SCORE_PAGE: MenuPageSpec = MenuPageSpec {
    dialog: CAMPAIGN_SCORE_DIALOG,
    title_key: "GUI:StandaloneScore",
    stacked: &[],
    back: MenuPageButtonSpec {
        id: CONTINUE_BUTTON,
        dlu_top: 346,
        csf_key: "GUI:Continue",
        tooltip_key: "STT:StandScoreButtonReturn",
        result: None,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignScoreControlKind {
    Button,
    Static,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignScoreAction {
    Continue,
}

/// Executed kind-1 timing from `0x602490`, `0x600CA0`, `0x6015E0` and
/// `0x601D20` for the campaign page's controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignScoreRevealSpec {
    pub interval_ms: u64,
    pub step: u32,
    pub highlight_range: u32,
}

const TEXT_REVEAL: CampaignScoreRevealSpec = CampaignScoreRevealSpec {
    interval_ms: 30,
    step: 1,
    highlight_range: 8,
};

const LONG_TEXT_REVEAL: CampaignScoreRevealSpec = CampaignScoreRevealSpec {
    interval_ms: 15,
    step: 3,
    highlight_range: 16,
};

/// One control from retail `RT_DIALOG 0x103`. `template_dlu` preserves the
/// resource's raw 6x13 dialog units; `native_style` is its original Win32
/// style word. A `None` reveal marks controls which do not use kind-1 text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignScoreControlSpec {
    pub id: u16,
    pub kind: CampaignScoreControlKind,
    pub template_dlu: RectPx,
    pub native_style: u32,
    pub caption_key: Option<&'static str>,
    pub tooltip_key: Option<&'static str>,
    pub action: Option<CampaignScoreAction>,
    pub reveal: Option<CampaignScoreRevealSpec>,
}

/// Controls are in the order stored in the retail `RT_DIALOG 0x103` template.
/// `0x72B` is a blank static frame (`0x50000007`); the template and available
/// native evidence do not establish a campaign-art filename for that region.
pub const CAMPAIGN_SCORE_CONTROLS: [CampaignScoreControlSpec; 12] = [
    CampaignScoreControlSpec {
        id: CONTINUE_BUTTON,
        kind: CampaignScoreControlKind::Button,
        template_dlu: RectPx::new(425, 346, 108, 23),
        native_style: 0x5000_000B,
        caption_key: Some("GUI:Continue"),
        tooltip_key: Some("STT:StandScoreButtonReturn"),
        action: Some(CampaignScoreAction::Continue),
        reveal: None,
    },
    CampaignScoreControlSpec {
        id: PAR_TIME_LABEL_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(125, 266, 55, 10),
        native_style: 0x5000_0001,
        caption_key: Some("GUI:ParTime"),
        tooltip_key: None,
        action: None,
        reveal: Some(TEXT_REVEAL),
    },
    CampaignScoreControlSpec {
        id: YOUR_TIME_LABEL_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(242, 266, 55, 10),
        native_style: 0x5000_0001,
        caption_key: Some("GUI:YourTime"),
        tooltip_key: None,
        action: None,
        reveal: Some(TEXT_REVEAL),
    },
    CampaignScoreControlSpec {
        id: PAR_TIME_VALUE_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(125, 277, 55, 10),
        native_style: 0x5000_0001,
        caption_key: Some("GUI:Blank"),
        tooltip_key: Some("STT:StandScoreParTime"),
        action: None,
        reveal: Some(TEXT_REVEAL),
    },
    CampaignScoreControlSpec {
        id: YOUR_TIME_VALUE_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(242, 277, 55, 10),
        native_style: 0x5000_0001,
        caption_key: Some("GUI:Blank"),
        tooltip_key: Some("STT:StandScoreYourTime"),
        action: None,
        reveal: Some(TEXT_REVEAL),
    },
    CampaignScoreControlSpec {
        id: RANK_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(85, 35, 257, 10),
        native_style: 0x5000_0000,
        caption_key: Some("GUI:Blank"),
        tooltip_key: Some("STT:StandScoreTitle"),
        action: None,
        reveal: Some(TEXT_REVEAL),
    },
    CampaignScoreControlSpec {
        id: COMMENDATION_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(85, 50, 257, 40),
        native_style: 0x5000_0000,
        caption_key: Some("GUI:Blank"),
        tooltip_key: Some("STT:StandScoreMessage"),
        action: None,
        reveal: Some(LONG_TEXT_REVEAL),
    },
    CampaignScoreControlSpec {
        id: ENEMIES_DESTROYED_LABEL_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(85, 100, 110, 10),
        native_style: 0x5000_0000,
        caption_key: Some("GUI:HeadsTaken"),
        tooltip_key: None,
        action: None,
        reveal: Some(TEXT_REVEAL),
    },
    CampaignScoreControlSpec {
        id: ENEMIES_DESTROYED_VALUE_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(200, 100, 142, 10),
        native_style: 0x5000_0000,
        caption_key: Some("GUI:Blank"),
        tooltip_key: Some("STT:StandScoreKills"),
        action: None,
        reveal: Some(TEXT_REVEAL),
    },
    CampaignScoreControlSpec {
        id: STATUS_HELP_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(10, 357, 303, 12),
        native_style: 0x5000_0200,
        caption_key: Some("GUI:Blank"),
        tooltip_key: None,
        action: None,
        reveal: Some(LONG_TEXT_REVEAL),
    },
    CampaignScoreControlSpec {
        id: SCORE_ART_REGION_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(72, 115, 282, 141),
        native_style: 0x5000_0007,
        caption_key: None,
        tooltip_key: None,
        action: None,
        reveal: None,
    },
    CampaignScoreControlSpec {
        id: TITLE_STATIC,
        kind: CampaignScoreControlKind::Static,
        template_dlu: RectPx::new(425, 1, 108, 10),
        native_style: 0x5002_0001,
        caption_key: Some("GUI:StandaloneScore"),
        tooltip_key: None,
        action: None,
        reveal: Some(TEXT_REVEAL),
    },
];

/// Already-resolved text from the campaign victory path. The app owns all
/// value formatting and sends these strings to the page unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CampaignScoreScreenModel {
    pub par_time: String,
    pub your_time: String,
    pub enemies_destroyed: String,
    pub rank: String,
    pub commendation: String,
}

/// CSF strings resolved by the app for the fixed labels and the Continue
/// status line. The status text is installed only when the controller reports
/// the corresponding hover/help action.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CampaignScoreTexts {
    pub title: String,
    pub continue_label: String,
    pub continue_status: String,
    pub par_time_label: String,
    pub your_time_label: String,
    pub enemies_destroyed_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignScoreControlRect {
    pub id: u16,
    pub rect: RectPx,
}

/// Geometry for the campaign score dialog at one window size. Text controls
/// use their template windows; the heading, status line and Continue button
/// reuse the shared shell family's runtime anchors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignScoreLayout {
    pub scale: ShellScale,
    pub screen: RectPx,
    pub title: RectPx,
    pub status_help: RectPx,
    pub continue_button: RectPx,
    pub controls: [CampaignScoreControlRect; CAMPAIGN_SCORE_CONTROLS.len()],
}

impl CampaignScoreLayout {
    /// Retail window placement, retaining the campaign values in their
    /// authored `0x103` template windows and using the shared shell anchors
    /// for the heading, status line and Continue control.
    pub fn compute_layout(screen_w: u32, screen_h: u32) -> Self {
        let page = menu_page::compute_layout(&CAMPAIGN_SCORE_PAGE, screen_w, screen_h);
        layout_from_menu_page(page, false)
    }

    /// Uniformly scale the 800x600 campaign score page into the supplied
    /// window. This is the app's fill-window path.
    pub fn compute_responsive_layout(screen_w: u32, screen_h: u32) -> Self {
        let page = menu_page::compute_responsive_layout(&CAMPAIGN_SCORE_PAGE, screen_w, screen_h);
        layout_from_menu_page(page, true)
    }

    pub fn control_rect(&self, id: u16) -> Option<RectPx> {
        self.controls
            .iter()
            .find(|control| control.id == id)
            .map(|control| control.rect)
    }
}

fn template_window(spec: &CampaignScoreControlSpec) -> RectPx {
    let rect = spec.template_dlu;
    dlu_rect(rect.x, rect.y, rect.w, rect.h)
}

fn layout_from_menu_page(page: MenuPageLayout, scale_body: bool) -> CampaignScoreLayout {
    let continue_button = page
        .button_rect(CONTINUE_BUTTON)
        .expect("campaign score page lays out Continue");
    let controls = std::array::from_fn(|index| {
        let spec = CAMPAIGN_SCORE_CONTROLS[index];
        let mut rect = template_window(&spec);
        if scale_body {
            rect = page.scale.scale_rect(rect);
        }
        rect = match spec.id {
            TITLE_STATIC => page.title,
            STATUS_HELP_STATIC => page.status_help,
            CONTINUE_BUTTON => continue_button,
            _ => rect,
        };
        CampaignScoreControlRect { id: spec.id, rect }
    });
    CampaignScoreLayout {
        scale: page.scale,
        screen: page.screen,
        title: page.title,
        status_help: page.status_help,
        continue_button,
        controls,
    }
}

/// Module-level retail layout entry point for callers which keep page layouts
/// alongside their sibling shell functions.
pub fn compute_layout(screen_w: u32, screen_h: u32) -> CampaignScoreLayout {
    CampaignScoreLayout::compute_layout(screen_w, screen_h)
}

/// Module-level responsive layout entry point.
pub fn compute_responsive_layout(screen_w: u32, screen_h: u32) -> CampaignScoreLayout {
    CampaignScoreLayout::compute_responsive_layout(screen_w, screen_h)
}

fn reveal_params(spec: CampaignScoreRevealSpec) -> Kind1Params {
    Kind1Params {
        interval: Duration::from_millis(spec.interval_ms),
        step: spec.step,
        range: spec.highlight_range,
    }
}

/// A resolved campaign score static with native kind-1 paint/reveal state.
#[derive(Debug, Clone)]
pub struct CampaignScoreStatic {
    pub id: u16,
    reveal_spec: CampaignScoreRevealSpec,
    reveal: PresentedKind1Static,
}

impl CampaignScoreStatic {
    fn new(
        spec: &CampaignScoreControlSpec,
        text: &str,
        reveal_spec: CampaignScoreRevealSpec,
        now: Instant,
    ) -> Self {
        let mut reveal = PresentedKind1Static::new(reveal_params(reveal_spec));
        reveal.set_text(text, now);
        Self {
            id: spec.id,
            reveal_spec,
            reveal,
        }
    }

    pub fn text(&self) -> &str {
        self.reveal.text()
    }

    pub fn reveal_spec(&self) -> CampaignScoreRevealSpec {
        self.reveal_spec
    }

    /// Reveal window for this composition. The app acknowledges a presented
    /// frame through [`CampaignScorePage::commit_presented`].
    pub(crate) fn paint(&mut self, now: Instant) -> Option<Kind1RevealWindow> {
        self.reveal.paint(now)
    }
}

/// The open campaign victory score dialog `0x103`.
#[derive(Debug, Clone)]
pub struct CampaignScorePage {
    pub model: CampaignScoreScreenModel,
    pub texts: CampaignScoreTexts,
    statics: Vec<CampaignScoreStatic>,
    emblem_animation: Option<CampaignScoreEmblemAnimation>,
}

/// The campaign `0x72B` SCRA static's kind-4 timer and frame. The timer is
/// presentation-only and belongs to this page instance; dropping/reopening
/// the page drops/restarts its animation epoch with the art.
#[derive(Debug, Clone)]
struct CampaignScoreEmblemAnimation {
    frame_count: usize,
    frame: usize,
    next_tick_at: Instant,
}

impl CampaignScoreEmblemAnimation {
    fn new(frame_count: usize, now: Instant) -> Option<Self> {
        (frame_count > 0).then_some(Self {
            frame_count,
            frame: 0,
            next_tick_at: now + CAMPAIGN_SCORE_EMBLEM_INTERVAL,
        })
    }

    /// A due native `WM_TIMER` advances exactly one frame; overdue timer
    /// periods coalesce instead of jumping through unpainted frames. On the
    /// SCRA wrap to zero the parent immediately selects frame 29 again.
    fn poll(&mut self, now: Instant) -> usize {
        if now >= self.next_tick_at {
            let next = (self.frame + 1) % self.frame_count;
            self.frame = if next == 0 && self.frame_count > CAMPAIGN_SCORE_EMBLEM_WRAP_FRAME {
                CAMPAIGN_SCORE_EMBLEM_WRAP_FRAME
            } else {
                next
            };
            let periods = now.duration_since(self.next_tick_at).as_nanos()
                / CAMPAIGN_SCORE_EMBLEM_INTERVAL.as_nanos()
                + 1;
            self.next_tick_at +=
                CAMPAIGN_SCORE_EMBLEM_INTERVAL * u32::try_from(periods).unwrap_or(u32::MAX);
        }
        self.frame
    }

    fn next_tick_at(&self) -> Instant {
        self.next_tick_at
    }
}

impl CampaignScorePage {
    /// Install the app-resolved values into the native `0x103` controls.
    pub fn open(model: CampaignScoreScreenModel, texts: CampaignScoreTexts, now: Instant) -> Self {
        let mut statics = Vec::with_capacity(10);
        for spec in CAMPAIGN_SCORE_CONTROLS {
            let Some(reveal) = spec.reveal else {
                continue;
            };
            let text: &str = match spec.id {
                TITLE_STATIC => &texts.title,
                STATUS_HELP_STATIC => "",
                PAR_TIME_LABEL_STATIC => &texts.par_time_label,
                YOUR_TIME_LABEL_STATIC => &texts.your_time_label,
                PAR_TIME_VALUE_STATIC => &model.par_time,
                YOUR_TIME_VALUE_STATIC => &model.your_time,
                RANK_STATIC => &model.rank,
                COMMENDATION_STATIC => &model.commendation,
                ENEMIES_DESTROYED_LABEL_STATIC => &texts.enemies_destroyed_label,
                ENEMIES_DESTROYED_VALUE_STATIC => &model.enemies_destroyed,
                _ => continue,
            };
            statics.push(CampaignScoreStatic::new(&spec, text, reveal, now));
        }
        Self {
            model,
            texts,
            statics,
            emblem_animation: None,
        }
    }

    /// Start the campaign emblem's native 100 ms kind-4 timer when the SCRA
    /// frames have been loaded for control `0x72B`.
    pub fn start_emblem_animation(&mut self, frame_count: usize, now: Instant) {
        self.emblem_animation = CampaignScoreEmblemAnimation::new(frame_count, now);
    }

    /// Current SCRA frame to draw after applying any due native timer tick.
    /// `None` means the page has no loaded SCRA sequence.
    pub fn poll_emblem_animation(&mut self, now: Instant) -> Option<usize> {
        self.emblem_animation
            .as_mut()
            .map(|animation| animation.poll(now))
    }

    /// Read the last timer-selected SCRA frame without advancing its clock.
    pub fn emblem_animation_frame(&self) -> Option<usize> {
        self.emblem_animation
            .as_ref()
            .map(|animation| animation.frame)
    }

    /// Next native timer deadline while the SCRA kind-4 animation is active.
    pub fn next_emblem_animation_tick(&self) -> Option<Instant> {
        self.emblem_animation
            .as_ref()
            .map(CampaignScoreEmblemAnimation::next_tick_at)
    }

    pub fn static_text(&self, control_id: u16) -> Option<&str> {
        self.statics
            .iter()
            .find(|static_text| static_text.id == control_id)
            .map(CampaignScoreStatic::text)
    }

    pub fn statics(&self) -> &[CampaignScoreStatic] {
        &self.statics
    }

    pub fn statics_mut(&mut self) -> &mut [CampaignScoreStatic] {
        &mut self.statics
    }

    /// Apply the common dialog's hover/help text to static `0x695`.
    pub fn set_status_help(&mut self, text: &str, now: Instant) {
        if let Some(status) = self
            .statics
            .iter_mut()
            .find(|static_text| static_text.id == STATUS_HELP_STATIC)
        {
            status.reveal.set_text(text, now);
        }
    }

    /// The shell's SHOW completion starts each kind-1 static.
    pub fn start_reveals(&mut self, now: Instant) {
        for static_text in &mut self.statics {
            static_text.reveal.start(now);
        }
    }

    /// The latest recomposition was presented by the renderer.
    pub fn commit_presented(&mut self) {
        for static_text in &mut self.statics {
            static_text.reveal.commit_presented();
        }
    }

    pub fn reveals_terminal(&self) -> bool {
        self.statics
            .iter()
            .all(|static_text| static_text.reveal.is_terminal())
    }
}

impl CampaignScoreControlSpec {
    pub fn for_id(id: u16) -> Option<&'static Self> {
        CAMPAIGN_SCORE_CONTROLS
            .iter()
            .find(|control| control.id == id)
    }

    pub fn action_for_id(id: u16) -> Option<CampaignScoreAction> {
        Self::for_id(id).and_then(|control| control.action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::shell::geom::{SHELL_BASE_H, SHELL_BASE_W};

    fn spec(id: u16) -> &'static CampaignScoreControlSpec {
        CampaignScoreControlSpec::for_id(id).unwrap_or_else(|| panic!("0x103/{id:#x}"))
    }

    #[test]
    fn control_contract_matches_campaign_template_and_help_lookup() {
        assert_eq!(CAMPAIGN_SCORE_CONTROLS.len(), 12);
        assert_eq!(
            spec(CONTINUE_BUTTON).template_dlu,
            RectPx::new(425, 346, 108, 23)
        );
        assert_eq!(spec(CONTINUE_BUTTON).caption_key, Some("GUI:Continue"));
        assert_eq!(
            spec(CONTINUE_BUTTON).tooltip_key,
            Some("STT:StandScoreButtonReturn")
        );
        assert_eq!(
            CampaignScoreControlSpec::action_for_id(CONTINUE_BUTTON),
            Some(CampaignScoreAction::Continue)
        );
        assert_eq!(spec(TITLE_STATIC).caption_key, Some("GUI:StandaloneScore"));
        assert_eq!(spec(PAR_TIME_LABEL_STATIC).caption_key, Some("GUI:ParTime"));
        assert_eq!(
            spec(YOUR_TIME_LABEL_STATIC).caption_key,
            Some("GUI:YourTime")
        );
        assert_eq!(
            spec(ENEMIES_DESTROYED_LABEL_STATIC).caption_key,
            Some("GUI:HeadsTaken")
        );
        assert_eq!(
            spec(PAR_TIME_VALUE_STATIC).tooltip_key,
            Some("STT:StandScoreParTime")
        );
        assert_eq!(
            spec(YOUR_TIME_VALUE_STATIC).tooltip_key,
            Some("STT:StandScoreYourTime")
        );
        assert_eq!(spec(RANK_STATIC).tooltip_key, Some("STT:StandScoreTitle"));
        assert_eq!(
            spec(COMMENDATION_STATIC).tooltip_key,
            Some("STT:StandScoreMessage")
        );
        assert_eq!(
            spec(ENEMIES_DESTROYED_VALUE_STATIC).tooltip_key,
            Some("STT:StandScoreKills")
        );
        assert_eq!(spec(SCORE_ART_REGION_STATIC).native_style, 0x5000_0007);
        assert_eq!(spec(SCORE_ART_REGION_STATIC).reveal, None);
    }

    #[test]
    fn campaign_emblem_timer_advances_one_frame_and_native_wrap_selects_29() {
        let opened = Instant::now();
        let mut page = CampaignScorePage::open(
            CampaignScoreScreenModel::default(),
            CampaignScoreTexts::default(),
            opened,
        );
        assert_eq!(page.poll_emblem_animation(opened), None);
        page.start_emblem_animation(0, opened);
        assert_eq!(page.poll_emblem_animation(opened), None);
        assert_eq!(page.next_emblem_animation_tick(), None);

        page.start_emblem_animation(44, opened);
        assert_eq!(page.emblem_animation_frame(), Some(0));
        assert_eq!(
            page.next_emblem_animation_tick(),
            Some(opened + Duration::from_millis(100))
        );
        assert_eq!(
            page.poll_emblem_animation(opened + Duration::from_millis(99)),
            Some(0)
        );
        assert_eq!(
            page.poll_emblem_animation(opened + Duration::from_millis(100)),
            Some(1)
        );

        for tick in 2..=43 {
            let at = opened + Duration::from_millis(tick * 100);
            assert_eq!(page.poll_emblem_animation(at), Some(tick as usize));
        }
        assert_eq!(
            page.poll_emblem_animation(opened + Duration::from_millis(4_400)),
            Some(29)
        );
        assert_eq!(
            page.poll_emblem_animation(opened + Duration::from_millis(4_500)),
            Some(30)
        );

        // A stalled event loop coalesces to one WM_TIMER callback.
        let mut stalled = CampaignScorePage::open(
            CampaignScoreScreenModel::default(),
            CampaignScoreTexts::default(),
            opened,
        );
        stalled.start_emblem_animation(44, opened);
        assert_eq!(
            stalled.poll_emblem_animation(opened + Duration::from_millis(5_000)),
            Some(1)
        );
        assert_eq!(stalled.emblem_animation_frame(), Some(1));
        assert_eq!(
            stalled.next_emblem_animation_tick(),
            Some(opened + Duration::from_millis(5_100))
        );
    }

    #[test]
    fn campaign_emblem_animation_restarts_with_a_new_page_epoch() {
        let opened = Instant::now();
        let mut page = CampaignScorePage::open(
            CampaignScoreScreenModel::default(),
            CampaignScoreTexts::default(),
            opened,
        );
        page.start_emblem_animation(44, opened);
        assert_eq!(
            page.poll_emblem_animation(opened + Duration::from_millis(800)),
            Some(1)
        );

        let reopened = opened + Duration::from_secs(20);
        let mut reopened_page = CampaignScorePage::open(
            CampaignScoreScreenModel::default(),
            CampaignScoreTexts::default(),
            reopened,
        );
        reopened_page.start_emblem_animation(44, reopened);
        assert_eq!(reopened_page.emblem_animation_frame(), Some(0));
        assert_eq!(
            reopened_page.poll_emblem_animation(reopened + Duration::from_millis(99)),
            Some(0)
        );
    }

    #[test]
    fn campaign_emblem_timer_remains_scheduled_after_text_reveals_finish() {
        let opened = Instant::now();
        let mut page = CampaignScorePage::open(
            CampaignScoreScreenModel::default(),
            CampaignScoreTexts::default(),
            opened,
        );
        page.start_emblem_animation(44, opened);
        page.start_reveals(opened);

        for elapsed_ms in (0..10_000).step_by(15) {
            let now = opened + Duration::from_millis(elapsed_ms);
            page.poll_emblem_animation(now);
            for static_text in page.statics_mut() {
                static_text.paint(now);
            }
            page.commit_presented();
            if page.reveals_terminal() {
                assert!(
                    page.next_emblem_animation_tick()
                        .is_some_and(|due| due > now)
                );
                return;
            }
        }
        panic!("campaign score text reveals did not reach their terminal state");
    }

    #[test]
    fn layout_uses_template_body_and_shared_shell_anchors() {
        let layout = compute_layout(800, 600);
        let shared = menu_page::compute_layout(&CAMPAIGN_SCORE_PAGE, 800, 600);
        assert_eq!(layout.title, shared.title);
        assert_eq!(layout.status_help, shared.status_help);
        assert_eq!(
            layout.continue_button,
            shared.button_rect(CONTINUE_BUTTON).unwrap()
        );
        assert_eq!(
            layout.control_rect(PAR_TIME_LABEL_STATIC),
            Some(RectPx::new(188, 432, 83, 16))
        );
        assert_eq!(
            layout.control_rect(PAR_TIME_VALUE_STATIC),
            Some(RectPx::new(188, 450, 83, 16))
        );
        assert_eq!(
            layout.control_rect(RANK_STATIC),
            Some(RectPx::new(128, 57, 386, 16))
        );
        assert_eq!(
            layout.control_rect(COMMENDATION_STATIC),
            Some(RectPx::new(128, 81, 386, 65))
        );
        assert_eq!(
            layout.control_rect(SCORE_ART_REGION_STATIC),
            Some(RectPx::new(108, 187, 423, 229))
        );
        assert_eq!(layout.control_rect(0xFFFF), None);
    }

    #[test]
    fn responsive_layout_scales_body_and_keeps_shared_controls_coherent() {
        let base = compute_layout(SHELL_BASE_W as u32, SHELL_BASE_H as u32);
        let same = compute_responsive_layout(SHELL_BASE_W as u32, SHELL_BASE_H as u32);
        assert_eq!(same, base);

        let large = compute_responsive_layout(1600, 1200);
        assert_eq!(
            large.control_rect(PAR_TIME_LABEL_STATIC),
            Some(RectPx::new(376, 864, 166, 32))
        );
        let shared = menu_page::compute_responsive_layout(&CAMPAIGN_SCORE_PAGE, 1600, 1200);
        assert_eq!(large.title, shared.title);
        assert_eq!(large.status_help, shared.status_help);
        assert_eq!(
            large.continue_button,
            shared.button_rect(CONTINUE_BUTTON).unwrap()
        );
    }

    #[test]
    fn page_binds_only_resolved_values_and_uses_native_reveal_groups() {
        let model = CampaignScoreScreenModel {
            par_time: "00:10:00".into(),
            your_time: "00:08:31".into(),
            enemies_destroyed: "17".into(),
            rank: "Major".into(),
            commendation: "Mission commendation".into(),
        };
        let texts = CampaignScoreTexts {
            title: "Mission Score".into(),
            continue_label: "Continue".into(),
            continue_status: "Continue to the next scenario.".into(),
            par_time_label: "Par Time".into(),
            your_time_label: "Your Time".into(),
            enemies_destroyed_label: "Enemies Destroyed".into(),
        };
        let now = Instant::now();
        let mut page = CampaignScorePage::open(model, texts, now);

        assert_eq!(page.static_text(TITLE_STATIC), Some("Mission Score"));
        assert_eq!(page.static_text(PAR_TIME_VALUE_STATIC), Some("00:10:00"));
        assert_eq!(page.static_text(YOUR_TIME_VALUE_STATIC), Some("00:08:31"));
        assert_eq!(page.static_text(RANK_STATIC), Some("Major"));
        assert_eq!(
            page.static_text(COMMENDATION_STATIC),
            Some("Mission commendation")
        );
        assert_eq!(page.static_text(ENEMIES_DESTROYED_VALUE_STATIC), Some("17"));
        assert_eq!(page.static_text(STATUS_HELP_STATIC), Some(""));
        assert_eq!(page.statics().len(), 10);
        assert_eq!(
            spec(COMMENDATION_STATIC).reveal,
            Some(CampaignScoreRevealSpec {
                interval_ms: 15,
                step: 3,
                highlight_range: 16,
            })
        );
        assert_eq!(spec(RANK_STATIC).reveal, Some(TEXT_REVEAL));

        page.set_status_help(
            "Continue to the next scenario.",
            now + Duration::from_millis(1),
        );
        assert_eq!(
            page.static_text(STATUS_HELP_STATIC),
            Some("Continue to the next scenario.")
        );
        page.start_reveals(now);
        assert!(
            page.statics_mut()
                .iter_mut()
                .all(|static_text| static_text.paint(now).is_some())
        );
        assert!(!page.reveals_terminal());
        page.commit_presented();
    }
}
