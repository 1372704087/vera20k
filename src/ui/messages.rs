//! Chat/system message list model: a bounded list of chat rows over the
//! tactical viewport, arranged in one of two layouts ([`MessageLayout`]).
//! The native layout stacks rows downward from the view's top edge, composes
//! prefixed rows as `name: text` and gives the open edit line one message
//! slot below the last row. The VeraX Minecraft chat layout grows rows upward
//! from the bottom-left corner as `<name> text`, with the edit line owning a
//! full-width bottom input bar that takes no message slot. The row cap,
//! expiry, edit buffer and wrapping rules are shared.
//! Pure: text measurement is injected
//! (`measure: &dyn Fn(&str) -> i32`, pixels) so the model never touches the
//! renderer; deadlines are injected `now_ms` values. The app driver supplies
//! the advancing process wall clock. Offline pauses defer Manage; expired
//! rows disappear on the first poll after the pause.
//!
//! ## Dependency rules
//! - ui/ module: std only — no render/, assets/, audio/, net/.

/// Slot-pool cap (native list walks up to 0xE label slots).
pub const MESSAGE_SLOTS: usize = 14;
/// Per-slot text cap (162 wide chars incl. the terminator).
pub const MESSAGE_SLOT_CHARS: usize = 161;
/// Per-line char cap handed to the fitter (native cap 0x6F — contract §4.2 step 4).
pub const MESSAGE_LINE_FIT_CHARS: usize = 111;
/// Pixel pad subtracted from the width budget (native `budget = MaxWidth −
/// prefix_width − 8`).
pub const MESSAGE_WIDTH_PAD_PX: i32 = 8;
/// Row stride (native Init hardcodes 0x13 = 19 px — NOT the font height).
pub const MESSAGE_LINE_HEIGHT_PX: i32 = 19;
/// Prefix separator in the Minecraft chat layout, which composes `<name> text`.
pub const MESSAGE_PREFIX_SEPARATOR: &str = " ";
/// Prefix separator in the native layout, which composes `name: text` with a
/// space after the colon (gamemd's LAN chat and WOL backend both show this
/// spacing; the CSF `TXT_FROM` template drives the edit prompt, not the line).
pub const MESSAGE_PREFIX_SEPARATOR_NATIVE: &str = ": ";
/// Minecraft chat presentation: rows sit on the bottom-left of the tactical
/// viewport, this many pixels above its bottom edge.
pub const MESSAGE_BOTTOM_INSET_PX: i32 = 2;
/// The edit line's full-width input bar (MC: 12 px around an 8 px font).
pub const MESSAGE_INPUT_BAR_HEIGHT_PX: i32 = 20;
/// Vertical inset of the typed text inside the input bar.
pub const MESSAGE_INPUT_TEXT_INSET_PX: i32 = 2;
/// Width of the ChatBox scrollbar strip at the right edge of the view.
pub const MESSAGE_SCROLLBAR_WIDTH_PX: i32 = 3;
/// The scrollbar thumb never shrinks below this height, so a long history
/// still shows a grabbable, visible marker.
pub const MESSAGE_SCROLLBAR_MIN_THUMB_PX: i32 = 16;
/// Inset of the scrollbar strip from the view's right edge.
pub const MESSAGE_SCROLLBAR_INSET_PX: i32 = 4;
/// Width of the right-click menu box.
pub const MESSAGE_MENU_WIDTH_PX: i32 = 132;
/// Height of one menu row, including its hover band.
pub const MESSAGE_MENU_ROW_HEIGHT_PX: i32 = 18;
/// Vertical padding above the first row and below the last.
pub const MESSAGE_MENU_PADDING_PX: i32 = 3;
/// Height of the unread accent line drawn at the bottom of the history area.
pub const MESSAGE_UNREAD_MARKER_PX: i32 = 2;
/// The prompt the open edit line starts with. It is the edit prefix, so
/// Backspace still stops right after it.
pub const MESSAGE_INPUT_PREFIX: &str = "> ";
/// Each row's backing box starts one pixel above its text so consecutive
/// boxes abut, as the MC chat strip does.
pub const MESSAGE_ROW_BOX_INSET_PX: i32 = 1;
/// Retail visible-message cap (Init maxMsg=6; clamped ≤ MESSAGE_SLOTS).
pub const MESSAGE_MAX_VISIBLE_RETAIL: usize = 6;
/// Minecraft-chat history cap. The native layout is bounded by the TextLabel
/// slot pool (`MESSAGE_SLOTS`); the chat box keeps a longer scroll-back so a
/// reopened box still shows recently sent lines. Chat rows never expire in
/// this layout, so the cap is the only bound on list growth.
pub const MESSAGE_MAX_VISIBLE_CHATBOX: usize = 100;
/// Minecraft chat shows at most this many rows at once; older lines stay
/// reachable by scrolling even when the viewport itself could fit more.
const CHATBOX_MAX_VISIBLE_ROWS: usize = 20;
/// Share of the view width a history row's backing strip covers, anchored
/// at the left edge. The scrollbar hugs the strip's right edge.
pub const CHATBOX_ROW_WIDTH_FRACTION: f32 = 2.0 / 3.0;
/// Read_Scenario's Init MaxChars `0x62` (`0x00684ABB`, clamp `0x70`): the
/// edit holds at most `MaxChars − 1` typed units.
const MESSAGE_EDIT_MAX_CHARS: usize = 0x62;
/// Init's OverflowStart `0x14` and OverflowEnd `0x62`, which Init clamps to
/// `MaxChars − 1` (`0x005D3A60`).
const MESSAGE_OVERFLOW_START: usize = 0x14;
const MESSAGE_OVERFLOW_END: usize = MESSAGE_EDIT_MAX_CHARS - 1;
/// Input's width margin: a unit stays only while the whole edit line is
/// narrower than the width limit minus 10 (`0x005D4510`).
const MESSAGE_EDIT_WIDTH_MARGIN_PX: i32 = 10;
/// Linear fade-out window at the end of a row's visible lifetime. A ChatBox
/// row interpolates 1.0→0.0 over the last MESSAGE_FADE_MS of its visible
/// window; native gamemd has no fade (hard cutoff). VeraX-only addition.
pub const MESSAGE_FADE_MS: u64 = 500;

/// On-screen arrangement of the rows and the edit line.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MessageLayout {
    /// Native MessageList: rows stack downward from the view's top edge, and
    /// the open edit line takes one message slot (`0x005D3CAA..0x005D3CFF`)
    /// below the last row.
    #[default]
    Native,
    /// VeraX Minecraft chat: rows grow upward from the viewport's bottom
    /// edge, and the edit line owns a full-width bottom input bar that takes
    /// no message slot.
    ChatBox,
}

/// One live message row.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub text: String,
    pub rgb: [f32; 3],
    /// Byte length of the sender tag at the head of `text` (`name` plus
    /// its separator). The chat box paints that head in the sender's
    /// colour and the rest in white, and a mention highlight never
    /// covers it.
    pub prefix_bytes: usize,
    /// None = never visually expires (native timeout −1 → deadline 0).
    pub visible_deadline_ms: Option<u64>,
    /// None = never removed from the retained list; typically ≥ visible_deadline_ms
    /// so the row can keep fading out before eviction.
    pub retain_deadline_ms: Option<u64>,
    /// Screen Y, restacked after every insert/expiry.
    pub y: i32,
    /// TextLabel+41/+44/+48. The list owns this state through insertion,
    /// painting, eviction and expiry; renderers only read the current count.
    reveal: Option<MessageReveal>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct MessageReveal {
    count: u32,
    last_wall_ms: u32,
}

impl Message {
    /// The edit line as a row to draw. Its reveal flag is never set — the
    /// chat cursor character 0 draws no caret, and the edit has no deadline.
    pub fn edit_row(edit: &EditLine) -> Self {
        Self {
            text: edit.text.clone(),
            rgb: edit.rgb,
            prefix_bytes: 0,
            visible_deadline_ms: None,
            retain_deadline_ms: None,
            y: edit.y,
            reveal: None,
        }
    }

    /// Original434500: zero draws the whole string; a positive count draws
    /// that many UTF-16 units, with a trailing eight-unit highlight.
    pub fn reveal_count(&self) -> u32 {
        self.reveal.map_or(0, |reveal| reveal.count)
    }

    /// True while the row should still be drawn (visible deadline not yet passed).
    /// `None` visible deadline means the row never visually expires.
    pub fn is_visible(&self, now_ms: u64) -> bool {
        self.visible_deadline_ms
            .map_or(true, |deadline| now_ms <= deadline)
    }

    /// Linear fade from 1.0 → 0.0 over the last MESSAGE_FADE_MS of the
    /// visible window. Returns 1.0 while ≥MESSAGE_FADE_MS remain, 0.0 once
    /// past the visible deadline, interpolated in between. A None visible
    /// deadline means the row never fades.
    pub fn alpha_at(&self, now_ms: u64) -> f32 {
        let Some(deadline) = self.visible_deadline_ms else {
            return 1.0;
        };
        if now_ms >= deadline {
            return 0.0;
        }
        let fade_start = deadline.saturating_sub(MESSAGE_FADE_MS);
        if now_ms <= fade_start {
            return 1.0;
        }
        let t = (now_ms - fade_start) as f32 / MESSAGE_FADE_MS as f32;
        (1.0 - t).clamp(0.0, 1.0)
    }
}

/// Outcome of an `add_message` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddOutcome {
    /// Number of rows added (≥1 when the text wrapped).
    pub added: usize,
    /// True when the caller should play the insert sound (top-level
    /// non-silent adds only; wrapped continuation rows are always silent).
    pub play_sound: bool,
}

/// One `add_message` request — the per-message inputs grouped so the call
/// signature stays small (the clock and measurer are passed separately
/// because they belong to the caller, not the message).
#[derive(Debug, Clone, Copy)]
pub struct MessagePost<'a> {
    /// Optional sender prefix, composed with the layout's separator before
    /// the text: native `name: text`, Minecraft `<name> text`.
    pub prefix: Option<&'a str>,
    pub text: &'a str,
    pub rgb: [f32; 3],
    /// None = never visually expires (native timeout −1 → deadline 0).
    pub visible_timeout_ms: Option<u64>,
    /// None = never removed from the retained list; typically ≥ visible_timeout_ms
    /// so the row can keep fading out before eviction.
    pub retain_timeout_ms: Option<u64>,
    /// Silent adds neither request the insert sound nor enable typing reveal
    /// (original5D3D95..5D3DBD); see [`MessageList::set_typing`].
    pub silent: bool,
}

/// The live edit line (MessageList `+0x19` editing, `+0x24` label,
/// `+0x28` buffer, `+0x2B0` Index, `+0x2B4` Start). The edit sits on the
/// layout's input row: the native slot below the last message row, or the
/// Minecraft chat's bottom input bar.
#[derive(Debug, Clone, PartialEq)]
struct MessageEdit {
    /// The prompt, then the typed UTF-16 units. Index is its length.
    buffer: Vec<u16>,
    /// Start: the prompt length, which Backspace never crosses.
    start: usize,
    rgb: [f32; 3],
    /// `+0x18`: split an overfull line instead of refusing the unit.
    overflow: bool,
}

/// One key as Input `0x005D4510` decodes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKey {
    /// A character unit (WM_CHAR, or End's mapped `\`).
    Unit(u16),
    /// The mapped `8` of a WM_CHAR backspace or of Home; a raw VK_BACK
    /// deletes nothing.
    Delete,
    Enter,
    Escape,
    /// Right mouse button: ends the edit; the game still sees the click.
    RightClick,
    /// ChatBox: scroll history up by one page (PageUp).
    PageUp,
    /// ChatBox: scroll history down by one page (PageDown).
    PageDown,
    /// ChatBox: scroll to the oldest message (Home).
    ScrollTop,
    /// ChatBox: scroll to the newest message (End).
    ScrollBottom,
    /// ChatBox: recall the previous sent message (ArrowUp).
    HistoryUp,
    /// ChatBox: recall the next sent message (ArrowDown).
    HistoryDown,
    /// Every other key: consumed, no effect.
    Other,
}

