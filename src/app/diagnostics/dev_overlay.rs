//! Developer overlay panel — runtime knobs and diagnostic readouts.
//!
//! Shown while debug pause is active (`Ctrl+Shift+J`). Pure egui rendering:
//! data-in / action-out. The app facade snapshots state into DevOverlayInfo,
//! draws, and dispatches the returned DevOverlayAction.
//!
//! Visual language matches `debug_panel.rs`: dark rounded panels, accent
//! section headers, stat cards, custom toggle switches and a painted
//! frame-time sparkline. All copy is zh-CN/en via [`i18n`], switchable from
//! the segmented control in the panel header or `Ctrl+Shift+I`.
//!
//! ## Dependency rules
//! - Part of the app layer — takes pure data in, returns actions out.
//! - No direct AppState dependency in this module (mirrors ui/pause_menu.rs).

use std::path::PathBuf;

use crate::app::diagnostics::debug_panel::{
    chip, frame_sparkline, panel_frame, section_header, stat_card, ButtonTone, PanelButton,
};
use crate::app::diagnostics::i18n::{self, Lang};

/// Speed slider hard bounds. Lower bound prevents throttle-math stalls;
/// upper bound is a sane dev maximum (already faster than gamemd allows).
const SPEED_MIN_TPS: u32 = 1;
const SPEED_MAX_TPS: u32 = 200;

/// Content width of the developer overlay window (default width 324 minus
/// the panel frame margins). `Ui::available_width()` must NOT be used for
/// row widths here: the window is auto-sized, so during the layout pass it
/// resolves to the whole screen, and any min width derived from it pins the
/// window to full screen width.
const PANEL_CONTENT_W: f32 = 296.0;
/// Outer width of the developer overlay window; also the centering anchor.
const PANEL_W: f32 = 324.0;
/// Nominal half-height of the developer overlay, used only to center the
/// auto-sized window vertically (see `draw_dev_overlay`).
const PANEL_NOMINAL_HALF_H: f32 = 215.0;

/// Accent colors shared with the panel module (kept local so this module
/// stays pure data-in/action-out without reaching into panel privates).
const ACCENT: egui::Color32 = egui::Color32::from_rgb(56, 189, 248);
const ACCENT2: egui::Color32 = egui::Color32::from_rgb(129, 140, 248);
const TEXT: egui::Color32 = egui::Color32::from_rgb(230, 237, 247);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(148, 163, 184);
const TEXT_FAINT: egui::Color32 = egui::Color32::from_rgb(100, 116, 139);
const GOOD: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);
const WARN: egui::Color32 = egui::Color32::from_rgb(251, 191, 36);
const TRACK_FILL: egui::Color32 = egui::Color32::from_rgba_unmultiplied_const(148, 163, 184, 36);

/// One row in the inline recent-saves list. Caller builds these from
/// `persistence.save_list_cache.entries()`. Owned strings so the panel doesn't borrow
/// the cache across the draw call.
pub(crate) struct RecentSaveRow {
    pub path: PathBuf,
    pub display_name: String,
    pub tick: u64,
    pub age_str: String,
}

/// Snapshot of app state passed into `draw_dev_overlay`.
///
/// The text-field buffer is borrowed mutably so egui can edit it in place.
pub(crate) struct DevOverlayInfo<'a> {
    pub sim_speed_tps: u32,
    pub paused: bool,
    pub music_volume: f64,
    pub sfx_volume: f64,
    pub show_pathgrid: bool,
    pub show_cell_grid: bool,
    pub show_heightmap: bool,
    pub show_unit_inspector: bool,
    pub show_foot_path: bool,
    pub reveal_map: bool,
    pub fps: f32,
    pub frame_ms: f32,
    /// Frame-time samples (ms, oldest first) for the sparkline.
    pub frame_ms_history: Vec<f32>,
    pub tick_budget_ms: f32,
    pub entity_count: usize,
    /// Current sim tick — the session heartbeat.
    pub tick: u64,
    /// Active UI language for the bilingual copy.
    pub lang: Lang,
    pub save_name_buf: &'a mut String,
    pub last_save_tick: Option<u64>,
    pub last_save_age: Option<String>,
    pub last_load_available: bool,
    pub last_load_display: Option<String>,
    pub recent_saves: Vec<RecentSaveRow>,
}

/// Actions produced by the dev overlay each frame.
#[derive(Debug, Clone)]
pub(crate) enum DevOverlayAction {
    None,
    SetGameSpeed(u32),
    SetMusicVolume(f64),
    SetSfxVolume(f64),
    TogglePause,
    /// Temporary quit-to-menu (the native in-game Options dialog has no quit
    /// button yet; see App::return_to_main_menu).
    ReturnToMenu,
    StepOneTick,
    TogglePathGrid,
    ToggleCellGrid,
    ToggleHeightmap,
    ToggleUnitInspector,
    ToggleFootPath,
    ToggleRevealMap,
    ResetGameSpeed,
    /// Flip the diagnostic UI language (zh-CN <-> en).
    ToggleLanguage,
    SaveAs,
    ReloadLastLoad,
    LoadSave(PathBuf),
}

