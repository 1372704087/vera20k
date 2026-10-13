//! Modern, bilingual debug panels — egui overlay for the optional dev-ui.
//!
//! Surfaces (all dark-themed, all zh-CN/en via [`i18n`]):
//! - **Terrain Debug** (top-left): PathGrid, cursor cell, terrain costs per
//!   SpeedType, entities at cell, selected-unit path and miner state.
//!   Shown while the PathGrid overlay is active (`Ctrl+Shift+P` / `F9`).
//! - **Event History** (right of it): per-unit sim event timeline.
//!   Shown while the unit inspector is active (`Ctrl+Shift+X`).
//! - **Hotkeys** (top-right): grouped hotkey reference (`Ctrl+Shift+F1`).
//!
//! Visual language: rounded dark panels, accent-tinted section headers,
//! stat cards, cost heat bars, state chips and a painted event timeline.
//! The SpeedType chips in Terrain Debug are clickable — they write the
//! per-overlay SpeedType override directly.
//!
//! ## Dependency rules
//! - Part of the app layer — may depend on everything.

use crate::app::AppState;
use crate::app::diagnostics::i18n::{self, Lang};
use crate::map::entities::EntityCategory;
use crate::rules::locomotor_type::SpeedType;
use crate::sim::debug_event_log::{DebugEvent, DebugEventKind};

// ---------------------------------------------------------------------------
// Palette — dark, modern. Kept in sync with the in-map overlay tints
// (see `debug_overlays.rs`) so panel bars and map washes read as one system.
// ---------------------------------------------------------------------------

/// Panel background (translucent so the tactical view faintly shows through).
const PANEL_FILL: egui::Color32 = egui::Color32::from_rgba_unmultiplied_const(16, 23, 36, 238);
/// Panel border.
const PANEL_STROKE: egui::Color32 = egui::Color32::from_rgb(51, 65, 85);
/// Card / inset background.
const CARD_FILL: egui::Color32 = egui::Color32::from_rgba_unmultiplied_const(30, 41, 59, 140);
/// Track behind bars and sparklines.
const TRACK_FILL: egui::Color32 = egui::Color32::from_rgba_unmultiplied_const(148, 163, 184, 36);
/// Fixed frame-time sparkline width in the developer overlay (window default
/// width 324 minus the panel frame margins). A window-relative width must
/// not be used: the panels are auto-sized, so `Ui::available_width()`
/// resolves to the whole screen during the layout pass, and any width
/// derived from it stretches the window to full screen width.
const SPARKLINE_W: f32 = 296.0;
/// Fixed terrain-cost track width inside the terrain panel (default width
/// 300). Same auto-size caveat as `SPARKLINE_W`.
const HEAT_BAR_W: f32 = 140.0;

/// Primary text.
const TEXT: egui::Color32 = egui::Color32::from_rgb(230, 237, 247);
/// Secondary text.
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(148, 163, 184);
/// Tertiary text (hints, ticks).
const TEXT_FAINT: egui::Color32 = egui::Color32::from_rgb(100, 116, 139);

/// Primary accent (sky).
const ACCENT: egui::Color32 = egui::Color32::from_rgb(56, 189, 248);
/// Secondary accent (indigo).
const ACCENT2: egui::Color32 = egui::Color32::from_rgb(129, 140, 248);
/// Positive / walkable / normal.
const GOOD: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);
/// Caution (rough terrain, warnings).
const WARN: egui::Color32 = egui::Color32::from_rgb(251, 191, 36);
/// Negative / blocked.
const BAD: egui::Color32 = egui::Color32::from_rgb(248, 113, 113);
/// Path / special movement.
const MAGENTA: egui::Color32 = egui::Color32::from_rgb(232, 121, 249);
/// Cyan — fast terrain, path steps.
const CYAN: egui::Color32 = egui::Color32::from_rgb(34, 211, 238);

/// Map a terrain cost value to a heat-bar color (mirrors the map overlay).
fn cost_band_color(cost: u8) -> egui::Color32 {
    match cost {
        0 => BAD,
        1..=65 => egui::Color32::from_rgb(251, 146, 60), // very rough — orange
        66..=80 => egui::Color32::from_rgb(245, 158, 11), // rough — amber
        81..=95 => WARN,                                  // slightly slow — yellow
        96..=105 => GOOD,                                 // normal — green
        _ => CYAN,                                        // fast / road — cyan
    }
}

// ---------------------------------------------------------------------------
// Frame & theme
// ---------------------------------------------------------------------------

/// The shared frame for every debug panel window.
pub(crate) fn panel_frame() -> egui::Frame {
    egui::Frame {
        fill: PANEL_FILL,
        stroke: egui::Stroke::new(1.0_f32, PANEL_STROKE),
        inner_margin: egui::Margin::same(10),
        outer_margin: egui::Margin::same(2),
        corner_radius: egui::CornerRadius::same(10),
        ..Default::default()
    }
}