/// Input's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditInput {
    /// 0: nothing changed (also every key while no edit is open).
    Unchanged,
    /// 1: one unit appended.
    Typed,
    /// 2: the last typed unit removed.
    Deleted,
    /// 2: the edit ended with nothing to send.
    Ended,
    /// 3: Enter ended the edit; the typed text with its appended space.
    Sent(String),
    /// 4: the line overflowed. This chunk is cut off and sent; the edit
    /// stays open with the rest and the new unit.
    Overflowed(String),
}

/// The edit line as drawn: the prompt plus typed text on the input row.
#[derive(Debug, Clone, PartialEq)]
pub struct EditLine {
    pub text: String,
    pub rgb: [f32; 3],
    pub y: i32,
}

#[derive(Debug)]
pub struct MessageList {
    x: i32,
    y: i32,
    /// Tactical viewport height; the Minecraft chat layout grows its rows
    /// upward from the bottom edge. Unused by the native layout.
    view_height: i32,
    max_visible: usize,
    max_width_px: i32,
    messages: Vec<Message>,
    edit: Option<MessageEdit>,
    layout: MessageLayout,
    /// Add_Message's per-mode reveal gate: LANScrollText `[0xA8D1F8]` in
    /// GameMode 3, WOLScrollText `[0xA8D1F9]` in 4, always on otherwise.
    typing: bool,
    /// ChatBox-only: how many rows the view is scrolled up from the bottom.
    /// 0 = pinned to newest; higher values reveal older history. Native
    /// layout ignores this (rows are always top-down from the view top).
    scroll_offset: usize,
    /// Sent-message history for ArrowUp/ArrowDown recall (ChatBox only).
    /// Newest entry is last.
    history: Vec<String>,
    /// Current position when browsing history with arrows; None = typing
    /// fresh text (or no history browsed yet).
    history_index: Option<usize>,
    /// ChatBox-only: a row arrived while the view was scrolled up, so the
    /// user has not seen everything. Cleared when the view returns to the
    /// newest row or the chat opens.
    has_unread: bool,
    /// ChatBox-only: the chat-history text selection. (anchor, focus) are
    /// kept in click order; [MessageList::selection] returns them sorted.
    selection: Option<(ChatPoint, ChatPoint)>,
    /// True while the left button drags the selection focus.
    selecting: bool,
    /// True while the scrollbar thumb is being dragged.
    dragging_scrollbar: bool,
    /// Multi-click streak state: how many clicks landed on the same point in
    /// quick succession, and when/where the last one was.
    click_streak: u8,
    last_click_ms: u64,
    last_click_point: Option<ChatPoint>,
    /// Pixels between the pointer and the thumb's top edge when the drag
    /// started, so grabbing mid-thumb does not snap it.
    scroll_grab: i32,
    /// Active history search. While set, typing edits the query instead of
    /// the message and rows that do not match are drawn dimmed.
    search: Option<String>,
    /// Open right-click menu, if any.
    menu: Option<ChatMenu>,
}

/// One action of the chat history's right-click menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatMenuAction {
    Copy,
    OpenLink,
}

/// The chat history's right-click menu: two fixed rows anchored at the click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMenu {
    pub x: i32,
    pub y: i32,
    /// Row 0 is Copy, enabled while a selection exists.
    pub copy_enabled: bool,
    /// The link under the click; row 1 is enabled only when this is `Some`.
    pub link: Option<String>,
    /// Row under the pointer, for the hover highlight.
    pub hover: Option<usize>,
}

impl ChatMenu {
    pub fn height(&self) -> i32 {
        MESSAGE_MENU_PADDING_PX * 2 + MESSAGE_MENU_ROW_HEIGHT_PX * 2
    }

    fn row_at(&self, x: i32, y: i32) -> Option<usize> {
        if x < self.x || x >= self.x + MESSAGE_MENU_WIDTH_PX {
            return None;
        }
        if y < self.y + MESSAGE_MENU_PADDING_PX
            || y >= self.y + MESSAGE_MENU_PADDING_PX + MESSAGE_MENU_ROW_HEIGHT_PX * 2
        {
            return None;
        }
        Some(((y - self.y - MESSAGE_MENU_PADDING_PX) / MESSAGE_MENU_ROW_HEIGHT_PX) as usize)
    }

    pub fn row_enabled(&self, row: usize) -> bool {
        match row {
            0 => self.copy_enabled,
            1 => self.link.is_some(),
            _ => false,
        }
    }
}
/// A character position inside the retained chat history: `row` indexes
/// [MessageList::messages] and char_index counts Unicode scalars from the
/// row start (the same units the bit font measures).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChatPoint {
    pub row: usize,
    pub char_index: usize,
}

/// ChatBox scrollbar geometry, in screen pixels. Rendered only while the
/// chat input is open and the history overflows the visible window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollBarGeometry {
    /// Left edge of the strip.
    pub x: i32,
    pub track_y: i32,
    pub track_height: i32,
    pub thumb_y: i32,
    pub thumb_height: i32,
}

impl MessageList {
    pub fn new(x: i32, y: i32, max_visible: usize, max_width_px: i32) -> Self {
        Self {
            x,
            y,
            view_height: 0,
            max_visible: max_visible.min(MESSAGE_SLOTS),
            max_width_px,
            messages: Vec::with_capacity(MESSAGE_SLOTS),
            edit: None,
            layout: MessageLayout::default(),
            typing: true,
            scroll_offset: 0,
            history: Vec::new(),
            history_index: None,
            has_unread: false,
            selection: None,
            selecting: false,
            dragging_scrollbar: false,
            click_streak: 0,
            last_click_ms: 0,
            last_click_point: None,
            scroll_grab: 0,
            search: None,
            menu: None,
        }
    }

    /// The current on-screen arrangement.
    pub fn layout(&self) -> MessageLayout {
        self.layout
    }

    /// Switch the on-screen arrangement; a change restacks the rows.
    pub fn set_layout(&mut self, layout: MessageLayout) {
        if self.layout != layout {
            self.layout = layout;
            self.restack();
        }
    }

    /// Cap on retained rows. The native layout is bounded by the TextLabel
    /// slot pool (`MESSAGE_SLOTS`); the chat box may keep a longer history.
    pub fn set_max_visible(&mut self, max_visible: usize) {
        self.max_visible = match self.layout {
            MessageLayout::Native => max_visible.min(MESSAGE_SLOTS),
            MessageLayout::ChatBox => max_visible,
        };
    }

    /// How many message rows fit in the ChatBox viewport above the input bar.
    /// Returns 0 when the viewport is not sized yet or the layout is Native.
    pub fn visible_row_count(&self) -> usize {
        if self.layout != MessageLayout::ChatBox || self.view_height <= 0 {
            return 0;
        }
        // The input bar and the blank clearance row above it are not history
        // rows, so they do not count toward the window.
        let reserved = MESSAGE_INPUT_BAR_HEIGHT_PX
            + if self.edit.is_some() {
                MESSAGE_LINE_HEIGHT_PX
            } else {
                0
            };
        let available = self.view_height - reserved - MESSAGE_BOTTOM_INSET_PX;
        let fits = (available / MESSAGE_LINE_HEIGHT_PX).max(0) as usize;
        fits.min(CHATBOX_MAX_VISIBLE_ROWS)
    }

    /// The maximum valid scroll offset: the number of rows beyond the
    /// visible window that can be scrolled into view. 0 when everything fits.
    pub fn max_scroll_offset(&self) -> usize {
        let visible = self.visible_row_count();
        self.messages.len().saturating_sub(visible)
    }

    /// Scroll the ChatBox history. lines > 0 scrolls up (older messages),
    /// lines < 0 scrolls down (newer). Clamped to [0, max_scroll_offset].
    /// No-op for the Native layout.
    pub fn scroll(&mut self, lines: i32) {
        if self.layout != MessageLayout::ChatBox {
            return;
        }
        let max = self.max_scroll_offset() as i64;
        let next = (self.scroll_offset as i64) + (lines as i64);
        let clamped = next.clamp(0, max) as usize;
        let changed = clamped != self.scroll_offset;
        self.scroll_offset = clamped;
        if self.scroll_offset == 0 {
            self.has_unread = false;
        }
        // Row y positions carry the scroll shift; refresh them so the newly
        // revealed window is painted where it is read.
        if changed {
            self.restack();
        }
    }

    /// ChatBox-only: a row arrived while the view was scrolled up.
    pub fn has_unread(&self) -> bool {
        self.has_unread
    }

    /// Scrollbar geometry for the open ChatBox, or None when the whole
    /// history fits in the viewport (or the layout is Native / input closed).
    pub fn scrollbar_geometry(&self) -> Option<ScrollBarGeometry> {
        if self.layout != MessageLayout::ChatBox || self.edit.is_none() {
            return None;
        }
        if self.search_active() {
            return None;
        }
        let max = self.max_scroll_offset();
        if max == 0 {
            return None;
        }
        let visible = self.visible_row_count() as i32;
        // The track spans exactly the visible history rows, so it starts
        // and ends with the painted row band, not the whole viewport.
        let track_height = visible * MESSAGE_LINE_HEIGHT_PX;
        let track_y = self.history_bottom() - track_height;
        if track_height <= 0 {
            return None;
        }
        let total = self.messages.len() as i32;
        let thumb_height = ((track_height * visible) / total)
            .max(MESSAGE_SCROLLBAR_MIN_THUMB_PX)
            .min(track_height);
        // start = how many rows are hidden above the window; 0 is the top.
        let start = max.saturating_sub(self.scroll_offset) as i32;
        let travel = track_height - thumb_height;
        let thumb_y = track_y + (travel * start) / max as i32;
        Some(ScrollBarGeometry {
            x: self.x - 1 + (self.max_width_px as f32 * CHATBOX_ROW_WIDTH_FRACTION).round() as i32
                - MESSAGE_SCROLLBAR_INSET_PX
                - MESSAGE_SCROLLBAR_WIDTH_PX,
            track_y,
            track_height,
            thumb_y,
            thumb_height,
        })
    }

    /// Screen y of the unread accent line, drawn just above the input bar,
    /// or None when nothing is unread.
    pub fn unread_marker_y(&self) -> Option<i32> {
        if self.layout != MessageLayout::ChatBox
            || self.edit.is_none()
            || !self.has_unread
            || self.scroll_offset == 0
            || self.search_active()
        {
            return None;
        }
        Some(
            self.y + self.view_height
                - MESSAGE_BOTTOM_INSET_PX
                - MESSAGE_INPUT_BAR_HEIGHT_PX
                - MESSAGE_UNREAD_MARKER_PX,
        )
    }

    /// Current scroll offset (ChatBox only; 0 otherwise).
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Width of the message view in pixels (the chat chrome spans it).
    pub fn view_width(&self) -> i32 {
        self.max_width_px
    }
    /// Screen y of the history area's bottom edge: above the input bar while
    /// the edit is open, at the view bottom otherwise.
    fn history_bottom(&self) -> i32 {
        let bar = if self.edit.is_some() {
            MESSAGE_INPUT_BAR_HEIGHT_PX + MESSAGE_LINE_HEIGHT_PX
        } else {
            0
        };
        self.y + self.view_height - MESSAGE_BOTTOM_INSET_PX - bar
    }

