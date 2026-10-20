//! The WOL page dialogs on the shared control owners: `0xCF` over `0x113`
//! or C2/C9, and `0x11E` over the empty backdrop. Both are the 451x326
//! compact popup (background mode 2) with owner-draw buttons, NewEdits and
//! an owner-draw list.

use std::time::Instant;

use super::SHELL_LABEL_TEXT_RGB;
use super::controls::list_controls;
use super::text::rect_to_text_rect;
use super::wol_lobby::{POPUP_DEPTHS, WolLobbyPaint, label, push_edit, row_rgb};
use crate::app::AppState;
use crate::render::shell_paint::{self, ModalButton, PaintLabel};
use crate::render::shell_text::{self, ShellAlign};
use crate::ui::lan_shell::{LanChatLine, LanListState};
use crate::ui::shell::button::ShellButtonInteraction;
use crate::ui::shell::geom::{RectPx, ShellScale};
use crate::ui::shell::list::ShellListGeometry;
use crate::ui::shell::modal::{WolPageLayout, WolPageReceivedLayout};
use crate::ui::wol_page::*;

const SCALE: ShellScale = ShellScale::RETAIL;

fn caption(
    state: &AppState,
    key: &str,
    rect: RectPx,
    align: ShellAlign,
    enabled: bool,
) -> PaintLabel<'static> {
    PaintLabel {
        text: label(state, key).into(),
        rect,
        align,
        rgb: if enabled {
            SHELL_LABEL_TEXT_RGB
        } else {
            shell_paint::SHELL_TEXT_RGB_DISABLED
        },
        path_a_reveal: None,
    }
}

fn button_caption(
    state: &AppState,
    key: &str,
    rect: RectPx,
    buttons: &ShellButtonInteraction<u16>,
    id: u16,
    enabled: bool,
) -> PaintLabel<'static> {
    caption(
        state,
        key,
        crate::app::frontend::shell_pass::owner_draw_button_label_rect(
            rect,
            buttons.is_pressed(id),
        ),
        ShellAlign(ShellAlign::H_CENTER.0 | ShellAlign::V_CENTER.0),
        enabled,
    )
}

/// The frame, scrollbar and rows of a transcript.
fn paint_transcript(
    paint: &mut WolLobbyPaint,
    state: &AppState,
    geometry: ShellListGeometry,
    list: &LanListState,
    lines: &[LanChatLine],
    selectable: bool,
) {
    let Some(atlas) = state.frontend.skirmish_shell_chrome.as_ref() else {
        return;
    };
    list_controls(
        &mut paint.controls,
        atlas,
        geometry,
        list,
        selectable,
        SCALE,
    );
    for (visible, line) in lines
        .iter()
        .skip(list.top_index)
        .take(geometry.visible_rows)
        .enumerate()
    {
        let row = geometry.row(visible);
        paint.texts.push(shell_text::draw_in_rect_scaled(
            &state.renderer.bit_font,
            &String::from_utf16_lossy(&line.text),
            rect_to_text_rect(RectPx::new(row.x + 2, row.y, row.w - 2, row.h)),
            row_rgb(line.color),
            ShellAlign::V_CENTER,
            [0.0; 2],
            POPUP_DEPTHS.text,
            SCALE.factor(),
        ));
    }
}

/// Draw the box first, then its children over it.
fn paint_box(
    state: &AppState,
    dialog: RectPx,
    buttons: &[ModalButton],
    labels: &[PaintLabel],
) -> WolLobbyPaint {
    let mut paint = WolLobbyPaint::default();
    let Some(atlas) = state.frontend.skirmish_shell_chrome.as_ref() else {
        return paint;
    };
    let draw = shell_paint::paint_modal_shp(
        &state.renderer.bit_font,
        atlas.mode_two_dialog_background(None),
        super::type3_button_frames(atlas),
        dialog,
        buttons,
        labels,
        SCALE,
        POPUP_DEPTHS,
    );
    paint.controls = draw.sprites;
    paint.texts.extend(draw.text);
    paint
}