/// Push the dark diagnostic visuals; returns the previous visuals so the
/// caller can restore them after the debug panels are drawn.
pub(crate) fn push_debug_visuals(ctx: &egui::Context) -> egui::Visuals {
    let prev = ctx.style().visuals.clone();
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = PANEL_FILL;
    visuals.panel_fill = PANEL_FILL;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, PANEL_STROKE);
    // Widget chrome tuned for the dark panels.
    visuals.widgets.noninteractive.bg_fill = CARD_FILL;
    visuals.widgets.noninteractive.weak_bg_fill = CARD_FILL;
    visuals.widgets.inactive.bg_fill =
        egui::Color32::from_rgba_unmultiplied_const(51, 65, 85, 200);
    visuals.widgets.hovered.bg_fill =
        egui::Color32::from_rgba_unmultiplied_const(71, 85, 105, 220);
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.open.bg_fill = egui::Color32::from_rgba_unmultiplied_const(51, 65, 85, 220);
    ctx.set_visuals(visuals);
    prev
}

/// Restore visuals after the debug panels are done.
pub(crate) fn pop_debug_visuals(ctx: &egui::Context, prev: egui::Visuals) {
    ctx.set_visuals(prev);
}

// ---------------------------------------------------------------------------
// Shared widgets
// ---------------------------------------------------------------------------

/// Monospace label — used for all numerics.
fn mono_label(ui: &mut egui::Ui, text: &str, color: egui::Color32) -> egui::Response {
    ui.label(
        egui::RichText::new(text)
            .color(color)
            .font(egui::FontId::monospace(11.0)),
    )
}

/// Section header: accent bar + bold title + optional right-aligned mono tag.
pub(crate) fn section_header(ui: &mut egui::Ui, title: &str, tag: Option<&str>) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let (bar, _) = ui.allocate_exact_size(egui::vec2(3.0, 13.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(bar, egui::CornerRadius::same(2), ACCENT);
        ui.add_space(6.0);
        ui.label(egui::RichText::new(title).strong().color(TEXT));
        if let Some(tag) = tag {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(tag)
                        .color(TEXT_FAINT)
                        .font(egui::FontId::monospace(10.0)),
                );
            });
        }
    });
    ui.add_space(3.0);
}

/// A small stat card: dim key on top, colored mono value below.
pub(crate) fn stat_card(ui: &mut egui::Ui, key: &str, value: &str, value_color: egui::Color32) {
    egui::Frame::new()
        .fill(CARD_FILL)
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgba_unmultiplied_const(148, 163, 184, 26),
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_min_width(118.0);
            ui.label(
                egui::RichText::new(key)
                    .color(TEXT_FAINT)
                    .font(egui::FontId::monospace(9.5)),
            );
            mono_label(ui, value, value_color);
        });
}

/// Terrain-cost heat bar: name, colored track proportional to cost, value.
fn heat_bar(ui: &mut egui::Ui, name: &str, cost: u8, active: bool) {
    let color = cost_band_color(cost);
    let color = if active { color } else { color.gamma_multiply(0.45) };
    let name_color = if active { ACCENT } else { TEXT_DIM };
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(name)
                .color(name_color)
                .font(egui::FontId::monospace(11.0)),
        );
        let bar_w = HEAT_BAR_W;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 8.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, egui::CornerRadius::same(4), TRACK_FILL);
        if cost == 0 {
            // Blocked: wash the whole bar in the blocked color.
            painter.rect_filled(
                rect,
                egui::CornerRadius::same(4),
                BAD.gamma_multiply(if active { 0.55 } else { 0.25 }),
            );
        } else {
            // 100 = normal speed; scale so cost 100 fills ~70% of the track.
            let frac = (cost as f32 / 140.0).clamp(0.06, 1.0);
            let fill =
                egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * frac, rect.height()));
            painter.rect_filled(fill, egui::CornerRadius::same(4), color);
        }
        if cost == 0 {
            mono_label(ui, "—", BAD);
        } else {
            mono_label(ui, &format!("{cost}"), if active { TEXT } else { TEXT_DIM });
        }
    });
}

/// Emphasis for [`add_panel_button`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ButtonTone {
    /// Primary action — accent-tinted.
    Accent,
    /// Ordinary action — card fill, quiet border.
    Neutral,
    /// Session-ending / destructive action — red-tinted.
    Danger,
    /// Unavailable action — dimmed; pair with `ui.add_enabled(false, …)`.
    Muted,
}