/// A custom toggle switch — painted track + knob, click to flip.
fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let (rect, mut resp) =
        ui.allocate_exact_size(egui::vec2(30.0, 17.0), egui::Sense::click());
    if resp.clicked() {
        *on = !*on;
        // The value was mutated after the widget was registered; without this
        // the caller's `.changed()` would never observe the flip.
        resp.mark_changed();
    }
    let painter = ui.painter();
    let (fill, knob) = if *on {
        (ACCENT.gamma_multiply(0.35), ACCENT)
    } else {
        (TRACK_FILL, TEXT_DIM)
    };
    painter.rect_filled(rect, egui::CornerRadius::same(9), fill);
    let knob_x = if *on {
        rect.right() - 10.0
    } else {
        rect.left() + 9.0
    };
    painter.circle_filled(egui::pos2(knob_x, rect.center().y), 5.0, knob);
    resp
}

/// A labeled slider row with a monospace readout.
fn slider_row_f32(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    display: impl Fn(f32) -> String,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.set_min_width(PANEL_CONTENT_W);
        ui.label(egui::RichText::new(label).color(TEXT_DIM));
        let resp = ui.add(egui::Slider::new(value, range).show_value(false));
        ui.label(
            egui::RichText::new(display(*value))
                .color(TEXT)
                .font(egui::FontId::monospace(10.5)),
        );
        resp
    })
    .inner
}

/// A labeled slider row with an integer readout.
fn slider_row_u32(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.set_min_width(PANEL_CONTENT_W);
        ui.label(egui::RichText::new(label).color(TEXT_DIM));
        let resp = ui.add(egui::Slider::new(value, range).show_value(false).integer());
        ui.label(
            egui::RichText::new(format!("{value}"))
                .color(TEXT)
                .font(egui::FontId::monospace(10.5)),
        );
        resp
    })
    .inner
}

/// A toggle row: label + optional key hint + switch, right-aligned.
fn toggle_row(
    ui: &mut egui::Ui,
    label: &str,
    key_hint: &str,
    value: &mut bool,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.set_min_width(PANEL_CONTENT_W);
        ui.label(egui::RichText::new(label).color(TEXT_DIM));
        if !key_hint.is_empty() {
            ui.label(
                egui::RichText::new(key_hint)
                    .color(TEXT_FAINT)
                    .font(egui::FontId::monospace(9.5)),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            toggle_switch(ui, value)
        })
        .inner
    })
    .inner
}