/// `0xCF`: no title; the name, text, transcript, status and two buttons.
pub(crate) fn paint_wol_page_dialog(
    state: &AppState,
    page: &WolPageDialog,
    layout: &WolPageLayout,
) -> WolLobbyPaint {
    let labels = vec![
        // SS_RIGHT, then two left-aligned statics.
        caption(
            state,
            "GUI:PageBuddy",
            layout.name_label,
            ShellAlign::H_RIGHT,
            true,
        ),
        caption(
            state,
            "GUI:TextToSend",
            layout.text_label,
            ShellAlign::NONE,
            true,
        ),
        caption(
            state,
            "GUI:FindStatus",
            layout.status_label,
            ShellAlign::NONE,
            true,
        ),
        PaintLabel {
            text: page.status.clone().into(),
            rect: layout.status,
            align: ShellAlign::NONE,
            rgb: SHELL_LABEL_TEXT_RGB,
            path_a_reveal: None,
        },
        button_caption(
            state,
            "GUI:Page",
            layout.send,
            &page.buttons,
            SEND,
            page.send_enabled,
        ),
        button_caption(
            state,
            "GUI:Cancel",
            layout.cancel,
            &page.buttons,
            CANCEL,
            true,
        ),
    ];
    let buttons = [
        ModalButton {
            rect: layout.send,
            pressed: page.buttons.is_pressed(SEND),
            enabled: page.send_enabled,
        },
        ModalButton {
            rect: layout.cancel,
            pressed: page.buttons.is_pressed(CANCEL),
            enabled: true,
        },
    ];
    let mut paint = paint_box(state, layout.dialog, &buttons, &labels);
    let caret = caret_visible(page.focus_at, Instant::now());
    for (edit, rect) in [(&page.name, layout.name), (&page.text, layout.text)] {
        push_edit(
            &mut paint,
            state,
            edit,
            rect,
            edit.focused && caret,
            SCALE,
            POPUP_DEPTHS.text,
        );
    }
    let geometry = crate::app::shell_wol::page::page_geometry(layout, page);
    paint_transcript(&mut paint, state, geometry, &page.list, &page.lines, false);
    paint
}

/// `0x11E`: the centred title, the selectable transcript, the reply edit,
/// "Send to:" with the pager and two buttons.
pub(crate) fn paint_wol_page_received(
    state: &AppState,
    received: &WolPageReceived,
    layout: &WolPageReceivedLayout,
) -> WolLobbyPaint {
    let labels = vec![
        caption(
            state,
            "GUI:PageReceived",
            layout.title,
            ShellAlign::H_CENTER,
            true,
        ),
        caption(
            state,
            "GUI:ReplyTo",
            layout.reply_to_label,
            ShellAlign::H_RIGHT,
            true,
        ),
        PaintLabel {
            text: received.reply_to.clone().into(),
            rect: layout.reply_to,
            align: ShellAlign::NONE,
            rgb: SHELL_LABEL_TEXT_RGB,
            path_a_reveal: None,
        },
        button_caption(
            state,
            "GUI:ReplyPage",
            layout.send,
            &received.buttons,
            SEND,
            received.send_enabled,
        ),
        button_caption(
            state,
            "GUI:Back",
            layout.back,
            &received.buttons,
            BACK,
            true,
        ),
    ];
    let buttons = [
        ModalButton {
            rect: layout.send,
            pressed: received.buttons.is_pressed(SEND),
            enabled: received.send_enabled,
        },
        ModalButton {
            rect: layout.back,
            pressed: received.buttons.is_pressed(BACK),
            enabled: true,
        },
    ];
    let mut paint = paint_box(state, layout.dialog, &buttons, &labels);
    push_edit(
        &mut paint,
        state,
        &received.reply,
        layout.reply,
        received.reply.focused && caret_visible(received.focus_at, Instant::now()),
        SCALE,
        POPUP_DEPTHS.text,
    );
    let geometry = crate::app::shell_wol::page::received_geometry(layout, received);
    paint_transcript(
        &mut paint,
        state,
        geometry,
        &received.list,
        &received.lines,
        true,
    );
    paint
}