impl ButtonTone {
    /// (inactive fill, hovered fill, active fill, stroke, label).
    fn colors(self) -> (egui::Color32, egui::Color32, egui::Color32, egui::Color32, egui::Color32) {
        match self {
            // Hover/active steps keep the accent hue and only raise opacity,
            // so the button never leaves the panel's palette.
            Self::Accent => (
                ACCENT.gamma_multiply(0.16),
                ACCENT.gamma_multiply(0.28),
                ACCENT.gamma_multiply(0.42),
                ACCENT.gamma_multiply(0.55),
                ACCENT,
            ),
            Self::Neutral => (
                CARD_FILL,
                CARD_FILL.gamma_multiply(1.35),
                CARD_FILL.gamma_multiply(1.65),
                PANEL_STROKE,
                TEXT,
            ),
            Self::Danger => (
                BAD.gamma_multiply(0.14),
                BAD.gamma_multiply(0.24),
                BAD.gamma_multiply(0.36),
                BAD.gamma_multiply(0.50),
                BAD,
            ),
            // No hover/active step: a muted button is disabled, so egui's
            // state machine reports inactive regardless of the pointer.
            Self::Muted => (
                CARD_FILL.gamma_multiply(0.45),
                CARD_FILL.gamma_multiply(0.45),
                CARD_FILL.gamma_multiply(0.45),
                PANEL_STROKE.gamma_multiply(0.6),
                TEXT_FAINT,
            ),
        }
    }
}

/// An action button in the diagnostic design language: tone-tinted fill,
/// matching 1px stroke, rounded corners, tone-colored label. Implements
/// [`egui::Widget`] so it also composes with `Ui::add_enabled`.
///
/// The tints are installed on `widgets` visuals for the button's lifetime
/// rather than via `Button::fill` on purpose: a custom `.fill()` short-
/// circuits egui's `fill.unwrap_or(visuals.weak_bg_fill)` and would freeze
/// the look, killing the hover/active affordance. The visuals are restored
/// right after the `add`, so sibling widgets never observe them.
pub(crate) struct PanelButton {
    text: String,
    tone: ButtonTone,
}

impl PanelButton {
    pub(crate) fn new(text: impl Into<String>, tone: ButtonTone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }
}

impl egui::Widget for PanelButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (base, hover, active, stroke_color, label) = self.tone.colors();
        let stroke = egui::Stroke::new(1.0_f32, stroke_color);
        let visuals = ui.visuals_mut();
        let saved = (
            visuals.widgets.inactive.weak_bg_fill,
            visuals.widgets.hovered.weak_bg_fill,
            visuals.widgets.active.weak_bg_fill,
            visuals.widgets.inactive.bg_stroke,
            visuals.widgets.hovered.bg_stroke,
            visuals.widgets.active.bg_stroke,
        );
        visuals.widgets.inactive.weak_bg_fill = base;
        visuals.widgets.hovered.weak_bg_fill = hover;
        visuals.widgets.active.weak_bg_fill = active;
        visuals.widgets.inactive.bg_stroke = stroke;
        visuals.widgets.hovered.bg_stroke = stroke;
        visuals.widgets.active.bg_stroke = stroke;
        let resp = ui.add(egui::Button::new(egui::RichText::new(self.text).color(label)));
        let visuals = ui.visuals_mut();
        visuals.widgets.inactive.weak_bg_fill = saved.0;
        visuals.widgets.hovered.weak_bg_fill = saved.1;
        visuals.widgets.active.weak_bg_fill = saved.2;
        visuals.widgets.inactive.bg_stroke = saved.3;
        visuals.widgets.hovered.bg_stroke = saved.4;
        visuals.widgets.active.bg_stroke = saved.5;
        resp
    }
}

/// A pill-shaped state chip with tinted border and fill.
///
/// The whole pill (not just the label) is clickable: the frame response
/// covers the full widget rect, so it is upgraded to `Sense::click` via
/// [`egui::Response::interact`]. Purely visual chips simply ignore the
/// returned response.
pub(crate) fn chip(ui: &mut egui::Ui, text: &str, color: egui::Color32) -> egui::Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.14))
        .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.45)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .color(color)
                    .font(egui::FontId::monospace(10.5)),
            )
        })
        .inner
        .interact(egui::Sense::click())
}

/// A key cap for the hotkey reference.
fn key_chip(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(ACCENT.gamma_multiply(0.10))
        .stroke(egui::Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.32)))
        .corner_radius(egui::CornerRadius::same(5))
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            // Fixed width: `set_min_width` alone lets the justified layout
            // below expand the pill to the full row width inside a horizontal
            // parent. Pin min and max so the chip stays a compact 78pt pill.
            ui.set_width(78.0);
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    ui.label(
                        egui::RichText::new(text)
                            .color(ACCENT)
                            .font(egui::FontId::monospace(10.5)),
                    );
                },
            );
        });
}

/// One event in the timeline: colored dot, tick, text.
fn timeline_item(ui: &mut egui::Ui, dot: egui::Color32, tick: u32, text: &str) {
    ui.horizontal(|ui| {
        let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(dot_rect.center(), 3.2, dot);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!("[{tick}]"))
                .color(TEXT_FAINT)
                .font(egui::FontId::monospace(9.5)),
        );
        ui.add_space(4.0);
        ui.label(egui::RichText::new(text).color(TEXT));
    });
}

