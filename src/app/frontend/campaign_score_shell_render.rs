//! Render glue for campaign victory dialog `0x103`.
//!
//! Native `GameExit::Victory` opens `0x103` through `0x6C8E80`; this renderer
//! consumes that dialog's retail controls and texts from
//! `ui::campaign_score_shell`. It deliberately does not borrow the Skirmish
//! score table (`0x108`) or its side-art owner.

use std::time::Instant;

use anyhow::Result;

use crate::app::AppState;
use crate::app::frontend::shell_pass::{
    ShellComposition, TexturedDraw, encode_shell_pass, owner_draw_button_label_rect, resolve_csf,
    software_cursor,
};
use crate::render::shell_paint::{
    self, CURSOR_DEPTH, PARENT_BACKGROUND_DEPTH, PaintButton, PaintLabel, SHELL_TEXT_RGB_ENABLED,
};
use crate::render::shell_text::ShellAlign;
use crate::render::shell_text_reveal::PathAReveal;
use crate::ui::campaign_score_shell::{
    CAMPAIGN_SCORE_PAGE, CONTINUE_BUTTON, CampaignScoreControlSpec, CampaignScoreLayout,
};

const SHELL_TEXT: [u8; 3] = [255, 255, 0];
const HIGHLIGHT: [u8; 3] = [255, 255, 255];

/// Paint campaign dialog `0x103`. It needs the shared shell chrome, but no
/// Skirmish score art: the retail 0x103 template has its own static controls.
pub(crate) fn render_campaign_score_page(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    destination: &wgpu::Texture,
) -> Result<bool> {
    if state.frontend.main_menu_shell_chrome.is_none()
        || state.frontend.campaign_score_page.is_none()
    {
        return Ok(false);
    }

    let (width, height) = crate::app::frontend::shell_layout::shell_screen_size(state);
    let layout = if crate::app::frontend::shell_layout::fills_window(state) {
        CampaignScoreLayout::compute_responsive_layout(width, height)
    } else {
        CampaignScoreLayout::compute_layout(width, height)
    };
    let page_layout =
        crate::app::frontend::shell_layout::menu_page_layout(state, &CAMPAIGN_SCORE_PAGE);
    let now = Instant::now();
    let controller = &state.frontend.shell_controller;
    let active = controller.top_id() == Some(CAMPAIGN_SCORE_PAGE.dialog);
    let hovered = active.then(|| controller.hovered()).flatten();
    let status = hovered
        .and_then(CampaignScoreControlSpec::for_id)
        .and_then(|control| control.tooltip_key)
        .map(|key| resolve_csf(state, key).into_owned())
        .unwrap_or_default();
    let pressed = active.then(|| controller.pressed()).flatten();

    let mut labels: Vec<PaintLabel<'static>> = Vec::new();
    let campaign_page = state
        .frontend
        .campaign_score_page
        .as_mut()
        .expect("checked before render");
    campaign_page.set_status_help(&status, now);
    for static_text in campaign_page.statics_mut() {
        let Some(window) = static_text.paint(now) else {
            continue;
        };
        let Some(rect) = layout.control_rect(static_text.id) else {
            continue;
        };
        labels.push(PaintLabel {
            text: static_text.text().to_owned().into(),
            rect,
            // The low two bits of Win32 STATIC style select left/center/right;
            // only SS_CENTER (1) is used by this dialog. The title keeps its
            // native centered style, while the data prose remains left aligned.
            align: if CampaignScoreControlSpec::for_id(static_text.id)
                .is_some_and(|control| control.native_style & 0x3 == 0x1)
            {
                ShellAlign::H_CENTER
            } else {
                ShellAlign::NONE
            },
            rgb: SHELL_TEXT_RGB_ENABLED,
            path_a_reveal: Some(PathAReveal {
                count: window.count,
                range: window.range,
                base_rgb: SHELL_TEXT,
                highlight_rgb: HIGHLIGHT,
            }),
        });
    }

    let chrome = state
        .frontend
        .main_menu_shell_chrome
        .as_ref()
        .expect("checked before render");
    let mut chrome_sprites = if state.frontend.campaign_score_art.is_none() {
        crate::app::frontend::main_menu_shell_render::shell_parent_background_instances(
            chrome,
            page_layout.screen.w,
            page_layout.screen.h,
            page_layout.scale,
        )
    } else {
        Vec::new()
    };
    chrome_sprites.extend(shell_paint::paint_chrome(
        chrome,
        page_layout.right_panel,
        Some(page_layout.lower_strip),
        page_layout.screen.w,
    ));

    let buttons = [PaintButton {
        rect: layout.continue_button,
        pressed: pressed == Some(CONTINUE_BUTTON),
        hovered: false,
        enabled: true,
    }];
    let button_sprites = shell_paint::paint_buttons(
        chrome,
        &buttons,
        crate::app::frontend::menu_page_render::MENU_PAGE_BUTTON_POLICY,
        now,
        None,
    );
    labels.push(PaintLabel {
        text: resolve_csf(state, "GUI:Continue").into_owned().into(),
        rect: owner_draw_button_label_rect(
            layout.continue_button,
            pressed == Some(CONTINUE_BUTTON),
        ),
        align: ShellAlign(ShellAlign::H_CENTER.0 | ShellAlign::V_CENTER.0),
        rgb: SHELL_TEXT_RGB_ENABLED,
        path_a_reveal: None,
    });
    let text =
        shell_paint::paint_labels_scaled(&state.renderer.bit_font, &labels, layout.scale.factor());
    let mut draws = Vec::with_capacity(3);
    if let Some(art) = state.frontend.campaign_score_art.as_ref() {
        let background = art.background;
        draws.push(TexturedDraw {
            texture: &art.texture,
            instances: vec![crate::render::batch::SpriteInstance {
                position: [0.0, 0.0],
                size: [page_layout.screen.w as f32, page_layout.screen.h as f32],
                uv_origin: background.uv_origin,
                uv_size: background.uv_size,
                depth: PARENT_BACKGROUND_DEPTH,
                tint: [1.0, 1.0, 1.0],
                alpha: 1.0,
                ..Default::default()
            }],
        });
    }
    draws.push(TexturedDraw {
        texture: &chrome.texture,
        instances: chrome_sprites,
    });
    draws.push(TexturedDraw {
        texture: &chrome.texture,
        instances: button_sprites,
    });
    encode_shell_pass(
        state,
        encoder,
        destination,
        "Campaign Score Shell",
        ShellComposition {
            draws: &draws,
            text: &text,
            cursor: software_cursor(state, CURSOR_DEPTH),
            effects: None,
        },
    );
    Ok(true)
}
