//! Scrollbar geometry of the skirmish shell's combo dropdowns (and the
//! Options resolution list, which shares their drop-down list): the thumb
//! height, its position and the pointer→top-row mapping for a track click or
//! a thumb drag. The shell list boxes use `ui::shell::list` instead.
//!
//! Depends only on `RectPx` and the scrollbar constants from `layout`; holds no
//! state, render, or UI dependency (pure integer pixel geometry).

use super::layout::{
    COMBO_DROPDOWN_SCROLLBAR_BUTTON_H, COMBO_DROPDOWN_SCROLLBAR_MIN_THUMB_H, DropdownMetrics,
    RectPx,
};

/// A combo dropdown's scroll model: rows visible at once are capped per
/// control (Side 7, Color/Start 9, AiType/Team unbounded). The thumb can be
/// dragged; the wheel does not move it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollModel {
    /// Rows shown at once; 0 is unbounded.
    cap: i32,
    /// The scrollbar's arrow-button height in the pixels the track is measured
    /// in: authored on the retail layout, stretched on a stretched page.
    button_h: i32,
    min_thumb_h: i32,
}

impl ScrollModel {
    /// `cap` is the per-combo max-visible (0 = unbounded), on the authored
    /// metrics.
    pub const fn combo(cap: i32) -> Self {
        Self {
            cap,
            button_h: COMBO_DROPDOWN_SCROLLBAR_BUTTON_H,
            min_thumb_h: COMBO_DROPDOWN_SCROLLBAR_MIN_THUMB_H,
        }
    }

    /// The same model on a stretched dropdown's device metrics.
    pub const fn combo_at(cap: i32, metrics: DropdownMetrics) -> Self {
        Self {
            cap,
            button_h: metrics.button_h,
            min_thumb_h: metrics.min_thumb_h,
        }
    }

    /// Visible-row count: `item_count.min(cap)`, or `item_count` unbounded.
    pub fn visible_rows(&self, item_count: usize) -> usize {
        if self.cap > 0 {
            item_count.min(self.cap as usize)
        } else {
            item_count
        }
    }

    /// `item_count − visible_rows`, saturating.
    pub fn max_top_index(&self, item_count: usize, visible_rows: usize) -> usize {
        item_count.saturating_sub(visible_rows)
    }

    /// Thumb height in pixels. `scrollbar_h` is the full track (scrollbar rect)
    /// height. An empty list takes the whole track (never reached: an empty
    /// combo shows no scrollbar).
    pub fn thumb_height(&self, visible_rows: usize, item_count: usize, scrollbar_h: i32) -> i32 {
        let track_h = (scrollbar_h - self.button_h * 2).max(1);
        if item_count == 0 || visible_rows == 0 {
            return track_h.max(self.min_thumb_h);
        }
        ((track_h * visible_rows as i32) / item_count as i32)
            .max(self.min_thumb_h)
            .min(track_h)
    }

    /// Thumb top-Y inside `scrollbar` for `top_index`. `thumb_h` from `thumb_height`;
    /// `max_top` from `max_top_index`.
    pub fn thumb_y(
        &self,
        scrollbar: RectPx,
        thumb_h: i32,
        top_index: usize,
        max_top: usize,
    ) -> i32 {
        let track_span = (scrollbar.h - self.button_h * 2 - thumb_h).max(1);
        scrollbar.y
            + self.button_h
            + if max_top == 0 {
                0
            } else {
                (track_span * top_index.min(max_top) as i32) / max_top as i32
            }
    }

    /// Pointer→top_index — the shared core for BOTH a track click and a thumb drag.
    /// `thumb_top_candidate` is `mouse_y − thumb_h/2` for a track click, or
    /// `mouse_y − grab_offset_y` for a drag.
    pub fn top_index_from_thumb_top(
        &self,
        scrollbar: RectPx,
        thumb_h: i32,
        max_top: usize,
        thumb_top_candidate: i32,
    ) -> usize {
        if max_top == 0 {
            return 0;
        }
        let track_span = (scrollbar.h - self.button_h * 2 - thumb_h).max(1);
        let thumb_top = thumb_top_candidate.clamp(
            scrollbar.y + self.button_h,
            scrollbar.y + scrollbar.h - self.button_h - thumb_h,
        );
        let local = thumb_top - scrollbar.y - self.button_h;
        ((local * max_top as i32 + track_span / 2) / track_span) as usize
    }
}