/// Frame-time sparkline (newest sample on the right). `samples` are in ms.
pub(crate) fn frame_sparkline(ui: &mut egui::Ui, samples: &[f32]) {
    let height = 30.0;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(SPARKLINE_W, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(5), TRACK_FILL);
    if samples.len() < 2 {
        return;
    }
    let max_ms = samples.iter().cloned().fold(16.0_f32, f32::max).max(16.0);
    let n = samples.len();
    let mut prev: Option<egui::Pos2> = None;
    for (i, &ms) in samples.iter().enumerate() {
        let x = rect.left() + rect.width() * (i as f32 / (n - 1) as f32);
        let y = rect.bottom() - 2.0 - (ms / max_ms) * (height - 6.0);
        let p = egui::pos2(x, y);
        if let Some(q) = prev {
            painter.line_segment([q, p], egui::Stroke::new(1.4_f32, ACCENT));
        }
        prev = Some(p);
    }
    if let Some(last) = prev {
        painter.circle_filled(last, 2.6, ACCENT2);
    }
}

/// Bilingual display name for a SpeedType (identifiers stay English).
fn speed_type_name(lang: Lang, st: SpeedType) -> &'static str {
    i18n::t(
        lang,
        match st {
            SpeedType::Foot => "st.foot",
            SpeedType::Track => "st.track",
            SpeedType::Wheel => "st.wheel",
            SpeedType::Hover => "st.hover",
            SpeedType::Winged => "st.winged",
            SpeedType::Float => "st.float",
            SpeedType::Amphibious => "st.amphibious",
            SpeedType::FloatBeach => "st.floatBeach",
            SpeedType::Invalid => "st.invalid",
        },
    )
}

/// Short category label for entity lists.
fn category_label(lang: Lang, cat: EntityCategory) -> &'static str {
    i18n::t(
        lang,
        match cat {
            EntityCategory::Unit => "cat.unit",
            EntityCategory::Infantry => "cat.infantry",
            EntityCategory::Structure => "cat.structure",
            EntityCategory::Aircraft => "cat.aircraft",
        },
    )
}

/// Format one debug event, bilingual. Returns (dot color, text).
fn format_event(lang: Lang, event: &DebugEvent) -> (egui::Color32, String) {
    let tick = event.tick;
    match &event.kind {
        DebugEventKind::PhaseChange { from, to, reason } => (
            GOOD,
            i18n::t_f(lang, "ev.phase", &[&tick, from, to, reason]),
        ),
        DebugEventKind::Repath {
            reason,
            new_path_len,
        } => (
            WARN,
            i18n::t_f(lang, "ev.repath", &[&tick, reason, new_path_len]),
        ),
        DebugEventKind::Blocked { by_entity, cell } => {
            let by = by_entity
                .map(|id| i18n::t_f(lang, "ev.blockedBy", &[&id]))
                .unwrap_or_default();
            (
                BAD,
                i18n::t_f(lang, "ev.blocked", &[&tick, &cell.0, &cell.1, &by]),
            )
        }
        DebugEventKind::StuckAbort { blocked_ticks } => (
            BAD,
            i18n::t_f(lang, "ev.stuck", &[&tick, blocked_ticks]),
        ),
        DebugEventKind::PathSegmentComplete { final_goal } => {
            let suffix = final_goal
                .map(|(x, y)| i18n::t_f(lang, "ev.segmentRepath", &[&x, &y]))
                .unwrap_or_default();
            (ACCENT, i18n::t_f(lang, "ev.segment", &[&tick, &suffix]))
        }
        DebugEventKind::MinerStateChange { from, to } => (
            egui::Color32::from_rgb(251, 146, 60),
            i18n::t_f(lang, "ev.miner", &[&tick, from, to]),
        ),
        DebugEventKind::SpecialMovementStart { kind } => {
            (MAGENTA, i18n::t_f(lang, "ev.specialStart", &[&tick, kind]))
        }
        DebugEventKind::SpecialMovementPhase { phase } => {
            (MAGENTA, i18n::t_f(lang, "ev.specialPhase", &[&tick, phase]))
        }
        DebugEventKind::SpecialMovementEnd => {
            (MAGENTA, i18n::t_f(lang, "ev.specialEnd", &[&tick]))
        }
        DebugEventKind::LocomotorOverride { kind, active } => {
            let key = if *active {
                "ev.overrideOn"
            } else {
                "ev.overrideOff"
            };
            (WARN, i18n::t_f(lang, key, &[&tick, kind]))
        }
    }
}

// ---------------------------------------------------------------------------
// Panels
// ---------------------------------------------------------------------------