    /// The retained row under a screen point, or None outside the history
    /// strip. Chat rows are 19 px apart and stack upward from the history
    /// bottom. Both axes are bounded: a sidebar click aligned with a chat row
    /// must not start a history selection. The projection is
    /// [`Self::display_rows`]: with a search open the matches compact
    /// bottom-up, so hit-testing the unfiltered window would resolve a click
    /// to a hidden row.
    pub fn row_at(&self, x: i32, y: i32) -> Option<usize> {
        if self.layout != MessageLayout::ChatBox {
            return None;
        }
        if x < self.x || x >= self.x + self.max_width_px {
            return None;
        }
        let bottom = self.history_bottom();
        if y < self.y || y >= bottom {
            return None;
        }
        let from_bottom = ((bottom - y) / MESSAGE_LINE_HEIGHT_PX) as usize;
        let rows = self.display_rows();
        rows.len()
            .checked_sub(from_bottom + 1)
            .and_then(|slot| rows.get(slot).copied())
    }

    /// Start a chat-history selection at point (left button press).
    pub fn begin_selection(&mut self, point: ChatPoint) {
        self.selection = Some((point, point));
        self.selecting = true;
    }

    /// Drag the selection focus; the anchor stays where the press landed.
    pub fn extend_selection(&mut self, point: ChatPoint) {
        if !self.selecting {
            return;
        }
        if let Some((anchor, _)) = self.selection {
            self.selection = Some((anchor, point));
        }
    }

    /// Left button release: the selection stays, the drag ends.
    pub fn end_selection(&mut self) {
        self.selecting = false;
    }

    /// True while a drag is extending the selection.
    pub fn is_selecting(&self) -> bool {
        self.selecting
    }
    /// The scrollbar strip under a screen point, widened by 2 px on each side
    /// so the 3 px strip stays easy to grab.
    pub fn scrollbar_hit(&self, x: i32, y: i32) -> bool {
        let Some(bar) = self.scrollbar_geometry() else {
            return false;
        };
        x >= bar.x - 2
            && x < bar.x + MESSAGE_SCROLLBAR_WIDTH_PX + 2
            && y >= bar.track_y
            && y < bar.track_y + bar.track_height
    }

    /// Grab the scrollbar thumb at `pointer_y`. False when there is nothing
    /// to scroll. The pointer keeps its offset inside the thumb, so grabbing
    /// the middle does not snap the thumb under the cursor.
    pub fn begin_scroll_drag(&mut self, pointer_y: i32) -> bool {
        let Some(bar) = self.scrollbar_geometry() else {
            return false;
        };
        self.dragging_scrollbar = true;
        self.scroll_grab = (pointer_y - bar.thumb_y).clamp(0, bar.thumb_height);
        true
    }

    /// Move the thumb so its centre follows `y`, converting the position back
    /// into a scroll offset. The thumb's travel maps onto the whole hidden
    /// history.
    pub fn drag_scroll_to(&mut self, y: i32) {
        if !self.dragging_scrollbar {
            return;
        }
        let Some(bar) = self.scrollbar_geometry() else {
            return;
        };
        let max = self.max_scroll_offset();
        if max == 0 {
            return;
        }
        let travel = bar.track_height - bar.thumb_height;
        if travel <= 0 {
            self.scroll_offset = 0;
            self.has_unread = false;
            return;
        }
        // The thumb keeps the grab offset the press established.
        let thumb_top = y - self.scroll_grab;
        let clamped = thumb_top.clamp(bar.track_y, bar.track_y + travel);
        let start = ((clamped - bar.track_y) as i64 * max as i64) / travel as i64;
        // start counts rows hidden above the window; offset counts them below.
        let offset = max as i64 - start;
        let next_offset = offset.clamp(0, max as i64) as usize;
        let changed = next_offset != self.scroll_offset;
        self.scroll_offset = next_offset;
        if self.scroll_offset == 0 {
            self.has_unread = false;
        }
        if changed {
            self.restack();
        }
    }

    pub fn is_dragging_scrollbar(&self) -> bool {
        self.dragging_scrollbar
    }
    /// Open the history search. Typing then edits the query instead of the
    /// message; Esc searches again from the message line.
    pub fn begin_search(&mut self) {
        if self.layout != MessageLayout::ChatBox || self.edit.is_none() {
            return;
        }
        self.search = Some(String::new());
        self.restack();
    }

    pub fn search_query(&self) -> Option<&str> {
        self.search.as_deref()
    }

    /// Append one query character (or drop the last on `Delete`).
    pub fn search_input(&mut self, unit: u16, delete: bool) {
        let Some(query) = self.search.as_mut() else {
            return;
        };
        if delete {
            query.pop();
        } else if unit >= 0x20 {
            query.push(char::from_u32(u32::from(unit)).unwrap_or(' '));
        }
        self.restack();
    }

    pub fn end_search(&mut self) {
        self.search = None;
        self.restack();
    }
    /// True while a non-empty query filters the history.
    pub fn search_active(&self) -> bool {
        self.search.as_ref().is_some_and(|query| !query.is_empty())
    }

    /// Matching rows that fit the history area, oldest first. While searching
    /// the stack holds only matches, so this is what `restack` positions and
    /// what the render pass paints.
    fn search_rows(&self) -> Vec<usize> {
        let capacity = ((self.history_bottom() - self.y) / MESSAGE_LINE_HEIGHT_PX).max(0) as usize;
        let matches: Vec<usize> = (0..self.messages.len())
            .filter(|&row| self.row_matches_search(row))
            .collect();
        let skip = matches.len().saturating_sub(capacity);
        matches.into_iter().skip(skip).collect()
    }

    /// Anchor the right-click menu at `x`/`y` and place it inside the view.
    pub fn open_menu(&mut self, x: i32, y: i32, copy_enabled: bool, link: Option<String>) {
        let max_x = (self.x + self.max_width_px - MESSAGE_MENU_WIDTH_PX).max(self.x);
        let max_y = (self.y + self.view_height
            - MESSAGE_MENU_PADDING_PX * 2
            - MESSAGE_MENU_ROW_HEIGHT_PX * 2)
            .max(self.y);
        self.menu = Some(ChatMenu {
            x: x.clamp(self.x, max_x),
            y: y.clamp(self.y, max_y),
            copy_enabled,
            link,
            hover: None,
        });
    }

    pub fn menu(&self) -> Option<&ChatMenu> {
        self.menu.as_ref()
    }

    /// The enabled action under a screen point, if any.
    pub fn menu_action_at(&self, x: i32, y: i32) -> Option<ChatMenuAction> {
        let menu = self.menu.as_ref()?;
        let row = menu.row_at(x, y)?;
        if !menu.row_enabled(row) {
            return None;
        }
        Some(match row {
            0 => ChatMenuAction::Copy,
            _ => ChatMenuAction::OpenLink,
        })
    }

    /// Track the hovered row for the highlight band.
    pub fn set_menu_hover(&mut self, x: i32, y: i32) {
        let Some(menu) = self.menu.as_mut() else {
            return;
        };
        menu.hover = menu.row_at(x, y).filter(|&row| menu.row_enabled(row));
    }

    pub fn close_menu(&mut self) {
        self.menu = None;
    }
    /// The rows the render pass paints, oldest first. With a search open these
    /// are the matching rows; otherwise the visible scroll window.
    pub fn display_rows(&self) -> Vec<usize> {
        if self.search_active() {
            return self.search_rows();
        }
        let (start, end) = self.visible_message_range();
        (start..end).collect()
    }

    /// Whether a row contains the active query (case-insensitive). Always true
    /// when no search is open.
    pub fn row_matches_search(&self, row: usize) -> bool {
        let Some(query) = self.search_query() else {
            return true;
        };
        if query.is_empty() {
            return true;
        }
        self.messages
            .get(row)
            .is_some_and(|message| message.text.to_lowercase().contains(&query.to_lowercase()))
    }

    pub fn end_scroll_drag(&mut self) {
        self.dragging_scrollbar = false;
    }

    /// A left press on a history character. Repeated presses on the same
    /// point within 400 ms walk the double-click (word) and triple-click
    /// (whole row) selection.
    pub fn press_at(&mut self, point: ChatPoint, now_ms: u64) {
        let repeated = self.last_click_point == Some(point)
            && now_ms.saturating_sub(self.last_click_ms) <= 400
            && now_ms >= self.last_click_ms;
        self.click_streak = if repeated {
            self.click_streak.saturating_add(1).min(3)
        } else {
            1
        };
        self.last_click_ms = now_ms;
        self.last_click_point = Some(point);
        match self.click_streak {
            2 => {
                let (start, end) = self.word_bounds(point);
                self.selection = Some((
                    ChatPoint {
                        row: point.row,
                        char_index: start,
                    },
                    ChatPoint {
                        row: point.row,
                        char_index: end,
                    },
                ));
                self.selecting = true;
            }
            3 => {
                let len = self
                    .messages
                    .get(point.row)
                    .map(|m| m.text.chars().count())
                    .unwrap_or(0);
                self.selection = Some((
                    ChatPoint {
                        row: point.row,
                        char_index: 0,
                    },
                    ChatPoint {
                        row: point.row,
                        char_index: len,
                    },
                ));
                self.selecting = true;
            }
            _ => self.begin_selection(point),
        }
    }

