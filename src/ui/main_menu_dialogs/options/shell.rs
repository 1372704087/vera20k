//! Physical-pixel presentation and input for launcher resource 0xD5.
//!
//! Retail RT_DIALOG/0xD5/1033 provides the DLU rectangles; common setup
//! 0x00608CD0/0x00609730 anchors the rail, and 0x0060B950 adjusts its title.
//! This module projects the retained OptionsDialogState instead of owning a
//! second set of option values or bypassing its ordered preview events.

use super::{
    LauncherCheckboxId, LauncherCue, LauncherOptionsEvent, LauncherParentResult,
    LauncherResolutionRow, LauncherTrackbarId, OptionsDialogState, PhysicalControlFrame,
};
use crate::ui::shell::descriptor::DialogId;
use crate::ui::shell::geom::{RectPx, ShellScale, center_offset, child_window, dlu_rect};
use crate::ui::shell::menu_page::{self, MenuPageButtonSpec, MenuPageSpec};
use crate::ui::shell::trackbar::{authored_track, thumb_left_at};
use crate::ui::skirmish_shell::{COMBO_FACE_H, DropdownMetrics, ScrollModel, combo_face_rect};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ShellInteraction {
    hovered_button: Option<LauncherParentResult>,
    pressed_button: Option<LauncherParentResult>,
    popup_hovered: Option<usize>,
    popup_top: usize,
    popup_scroll_grab: Option<i32>,
    /// Status help last written by a dialog hit test (`0x00622CCB`).
    status_help: Option<&'static str>,
}

pub(crate) const KEYBOARD_BUTTON: u16 = 0x05CE;
pub(crate) const NETWORK_BUTTON: u16 = 0x05CD;
pub(crate) const MAIN_MENU_BUTTON: u16 = 0x0686;