/// Draw the hotkey reference overlay (top-right corner).
pub(crate) fn draw_hotkey_help(ctx: &egui::Context, lang: Lang) {
    egui::Window::new(i18n::t(lang, "panel.hotkeys"))
        .default_pos([ctx.content_rect().max.x - 300.0, 4.0])
        .default_width(280.0)
        .frame(panel_frame())
        .collapsible(true)
        .resizable(false)
        // No `vscroll`: a ScrollArea pins the window to its first-frame
        // sizing-pass height (~400pt) and never grows with content. Panels
        // auto-size to their contents instead, matching upstream behavior.
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(i18n::t(lang, "hk.devChord"))
                    .small()
                    .color(TEXT_FAINT),
            );
            ui.add_space(4.0);

            // ---- game controls ----
            section_header(ui, i18n::t(lang, "hk.game"), None);
            let game_keys: &[(&str, &str)] = &[
                ("S", "hk.stop"),
                ("D", "hk.deploy"),
                ("G", "hk.guard"),
                ("T", "hk.selectType"),
                ("0-9", "hk.ctrlGroup"),
                ("Ctrl+0-9", "hk.assignGroup"),
                ("H", "hk.base"),
                ("Space", "hk.radar"),
                ("Esc", "hk.esc"),
            ];
            for (key, desc_key) in game_keys {
                ui.horizontal(|ui| {
                    key_chip(ui, key);
                    ui.label(egui::RichText::new(i18n::t(lang, desc_key)).color(TEXT_DIM));
                });
            }

            ui.add_space(4.0);
            section_header(ui, i18n::t(lang, "hk.debug"), None);
            let debug_keys: &[(&str, &str)] = &[
                ("F1", "hk.f1"),
                ("^+M", "hk.quicksave"),
                ("^+N", "hk.quickload"),
                ("^+F5", "hk.saveLoad"),
                ("^+A", "hk.attackMove"),
                ("^+B", "hk.move"),
                ("^+P/F9", "hk.pathgrid"),
                ("^+[ ]", "hk.cycleSt"),
                ("^+K", "hk.heightmap"),
                ("^+L", "hk.cellgrid"),
                ("^+V/F10", "hk.fog"),
                ("^+X", "hk.inspector"),
                ("^+O", "hk.footPath"),
                ("^+J", "hk.debugPause"),
                ("^+.", "hk.frameStep"),
                ("^+I", "hk.lang"),
            ];
            for (key, desc_key) in debug_keys {
                ui.horizontal(|ui| {
                    key_chip(ui, key);
                    ui.label(egui::RichText::new(i18n::t(lang, desc_key)).color(TEXT_DIM));
                });
            }

            ui.add_space(4.0);
            section_header(ui, i18n::t(lang, "hk.mouse"), None);
            let mouse_keys: &[(&str, &str)] = &[
                ("LMB", "hk.lmb"),
                ("RMB", "hk.rmb"),
                ("Wheel", "hk.wheel"),
            ];
            for (key, desc_key) in mouse_keys {
                ui.horizontal(|ui| {
                    key_chip(ui, key);
                    ui.label(egui::RichText::new(i18n::t(lang, desc_key)).color(TEXT_DIM));
                });
            }
        });
}