    /// The whitespace-delimited word around `point` in its row.
    fn word_bounds(&self, point: ChatPoint) -> (usize, usize) {
        let Some(message) = self.messages.get(point.row) else {
            return (point.char_index, point.char_index);
        };
        let chars: Vec<char> = message.text.chars().collect();
        let at = point.char_index.min(chars.len());
        let mut start = at;
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        let mut end = at;
        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }
        // Clicking the space after a word selects that word.
        if start == end && start > 0 {
            return self.word_bounds(ChatPoint {
                row: point.row,
                char_index: start - 1,
            });
        }
        (start, end)
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.selecting = false;
    }

    /// The selection as a sorted (start, end) pair; None when empty.
    pub fn selection(&self) -> Option<(ChatPoint, ChatPoint)> {
        let (a, b) = self.selection?;
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    /// The selected history text, one line per row, or None when nothing is
    /// selected. Rows are joined with '\n', which is what a chat log paste
    /// expects; a selection ending at char 0 includes only the rows above it.
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection()?;
        let mut lines = Vec::new();
        for row in start.row..=end.row.min(self.messages.len().saturating_sub(1)) {
            let Some(message) = self.messages.get(row) else {
                continue;
            };
            let chars: Vec<char> = message.text.chars().collect();
            let from = if row == start.row {
                start.char_index.min(chars.len())
            } else {
                0
            };
            let to = if row == end.row {
                end.char_index.min(chars.len())
            } else {
                chars.len()
            };
            if to > from {
                lines.push(chars[from..to].iter().collect::<String>());
            }
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    /// The index range [start, end) of messages visible in the ChatBox
    /// viewport given the current scroll offset. start is clamped so the
    /// newest rows stay pinned to the bottom when not scrolled.
    pub fn visible_message_range(&self) -> (usize, usize) {
        let total = self.messages.len();
        let visible = self.visible_row_count();
        if visible == 0 || total == 0 {
            return (0, 0);
        }
        let end = total.saturating_sub(self.scroll_offset);
        let start = end.saturating_sub(visible);
        (start, end)
    }

    /// Re-anchor on viewport/layout change (the native viewport re-init,
    /// contract §4.1): x = tactical_x + 3, y = tactical_y, width = tactical_w − 14.
    /// `view_height` is the tactical viewport height the Minecraft chat
    /// stacks its rows against.
    /// A width change is Set_View_Dimensions' Init (`0x004A8BB8`), which
    /// drops a live edit without a message.
    /// RESIDUAL: that Init also deletes every row; VERA keeps them.
    pub fn set_view(&mut self, x: i32, y: i32, max_width_px: i32, view_height: i32) {
        if max_width_px != self.max_width_px && self.edit.is_some() {
            self.end_edit();
        }
        self.x = x;
        self.y = y;
        self.max_width_px = max_width_px;
        self.view_height = view_height;
        self.restack();
    }

    /// The mode's reveal gate for later non-silent adds.
    pub fn set_typing(&mut self, typing: bool) {
        self.typing = typing;
    }

    /// `+0x19`: an edit line is open.
    pub fn editing(&self) -> bool {
        self.edit.is_some()
    }

    pub fn edit_line(&self) -> Option<EditLine> {
        self.edit.as_ref().map(|edit| EditLine {
            text: String::from_utf16_lossy(&edit.buffer),
            rgb: edit.rgb,
            y: match self.layout {
                // The edit's message slot sits below the last row.
                MessageLayout::Native => {
                    self.y + self.messages.len() as i32 * MESSAGE_LINE_HEIGHT_PX
                }
                MessageLayout::ChatBox => self.input_row_y(),
            },
        })
    }

    /// Minecraft chat layout: text Y of the input row inside the bottom bar.
    fn input_row_y(&self) -> i32 {
        self.y + self.view_height - MESSAGE_BOTTOM_INSET_PX - MESSAGE_INPUT_BAR_HEIGHT_PX
            + MESSAGE_INPUT_TEXT_INSET_PX
    }

    /// AddEdit `0x005D4210`: refused while an edit is open. Opens the edit
    /// with `prefix`. The native layout's edit takes one message slot, so
    /// opening it at the cap evicts the oldest row; the Minecraft input bar
    /// occupies its own bottom row, so no row is evicted or hidden while
    /// typing. The caller sets the overflow byte `+0x18`.
    pub fn add_edit(&mut self, prefix: &str, rgb: [f32; 3], overflow: bool) -> bool {
        if self.edit.is_some() {
            return false;
        }
        if self.layout == MessageLayout::Native
            && self.max_visible > 0
            && self.messages.len() + 1 > self.max_visible
        {
            self.messages.remove(0);
        }
        let buffer: Vec<u16> = prefix.encode_utf16().collect();
        self.edit = Some(MessageEdit {
            start: buffer.len(),
            buffer,
            rgb,
            overflow,
        });
        self.scroll_offset = 0;
        self.history_index = None;
        self.has_unread = false;
        self.clear_selection();
        self.close_menu();
        self.restack();
        true
    }

    /// Input `0x005D4510` for one decoded key.
    pub fn input(&mut self, key: EditKey, measure: &dyn Fn(&str) -> i32) -> EditInput {
        let limit = self.max_width_px - MESSAGE_EDIT_WIDTH_MARGIN_PX;
        let Some(edit) = self.edit.as_mut() else {
            return EditInput::Unchanged;
        };
        match key {
            EditKey::RightClick | EditKey::Escape => {
                self.end_edit();
                EditInput::Ended
            }
            EditKey::Delete => {
                self.history_index = None;
                if edit.buffer.len() > edit.start {
                    edit.buffer.pop();
                    EditInput::Deleted
                } else {
                    EditInput::Unchanged
                }
            }
            EditKey::Enter => {
                let typed = edit.buffer.len() - edit.start;
                if typed == 0 {
                    self.end_edit();
                    return EditInput::Ended;
                }
                if typed < MESSAGE_EDIT_MAX_CHARS - 1 {
                    edit.buffer.push(u16::from(b' '));
                }
                let text = String::from_utf16_lossy(&edit.buffer[edit.start..]);
                self.history.push(text.trim_end().to_string());
                self.history_index = None;
                self.end_edit();
                EditInput::Sent(text)
            }
            EditKey::Unit(unit) if unit >= 0x20 => {
                self.history_index = None;
                if edit.buffer.len() - edit.start < MESSAGE_EDIT_MAX_CHARS - 1 {
                    edit.buffer.push(unit);
                    if measure(&String::from_utf16_lossy(&edit.buffer)) < limit {
                        return EditInput::Typed;
                    }
                    edit.buffer.pop();
                }
                if !edit.overflow {
                    return EditInput::Unchanged;
                }
                let typed = &edit.buffer[edit.start..];
                let cut = trim_message_cut(typed, MESSAGE_OVERFLOW_START, MESSAGE_OVERFLOW_END);
                let chunk = String::from_utf16_lossy(&typed[..cut]);
                edit.buffer.drain(edit.start..edit.start + cut);
                edit.buffer.push(unit);
                EditInput::Overflowed(chunk)
            }
            EditKey::PageUp => {
                let n = self.visible_row_count().max(1) as i32;
                self.scroll(n);
                EditInput::Unchanged
            }
            EditKey::PageDown => {
                let n = self.visible_row_count().max(1) as i32;
                self.scroll(-n);
                EditInput::Unchanged
            }
            EditKey::ScrollTop => {
                let max = self.max_scroll_offset() as i32;
                self.scroll(max);
                EditInput::Unchanged
            }
            EditKey::ScrollBottom => {
                self.scroll(-(self.scroll_offset as i32));
                EditInput::Unchanged
            }
            EditKey::HistoryUp => {
                if self.history.is_empty() {
                    return EditInput::Unchanged;
                }
                let idx = match self.history_index {
                    None => self.history.len() - 1,
                    Some(i) => i.saturating_sub(1),
                };
                self.history_index = Some(idx);
                Self::load_history_into_edit(edit, &self.history[idx]);
                EditInput::Typed
            }
            EditKey::HistoryDown => {
                let Some(idx) = self.history_index else {
                    return EditInput::Unchanged;
                };
                let next = idx + 1;
                if next >= self.history.len() {
                    self.history_index = None;
                    edit.buffer.truncate(edit.start);
                } else {
                    self.history_index = Some(next);
                    Self::load_history_into_edit(edit, &self.history[next]);
                }
                EditInput::Typed
            }
            EditKey::Unit(_) | EditKey::Other => {
                // Any fresh typing leaves history-browse mode.
                self.history_index = None;
                EditInput::Unchanged
            }
        }
    }

    /// TAB completion: expand the word being typed to a player name. One
    /// match replaces the word and adds a trailing space; several matches
    /// extend only the part they share. False when nothing matched or the
    /// result would not fit the edit line.
    pub fn complete(&mut self, candidates: &[String], measure: &dyn Fn(&str) -> i32) -> bool {
        let limit = self.max_width_px - MESSAGE_EDIT_WIDTH_MARGIN_PX;
        let Some(edit) = self.edit.as_mut() else {
            return false;
        };
        let typed = String::from_utf16_lossy(&edit.buffer[edit.start..]);
        // rfind on bytes cannot split multibyte whitespace (NBSP and friends):
        // step over the whole character the match starts at.
        let split = typed
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map_or(0, |(index, c)| index + c.len_utf8());
        let token = &typed[split..];
        if token.is_empty() {
            return false;
        }
        let lower = token.to_lowercase();
        let mut matches: Vec<&str> = candidates
            .iter()
            .map(String::as_str)
            .filter(|name| name.to_lowercase().starts_with(&lower))
            .collect();
        if matches.is_empty() {
            return false;
        }
        matches.sort_unstable();
        matches.dedup();
        let replacement = if matches.len() == 1 {
            format!("{} ", matches[0])
        } else {
            common_prefix(&matches).to_string()
        };
        if replacement.len() <= token.len() {
            return false;
        }
        let mut buffer: Vec<u16> = edit.buffer[..edit.start].to_vec();
        buffer.extend(typed[..split].encode_utf16());
        buffer.extend(replacement.encode_utf16());
        if measure(&String::from_utf16_lossy(&buffer)) >= limit {
            return false;
        }
        edit.buffer = buffer;
        true
    }
    /// Replace the edit buffer's typed portion with a history entry.
    fn load_history_into_edit(edit: &mut MessageEdit, text: &str) {
        edit.buffer.truncate(edit.start);
        edit.buffer.extend(text.encode_utf16());
    }

    /// Every end-of-edit path (`0x005D4390` and Input's copies): the input
    /// bar closes and the rows restack.
    fn end_edit(&mut self) {
        if self.edit.take().is_some() {
            self.scroll_offset = 0;
            self.clear_selection();
            self.close_menu();
            self.restack();
        }
    }

    pub fn x(&self) -> i32 {
        self.x
    }

    /// Rows in walk order = insertion order. The native layout paints them
    /// top-down in this order; the Minecraft layout paints them bottom-up, so
    /// the newest row is the lowest one on screen.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Contract §4.2: compose `prefix separator text`, evict the head when
    /// full, tail-insert, restack, recurse silently on the remainder.
    pub fn add_message(
        &mut self,
        post: &MessagePost,
        now_ms: u64,
        measure: &dyn Fn(&str) -> i32,
    ) -> AddOutcome {
        let mut outcome = AddOutcome {
            added: 0,
            play_sound: false,
        };
        self.add_inner(post, post.text, post.silent, now_ms, measure, &mut outcome);
        outcome
    }

    /// `text`/`silent` override the `post` fields: the wrap recursion re-adds
    /// the remainder with the SAME prefix/color/timeout but always silently.
    fn add_inner(
        &mut self,
        post: &MessagePost,
        text: &str,
        silent: bool,
        now_ms: u64,
        measure: &dyn Fn(&str) -> i32,
        outcome: &mut AddOutcome,
    ) {
        if text.is_empty() {
            return;
        }
        // Step 2 — compose the prefix with the layout's separator.
        let separator = match self.layout {
            MessageLayout::Native => MESSAGE_PREFIX_SEPARATOR_NATIVE,
            MessageLayout::ChatBox => MESSAGE_PREFIX_SEPARATOR,
        };
        let compose_prefix = match post.prefix {
            Some(p) => format!("{p}{separator}"),
            None => String::new(),
        };
        // Step 3 — pixel budget for the text part.
        let budget = self.max_width_px - measure(&compose_prefix) - MESSAGE_WIDTH_PAD_PX;
        if budget <= 0 {
            return;
        }
        // Step 4 — fit (char cap + px budget, word break).
        let fit_bytes = fit_prefix_bytes(text, budget, MESSAGE_LINE_FIT_CHARS, measure);
        if fit_bytes == 0 {
            return;
        }
        let prefix_bytes = compose_prefix.len();
        let mut line = compose_prefix;
        line.push_str(&text[..fit_bytes]);
        let line: String = line.chars().take(MESSAGE_SLOT_CHARS).collect();
        // Step 5 — evict the oldest once at the cap. The native layout counts
        // the open edit as one slot (0x005D3CAA..0x005D3CFF); the Minecraft
        // input bar has its own bottom row and takes no slot.
        let edit_slot = usize::from(self.layout == MessageLayout::Native && self.edit.is_some());
        if self.max_visible > 0
            && self.max_visible < self.messages.len() + edit_slot + 1
            && !self.messages.is_empty()
        {
            self.messages.remove(0);
        }
        // Slot-pool guard. The native layout is bounded by the TextLabel slot
        // pool (`MESSAGE_SLOTS`); the chat box keeps a longer scroll-back, so
        // its cap is `max_visible` (set via `set_max_visible`).
        let pool_cap = match self.layout {
            MessageLayout::Native => MESSAGE_SLOTS,
            MessageLayout::ChatBox => self.max_visible.max(1),
        };
        if self.messages.len() >= pool_cap {
            return;
        }
        // Steps 6-12 — tail insert + restack.
        let prefix_bytes = prefix_bytes.min(line.len());
        self.messages.push(Message {
            text: line,
            rgb: post.rgb,
            prefix_bytes,
            visible_deadline_ms: post.visible_timeout_ms.map(|t| now_ms + t),
            retain_deadline_ms: post.retain_timeout_ms.map(|t| now_ms + t),
            y: 0,
            reveal: (!silent && self.typing).then(MessageReveal::default),
        });
        self.restack();
        outcome.added += 1;
        if !silent {
            outcome.play_sound = true;
        }
        // A row arriving while the user is scrolled up must not yank the
        // view: keep the same absolute window and flag the unseen row.
        if self.scroll_offset > 0 {
            self.scroll_offset = (self.scroll_offset + 1).min(self.max_scroll_offset());
            self.has_unread = true;
        }
        // Step 13 — wrap recursion: skip control chars after the break, then
        // re-add the remainder with the SAME prefix, always silent.
        if fit_bytes < text.len() {
            let remainder = text[fit_bytes..].trim_start_matches(|c: char| (c as u32) < 0x20);
            if !remainder.is_empty() {
                self.add_inner(post, remainder, true, now_ms, measure, outcome);
            }
        }
    }

    /// Contract §4.3 — expiry: remove rows whose retain deadline passed strictly
    /// (`now > retain_deadline`), then restack. Returns true when anything expired.
    /// Add5D3DD5 and Manage5D4449 use the same process timer887338/887340.
    /// The driver skips this call while Main_Tick is blocked, without
    /// subtracting that interval from `now_ms`. Source: original gamemd.exe.
    pub fn manage(&mut self, now_ms: u64) -> bool {
        let before = self.messages.len();
        self.messages
            .retain(|m| m.retain_deadline_ms.is_none_or(|d| now_ms <= d));
        let expired = self.messages.len() != before;
        if expired {
            self.restack();
        }
        expired
    }

    /// Successful TextLabel paint72A5B1..72A609, once per drawn frame.
    /// Unlike expiry, reveal uses raw wrapping timeGetTime milliseconds,
    /// including time spent paused. First paint starts at one unit; subsequent
    /// paints consume complete16ms buckets and discard the remainder only
    /// when the count changes. Every paint still within the UTF-16 length
    /// requests MessageCharTyped, even when no new bucket elapsed.
    /// Returns requests, not admitted audio channels; audio owns admission.
    pub fn advance_reveal(&mut self, wall_ms: u32) -> usize {
        let mut sound_requests = 0;
        for message in &mut self.messages {
            let Some(reveal) = &mut message.reveal else {
                continue;
            };
            let previous = reveal.count;
            reveal.count = if previous == 0 {
                1
            } else {
                previous.wrapping_add(wall_ms.wrapping_sub(reveal.last_wall_ms) >> 4)
            };
            if previous != reveal.count {
                reveal.last_wall_ms = wall_ms;
            }
            if (reveal.count as i32) <= message.text.encode_utf16().count() as i32 {
                sound_requests += 1;
            }
        }
        sound_requests
    }

    /// Native: rows stack downward from the view's top edge. Minecraft chat:
    /// rows grow upward from the bottom-left edge, and the open input bar
    /// takes the bottom row. Each MC row's box starts one pixel above its
    /// text, so the boxes abut into one strip.
    fn restack(&mut self) {
        match self.layout {
            MessageLayout::Native => {
                for (i, m) in self.messages.iter_mut().enumerate() {
                    m.y = self.y + i as i32 * MESSAGE_LINE_HEIGHT_PX;
                }
            }
            MessageLayout::ChatBox => {
                let content_bottom = self.y + self.view_height - MESSAGE_BOTTOM_INSET_PX;
                // The open input bar keeps one empty row between itself and
                // the newest history line.
                let rows_bottom = if self.edit.is_some() {
                    content_bottom - MESSAGE_INPUT_BAR_HEIGHT_PX - MESSAGE_LINE_HEIGHT_PX
                } else {
                    content_bottom
                };
                // An active search hides the rows that do not match: the
                // matches stack bottom-up as if only they existed, and every
                // other row is parked above the view.
                if self.search_active() {
                    let shown = self.search_rows();
                    let count = shown.len();
                    for (slot, &i) in shown.iter().enumerate() {
                        let from_bottom = (count - 1 - slot) as i32;
                        self.messages[i].y = rows_bottom
                            - (from_bottom + 1) * MESSAGE_LINE_HEIGHT_PX
                            + MESSAGE_ROW_BOX_INSET_PX;
                    }
                    for (i, m) in self.messages.iter_mut().enumerate() {
                        if !shown.contains(&i) {
                            m.y = self.y - MESSAGE_LINE_HEIGHT_PX;
                        }
                    }
                    return;
                }
                let count = self.messages.len();
                // Scrolling shifts the painted history DOWN by scroll_offset
                // rows: the window itself stays where it is, older rows enter
                // at its top and the rows hidden below are clipped away by the
                // render pass filtering to the visible window.
                let scroll_px = (self.scroll_offset as i32) * MESSAGE_LINE_HEIGHT_PX;
                for (i, m) in self.messages.iter_mut().enumerate() {
                    let from_bottom = (count - 1 - i) as i32;
                    m.y = rows_bottom - (from_bottom + 1) * MESSAGE_LINE_HEIGHT_PX
                        + MESSAGE_ROW_BOX_INSET_PX
                        + scroll_px;
                }
            }
        }
    }
}

/// TrimMessage `0x005D4B20` with scandir 1: the cut is the largest length
/// in `min..=max` (max capped at the text length) whose last unit is below
/// `0x21`, else `min`. Native leaves a text shorter than `min` undefined;
/// the cut is capped at the text length.
fn trim_message_cut(text: &[u16], min: usize, max: usize) -> usize {
    let max = max.min(text.len());
    (min..=max)
        .rev()
        .find(|&cut| cut > 0 && text[cut - 1] < 0x21)
        .unwrap_or(min)
        .min(text.len())
}

/// Greedy px-budget fitter with word break (contract lane §4.2 step-4 fitter
/// shape, doc fidelity — exact boundary semantics are a plan deferred item):
/// fits chars while BOTH the char cap and the pixel budget hold; on overflow
/// backs up to just-after the last space when one exists. Returns the fitted
/// BYTE length.
fn fit_prefix_bytes(
    text: &str,
    budget_px: i32,
    max_chars: usize,
    measure: &dyn Fn(&str) -> i32,
) -> usize {
    let mut fitted = 0usize;
    let mut last_break: Option<usize> = None;
    // `chars` counts the characters already fitted before this iteration.
    for (chars, (idx, ch)) in text.char_indices().enumerate() {
        let end = idx + ch.len_utf8();
        if chars + 1 > max_chars || measure(&text[..end]) > budget_px {
            return last_break.unwrap_or(idx);
        }
        if ch == ' ' {
            last_break = Some(end);
        }
        fitted = end;
    }
    fitted
}

/// The longest prefix every candidate shares.
fn common_prefix<'a>(names: &[&'a str]) -> &'a str {
    let Some(&first) = names.first() else {
        return "";
    };
    let mut end = first.len();
    for name in &names[1..] {
        let mut matched = 0usize;
        for (a, b) in first.chars().zip(name.chars()) {
            if a != b {
                break;
            }
            matched += a.len_utf8();
        }
        end = end.min(matched);
        if end == 0 {
            break;
        }
    }
    &first[..end]
}
#[cfg(test)]
mod tests {
    use super::*;