/// `0xD5` as a family page (Session `+0x30D8` clear at the main menu): the
/// right-panel column is Keyboard and Network (`0x006092CB`), then Main Menu
/// (`0x0060982C`); the heading and status line sit at the family places.
/// Button captions come from the dialog's own labels.
pub(crate) const OPTIONS_PAGE: MenuPageSpec = MenuPageSpec {
    dialog: DialogId(0x00D5),
    title_key: "GUI:OptionsMenu",
    stacked: &[
        MenuPageButtonSpec {
            id: KEYBOARD_BUTTON,
            dlu_top: 122,
            csf_key: "GUI:Keyboard",
            tooltip_key: "STT:MainOptButtonKeyboard",
            result: None,
        },
        MenuPageButtonSpec {
            id: NETWORK_BUTTON,
            dlu_top: 149,
            csf_key: "GUI:Network",
            tooltip_key: "STT:MainOptButtonNetwork",
            result: None,
        },
    ],
    back: MenuPageButtonSpec {
        id: MAIN_MENU_BUTTON,
        dlu_top: 346,
        csf_key: "GUI:MainMenu",
        tooltip_key: "STT:MainOptButtonBack",
        result: Some(0x05CB),
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LauncherOptionsLayout {
    pub(crate) title: RectPx,
    pub(crate) warning: RectPx,
    /// Status line `0x695`.
    pub(crate) status_help: RectPx,
    pub(crate) resolution: RectPx,
    pub(crate) trackbars: [(LauncherTrackbarId, RectPx); 6],
    pub(crate) checkboxes: [(LauncherCheckboxId, RectPx); 3],
    pub(crate) buttons: [(LauncherParentResult, RectPx); 3],
    /// The mapping the rects are in: [`ShellScale::RETAIL`] centres the
    /// authored dialog in the window, otherwise the authored dialog is
    /// stretched over it. Control sub-rects and art fits key on this.
    pub(crate) scale: ShellScale,
    offset_x: i32,
    offset_y: i32,
}

impl LauncherOptionsLayout {
    /// Gamemd's centred `0xD5`: the authored dialog placed in the window at
    /// [`ShellScale::RETAIL`].
    pub(crate) fn new(width: i32, height: i32) -> Self {
        Self::build(width, height, ShellScale::RETAIL)
    }

    /// The same dialog stretched over the window (`menu_fill_window`).
    pub(crate) fn responsive(width: i32, height: i32) -> Self {
        Self::build(width, height, ShellScale::from_screen(width, height))
    }

    fn build(width: i32, height: i32, scale: ShellScale) -> Self {
        // A stretched page fills the window, so it has no centring offset and
        // every authored rect goes through the one scale.
        let stretch = !scale.is_retail();
        let offset_x = if stretch {
            0
        } else {
            center_offset(width, 800)
        };
        let offset_y = if stretch {
            0
        } else {
            center_offset(height, 600)
        };
        let px = |rect: RectPx| scale.scale_rect(rect.translate(offset_x, offset_y));
        let dlu = |x, y, w, h| px(dlu_rect(x, y, w, h));
        // The sliders run one pixel wider and taller than the template
        // (retail stills: 181x22 and 129x22).
        let slider = |x, y, w, h| px(child_window(x, y, w, h));
        // The right-panel children follow the panel, not the dialog centring.
        let page = if stretch {
            menu_page::compute_responsive_layout(&OPTIONS_PAGE, width as u32, height as u32)
        } else {
            menu_page::compute_layout(&OPTIONS_PAGE, width as u32, height as u32)
        };
        let button = |id| page.button_rect(id).expect("0xD5 page button");
        Self {
            title: page.title,
            warning: page.warning_monitor,
            status_help: page.status_help,
            resolution: px(RectPx::new(351, 86, 180, COMBO_FACE_H)),
            trackbars: [
                (LauncherTrackbarId::Detail, slider(89, 53, 120, 13)),
                (LauncherTrackbarId::Difficulty, slider(92, 128, 120, 13)),
                (LauncherTrackbarId::Scroll, slider(238, 203, 120, 13)),
                (LauncherTrackbarId::Score, slider(82, 289, 85, 13)),
                (LauncherTrackbarId::Sound, slider(179, 289, 85, 13)),
                (LauncherTrackbarId::Voice, slider(277, 289, 85, 13)),
            ],
            checkboxes: [
                (LauncherCheckboxId::Tooltips, dlu(93, 188, 130, 10)),
                (LauncherCheckboxId::TargetLines, dlu(93, 203, 130, 10)),
                (LauncherCheckboxId::ShowHidden, dlu(93, 218, 130, 10)),
            ],
            buttons: [
                (LauncherParentResult::Keyboard, button(KEYBOARD_BUTTON)),
                (LauncherParentResult::Network, button(NETWORK_BUTTON)),
                (LauncherParentResult::Back, button(MAIN_MENU_BUTTON)),
            ],
            scale,
            offset_x,
            offset_y,
        }
    }

    fn label_rect(&self, x: i32, y: i32, w: i32, h: i32) -> RectPx {
        self.scale
            .scale_rect(dlu_rect(x, y, w, h).translate(self.offset_x, self.offset_y))
    }

    fn trackbar_rect(&self, id: LauncherTrackbarId) -> RectPx {
        self.trackbars
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .unwrap()
            .1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LauncherLabelAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LauncherShellLabel<'a> {
    pub(crate) rect: RectPx,
    pub(crate) text: &'a str,
    pub(crate) align: LauncherLabelAlign,
    pub(crate) title: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LauncherResolutionPopup {
    pub(crate) rect: RectPx,
    pub(crate) content: RectPx,
    pub(crate) row_height: i32,
    pub(crate) first_row: usize,
    pub(crate) visible_rows: usize,
    pub(crate) scrollbar: Option<RectPx>,
    pub(crate) thumb: Option<RectPx>,
    pub(crate) selected: Option<usize>,
    pub(crate) hovered: Option<usize>,
}

impl LauncherResolutionPopup {
    fn row_at(self, x: i32, y: i32) -> Option<usize> {
        self.content
            .contains(x, y)
            .then(|| self.first_row + ((y - self.content.y) / self.row_height) as usize)
    }
}

/// The control-relative cursor and window in the authored pixels the native
/// trackbar and checkbox arithmetic runs on, so a stretched control admits the
/// same strip and thumb the paint side draws.
fn frame(rect: RectPx, x: i32, y: i32, scale: ShellScale) -> PhysicalControlFrame {
    let track = authored_track(rect, (x, y), scale);
    PhysicalControlFrame {
        local_x: track.local_x,
        local_y: track.local_y,
        width: track.width,
        height: track.height,
    }
}

impl OptionsDialogState {
    /// Native61E3AF..61E44E drops held/drag state once MK_LBUTTON is absent.
    /// Focus loss is the host's authoritative lost-gesture boundary.
    pub(crate) fn shell_cancel_pointer_gesture(&mut self) {
        self.capture = None;
        self.shell_interaction.pressed_button = None;
        self.shell_interaction.hovered_button = None;
        self.shell_interaction.popup_scroll_grab = None;
    }
    pub(crate) fn shell_labels(
        &self,
        layout: &LauncherOptionsLayout,
    ) -> Vec<LauncherShellLabel<'_>> {
        use LauncherLabelAlign::{Center, Left, Right};
        let labels = &self.labels;
        let mut result = vec![LauncherShellLabel {
            rect: layout.title,
            text: &labels.options,
            align: Center,
            title: true,
        }];
        for (text, x, y, w, h, align) in [
            (labels.display_options.as_str(), 20, 18, 140, 10, Left),
            (&labels.game_options, 20, 93, 298, 10, Left),
            (&labels.ui_options, 20, 168, 298, 10, Left),
            (&labels.audio_options, 20, 254, 298, 10, Left),
            (&labels.visual_details, 89, 38, 60, 10, Left),
            (&self.detail_caption, 149, 38, 60, 10, Right),
            (&labels.set_resolution, 234, 38, 120, 10, Left),
            (&labels.difficulty, 92, 113, 60, 10, Left),
            (&self.difficulty_caption, 152, 113, 60, 10, Right),
            (&labels.scroll_rate, 238, 188, 60, 10, Left),
            (&self.scroll_caption, 298, 188, 60, 10, Right),
            (&labels.music_volume, 82, 274, 85, 10, Center),
            (&labels.sound_volume, 179, 274, 85, 10, Center),
            (&labels.voice_volume, 277, 274, 85, 10, Center),
            (&labels.blank, 2, 355, 303, 12, Left),
        ] {
            result.push(LauncherShellLabel {
                rect: layout.label_rect(x, y, w, h),
                text,
                align,
                title: false,
            });
        }
        result
    }

    pub(crate) fn shell_checkbox_label(&self, id: LauncherCheckboxId) -> &str {
        match id {
            LauncherCheckboxId::Tooltips => &self.labels.tooltips,
            LauncherCheckboxId::TargetLines => &self.labels.target_lines,
            LauncherCheckboxId::ShowHidden => &self.labels.show_hidden,
        }
    }

    pub(crate) fn shell_button_label(&self, id: LauncherParentResult) -> &str {
        match id {
            LauncherParentResult::Keyboard => &self.labels.keyboard,
            LauncherParentResult::Network => &self.labels.network,
            LauncherParentResult::Back => &self.labels.main_menu,
            LauncherParentResult::Terminal => "",
        }
    }

    pub(crate) fn shell_title_text(&self) -> &str {
        &self.labels.options
    }

    pub(crate) fn shell_trackbar_thumb_left(
        &self,
        id: LauncherTrackbarId,
        window: RectPx,
        scale: ShellScale,
    ) -> i32 {
        thumb_left_at(
            i32::from(self.trackbar_position(id)),
            window,
            id.plaque_reserve(),
            i32::from(id.maximum()),
            scale,
        )
    }

    pub(crate) fn shell_checkbox_checked(&self, id: LauncherCheckboxId) -> bool {
        match id {
            LauncherCheckboxId::Tooltips => self.values.tooltips,
            LauncherCheckboxId::TargetLines => self.values.target_lines,
            LauncherCheckboxId::ShowHidden => self.values.show_hidden,
        }
    }

    pub(crate) fn shell_selected_resolution_text(&self) -> &str {
        self.selected_resolution_label()
    }
    pub(crate) fn shell_resolution_rows(&self) -> &[LauncherResolutionRow] {
        &self.resolution_rows
    }
    pub(crate) fn shell_button_hovered(&self, id: LauncherParentResult) -> bool {
        self.shell_interaction.hovered_button == Some(id)
    }
    pub(crate) fn shell_button_pressed(&self, id: LauncherParentResult) -> bool {
        self.shell_interaction.pressed_button == Some(id) && self.shell_button_hovered(id)
    }

    pub(crate) fn shell_popup(
        &self,
        layout: &LauncherOptionsLayout,
    ) -> Option<LauncherResolutionPopup> {
        if !self.resolution_popup_open || self.resolution_rows.is_empty() {
            return None;
        }
        // Original ComboDropWin creation 0x006180CF..0x00618205 truncates
        // the retained dropped allocation to whole item-height rows. D5 has
        // 74 DLU (120px) allocated, admitting five 23px rows. Actual HWND
        // border adjustments remain outside this resource-derived bound.
        let metrics = DropdownMetrics::scaled(layout.scale);
        let model = ScrollModel::combo_at(5, metrics);
        let visible_rows = model.visible_rows(self.resolution_rows.len());
        // The list box hangs under the painted face; its rows and scrollbar use
        // this window's dropdown device metrics, the same the combo pages use.
        let face = combo_face_rect(layout.resolution, layout.scale);
        let rect = RectPx::new(
            layout.resolution.x,
            face.y + face.h + 1,
            layout.resolution.w,
            visible_rows as i32 * metrics.row_h,
        );
        let max_top = self.resolution_rows.len().saturating_sub(visible_rows);
        let first_row = self.shell_interaction.popup_top.min(max_top);
        let scrollbar = (max_top > 0).then(|| {
            RectPx::new(
                rect.x + rect.w - metrics.scrollbar_w,
                rect.y,
                metrics.scrollbar_w,
                rect.h,
            )
        });
        let thumb = scrollbar.map(|scrollbar| {
            let height = model.thumb_height(visible_rows, self.resolution_rows.len(), scrollbar.h);
            RectPx::new(
                scrollbar.x,
                model.thumb_y(scrollbar, height, first_row, max_top),
                scrollbar.w,
                height,
            )
        });
        Some(LauncherResolutionPopup {
            rect,
            content: RectPx::new(
                rect.x,
                rect.y,
                rect.w - scrollbar.map_or(0, |s| s.w),
                rect.h,
            ),
            row_height: metrics.row_h,
            first_row,
            visible_rows,
            scrollbar,
            thumb,
            selected: self.selected_resolution,
            hovered: self.shell_interaction.popup_hovered,
        })
    }

    /// The status help the line shows: the key the last dialog hit test
    /// wrote. While a slider or button holds the mouse capture, or the
    /// resolution list is open, the dialog gets no hit test and the text
    /// stays.
    pub(crate) fn shell_status_help(&self) -> Option<&'static str> {
        self.shell_interaction.status_help
    }

    /// Status help (`0x00604729..0x00604838`) for the control under the
    /// pointer. Hover is enable-unfiltered; the open resolution list is a
    /// window of its own with no entry.
    pub(crate) fn shell_status_help_key(
        &self,
        x: i32,
        y: i32,
        layout: &LauncherOptionsLayout,
    ) -> Option<&'static str> {
        if self
            .shell_popup(layout)
            .is_some_and(|popup| popup.rect.contains(x, y))
        {
            return None;
        }
        if let Some((id, _)) = layout.buttons.iter().find(|(_, rect)| rect.contains(x, y)) {
            let control = match id {
                LauncherParentResult::Keyboard => KEYBOARD_BUTTON,
                LauncherParentResult::Network => NETWORK_BUTTON,
                LauncherParentResult::Back | LauncherParentResult::Terminal => MAIN_MENU_BUTTON,
            };
            return OPTIONS_PAGE
                .button(control)
                .map(|button| button.tooltip_key);
        }
        if let Some((id, _)) = layout
            .trackbars
            .iter()
            .find(|(_, rect)| rect.contains(x, y))
        {
            return Some(match id {
                LauncherTrackbarId::Detail => "STT:MainOptSliderVisual",
                LauncherTrackbarId::Difficulty => "STT:MainOptSliderDifficulty",
                LauncherTrackbarId::Scroll => "STT:MainOptSliderScroll",
                LauncherTrackbarId::Score => "STT:MainOptSliderMusic",
                LauncherTrackbarId::Sound => "STT:MainOptSliderSound",
                LauncherTrackbarId::Voice => "STT:MainOptSliderVoice",
            });
        }
        if let Some((id, _)) = layout
            .checkboxes
            .iter()
            .find(|(_, rect)| rect.contains(x, y))
        {
            return Some(match id {
                LauncherCheckboxId::Tooltips => "STT:MainOptCBoxTooltips",
                LauncherCheckboxId::TargetLines => "STT:MainOptCBoxTargetLines",
                LauncherCheckboxId::ShowHidden => "STT:MainOptCBoxHidden",
            });
        }
        layout
            .resolution
            .contains(x, y)
            .then_some("STT:MainOptComboModes")
    }

    /// The caller routes all pointer messages here while this parent is topmost.
    pub(crate) fn shell_mouse_down(&mut self, x: i32, y: i32, layout: &LauncherOptionsLayout) {
        let scale = layout.scale;
        if self.capture.is_some() {
            return;
        }
        if self.resolution_popup_open {
            if let Some(popup) = self.shell_popup(layout)
                && let Some(scrollbar) = popup.scrollbar.filter(|rect| rect.contains(x, y))
            {
                let metrics = DropdownMetrics::scaled(scale);
                let max_top = self
                    .resolution_rows
                    .len()
                    .saturating_sub(popup.visible_rows);
                if y < scrollbar.y + metrics.button_h {
                    self.shell_interaction.popup_top = popup.first_row.saturating_sub(1);
                } else if y >= scrollbar.y + scrollbar.h - metrics.button_h {
                    self.shell_interaction.popup_top = (popup.first_row + 1).min(max_top);
                } else if let Some(thumb) = popup.thumb {
                    if thumb.contains(x, y) {
                        self.shell_interaction.popup_scroll_grab = Some(y - thumb.y);
                    } else {
                        self.shell_interaction.popup_top = ScrollModel::combo_at(5, metrics)
                            .top_index_from_thumb_top(scrollbar, thumb.h, max_top, y - thumb.h / 2);
                    }
                }
                self.shell_interaction.popup_hovered = None;
                return;
            }
            // ComboDropWin 0x0060E4E9..0x0060E500 plays Rules+0x1A8
            // (GUIComboCloseSound) before selection or outside dismissal;
            // scrollbar forwarding above returns before this cue.
            self.pending_events
                .push(LauncherOptionsEvent::Cue(LauncherCue::ComboClose));
            if let Some(index) = self
                .shell_popup(layout)
                .and_then(|popup| popup.row_at(x, y))
            {
                self.select_resolution(index);
            } else {
                self.resolution_popup_open = false;
            }
            self.shell_interaction = ShellInteraction::default();
            return;
        }
        self.shell_mouse_move(x, y, layout);
        if let Some((id, _)) = layout.buttons.iter().find(|(_, rect)| rect.contains(x, y)) {
            self.shell_interaction.pressed_button = Some(*id);
            self.main_button_mouse_down();
        } else if let Some((id, rect)) = layout
            .trackbars
            .iter()
            .find(|(_, rect)| rect.contains(x, y))
        {
            self.trackbar_mouse_down(*id, frame(*rect, x, y, scale));
        } else if let Some((id, rect)) = layout
            .checkboxes
            .iter()
            .find(|(_, rect)| rect.contains(x, y))
        {
            self.checkbox_mouse_down(*id, frame(*rect, x, y, scale));
        } else if layout.resolution.contains(x, y) {
            self.combo_mouse_down(frame(layout.resolution, x, y, scale));
            self.shell_interaction.hovered_button = None;
            // The fresh custom popup record is zeroed. Its 0x7E8 refresh
            // copies CB_GETCURSEL to highlight +E8, leaving top-index +F0
            // zero (0x0060F276..0x0060F283).
            self.shell_interaction.popup_top = 0;
        }
    }

    pub(crate) fn shell_mouse_move(&mut self, x: i32, y: i32, layout: &LauncherOptionsLayout) {
        let scale = layout.scale;
        if self.capture.is_none()
            && self.shell_interaction.pressed_button.is_none()
            && !self.resolution_popup_open
        {
            self.shell_interaction.status_help = self.shell_status_help_key(x, y, layout);
        }
        if let Some(hold) = self.capture {
            let rect = layout.trackbar_rect(hold.id);
            self.trackbar_mouse_move(hold.id, frame(rect, x, y, scale));
            self.shell_interaction.hovered_button = None;
            return;
        }
        if self.resolution_popup_open {
            if let Some(grab) = self.shell_interaction.popup_scroll_grab {
                if let Some(popup) = self.shell_popup(layout)
                    && let (Some(scrollbar), Some(thumb)) = (popup.scrollbar, popup.thumb)
                {
                    let max_top = self
                        .resolution_rows
                        .len()
                        .saturating_sub(popup.visible_rows);
                    self.shell_interaction.popup_top =
                        ScrollModel::combo_at(5, DropdownMetrics::scaled(scale))
                            .top_index_from_thumb_top(scrollbar, thumb.h, max_top, y - grab);
                }
                self.shell_interaction.popup_hovered = None;
                return;
            }
            self.shell_interaction.popup_hovered = self
                .shell_popup(layout)
                .and_then(|popup| popup.row_at(x, y));
            self.shell_interaction.hovered_button = None;
        } else {
            self.shell_interaction.hovered_button = layout
                .buttons
                .iter()
                .find(|(_, rect)| rect.contains(x, y))
                .map(|(id, _)| *id);
        }
    }

    pub(crate) fn shell_mouse_up(&mut self, x: i32, y: i32, layout: &LauncherOptionsLayout) {
        self.shell_interaction.popup_scroll_grab = None;
        if let Some(hold) = self.capture {
            self.trackbar_mouse_up(hold.id);
        }
        self.shell_mouse_move(x, y, layout);
        if let Some(id) = self.shell_interaction.pressed_button.take()
            && self.shell_interaction.hovered_button == Some(id)
            && !self.resolution_popup_open
        {
            self.request_result(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::main_menu_dialogs::options::{
        LauncherCue, LauncherOptionsEvent, LauncherOptionsLabels, LauncherOptionsValues,
    };
    use crate::ui::shell::trackbar::TrackbarHold;

    fn state() -> OptionsDialogState {
        OptionsDialogState::new(
            LauncherOptionsLabels::resolve(&|_| None),
            LauncherOptionsValues::default(),
            vec![
                LauncherResolutionRow::new(800, 600),
                LauncherResolutionRow::new(1024, 768),
            ],
            Some(0),
            true,
        )
    }

    #[test]
    fn right_panel_children_follow_the_family_places() {
        let layout = LauncherOptionsLayout::new(800, 600);
        assert_eq!(layout.title, RectPx::new(635, 9, 163, 18));
        assert_eq!(layout.status_help, RectPx::new(10, 578, 456, 21));
        let buttons = layout.buttons.map(|(_, rect)| rect);
        assert_eq!(
            buttons,
            [
                RectPx::new(644, 199, 156, 42),
                RectPx::new(644, 241, 156, 42),
                RectPx::new(644, 535, 156, 42),
            ]
        );
    }

    #[test]
    fn responsive_layout_stretches_the_dialog_and_keeps_its_right_panel() {
        // Identity at the authored size: with the switch off the dialog is
        // what it is today.
        assert_eq!(
            LauncherOptionsLayout::responsive(800, 600),
            LauncherOptionsLayout::new(800, 600)
        );
        let base = LauncherOptionsLayout::new(800, 600);
        let scale = ShellScale::from_screen(1600, 1200);
        let wide = LauncherOptionsLayout::responsive(1600, 1200);
        assert_eq!(wide.scale, scale);
        // No centring offset: the dialog fills the window.
        assert_eq!((wide.offset_x, wide.offset_y), (0, 0));
        assert_eq!(wide.title, RectPx::new(1270, 18, 326, 36));
        assert_eq!(wide.status_help, RectPx::new(20, 1156, 912, 42));
        assert_eq!(wide.resolution, RectPx::new(702, 172, 360, 48));
        assert_eq!(
            wide.buttons.map(|(_, rect)| rect),
            [
                RectPx::new(1288, 398, 312, 84),
                RectPx::new(1288, 482, 312, 84),
                RectPx::new(1288, 1070, 312, 84),
            ]
        );
        assert_eq!(
            wide.trackbars.map(|(_, rect)| rect),
            base.trackbars.map(|(_, rect)| scale.scale_rect(rect))
        );
        assert_eq!(
            wide.checkboxes.map(|(_, rect)| rect),
            base.checkboxes.map(|(_, rect)| scale.scale_rect(rect))
        );
    }

    #[test]
    fn a_stretched_slider_maps_the_pointer_back_to_its_authored_track() {
        let mut dialog = state();
        let base = LauncherOptionsLayout::new(800, 600);
        let scale = ShellScale::from_screen(1600, 1200);
        let wide = LauncherOptionsLayout::responsive(1600, 1200);
        let authored = base.trackbar_rect(LauncherTrackbarId::Score);
        let stretched = wide.trackbar_rect(LauncherTrackbarId::Score);
        assert_eq!(stretched, scale.scale_rect(authored));
        // The thumb travels by the same authored span, then stretches.
        let thumb = dialog.shell_trackbar_thumb_left(
            LauncherTrackbarId::Score,
            authored,
            ShellScale::RETAIL,
        );
        let wide_thumb =
            dialog.shell_trackbar_thumb_left(LauncherTrackbarId::Score, stretched, scale);
        assert_eq!(wide_thumb, (thumb as f32 * scale.x()).round() as i32);
        // The same authored point addressed on either page takes the same
        // thumb and the same position.
        let press = (authored.x + thumb + 5, authored.y + authored.h - 3);
        dialog.shell_mouse_down(press.0, press.1, &base);
        let held = Some(TrackbarHold {
            id: LauncherTrackbarId::Score,
            dragging: true,
        });
        assert_eq!(dialog.capture, held);
        let pressed_position = dialog.trackbar_position(LauncherTrackbarId::Score);
        dialog.shell_mouse_up(press.0, press.1, &base);
        let device = scale.scale_rect(RectPx::new(press.0, press.1, 1, 1));
        let mut stretched_dialog = state();
        stretched_dialog.shell_mouse_down(device.x, device.y, &wide);
        assert_eq!(
            stretched_dialog.capture, held,
            "{device:?} over {stretched:?}"
        );
        assert_eq!(
            stretched_dialog.trackbar_position(LauncherTrackbarId::Score),
            pressed_position
        );
        // A jump press beside the thumb quantizes on the authored width.
        let jump = (authored.x + authored.w - 2, authored.y + authored.h - 3);
        dialog.shell_mouse_down(jump.0, jump.1, &base);
        let jumped = dialog.trackbar_position(LauncherTrackbarId::Score);
        dialog.shell_mouse_up(jump.0, jump.1, &base);
        let device = scale.scale_rect(RectPx::new(jump.0, jump.1, 1, 1));
        let mut stretched_dialog = state();
        stretched_dialog.shell_mouse_down(device.x, device.y, &wide);
        assert_eq!(
            stretched_dialog.trackbar_position(LauncherTrackbarId::Score),
            jumped,
            "{device:?} over {stretched:?}"
        );
    }

    #[test]
    fn status_help_names_the_control_under_the_pointer() {
        let state = state();
        let layout = LauncherOptionsLayout::new(800, 600);
        let at = |rect: RectPx| state.shell_status_help_key(rect.x + 2, rect.y + 2, &layout);
        assert_eq!(at(layout.buttons[0].1), Some("STT:MainOptButtonKeyboard"));
        assert_eq!(at(layout.buttons[2].1), Some("STT:MainOptButtonBack"));
        assert_eq!(
            at(layout.trackbar_rect(LauncherTrackbarId::Score)),
            Some("STT:MainOptSliderMusic")
        );
        assert_eq!(at(layout.resolution), Some("STT:MainOptComboModes"));
        assert_eq!(state.shell_status_help_key(5, 5, &layout), None);
    }

    #[test]
    fn a_captured_slider_keeps_the_status_help_until_release() {
        let mut state = state();
        let layout = LauncherOptionsLayout::new(800, 600);
        let rect = layout.trackbar_rect(LauncherTrackbarId::Score);
        let thumb = state.shell_trackbar_thumb_left(LauncherTrackbarId::Score, rect, layout.scale);
        state.shell_mouse_down(rect.x + thumb + 5, rect.y + 10, &layout);
        assert_eq!(state.shell_status_help(), Some("STT:MainOptSliderMusic"));
        let back = layout.buttons[2].1;
        state.shell_mouse_move(back.x + 5, back.y + 5, &layout);
        assert_eq!(state.shell_status_help(), Some("STT:MainOptSliderMusic"));
        state.shell_mouse_up(back.x + 5, back.y + 5, &layout);
        assert_eq!(state.shell_status_help(), Some("STT:MainOptButtonBack"));
    }

    #[test]
    fn physical_slider_capture_crosses_other_controls_without_activating_them() {
        let mut state = state();
        let layout = LauncherOptionsLayout::new(800, 600);
        let rect = layout.trackbar_rect(LauncherTrackbarId::Score);
        let thumb = state.shell_trackbar_thumb_left(LauncherTrackbarId::Score, rect, layout.scale);
        state.shell_mouse_down(rect.x + thumb + 5, rect.y + 10, &layout);
        assert_eq!(
            state.capture,
            Some(TrackbarHold {
                id: LauncherTrackbarId::Score,
                dragging: true,
            })
        );
        state.shell_mouse_move(799, 550, &layout);
        state.shell_mouse_up(799, 550, &layout);
        assert_eq!(state.trackbar_position(LauncherTrackbarId::Score), 10);
        assert_eq!(state.capture, None);
        let output = state.drain_output();
        assert_eq!(output.events, [LauncherOptionsEvent::ScorePreview(1.0)]);
        assert_eq!(output.result, None);
    }

    #[test]
    fn lost_focus_cancels_drag_before_pointer_reentry() {
        let mut state = state();
        let layout = LauncherOptionsLayout::new(800, 600);
        // Fixture starts at4: original128px/50px plaque projection puts the
        // thumb at client27 (screen150), so155 is inside its12px capture box.
        state.shell_mouse_down(155, 475, &layout);
        assert_eq!(
            state.capture,
            Some(TrackbarHold {
                id: LauncherTrackbarId::Score,
                dragging: true,
            })
        );
        state.shell_cancel_pointer_gesture();
        let before = state.pack();
        state.shell_mouse_move(799, 550, &layout);
        state.shell_mouse_up(799, 550, &layout);
        assert_eq!(state.pack(), before);
        assert!(state.drain_output().events.is_empty());
    }

    #[test]
    fn every_slider_press_holds_the_mouse_until_release() {
        let mut state = state();
        let layout = LauncherOptionsLayout::new(800, 600);
        let held = Some(TrackbarHold {
            id: LauncherTrackbarId::Difficulty,
            dragging: false,
        });
        // Above the admitted strip the press only takes the capture.
        state.shell_mouse_down(140, 210, &layout);
        assert_eq!(
            state.trackbar_position(LauncherTrackbarId::Difficulty),
            1,
            "native lower strip rejects y=2"
        );
        assert_eq!(state.capture, held);
        state.shell_mouse_up(140, 210, &layout);
        // Beside the thumb it jumps once and holds without dragging.
        state.shell_mouse_down(140, 218, &layout);
        assert_eq!(state.trackbar_position(LauncherTrackbarId::Difficulty), 0);
        assert_eq!(state.capture, held);
        assert_eq!(
            state.drain_output().events,
            [LauncherOptionsEvent::Cue(LauncherCue::GenericClick)]
        );
        state.shell_mouse_move(317, 218, &layout);
        assert_eq!(state.trackbar_position(LauncherTrackbarId::Difficulty), 0);
        let back = layout.buttons[2].1;
        state.shell_mouse_move(back.x + 5, back.y + 5, &layout);
        assert_eq!(
            state.shell_status_help(),
            Some("STT:MainOptSliderDifficulty")
        );
        state.shell_mouse_up(back.x + 5, back.y + 5, &layout);
        assert_eq!(state.capture, None);
        assert_eq!(state.shell_status_help(), Some("STT:MainOptButtonBack"));
        assert_eq!(state.drain_output().result, None);
        // The checkbox text takes no press.
        state.shell_mouse_down(170, 310, &layout);
        assert!(state.shell_checkbox_checked(LauncherCheckboxId::Tooltips));
        state.shell_mouse_down(150, 310, &layout);
        assert!(!state.shell_checkbox_checked(LauncherCheckboxId::Tooltips));
        assert_eq!(
            state.drain_output().events,
            [LauncherOptionsEvent::Cue(LauncherCue::Checkbox)]
        );
    }

    #[test]
    fn button_release_requires_the_pressed_button_and_popup_dismissal_consumes_click() {
        let mut state = state();
        let layout = LauncherOptionsLayout::new(800, 600);
        state.shell_mouse_down(700, 210, &layout);
        assert!(state.shell_button_pressed(LauncherParentResult::Keyboard));
        state.shell_mouse_up(700, 250, &layout);
        assert_eq!(state.drain_output().result, None);
        state.shell_mouse_down(520, 95, &layout);
        assert!(state.resolution_popup_open);
        state.shell_mouse_down(700, 550, &layout);
        state.shell_mouse_up(700, 550, &layout);
        assert!(!state.resolution_popup_open);
        let dismissed = state.drain_output();
        assert_eq!(dismissed.result, None);
        assert_eq!(
            dismissed.events,
            [
                LauncherOptionsEvent::Cue(LauncherCue::ComboOpen),
                LauncherOptionsEvent::Cue(LauncherCue::ComboClose)
            ]
        );
        state.shell_mouse_down(700, 550, &layout);
        let down = state.drain_output();
        assert_eq!(
            down.events,
            [LauncherOptionsEvent::Cue(LauncherCue::MainButton)]
        );
        assert_eq!(down.result, None);
        state.shell_mouse_up(700, 550, &layout);
        assert_eq!(
            state.drain_output().result,
            Some(LauncherParentResult::Back)
        );
    }

    #[test]
    fn resolution_selection_uses_the_painted_row_and_emits_immediate_pair() {
        let mut state = state();
        let layout = LauncherOptionsLayout::new(800, 600);
        state.shell_mouse_down(520, 95, &layout);
        state.drain_output();
        let popup = state.shell_popup(&layout).unwrap();
        let x = popup.rect.x + 10;
        let y = popup.rect.y + popup.row_height + 5;
        state.shell_mouse_move(x, y, &layout);
        assert_eq!(state.shell_popup(&layout).unwrap().hovered, Some(1));
        state.shell_mouse_down(x, y, &layout);
        assert_eq!(state.shell_selected_resolution_text(), "1024 x 768 x 16");
        assert!(state.shell_popup(&layout).is_none());
        assert_eq!(
            state.drain_output().events,
            [
                LauncherOptionsEvent::Cue(LauncherCue::ComboClose),
                LauncherOptionsEvent::ResolutionSelected {
                    width: 1024,
                    height: 768
                }
            ]
        );
    }

    #[test]
    fn resolution_popup_scroll_reaches_later_rows_without_selecting_under_scrollbar() {
        let mut state = state();
        state.resolution_rows = (0..8)
            .map(|index| LauncherResolutionRow::new(800 + index * 100, 600))
            .collect();
        let layout = LauncherOptionsLayout::new(800, 600);
        state.shell_mouse_down(520, 95, &layout);
        state.drain_output();
        let popup = state.shell_popup(&layout).unwrap();
        assert_eq!(popup.visible_rows, 5);
        assert_eq!(popup.rect.h, 115);
        let scrollbar = popup.scrollbar.unwrap();
        for _ in 0..3 {
            state.shell_mouse_down(scrollbar.x + 5, scrollbar.y + scrollbar.h - 5, &layout);
            state.shell_mouse_up(scrollbar.x + 5, scrollbar.y + scrollbar.h - 5, &layout);
        }
        let popup = state.shell_popup(&layout).unwrap();
        assert_eq!(popup.first_row, 3);
        assert!(state.drain_output().events.is_empty());
        state.shell_mouse_down(
            popup.content.x + 5,
            popup.content.y + 4 * popup.row_height + 5,
            &layout,
        );
        assert_eq!(state.shell_selected_resolution_text(), "1500 x 600 x 16");
        assert_eq!(
            state.drain_output().events,
            [
                LauncherOptionsEvent::Cue(LauncherCue::ComboClose),
                LauncherOptionsEvent::ResolutionSelected {
                    width: 1500,
                    height: 600
                }
            ]
        );
    }
}