/// Draw the Terrain Debug panel as an egui window (top-left corner).
///
/// Takes `&mut AppState` because the SpeedType chips write the per-overlay
/// override directly. Only call when `state.diag.debug_show_pathgrid` is true.
pub(crate) fn draw_debug_panel(ctx: &egui::Context, state: &mut AppState) {
    let lang = state.diag.lang;

    // Convert cursor screen position to world coordinates, then to iso cell.
    let (cursor_rx, cursor_ry) = crate::app::match_runtime::sim_tick::screen_point_to_world_cell(
        state,
        state.match_state.input.cursor_x,
        state.match_state.input.cursor_y,
    );
    // NOTE: the path grid is deliberately re-derived INSIDE the window
    // closure — holding the `&state` borrow across the closure would conflict
    // with the SpeedType chips writing `state.diag` (E0500).
    let active_st = crate::app::diagnostics::debug_overlays::resolve_debug_speed_type(state);
    let has_sim = state.match_state.sim_runtime.is_some();

    egui::Window::new(i18n::t(lang, "panel.terrain"))
        .default_pos([4.0, 4.0])
        .default_width(300.0)
        .frame(panel_frame())
        .collapsible(true)
        .resizable(false)
        // No `vscroll`: a ScrollArea pins the window to its first-frame
        // sizing-pass height (~400pt) and never grows with content. Panels
        // auto-size to their contents instead, matching upstream behavior.
        .show(ctx, |ui| {
            let path_grid = state
                .match_state
                .sim_runtime
                .as_ref()
                .map(|rt| &rt.simulation)
                .and_then(crate::sim::world::Simulation::path_grid);
            // ---- status cards ----
            let grid_text = match path_grid {
                Some(grid) => format!("{}x{}", grid.width(), grid.height()),
                None => i18n::t(lang, "terrain.none").to_owned(),
            };
            egui::Grid::new("terrain_status_cards")
                .num_columns(2)
                .spacing([6.0, 6.0])
                .show(ui, |ui| {
                    stat_card(ui, i18n::t(lang, "terrain.grid"), &grid_text, TEXT);
                    stat_card(
                        ui,
                        i18n::t(lang, "terrain.cursor"),
                        &format!("({cursor_rx}, {cursor_ry})"),
                        TEXT,
                    );
                    let z: u8 = state
                        .height_map()
                        .get(&(cursor_rx, cursor_ry))
                        .copied()
                        .unwrap_or(0);
                    stat_card(ui, i18n::t(lang, "terrain.elevation"), &format!("{z}"), ACCENT);
                    let (walk_text, walk_color) = match path_grid {
                        Some(grid) if grid.is_walkable(cursor_rx, cursor_ry) => {
                            (i18n::t(lang, "terrain.walkable"), GOOD)
                        }
                        Some(_) => (i18n::t(lang, "terrain.blocked"), BAD),
                        None => (i18n::t(lang, "terrain.none"), TEXT_FAINT),
                    };
                    stat_card(ui, i18n::t(lang, "terrain.pathgrid"), walk_text, walk_color);
                    ui.end_row();
                });

            // VXL pipeline readout.
            if let Some(atlas) = &state.match_state.match_presentation.unit_atlas {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(i18n::t(lang, "terrain.vxl")).color(TEXT_DIM));
                    mono_label(ui, &format!("{}", atlas.rendered), GOOD);
                });
            }

            // ---- terrain costs ----
            section_header(
                ui,
                i18n::t(lang, "terrain.costs"),
                Some(i18n::t(lang, "ov.speedTypeHint")),
            );
            ui.add_space(2.0);

            // SpeedType chips: AUTO + the five locomotor classes. Clicking a
            // chip pins the overlay; AUTO clears the override.
            ui.horizontal_wrapped(|ui| {
                let auto_active = state.diag.debug_terrain_cost_speed_type.is_none();
                let auto = chip(
                    ui,
                    i18n::t(lang, "ov.auto"),
                    if auto_active { ACCENT } else { TEXT_DIM },
                );
                if auto.clicked() && !auto_active {
                    state.diag.debug_terrain_cost_speed_type = None;
                }
                for st in [
                    SpeedType::Foot,
                    SpeedType::Track,
                    SpeedType::Wheel,
                    SpeedType::Float,
                    SpeedType::Amphibious,
                ] {
                    let active = state.diag.debug_terrain_cost_speed_type == Some(st);
                    let resp =
                        chip(ui, speed_type_name(lang, st), if active { ACCENT } else { TEXT_DIM });
                    if resp.clicked() && !active {
                        state.diag.debug_terrain_cost_speed_type = Some(st);
                    }
                }
            });
            ui.add_space(4.0);

            if has_sim {
                let sim = state
                    .match_state
                    .sim_runtime
                    .as_ref()
                    .map(|rt| &rt.simulation)
                    .expect("sim present");
                for st in [
                    SpeedType::Foot,
                    SpeedType::Track,
                    SpeedType::Wheel,
                    SpeedType::Float,
                    SpeedType::Amphibious,
                ] {
                    if let Some(cg) = sim.terrain_costs.get(&st) {
                        let cost = cg.cost_at(cursor_rx, cursor_ry);
                        heat_bar(ui, speed_type_name(lang, st), cost, st == active_st);
                    }
                }
            }

            // ---- entities at cell ----
            section_header(ui, i18n::t(lang, "terrain.entities"), None);
            if let Some(sim) = state
                .match_state
                .sim_runtime
                .as_ref()
                .map(|rt| &rt.simulation)
            {
                let mut found: Vec<String> = Vec::new();
                for entity in sim.entities().values() {
                    if entity.position.rx == cursor_rx && entity.position.ry == cursor_ry {
                        let max_hp = state
                            .rules()
                            .and_then(|rules| {
                                rules.object(sim.interner.resolve(entity.type_ref()))
                            })
                            .map_or_else(|| "?".to_owned(), |obj| obj.strength.to_string());
                        found.push(format!(
                            "{} ({}) [{}] hp={}/{}",
                            sim.interner.resolve(entity.type_ref()),
                            category_label(lang, entity.category),
                            sim.interner.resolve(entity.owner()),
                            entity.health.current,
                            max_hp,
                        ));
                    }
                }
                if found.is_empty() {
                    ui.label(
                        egui::RichText::new(i18n::t(lang, "terrain.none"))
                            .color(TEXT_FAINT)
                            .italics(),
                    );
                } else {
                    for desc in &found {
                        ui.label(
                            egui::RichText::new(format!("  {desc}"))
                                .color(TEXT)
                                .font(egui::FontId::monospace(10.5)),
                        );
                    }
                }

                // Building footprint under the cursor.
                if let Some(rules) = state.rules() {
                    for entity in sim.entities().values() {
                        if entity.category != EntityCategory::Structure {
                            continue;
                        }
                        let foundation = rules
                            .object(sim.interner.resolve(entity.type_ref()))
                            .map(|obj| obj.foundation.as_str())
                            .unwrap_or("1x1");
                        let (fw, fh) = crate::rules::foundation::foundation_dimensions(foundation);
                        let ex: u16 = entity.position.rx;
                        let ey: u16 = entity.position.ry;
                        if cursor_rx >= ex && cursor_rx < ex + fw && cursor_ry >= ey && cursor_ry < ey + fh
                        {
                            ui.add_space(2.0);
                            chip(
                                ui,
                                &format!(
                                    "{} {} ({}) @ ({},{}) {}",
                                    i18n::t(lang, "terrain.footprint"),
                                    sim.interner.resolve(entity.type_ref()),
                                    foundation,
                                    ex,
                                    ey,
                                    sim.interner.resolve(entity.owner())
                                ),
                                WARN,
                            );
                        }
                    }
                }
            }

            // ---- selected unit ----
            section_header(ui, i18n::t(lang, "terrain.selected"), None);
            let Some(sim) = state
                .match_state
                .sim_runtime
                .as_ref()
                .map(|rt| &rt.simulation)
            else {
                ui.label(
                    egui::RichText::new(i18n::t(lang, "terrain.selNone"))
                        .color(TEXT_FAINT)
                        .italics(),
                );
                return;
            };
            let selected: Vec<u64> = sim
                .entities()
                .values()
                .filter(|e| e.selected)
                .map(|e| e.stable_id())
                .collect();
            if selected.is_empty() {
                ui.label(
                    egui::RichText::new(i18n::t(lang, "terrain.selNone"))
                        .color(TEXT_FAINT)
                        .italics(),
                );
                return;
            }
            for &sid in &selected {
                let Some(entity) = sim.entities().get(sid) else {
                    continue;
                };
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(sim.interner.resolve(entity.type_ref()))
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "@ ({},{}) sub=({},{})",
                            entity.position.rx,
                            entity.position.ry,
                            entity.position.sub_x,
                            entity.position.sub_y
                        ))
                        .color(TEXT_DIM)
                        .font(egui::FontId::monospace(10.0)),
                    );
                });

                if entity.movement_target.is_some() {
                    let path: Vec<(u16, u16)> = entity
                        .navigation
                        .path_replay
                        .remaining_cells()
                        .into_iter()
                        .map(|(x, y)| (x as u16, y as u16))
                        .collect();
                    ui.horizontal_wrapped(|ui| {
                        chip(
                            ui,
                            &format!("{} {}", i18n::t(lang, "terrain.stepsLeft"), path.len()),
                            ACCENT2,
                        );
                        if entity.navigation.path_runtime.path_blocked {
                            chip(ui, i18n::t(lang, "terrain.blockedBy"), BAD);
                        }
                        if let Some(goal) = crate::sim::movement::movement_goal_cell(entity) {
                            chip(
                                ui,
                                &format!(
                                    "{} ({},{})",
                                    i18n::t(lang, "terrain.goal"),
                                    goal.0,
                                    goal.1
                                ),
                                WARN,
                            );
                        }
                    });
                    // First few steps with walkability coloring.
                    let grid = path_grid;
                    ui.horizontal_wrapped(|ui| {
                        for (i, &(px, py)) in path.iter().take(8).enumerate() {
                            let w = grid.is_none_or(|g| g.is_walkable(px, py));
                            let marker = if i == 0 { ">" } else { " " };
                            let color = if w { GOOD } else { BAD };
                            chip(
                                ui,
                                &format!("{marker}[{i}] ({px},{py}){}", if w { "" } else { " ✕" }),
                                color,
                            );
                        }
                    });
                } else {
                    chip(ui, i18n::t(lang, "terrain.pathIdle"), TEXT_FAINT);
                }

                // ---- miner detail ----
                let Some(ref miner) = entity.miner else {
                    continue;
                };
                ui.add_space(4.0);
                section_header(
                    ui,
                    &format!(
                        "{} · {}",
                        i18n::t(lang, "terrain.miner"),
                        sim.interner.resolve(entity.type_ref())
                    ),
                    None,
                );
                let miner_state = entity
                    .miner_state()
                    .unwrap_or(crate::sim::miner::MinerState::SearchOre);
                let state_color = match miner_state {
                    crate::sim::miner::MinerState::Harvest => GOOD,
                    crate::sim::miner::MinerState::SearchOre => ACCENT,
                    crate::sim::miner::MinerState::ReturnToRefinery => WARN,
                    crate::sim::miner::MinerState::Dock => egui::Color32::from_rgb(251, 146, 60),
                    crate::sim::miner::MinerState::WaitNoOre => BAD,
                    _ => TEXT_DIM,
                };
                ui.horizontal_wrapped(|ui| {
                    chip(ui, &format!("{miner_state:?}"), state_color);
                    chip(
                        ui,
                        &format!(
                            "{} {}/{}",
                            i18n::t(lang, "terrain.cargo"),
                            miner.cargo.len(),
                            miner.capacity_bales
                        ),
                        ACCENT2,
                    );
                    chip(
                        ui,
                        &format!(
                            "{} {} ({} {})",
                            i18n::t(lang, "terrain.stage"),
                            entity.native_stage().value(),
                            i18n::t(lang, "ov.keyHint"),
                            entity.native_stage().rate()
                        ),
                        TEXT_DIM,
                    );
                });
                if let Some(ref_id) = miner.reserved_refinery {
                    let ref_type = sim
                        .entities()
                        .get(ref_id)
                        .map(|e| sim.interner.resolve(e.type_ref()))
                        .unwrap_or("?");
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {} (id={})",
                            i18n::t(lang, "terrain.refinery"),
                            ref_type,
                            ref_id
                        ))
                        .color(TEXT_DIM)
                        .font(egui::FontId::monospace(10.0)),
                    );
                }
                if let Some(crate::sim::combat::TargetKind::Cell(x, y)) = entity.archive_target() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: ({x},{y})",
                            i18n::t(lang, "terrain.archivedOre")
                        ))
                        .color(TEXT_DIM)
                        .font(egui::FontId::monospace(10.0)),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    chip(
                        ui,
                        &format!(
                            "{} {}",
                            i18n::t(lang, "terrain.movement"),
                            if entity.movement_target.is_some() {
                                i18n::t(lang, "terrain.active")
                            } else {
                                i18n::t(lang, "terrain.idle")
                            }
                        ),
                        if entity.movement_target.is_some() {
                            GOOD
                        } else {
                            TEXT_FAINT
                        },
                    );
                    chip(
                        ui,
                        &format!(
                            "Teleport {}",
                            if entity.teleport_state().is_some() {
                                i18n::t(lang, "terrain.active")
                            } else {
                                i18n::t(lang, "terrain.idle")
                            }
                        ),
                        if entity.teleport_state().is_some() {
                            MAGENTA
                        } else {
                            TEXT_FAINT
                        },
                    );
                });
            }
        });
}