/// Draw the dev overlay. Returns the chosen action, if any.
pub(crate) fn draw_dev_overlay(
    ctx: &egui::Context,
    info: &mut DevOverlayInfo<'_>,
) -> DevOverlayAction {
    let mut action = DevOverlayAction::None;
    let lang = info.lang;

    egui::Window::new(i18n::t(lang, "panel.sim"))
        // Centered on the content rect. The panel auto-sizes, so the
        // vertical anchor uses a nominal half-height (the observed range is
        // ~430..500pt); the window clamps inside the screen on its own once
        // taller than the estimate. `PANEL_CONTENT_W / 2` is the horizontal
        // half of the default width below.
        .default_pos([
            (ctx.content_rect().center().x - PANEL_W / 2.0).max(4.0),
            (ctx.content_rect().center().y - PANEL_NOMINAL_HALF_H).max(4.0),
        ])
        .default_width(PANEL_W)
        .frame(panel_frame())
        .collapsible(true)
        .resizable(true)
        // No `vscroll`: a ScrollArea pins the window to its first-frame
        // sizing-pass height (~400pt) and never grows with content. Panels
        // auto-size to their contents instead, matching upstream behavior.
        .show(ctx, |ui| {
            // ---- header: language segmented control ----
            ui.horizontal(|ui| {
                ui.set_min_width(PANEL_CONTENT_W);
                ui.label(
                    egui::RichText::new(i18n::t(lang, "top.lang"))
                        .color(TEXT_FAINT)
                        .font(egui::FontId::monospace(9.5)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let en_active = lang == Lang::En;
                    let en = ui.add_enabled(
                        !en_active,
                        PanelButton::new(
                            "EN",
                            if en_active {
                                ButtonTone::Accent
                            } else {
                                ButtonTone::Muted
                            },
                        ),
                    );
                    if en.clicked() {
                        action = DevOverlayAction::ToggleLanguage;
                    }
                    let zh_active = lang == Lang::Zh;
                    let zh = ui.add_enabled(
                        !zh_active,
                        PanelButton::new(
                            "中文",
                            if zh_active {
                                ButtonTone::Accent
                            } else {
                                ButtonTone::Muted
                            },
                        ),
                    );
                    if zh.clicked() {
                        action = DevOverlayAction::ToggleLanguage;
                    }
                });
            });

            // ---- sim ----
            section_header(ui, i18n::t(lang, "panel.sim"), None);
            ui.horizontal(|ui| {
                ui.set_min_width(PANEL_CONTENT_W);
                let mut tps = info.sim_speed_tps;
                let resp = slider_row_u32(ui, i18n::t(lang, "sim.speed"), &mut tps, SPEED_MIN_TPS..=SPEED_MAX_TPS);
                if resp.changed() && tps != info.sim_speed_tps {
                    action = DevOverlayAction::SetGameSpeed(tps);
                }
                if ui
                    .add(PanelButton::new(
                        i18n::t(lang, "sim.reset"),
                        ButtonTone::Neutral,
                    ))
                    .clicked()
                {
                    action = DevOverlayAction::ResetGameSpeed;
                }
            });
            ui.horizontal_wrapped(|ui| {
                let pause_label = if info.paused {
                    i18n::t(lang, "sim.resume")
                } else {
                    i18n::t(lang, "sim.pause")
                };
                if ui
                    .add(PanelButton::new(pause_label, ButtonTone::Accent))
                    .clicked()
                {
                    action = DevOverlayAction::TogglePause;
                }
                let step = ui.add_enabled(
                    info.paused,
                    PanelButton::new(
                        i18n::t(lang, "sim.step"),
                        if info.paused {
                            ButtonTone::Neutral
                        } else {
                            ButtonTone::Muted
                        },
                    ),
                );
                if step.clicked() {
                    action = DevOverlayAction::StepOneTick;
                }
                if info.paused {
                    chip(ui, i18n::t(lang, "status.paused"), WARN);
                } else {
                    chip(ui, i18n::t(lang, "status.live"), GOOD);
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .add(PanelButton::new(
                        i18n::t(lang, "sim.toMenu"),
                        ButtonTone::Danger,
                    ))
                    .clicked()
                {
                    action = DevOverlayAction::ReturnToMenu;
                }
            });
            egui::Grid::new("dev_sim_stats")
                .num_columns(2)
                .spacing([6.0, 6.0])
                .show(ui, |ui| {
                    stat_card(
                        ui,
                        i18n::t(lang, "sim.tickBudget"),
                        &format!("{:.2} ms", info.tick_budget_ms),
                        TEXT,
                    );
                    stat_card(
                        ui,
                        i18n::t(lang, "sim.tick"),
                        &format!("{}", info.tick),
                        ACCENT2,
                    );
                    stat_card(
                        ui,
                        i18n::t(lang, "sim.entities"),
                        &format!("{}", info.entity_count),
                        ACCENT,
                    );
                    stat_card(
                        ui,
                        i18n::t(lang, "sim.speed"),
                        &format!("{} tps", info.sim_speed_tps),
                        TEXT,
                    );
                    ui.end_row();
                });

            // ---- render ----
            section_header(ui, i18n::t(lang, "sim.fps"), None);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{:.1}", info.fps))
                        .color(ACCENT)
                        .font(egui::FontId::monospace(20.0)),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{} {:.2} ms",
                        i18n::t(lang, "sim.frame"),
                        info.frame_ms
                    ))
                    .color(TEXT_DIM)
                    .font(egui::FontId::monospace(10.5)),
                );
            });
            frame_sparkline(ui, &info.frame_ms_history);

            // ---- audio ----
            section_header(ui, i18n::t(lang, "panel.audio"), None);
            ui.horizontal(|ui| {
                ui.set_min_width(PANEL_CONTENT_W);
                let mut music = info.music_volume as f32;
                let resp = slider_row_f32(ui, "Music", &mut music, 0.0..=1.0, |v| {
                    format!("{}%", (v * 100.0) as u32)
                });
                if resp.changed() {
                    action = DevOverlayAction::SetMusicVolume(music as f64);
                }
            });
            ui.horizontal(|ui| {
                ui.set_min_width(PANEL_CONTENT_W);
                let mut sfx = info.sfx_volume as f32;
                let resp = slider_row_f32(ui, "SFX", &mut sfx, 0.0..=1.0, |v| {
                    format!("{}%", (v * 100.0) as u32)
                });
                if resp.changed() {
                    action = DevOverlayAction::SetSfxVolume(sfx as f64);
                }
            });

            // ---- debug overlays ----
            section_header(ui, i18n::t(lang, "ov.title"), None);
            let mut b = info.show_pathgrid;
            if toggle_row(ui, i18n::t(lang, "ov.pathgrid"), "F9/P", &mut b).changed() {
                action = DevOverlayAction::TogglePathGrid;
            }
            let mut b = info.show_cell_grid;
            if toggle_row(ui, i18n::t(lang, "ov.cellgrid"), "L", &mut b).changed() {
                action = DevOverlayAction::ToggleCellGrid;
            }
            let mut b = info.show_heightmap;
            if toggle_row(ui, i18n::t(lang, "ov.heightmap"), "K", &mut b).changed() {
                action = DevOverlayAction::ToggleHeightmap;
            }
            let mut b = info.show_unit_inspector;
            if toggle_row(ui, i18n::t(lang, "ov.inspector"), "X", &mut b).changed() {
                action = DevOverlayAction::ToggleUnitInspector;
            }
            let mut b = info.show_foot_path;
            if toggle_row(ui, i18n::t(lang, "ov.footPath"), "O", &mut b).changed() {
                action = DevOverlayAction::ToggleFootPath;
            }
            let mut b = info.reveal_map;
            if toggle_row(ui, i18n::t(lang, "ov.reveal"), "F10/V", &mut b).changed() {
                action = DevOverlayAction::ToggleRevealMap;
            }

            // ---- save / load ----
            section_header(ui, i18n::t(lang, "sim.saveLoad"), None);
            ui.horizontal(|ui| {
                ui.set_min_width(PANEL_CONTENT_W);
                ui.label(egui::RichText::new(i18n::t(lang, "sim.saveName")).color(TEXT_DIM));
                ui.add(
                    egui::TextEdit::singleline(info.save_name_buf)
                        .desired_width(150.0)
                        .hint_text(i18n::t(lang, "sim.saveName")),
                );
                let can_save = !info.save_name_buf.trim().is_empty();
                if ui
                    .add_enabled(
                        can_save,
                        PanelButton::new(
                            i18n::t(lang, "sim.saveAs"),
                            if can_save {
                                ButtonTone::Accent
                            } else {
                                ButtonTone::Muted
                            },
                        ),
                    )
                    .clicked()
                {
                    action = DevOverlayAction::SaveAs;
                }
            });

            // Recent saves (top 5).
            ui.add_space(4.0);
            ui.label(egui::RichText::new(i18n::t(lang, "sim.recent")).italics().color(TEXT_FAINT));
            if info.recent_saves.is_empty() {
                ui.label(
                    egui::RichText::new(i18n::t(lang, "sim.noSaves"))
                        .italics()
                        .color(TEXT_FAINT),
                );
            } else {
                for row in &info.recent_saves {
                    ui.horizontal(|ui| {
                        if ui
                            .add(PanelButton::new(
                                i18n::t(lang, "sim.load"),
                                ButtonTone::Neutral,
                            ))
                            .clicked()
                        {
                            action = DevOverlayAction::LoadSave(row.path.clone());
                        }
                        ui.label(
                            egui::RichText::new(format!(
                                "{}  tick {}  {}",
                                row.display_name, row.tick, row.age_str
                            ))
                            .color(TEXT_DIM)
                            .font(egui::FontId::monospace(10.0)),
                        );
                    });
                }
            }

            ui.add_space(4.0);
            let reload_label = match &info.last_load_display {
                Some(name) => format!("{}: {name}", i18n::t(lang, "sim.reload")),
                None => i18n::t(lang, "sim.reload").to_owned(),
            };
            if ui
                .add_enabled(
                    info.last_load_available,
                    PanelButton::new(
                        reload_label,
                        if info.last_load_available {
                            ButtonTone::Neutral
                        } else {
                            ButtonTone::Muted
                        },
                    ),
                )
                .clicked()
            {
                action = DevOverlayAction::ReloadLastLoad;
            }

            // Last-save readout.
            match (info.last_save_tick, &info.last_save_age) {
                (Some(tick), Some(age)) => {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: tick {tick} ({age})",
                            i18n::t(lang, "sim.lastSave")
                        ))
                        .color(TEXT_DIM)
                        .font(egui::FontId::monospace(10.0)),
                    );
                }
                (Some(tick), None) => {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: tick {tick}",
                            i18n::t(lang, "sim.lastSave")
                        ))
                        .color(TEXT_DIM)
                        .font(egui::FontId::monospace(10.0)),
                    );
                }
                _ => {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {}",
                            i18n::t(lang, "sim.lastSave"),
                            i18n::t(lang, "sim.lastSaveNone")
                        ))
                        .italics()
                        .color(TEXT_FAINT),
                    );
                }
            }
        });

    action
}