    /// 10 px per char monospace fake measure.
    fn mono(s: &str) -> i32 {
        s.chars().count() as i32 * 10
    }

    const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
    /// Tactical viewport height the chat stacks against in these tests.
    const VIEW_H: i32 = 100;

    fn list(max_visible: usize, width: i32) -> MessageList {
        let mut list = MessageList::new(3, 0, max_visible, width);
        list.set_view(3, 0, width, VIEW_H);
        list.set_layout(MessageLayout::ChatBox);
        list
    }

    /// A list in the default native layout, anchored at y=10.
    fn native_list(max_visible: usize, width: i32) -> MessageList {
        let mut list = MessageList::new(3, 10, max_visible, width);
        list.set_view(3, 10, width, VIEW_H);
        list
    }

    /// Text Y of the bottom-most row (above the input bar while editing).
    fn bottom_row_y(editing: bool) -> i32 {
        let bottom = VIEW_H
            - MESSAGE_BOTTOM_INSET_PX
            - if editing {
                MESSAGE_INPUT_BAR_HEIGHT_PX + MESSAGE_LINE_HEIGHT_PX
            } else {
                0
            };
        bottom - MESSAGE_LINE_HEIGHT_PX + MESSAGE_ROW_BOX_INSET_PX
    }

    fn post<'a>(
        prefix: Option<&'a str>,
        text: &'a str,
        timeout_ms: Option<u64>,
        silent: bool,
    ) -> MessagePost<'a> {
        MessagePost {
            prefix,
            text,
            rgb: WHITE,
            visible_timeout_ms: timeout_ms,
            retain_timeout_ms: timeout_ms,
            silent,
        }
    }

    #[test]
    fn native_rows_stack_downward_with_a_colon_separator() {
        let mut l = native_list(6, 500);
        assert_eq!(l.layout(), MessageLayout::Native, "default layout");
        let o = l.add_message(&post(Some("Boris"), "attack", None, false), 0, &mono);
        assert_eq!(o.added, 1);
        l.add_message(&post(None, "second", None, true), 0, &mono);
        let rows = l.messages();
        assert_eq!(rows[0].text, "Boris: attack");
        assert_eq!(rows[0].y, 10, "first row on the view's top edge");
        assert_eq!(rows[1].text, "second");
        assert_eq!(rows[1].y, 10 + MESSAGE_LINE_HEIGHT_PX, "19 px stride down");
    }

    #[test]
    fn the_native_edit_takes_one_message_slot_below_the_last_row() {
        let mut l = native_list(3, 500);
        for i in 0..3 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert!(l.add_edit("P ", WHITE, true));
        let texts: Vec<&str> = l.messages().iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts, ["m1", "m2"], "the edit's slot evicts the oldest row");
        assert_eq!(
            l.edit_line().unwrap().y,
            10 + 2 * MESSAGE_LINE_HEIGHT_PX,
            "the edit sits below the surviving rows"
        );
        // A new row at the cap evicts again: rows + edit stay within the cap.
        l.add_message(&post(None, "n", None, true), 0, &mono);
        let texts: Vec<&str> = l.messages().iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts, ["m2", "n"]);
        assert_eq!(l.edit_line().unwrap().y, 10 + 2 * MESSAGE_LINE_HEIGHT_PX);
        // Closing the edit frees its slot; rows do not move.
        assert_eq!(l.input(EditKey::RightClick, &mono), EditInput::Ended);
        l.add_message(&post(None, "o", None, true), 0, &mono);
        assert_eq!(l.messages().len(), 3);
        assert_eq!(l.messages()[2].y, 10 + 2 * MESSAGE_LINE_HEIGHT_PX);
    }

    #[test]
    fn set_layout_restacks_the_rows() {
        let mut l = native_list(6, 500);
        for i in 0..2 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert_eq!(l.messages()[1].y, 10 + MESSAGE_LINE_HEIGHT_PX);
        l.set_layout(MessageLayout::ChatBox);
        // bottom_row_y is anchored at y=0; this list is anchored at y=10.
        assert_eq!(l.messages()[1].y, 10 + bottom_row_y(false));
        l.set_layout(MessageLayout::ChatBox);
        assert_eq!(
            l.messages()[1].y,
            10 + bottom_row_y(false),
            "unchanged: no restack"
        );
    }

    #[test]
    fn add_composes_name_prefix_and_stacks_up_from_the_bottom() {
        let mut l = list(6, 500);
        let o = l.add_message(&post(Some("<Boris>"), "attack", None, false), 0, &mono);
        assert_eq!(o.added, 1);
        assert!(o.play_sound);
        let o = l.add_message(&post(None, "second", None, true), 0, &mono);
        assert!(!o.play_sound, "silent add suppresses the sound");
        let rows = l.messages();
        assert_eq!(rows[0].text, "<Boris> attack");
        assert_eq!(rows[1].text, "second");
        assert_eq!(rows[1].y, bottom_row_y(false), "newest row sits lowest");
        assert_eq!(
            rows[0].y,
            bottom_row_y(false) - MESSAGE_LINE_HEIGHT_PX,
            "19 px stride upward"
        );
    }

    #[test]
    fn eviction_drops_oldest_at_cap() {
        let mut l = list(3, 500);
        for i in 0..4 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        let texts: Vec<&str> = l.messages().iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts, vec!["m1", "m2", "m3"], "head (oldest) evicted");
        assert_eq!(
            l.messages()[0].y,
            bottom_row_y(false) - 2 * MESSAGE_LINE_HEIGHT_PX,
            "restacked from the bottom"
        );
    }

    #[test]
    fn wrap_recursion_reincludes_prefix_and_is_silent() {
        // Budget: width 200 − prefix "P " 20 − pad 8 = 172 px → 17 chars/line.
        let mut l = list(6, 200);
        let text = "aaaa bbbb cccc dddd eeee"; // 24 chars → must wrap
        let o = l.add_message(&post(Some("P"), text, None, false), 0, &mono);
        assert!(o.added >= 2, "wrapped into multiple rows");
        assert!(o.play_sound, "ONE sound for the top-level add");
        for row in l.messages() {
            assert!(
                row.text.starts_with("P "),
                "prefix re-included on wraps: {}",
                row.text
            );
            assert!(mono(&row.text) <= 200 - MESSAGE_WIDTH_PAD_PX + 20);
        }
        assert_eq!(
            l.advance_reveal(100),
            1,
            "only the nonsilent first row types"
        );
        assert_eq!(l.messages()[0].reveal_count(), 1);
        assert!(l.messages()[1..].iter().all(|row| row.reveal_count() == 0));
        // All input chars survive across the rows (minus break spaces).
        let joined: String = l
            .messages()
            .iter()
            .map(|m| m.text.trim_start_matches("P "))
            .collect::<Vec<_>>()
            .join(" ");
        let normalize = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(normalize(&joined), normalize(text));
    }

    #[test]
    fn word_break_backs_up_to_last_space() {
        // budget 100 px = 10 chars; "hello worlds" breaks after "hello ".
        let fit = fit_prefix_bytes("hello worlds", 100, 111, &mono);
        assert_eq!(&"hello worlds"[..fit], "hello ");
    }

    #[test]
    fn zero_budget_or_unfittable_adds_nothing() {
        let mut l = list(6, 5); // budget 5−0−8 < 0
        let o = l.add_message(&post(None, "text", None, false), 0, &mono);
        assert_eq!(o.added, 0);
        assert!(!o.play_sound);
        // First char wider than the budget → fit 0 → nothing.
        let mut l2 = list(6, 17); // budget 17−0−8 = 9 < 10
        let o = l2.add_message(&post(None, "x", None, false), 0, &mono);
        assert_eq!(o.added, 0);
    }

    #[test]
    fn manage_expires_strictly_after_deadline() {
        let mut l = list(6, 500);
        l.add_message(&post(None, "temp", Some(1000), true), 0, &mono);
        l.add_message(&post(None, "forever", None, true), 0, &mono);
        assert!(!l.manage(1000), "now == deadline: kept (strict >)");
        assert!(l.manage(1001), "now > deadline: expired");
        let texts: Vec<&str> = l.messages().iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts, vec!["forever"], "0/None = never expires");
        assert_eq!(
            l.messages()[0].y,
            bottom_row_y(false),
            "restacked onto the bottom edge"
        );
    }

    #[test]
    fn first_poll_after_a_pause_expires_elapsed_deadlines() {
        let mut l = list(6, 500);
        l.add_message(&post(None, "temp", Some(4_000), true), 0, &mono);
        assert!(!l.manage(1_000));
        // The modal does not poll Manage. The process clock keeps running.
        assert_eq!(l.messages().len(), 1);
        assert!(l.manage(11_000));
        assert!(l.messages().is_empty());
    }

    #[test]
    fn chatbox_layout_keeps_chat_history_past_deadline() {
        // A chat row posted with no timeout survives Manage, while a timed
        // system row still expires. Reopening the chat box therefore shows the
        // chat line even after the system feedback has faded.
        let mut l = list(6, 500);
        l.add_message(&post(Some("<Player>"), "hello", None, false), 0, &mono);
        l.add_message(&post(None, "temp", Some(1_000), true), 0, &mono);
        assert!(l.manage(1_001), "timed row expired");
        let texts: Vec<&str> = l.messages().iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts, vec!["<Player> hello"], "chat history retained");
    }

    #[test]
    fn chatbox_layout_raises_the_row_cap_beyond_the_slot_pool() {
        // The native layout clamps max_visible to MESSAGE_SLOTS (14); the chat
        // box keeps a longer scroll-back, so set_max_visible must not clamp.
        let mut l = list(6, 500);
        l.set_max_visible(MESSAGE_MAX_VISIBLE_CHATBOX);
        assert!(l.messages().is_empty());
        for i in 0..20 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert_eq!(l.messages().len(), 20, "chat box keeps rows past 14");
    }

    #[test]
    fn typing_counts_utf16_paints_and_raw_wall_time_independently_of_expiry() {
        let mut l = list(6, 500);
        // Five UTF-16 units, including a surrogate pair and a space.
        l.add_message(&post(None, "A😀 B", Some(1_000), false), 0, &mono);
        assert_eq!(l.advance_reveal(100), 1);
        assert_eq!(l.messages()[0].reveal_count(), 1);
        assert_eq!(l.advance_reveal(115), 1, "requests every successful paint");
        assert_eq!(l.messages()[0].reveal_count(), 1);
        assert_eq!(l.advance_reveal(131), 1);
        assert_eq!(l.messages()[0].reveal_count(), 2);
        assert_eq!(l.advance_reveal(147), 1, "31ms remainder was discarded");
        assert_eq!(l.messages()[0].reveal_count(), 3);
        assert_eq!(
            l.advance_reveal(179),
            1,
            "count == UTF16 length still types"
        );
        assert_eq!(l.messages()[0].reveal_count(), 5);
        // Paint/reveal may be serviced while expiry polling is suspended.
        assert_eq!(l.messages().len(), 1);
        assert_eq!(l.advance_reveal(10_000), 0, "typing uses raw wall time");
        assert!(l.messages()[0].reveal_count() > 5 + 8);
        assert!(l.manage(11_000));
        assert_eq!(l.advance_reveal(11_001), 0, "expired labels own no state");
    }

    #[test]
    fn typing_clock_wraps_like_time_get_time_and_eviction_resets_new_rows() {
        let mut l = list(1, 500);
        l.add_message(&post(None, "abc", None, false), 0, &mono);
        assert_eq!(l.advance_reveal(u32::MAX - 7), 1);
        assert_eq!(l.advance_reveal(8), 1);
        assert_eq!(l.messages()[0].reveal_count(), 2);
        l.add_message(&post(None, "new", None, false), 0, &mono);
        assert_eq!(l.advance_reveal(100_000), 1);
        assert_eq!(l.messages()[0].reveal_count(), 1);
    }

    fn typed(list: &mut MessageList, text: &str) -> Vec<EditInput> {
        text.encode_utf16()
            .map(|unit| list.input(EditKey::Unit(unit), &mono))
            .collect()
    }

    #[test]
    fn the_input_bar_owns_the_bottom_row_without_taking_a_message_slot() {
        let mut l = list(6, 500);
        for i in 0..6 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        assert!(
            !l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true),
            "refused while editing"
        );
        let texts: Vec<&str> = l.messages().iter().map(|m| m.text.as_str()).collect();
        assert_eq!(
            texts,
            ["m0", "m1", "m2", "m3", "m4", "m5"],
            "opening the chat keeps every message row"
        );
        assert_eq!(
            l.messages()[5].y,
            bottom_row_y(true),
            "newest row sits above the input bar"
        );
        assert_eq!(
            l.messages()[0].y,
            bottom_row_y(true) - 5 * MESSAGE_LINE_HEIGHT_PX
        );
        assert_eq!(
            l.edit_line().unwrap().y,
            VIEW_H - MESSAGE_BOTTOM_INSET_PX - MESSAGE_INPUT_BAR_HEIGHT_PX
                + MESSAGE_INPUT_TEXT_INSET_PX,
            "typed text sits inside the bar"
        );
        // The row cap no longer counts the edit line.
        l.add_message(&post(None, "n", None, true), 0, &mono);
        assert_eq!(l.messages().len(), 6);
        assert_eq!(l.messages()[5].text, "n");
        assert_eq!(l.input(EditKey::RightClick, &mono), EditInput::Ended);
        assert!(!l.editing());
        assert_eq!(
            l.messages()[5].y,
            bottom_row_y(false),
            "rows drop back to the bottom edge"
        );
        l.add_message(&post(None, "o", None, true), 0, &mono);
        assert_eq!(l.messages().len(), 6);
    }

    #[test]
    fn edit_is_append_only_and_backspace_stops_at_the_prompt() {
        let mut l = list(6, 500);
        assert_eq!(l.input(EditKey::Unit(0x41), &mono), EditInput::Unchanged);
        l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true);
        assert_eq!(typed(&mut l, "hi"), [EditInput::Typed, EditInput::Typed]);
        assert_eq!(l.input(EditKey::Unit(0x09), &mono), EditInput::Unchanged);
        assert_eq!(l.input(EditKey::Other, &mono), EditInput::Unchanged);
        assert_eq!(l.edit_line().unwrap().text, "> hi");
        assert_eq!(l.input(EditKey::Delete, &mono), EditInput::Deleted);
        assert_eq!(l.input(EditKey::Delete, &mono), EditInput::Deleted);
        assert_eq!(l.input(EditKey::Delete, &mono), EditInput::Unchanged);
        assert_eq!(l.edit_line().unwrap().text, "> ");
        assert_eq!(l.input(EditKey::Enter, &mono), EditInput::Ended, "empty");
        l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true);
        typed(&mut l, "go");
        assert_eq!(
            l.input(EditKey::Enter, &mono),
            EditInput::Sent("go ".into()),
            "Enter appends one space"
        );
        l.add_edit("P ", WHITE, true);
        assert_eq!(l.input(EditKey::Escape, &mono), EditInput::Ended);
    }

    #[test]
    fn a_full_line_is_split_at_the_last_space_after_unit_20() {
        // 97 units fit; the 98th overflows.
        let mut l = list(6, 100_000);
        l.add_edit("P ", WHITE, true);
        let text = format!("{} {}", "a".repeat(30), "b".repeat(66));
        assert!(typed(&mut l, &text).iter().all(|r| *r == EditInput::Typed));
        assert_eq!(
            l.input(EditKey::Unit(u16::from(b'c')), &mono),
            EditInput::Overflowed(format!("{} ", "a".repeat(30))),
            "the cut includes the space"
        );
        assert_eq!(
            l.edit_line().unwrap().text,
            format!("P {}c", "b".repeat(66))
        );
        // No break at or after unit 20: a 20-unit chunk.
        let mut l = list(6, 100_000);
        l.add_edit("P ", WHITE, true);
        typed(&mut l, &"x".repeat(97));
        assert_eq!(
            l.input(EditKey::Unit(u16::from(b'y')), &mono),
            EditInput::Overflowed("x".repeat(20))
        );
        // Overflow off: the unit is refused.
        let mut l = list(6, 100_000);
        l.add_edit("P ", WHITE, false);
        typed(&mut l, &"x".repeat(97));
        assert_eq!(l.input(EditKey::Unit(0x79), &mono), EditInput::Unchanged);
    }

    #[test]
    fn the_whole_edit_line_must_stay_10_px_inside_the_width() {
        // limit 200 − 10: "P " + 16 units = 180 px fits, 190 px does not.
        let mut l = list(6, 200);
        l.add_edit("P ", WHITE, false);
        assert!(
            typed(&mut l, &"x".repeat(16))
                .iter()
                .all(|r| *r == EditInput::Typed)
        );
        assert_eq!(l.input(EditKey::Unit(0x78), &mono), EditInput::Unchanged);
        assert_eq!(
            l.input(EditKey::Enter, &mono),
            EditInput::Sent(format!("{} ", "x".repeat(16)))
        );
    }

    #[test]
    fn a_width_change_drops_the_edit() {
        let mut l = list(6, 500);
        l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true);
        l.set_view(3, 0, 500, VIEW_H);
        assert!(l.editing());
        l.set_view(3, 0, 600, VIEW_H);
        assert!(!l.editing());
    }

    #[test]
    fn a_closed_scroll_text_gate_adds_whole_rows() {
        let mut l = list(6, 500);
        l.set_typing(false);
        let o = l.add_message(&post(None, "abc", None, false), 0, &mono);
        assert!(o.play_sound, "the sound does not depend on the gate");
        assert_eq!(l.advance_reveal(100), 0);
        assert_eq!(l.messages()[0].reveal_count(), 0, "drawn whole");
    }

    #[test]
    fn line_char_cap_111_applies() {
        let mut l = list(14, 100_000);
        let long = "y".repeat(300);
        let o = l.add_message(&post(None, &long, None, true), 0, &mono);
        assert!(o.added >= 2);
        assert_eq!(
            l.messages()[0].text.chars().count(),
            MESSAGE_LINE_FIT_CHARS,
            "first row capped at 111 chars"
        );
    }

    #[test]
    fn alpha_fades_linearly_over_the_last_fade_window() {
        let m = Message {
            text: String::new(),
            rgb: WHITE,
            prefix_bytes: 0,
            visible_deadline_ms: Some(2000),
            retain_deadline_ms: Some(2000),
            y: 0,
            reveal: None,
        };
        // Before the fade window: fully visible, full alpha.
        assert!(m.is_visible(1000));
        assert_eq!(m.alpha_at(1000), 1.0);
        // Fade start = 2000 - 500 = 1500; at exactly fade start alpha is 1.0.
        assert_eq!(m.alpha_at(1500), 1.0);
        // Midpoint of fade window (1750): alpha ≈ 0.5.
        let a = m.alpha_at(1750);
        assert!((a - 0.5).abs() < 0.001, "midpoint alpha = {a}");
        // One ms before deadline: alpha = 1/500 ≈ 0.002.
        let a = m.alpha_at(1999);
        assert!((a - 0.002).abs() < 0.001, "near-deadline alpha = {a}");
        // At the deadline boundary: is_visible uses <= so still visible,
        // but alpha_at uses >= so already 0.0 (fade completes).
        assert!(m.is_visible(2000), "is_visible uses <= boundary");
        assert_eq!(m.alpha_at(2000), 0.0);
        // Past the deadline: both invisible and alpha 0.0.
        assert!(!m.is_visible(2001));
        assert_eq!(m.alpha_at(2001), 0.0);
        assert!(!m.is_visible(3000));
        assert_eq!(m.alpha_at(3000), 0.0);

        // visible_deadline_ms = None: never visually expires, alpha always 1.0.
        let never = Message {
            text: String::new(),
            rgb: WHITE,
            prefix_bytes: 0,
            visible_deadline_ms: None,
            retain_deadline_ms: None,
            y: 0,
            reveal: None,
        };
        assert!(never.is_visible(0));
        assert_eq!(never.alpha_at(0), 1.0);
        assert!(never.is_visible(9_999_999));
        assert_eq!(never.alpha_at(9_999_999), 1.0);
    }

    #[test]
    fn chat_row_visible_deadline_passes_but_retain_is_none_manage_keeps_it() {
        let mut l = list(6, 500);
        // Chat-row semantics: visible_timeout_ms = Some (fades), retain_timeout_ms = None (kept forever).
        l.add_message(
            &MessagePost {
                prefix: Some("<Player>"),
                text: "hello",
                rgb: WHITE,
                visible_timeout_ms: Some(1000),
                retain_timeout_ms: None,
                silent: false,
            },
            0,
            &mono,
        );
        assert_eq!(l.messages().len(), 1);
        // manage(1001): retain_deadline_ms is None, so nothing is removed → returns false.
        assert!(
            !l.manage(1001),
            "chat row with retain=None is never managed away"
        );
        assert_eq!(l.messages().len(), 1, "chat row still retained");
        // visible_deadline_ms of the chat row has indeed passed, but the row lingers in the list.
        assert!(
            !l.messages()[0].is_visible(1001),
            "row no longer visible but retained"
        );

        // Now add a system row where both timeouts are Some(1000).
        l.add_message(
            &MessagePost {
                prefix: None,
                text: "system",
                rgb: WHITE,
                visible_timeout_ms: Some(1000),
                retain_timeout_ms: Some(1000),
                silent: true,
            },
            0,
            &mono,
        );
        assert_eq!(l.messages().len(), 2);
        // manage(1001) now expires the system row but keeps the chat row.
        assert!(l.manage(1001), "system row with both deadlines expired");
        let texts: Vec<&str> = l.messages().iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts, vec!["<Player> hello"], "only chat row survives");
    }
    #[test]
    fn chat_scroll_shifts_visible_window_and_clamps_to_history() {
        let mut l = list(100, 500);
        // VIEW_H=100, bar=20, inset=2, line=19 => visible = (100-20-2)/19 = 4.
        for i in 0..6 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert_eq!(l.visible_row_count(), 4);
        assert_eq!(l.max_scroll_offset(), 2);
        assert_eq!(l.scroll_offset(), 0);
        let (start, end) = l.visible_message_range();
        assert_eq!((start, end), (2, 6), "pinned to newest four rows");
        // Scroll up two rows: reveals the oldest pair.
        l.scroll(2);
        assert_eq!(l.scroll_offset(), 2);
        let (start, end) = l.visible_message_range();
        assert_eq!((start, end), (0, 4), "scrolled to oldest four rows");
        // Scrolling past the oldest is clamped.
        l.scroll(10);
        assert_eq!(l.scroll_offset(), 2, "clamped to max_scroll_offset");
        // Scroll back down to newest.
        l.scroll(-10);
        assert_eq!(l.scroll_offset(), 0, "clamped to zero when scrolling down");
        let (start, end) = l.visible_message_range();
        assert_eq!((start, end), (2, 6), "back to newest window");
    }

    #[test]
    fn arrow_keys_recall_sent_message_history() {
        let mut l = list(100, 500);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        // Send "hello"
        for u in "hello".encode_utf16() {
            l.input(EditKey::Unit(u), &mono);
        }
        let _ = l.input(EditKey::Enter, &mono);
        // Reopen and send "world"
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        for u in "world".encode_utf16() {
            l.input(EditKey::Unit(u), &mono);
        }
        let _ = l.input(EditKey::Enter, &mono);
        // Reopen: ArrowUp recalls "world", then "hello".
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        l.input(EditKey::HistoryUp, &mono);
        assert_eq!(typed_text(&l), "world");
        l.input(EditKey::HistoryUp, &mono);
        assert_eq!(typed_text(&l), "hello");
        // ArrowUp at the oldest entry stays there.
        l.input(EditKey::HistoryUp, &mono);
        assert_eq!(typed_text(&l), "hello");
        // ArrowDown goes forward to "world", then past the end clears.
        l.input(EditKey::HistoryDown, &mono);
        assert_eq!(typed_text(&l), "world");
        l.input(EditKey::HistoryDown, &mono);
        assert_eq!(typed_text(&l), "");
    }

    #[test]
    fn dragging_the_scrollbar_maps_thumb_travel_onto_the_history() {
        let mut l = list(100, 500);
        for i in 0..6 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert!(!l.begin_scroll_drag(40), "closed chat has no scrollbar");
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        assert!(l.begin_scroll_drag(40));
        // Track 78 px, thumb 39 px (3 of 6 rows), travel 39 px.
        l.drag_scroll_to(26);
        assert_eq!(l.scroll_offset(), 2, "thumb at the top shows the oldest");
        l.drag_scroll_to(52);
        assert_eq!(l.scroll_offset(), 0, "thumb at the bottom shows the newest");
        // Past either end clamps.
        l.drag_scroll_to(-500);
        assert_eq!(l.scroll_offset(), 3);
        l.drag_scroll_to(50_000);
        assert_eq!(l.scroll_offset(), 0);
        l.end_scroll_drag();
        assert!(!l.is_dragging_scrollbar());
    }

    #[test]
    fn search_compacts_matching_rows_and_hides_the_rest() {
        let mut l = list(100, 500);
        for text in ["alpha one", "beta two", "alpha three", "gamma"] {
            l.add_message(&post(None, text, None, true), 0, &mono);
        }
        assert_eq!(l.display_rows(), vec![0, 1, 2, 3], "no search: the window");
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        l.begin_search();
        for unit in "alpha".encode_utf16() {
            l.search_input(unit, false);
        }
        assert_eq!(l.display_rows(), vec![0, 2], "only matches are painted");
        // Matches stack bottom-up contiguously; the rest is parked off-screen.
        let rows_bottom =
            VIEW_H - MESSAGE_BOTTOM_INSET_PX - MESSAGE_INPUT_BAR_HEIGHT_PX - MESSAGE_LINE_HEIGHT_PX;
        assert_eq!(
            l.messages()[2].y,
            rows_bottom - MESSAGE_LINE_HEIGHT_PX + MESSAGE_ROW_BOX_INSET_PX
        );
        assert_eq!(
            l.messages()[0].y,
            rows_bottom - 2 * MESSAGE_LINE_HEIGHT_PX + MESSAGE_ROW_BOX_INSET_PX
        );
        assert_eq!(l.messages()[1].y, -MESSAGE_LINE_HEIGHT_PX, "parked");
        l.end_search();
        assert_eq!(
            l.display_rows(),
            vec![1, 2, 3],
            "the editing window shows its three rows again"
        );
    }

    #[test]
    fn right_click_menu_reports_the_enabled_row_under_the_pointer() {
        let mut l = list(100, 500);
        l.add_message(&post(None, "hi", None, true), 0, &mono);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        l.open_menu(10, 20, true, Some("https://example.com".to_owned()));
        let menu = l.menu().expect("open");
        assert_eq!(
            menu.height(),
            MESSAGE_MENU_PADDING_PX * 2 + MESSAGE_MENU_ROW_HEIGHT_PX * 2
        );
        assert_eq!(l.menu_action_at(12, 24), Some(ChatMenuAction::Copy));
        assert_eq!(
            l.menu_action_at(12, 24 + MESSAGE_MENU_ROW_HEIGHT_PX),
            Some(ChatMenuAction::OpenLink)
        );
        assert_eq!(l.menu_action_at(400, 24), None, "outside the box");
        // Disabled rows report no action.
        l.open_menu(10, 20, false, None);
        assert_eq!(l.menu_action_at(12, 24), None);
        l.close_menu();
        assert!(l.menu().is_none());
    }
    #[test]
    fn grabbing_the_middle_of_the_thumb_keeps_the_pointer_offset() {
        let mut l = list(100, 500);
        for i in 0..6 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        let bar = l.scrollbar_geometry().unwrap();
        // Press 10 px below the thumb top, then move the pointer 10 px lower:
        // the thumb follows by exactly that much instead of snapping.
        assert!(l.begin_scroll_drag(bar.thumb_y + 10));
        l.drag_scroll_to(bar.track_y + 10 + 13);
        // A third of the 39 px travel reveals two rows of older history.
        assert_eq!(l.scroll_offset(), 2);
    }

    #[test]
    fn search_matches_rows_case_insensitively_and_clears_on_close() {
        let mut l = list(100, 500);
        l.add_message(&post(None, "Hello world", None, true), 0, &mono);
        l.add_message(&post(None, "other line", None, true), 0, &mono);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        assert!(l.row_matches_search(1), "no search: every row matches");
        l.begin_search();
        assert_eq!(l.search_query(), Some(""));
        for unit in "WORLD".encode_utf16() {
            l.search_input(unit, false);
        }
        assert_eq!(l.search_query(), Some("WORLD"));
        assert!(l.row_matches_search(0));
        assert!(!l.row_matches_search(1));
        l.search_input(0, true);
        assert_eq!(l.search_query(), Some("WORL"));
        l.end_search();
        assert_eq!(l.search_query(), None);
        assert!(l.row_matches_search(1));
    }
    #[test]
    fn repeated_presses_walk_word_then_row_selection() {
        let mut l = list(100, 500);
        l.add_message(&post(None, "hello world link", None, true), 0, &mono);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        // Double click inside "world" (chars 6..11).
        l.press_at(
            ChatPoint {
                row: 0,
                char_index: 8,
            },
            1_000,
        );
        l.press_at(
            ChatPoint {
                row: 0,
                char_index: 8,
            },
            1_200,
        );
        assert_eq!(l.selected_text().as_deref(), Some("world"));
        // Triple click takes the whole row.
        l.press_at(
            ChatPoint {
                row: 0,
                char_index: 8,
            },
            1_400,
        );
        assert_eq!(l.selected_text().as_deref(), Some("hello world link"));
        // A slow press starts over: a single click selects no text.
        l.press_at(
            ChatPoint {
                row: 0,
                char_index: 2,
            },
            9_000,
        );
        assert_eq!(l.selected_text(), None);
        assert!(l.selection().is_some_and(|(a, b)| a == b));
    }

    #[test]
    fn tab_completion_expands_a_unique_name_and_only_the_common_prefix() {
        let mut l = list(100, 500);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        for unit in "al".encode_utf16() {
            l.input(EditKey::Unit(unit), &mono);
        }
        let names = vec!["alice".to_string()];
        assert!(l.complete(&names, &mono));
        assert_eq!(typed_text(&l), "alice ", "unique match adds a space");
        // Two matches only extend the shared prefix.
        let mut l = list(100, 500);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        for unit in "al".encode_utf16() {
            l.input(EditKey::Unit(unit), &mono);
        }
        let names = vec!["alice".to_string(), "alicia".to_string()];
        assert!(l.complete(&names, &mono));
        assert_eq!(typed_text(&l), "alic");
        // Nothing to add when the prefix already covers every match.
        assert!(!l.complete(&names, &mono));
    }
    #[test]
    fn tab_completion_survives_a_multibyte_whitespace_before_the_token() {
        // A non-breaking space is two UTF-8 bytes; the token split used to
        // slice one byte past `rfind` and panic inside the NBSP's encoding.
        let mut l = list(100, 500);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        for unit in "hello\u{00a0}al".encode_utf16() {
            l.input(EditKey::Unit(unit), &mono);
        }
        let names = vec!["alice".to_string()];
        assert!(l.complete(&names, &mono));
        assert_eq!(typed_text(&l), "hello\u{00a0}alice ");
    }
    #[test]
    fn row_hit_test_counts_19_px_rows_upward_from_the_history_bottom() {
        let mut l = list(100, 500);
        for i in 0..6 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        // History bottom = 100 - 2 inset - 20 input bar - 19 clearance = 59.
        assert_eq!(l.row_at(50, 58), Some(5), "bottom row");
        assert_eq!(l.row_at(50, 39), Some(4), "one 19 px row up");
        assert_eq!(l.row_at(50, 59), None, "the clearance row is not history");
        assert_eq!(l.row_at(50, -1), None, "above the view");
    }

    #[test]
    fn row_hit_test_rejects_x_outside_the_history_strip() {
        let mut l = list(100, 500);
        l.add_message(&post(None, "m0", None, true), 0, &mono);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        // A sidebar click aligned with a chat row must keep reaching the
        // gadget dispatch, not start a history selection. Strip: [3, 503).
        assert_eq!(l.row_at(2, 58), None, "left of the strip");
        assert_eq!(l.row_at(503, 58), None, "right of the strip");
        assert_eq!(l.row_at(3, 58), Some(0), "the strip's left edge hits");
        assert_eq!(l.row_at(502, 58), Some(0), "the strip's right edge hits");
    }

    #[test]
    fn row_hit_test_follows_the_rows_search_paints() {
        // With a search open the matches compact bottom-up; the older hidden
        // row must not answer a click aimed at the painted one.
        let mut l = list(100, 500);
        l.add_message(
            &post(None, "alpha https://a.test/old", None, true),
            0,
            &mono,
        );
        l.add_message(&post(None, "beta https://b.test/new", None, true), 0, &mono);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        l.begin_search();
        for unit in "alpha".encode_utf16() {
            l.search_input(unit, false);
        }
        assert_eq!(l.display_rows(), vec![0], "only the alpha row is painted");
        // The painted bottom row resolves to the alpha message's row, not to
        // the unfiltered window's bottom (beta).
        assert_eq!(l.row_at(50, 58), Some(0));
        l.end_search();
        assert_eq!(l.row_at(50, 58), Some(1), "unfiltered window restored");
    }

    #[test]
    fn dragging_a_selection_joins_rows_with_newlines() {
        let mut l = list(100, 500);
        l.add_message(&post(None, "https://example.com/a", None, true), 0, &mono);
        l.add_message(&post(None, "second line", None, true), 0, &mono);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        l.begin_selection(ChatPoint {
            row: 0,
            char_index: 8,
        });
        l.extend_selection(ChatPoint {
            row: 1,
            char_index: 6,
        });
        assert_eq!(
            l.selected_text().as_deref(),
            Some("example.com/a\nsecond"),
            "rows join with a newline"
        );
        // A drag upwards keeps the same text: the pair is normalized.
        l.begin_selection(ChatPoint {
            row: 1,
            char_index: 6,
        });
        l.extend_selection(ChatPoint {
            row: 0,
            char_index: 8,
        });
        assert_eq!(l.selected_text().as_deref(), Some("example.com/a\nsecond"));
        // Closing the chat drops the selection.
        l.input(EditKey::Escape, &mono);
        assert!(l.selection().is_none());
    }
    #[test]
    fn scrollbar_thumb_tracks_the_window_and_spans_the_full_history() {
        let mut l = list(100, 500);
        for i in 0..6 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        // Closed chat has no scrollbar chrome.
        assert!(l.scrollbar_geometry().is_none());
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        let bar = l.scrollbar_geometry().expect("history overflows the view");
        // The bar hugs the right edge of the two-thirds history strip,
        // not the right edge of the whole view.
        assert_eq!(bar.x, 328);
        // The track spans the three visible history rows, not the view.
        assert_eq!(bar.track_height, 3 * MESSAGE_LINE_HEIGHT_PX);
        assert_eq!(bar.track_y, 2, "flush with the top history row");
        assert_eq!(bar.thumb_height, bar.track_height * 3 / 6);
        // Pinned to newest: the thumb sits at the bottom of the track.
        assert_eq!(
            bar.thumb_y,
            bar.track_y + bar.track_height - bar.thumb_height
        );
        // Scrolled to the oldest: the thumb sits at the top.
        l.scroll(3);
        assert_eq!(l.scrollbar_geometry().unwrap().thumb_y, bar.track_y);
    }

    #[test]
    fn a_row_landing_while_scrolled_up_keeps_the_window_and_flags_unread() {
        let mut l = list(100, 500);
        for i in 0..6 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        l.scroll(3);
        let (start, _) = l.visible_message_range();
        assert_eq!(start, 0);
        assert!(!l.has_unread());
        // A new row keeps the same absolute window instead of yanking it.
        l.add_message(&post(None, "m6", None, true), 0, &mono);
        assert_eq!(l.scroll_offset(), 4, "offset grows with the history");
        assert_eq!(l.visible_message_range(), (0, 3), "same absolute window");
        assert!(l.has_unread());
        assert!(l.unread_marker_y().is_some());
        // Returning to the newest row clears the flag and the marker.
        l.input(EditKey::ScrollBottom, &mono);
        assert_eq!(l.scroll_offset(), 0);
        assert!(!l.has_unread());
        assert!(l.unread_marker_y().is_none());
    }
    #[test]
    fn page_and_home_end_keys_scroll_chat_history() {
        let mut l = list(100, 500);
        for i in 0..6 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        assert_eq!(l.scroll_offset(), 0);
        // PageUp scrolls up by the visible row count (3).
        l.input(EditKey::PageUp, &mono);
        assert_eq!(l.scroll_offset(), 3, "clamped to max_scroll_offset");
        // End scrolls back to newest.
        l.input(EditKey::ScrollBottom, &mono);
        assert_eq!(l.scroll_offset(), 0);
        // Home scrolls to the oldest.
        l.input(EditKey::ScrollTop, &mono);
        assert_eq!(l.scroll_offset(), 3);
        // PageDown scrolls back down.
        l.input(EditKey::PageDown, &mono);
        assert_eq!(l.scroll_offset(), 0);
    }

    #[test]
    fn chatbox_windows_twenty_rows_and_rescrolls_the_painted_rows() {
        let mut l = list(100, 500);
        // A viewport tall enough for far more than twenty rows.
        l.set_view(3, 0, 500, 800);
        l.set_max_visible(MESSAGE_MAX_VISIBLE_CHATBOX);
        for i in 0..21 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        assert_eq!(l.visible_row_count(), 20, "the window stops at 20 rows");
        assert_eq!(l.max_scroll_offset(), 1, "the 21st row scrolls out of view");
        assert_eq!(l.display_rows(), (1..21).collect::<Vec<_>>());
        let newest_before = l.messages()[20].y;
        l.scroll(1);
        assert_eq!(l.display_rows(), (0..20).collect::<Vec<_>>());
        assert_eq!(
            l.scrollbar_geometry().expect("21 rows scroll").track_height,
            20 * MESSAGE_LINE_HEIGHT_PX,
            "the track matches the 20-row window"
        );
        assert_eq!(
            l.messages()[20].y,
            newest_before + MESSAGE_LINE_HEIGHT_PX,
            "older rows move down inside the fixed window"
        );
        l.input(EditKey::PageDown, &mono);
        assert_eq!(l.scroll_offset(), 0);
    }

    fn typed_text(l: &MessageList) -> String {
        l.edit_line()
            .map(|e| {
                e.text
                    .strip_prefix(MESSAGE_INPUT_PREFIX)
                    .unwrap_or(&e.text)
                    .to_string()
            })
            .unwrap_or_default()
    }
    #[test]
    fn opening_or_closing_chat_resets_scroll_to_newest() {
        let mut l = list(100, 500);
        for i in 0..6 {
            l.add_message(&post(None, &format!("m{i}"), None, true), 0, &mono);
        }
        l.scroll(2);
        assert_eq!(l.scroll_offset(), 2);
        assert!(l.add_edit(MESSAGE_INPUT_PREFIX, WHITE, true));
        assert_eq!(l.scroll_offset(), 0, "opening chat pins to newest");
        l.scroll(2);
        l.input(EditKey::Escape, &mono);
        assert_eq!(l.scroll_offset(), 0, "closing chat resets scroll");
    }
}