/// Draw the unit event history inspector as a separate window (right of the
/// Terrain Debug panel). Only shown when `debug_unit_inspector` is true.
pub(crate) fn draw_event_history_panel(ctx: &egui::Context, state: &AppState) {
    if !state.diag.debug_unit_inspector {
        return;
    }
    let lang = state.diag.lang;
    let Some(sim) = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)
    else {
        return;
    };

    egui::Window::new(i18n::t(lang, "ent.title"))
        .default_pos([312.0, 4.0])
        .default_width(400.0)
        .frame(panel_frame())
        .collapsible(true)
        .resizable(true)
        // No `vscroll`: a ScrollArea pins the window to its first-frame
        // sizing-pass height (~400pt) and never grows with content. Panels
        // auto-size to their contents instead, matching upstream behavior.
        .show(ctx, |ui| {
            let selected: Vec<_> = sim.entities().values().filter(|e| e.selected).collect();
            if selected.is_empty() {
                ui.label(
                    egui::RichText::new(i18n::t(lang, "ent.hint"))
                        .italics()
                        .color(TEXT_FAINT),
                );
                return;
            }

            for entity in &selected {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} (id={})",
                            sim.interner.resolve(entity.type_ref()),
                            entity.stable_id()
                        ))
                        .strong()
                        .color(TEXT),
                    );
                    ui.label(
                        egui::RichText::new(i18n::t(lang, "ent.recent"))
                            .small()
                            .color(TEXT_FAINT),
                    );
                });
                ui.separator();

                let Some(ref log) = entity.debug_log else {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} — {}",
                            sim.interner.resolve(entity.type_ref()),
                            i18n::t(lang, "ent.notActive")
                        ))
                        .color(TEXT_FAINT),
                    );
                    continue;
                };

                if log.events.is_empty() {
                    ui.label(
                        egui::RichText::new(i18n::t(lang, "ent.noEvents"))
                            .color(TEXT_FAINT),
                    );
                    continue;
                }

                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        let events: Vec<_> = log.events.iter().rev().take(30).collect();
                        for event in events {
                            let (color, text) = format_event(lang, event);
                            timeline_item(ui, color, event.tick, &text);
                        }
                    });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_bands_are_distinct_and_ordered() {
        // Blocked and normal must differ; rougher costs must trend hotter.
        assert_ne!(cost_band_color(0), cost_band_color(100));
        assert_ne!(cost_band_color(30), cost_band_color(100));
        assert_eq!(cost_band_color(0), BAD);
        assert_eq!(cost_band_color(100), GOOD);
        assert_eq!(cost_band_color(140), CYAN);
    }

    #[test]
    fn panel_frame_is_dark_and_rounded() {
        let frame = panel_frame();
        assert_eq!(frame.fill, PANEL_FILL);
        assert_eq!(frame.corner_radius, egui::CornerRadius::same(10));
    }

    #[test]
    fn speed_type_names_resolve_both_languages() {
        assert_eq!(speed_type_name(Lang::En, SpeedType::Foot), "Foot");
        assert_eq!(speed_type_name(Lang::Zh, SpeedType::Foot), "步兵 Foot");
        assert_eq!(speed_type_name(Lang::Zh, SpeedType::Wheel), "轮式 Wheel");
    }
}
