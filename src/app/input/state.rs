//! Match input owner (F12 `MatchInputState`): camera, zoom, cursor, held
//! keys, hotkey bindings, and the TypeSelect input machine.
//!
//! These are the app-side input/viewport facts for the running match (and the
//! shells that reuse the same cursor/key plumbing). Sim-authoritative input —
//! queued commands — lives in the simulation, never here.

use std::collections::HashSet;
use winit::keyboard::{KeyCode, ModifiersState};

use crate::sim::snapshot::saved_local_view::{
    CONTROL_GROUP_SLOTS, SavedLocalView, SavedLocalViewError,
};

/// A saved view whose identities and selection timeline were checked against
/// the prepared world. Commit performs no further admission or command work.
#[derive(Debug)]
pub(crate) struct PreparedSavedLocalView {
    saved: SavedLocalView,
}

impl PreparedSavedLocalView {
    /// Validate before replacing either the world or its input owner. Stable
    /// IDs are resolved only against the incoming world, never the outgoing one.
    pub(crate) fn new(
        saved: SavedLocalView,
        simulation: &crate::sim::world::Simulation,
        rules: &crate::rules::ruleset::RuleSet,
    ) -> Result<Self, SavedLocalViewError> {
        saved.validate_structure()?;
        let follow_target = saved.follow_target();
        for (field, references) in [
            ("selection", saved.selection_order()),
            ("Follow", follow_target.as_slice()),
        ] {
            for &object_id in references {
                if simulation.entities().get(object_id).is_none() {
                    return Err(SavedLocalViewError::MissingEntityReference { field, object_id });
                }
            }
        }
        for &object_id in saved.control_groups().iter().flatten() {
            if simulation.entities().get(object_id).is_none() {
                return Err(SavedLocalViewError::MissingEntityReference {
                    field: "control group",
                    object_id,
                });
            }
        }
        let (order, pending) = crate::app::input::dispatch::selection_order_for_world(
            simulation,
            Some(rules),
            saved.selection_order(),
            saved.selection_order_pending(),
        );
        if order != saved.selection_order() || pending != saved.selection_order_pending() {
            return Err(SavedLocalViewError::SelectionTimelineMismatch);
        }
        Ok(Self { saved })
    }

    /// Restore exactly the saved owners after navigation's world-replacement
    /// invalidation. Apply the returned world-pixel center through
    /// camera::restore_saved_camera_center once new geometry is installed.
    pub(crate) fn commit_into(self, input: &mut MatchInputState) -> [f32; 2] {
        let center = self.saved.camera_center_world_px();
        input
            .view_bookmarks
            .restore_saved_cells(self.saved.view_bookmarks());
        let (order, pending, groups, follow) = self.saved.into_local_input();
        input.selection_order = order;
        input.selection_order_pending = pending;
        input.control_groups = groups.into();
        input.follow_target = follow;
        // ClearScene685640 and PostLoad6851D2 call734210, which resets the
        // last team slot to -1 and rewinds both click clocks. None has the
        // same next-press behavior without persisting a wall-clock Instant.
        // Original reset witnesses: saved_game_layout.json/postload_clock_reset.
        input.last_control_group_press = None;
        center
    }
}

pub(crate) struct MatchInputState {
    pub(crate) camera_x: f32,
    pub(crate) camera_y: f32,
    /// Camera-owned requests awaiting Tactical__AI's next single commit.
    pub(crate) pending_camera_scroll: crate::app::input::camera::PendingCameraScroll,
    /// Current zoom level for the game viewport. 1.0 = native pixel scale,
    /// >1.0 = zoomed in (world appears larger), <1.0 = zoomed out (see more map).
    /// Animated each frame toward `zoom_target`.
    pub(crate) zoom_level: f32,
    /// Target zoom level — mouse wheel sets this; `zoom_level` eases toward it.
    pub(crate) zoom_target: f32,
    /// World-space anchor point for zoom animation. The camera adjusts each frame
    /// so this world point stays at `zoom_anchor_screen` during the zoom ease.
    pub(crate) zoom_anchor_world: [f32; 2],
    /// Screen-space position of the zoom anchor (cursor position when wheel fired).
    pub(crate) zoom_anchor_screen: [f32; 2],
    /// Mouse edge auto-scroll ramp state (gamemd's CoastLevel and its 16 ms timer).
    pub(crate) edge_scroll: crate::app::input::camera::EdgeScrollState,
    /// Tactical mouse capture and right-drag pan anchor.
    pub(crate) tactical_mouse: crate::app::input::camera::TacticalMouseState,
    /// The four camera bookmarks (View1..4 / SetView1..4).
    pub(crate) view_bookmarks: crate::app::input::camera::ViewBookmarks,
    pub(crate) cursor_x: f32,
    pub(crate) cursor_y: f32,
    pub(crate) keys_held: HashSet<KeyCode>,
    pub(crate) hotkey_bindings: crate::app::input::hotkeys::HotkeyBindings,
    pub(crate) hotkey_modifiers: ModifiersState,
    /// Hybrid held/tap state for the retail TypeSelect command.
    pub(crate) type_select: crate::app::types::TypeSelectInputState,
    pub(crate) health_navigation:
        crate::app::input::dispatch::selection_navigation::HealthNavigation,
    /// Original CursorCheat toggle A8F7D8, consumed by tactical tooltips.
    pub(crate) cursor_coordinates: bool,
    /// One-shot Shift+S request, consumed at the next render submission.
    pub(crate) retail_screenshot_requested: bool,
    /// True while left-dragging on minimap (camera pan mode).
    pub(crate) minimap_dragging: bool,
    /// Selection drag state — tracks mouse drag for box-select.
    pub(crate) selection_state: crate::sim::selection::SelectionState,
    /// Player-side `g_CurrentObjects` order. Selection commands update this
    /// immediately; the post-sim reconciliation removes lifecycle departures.
    pub(crate) selection_order: Vec<u64>,
    /// A queued selection command has not yet reached the simulation tick.
    pub(crate) selection_order_pending: bool,
    /// Existing selection paths speak by default; held TypeSelect batches
    /// temporarily suppress and restore this latch.
    pub(crate) selection_voice_enabled: bool,
    /// Pending order mode for the next right-click command.
    pub(crate) queued_order_mode: crate::app::types::OrderMode,
    /// Control group slots (0-9) storing stable entity ids.
    pub(crate) control_groups: Vec<Vec<u64>>,
    /// Slot and wall-clock instant of the last plain control-group recall, for
    /// the 800 ms double-tap that centres the camera. Wall clock, never sim
    /// state: the original stamps `timeGetTime()` here and only a recall writes
    /// it — assigning with Ctrl+digit never does.
    pub(crate) last_control_group_press: Option<(usize, std::time::Instant)>,
    /// The object the camera is following, i.e. `DisplayClass +0x11A0` behind
    /// its valid byte `+0x119C`. `None` is the cleared pair.
    pub(crate) follow_target: Option<u64>,
    /// Mutually-exclusive cursor-on-tactical-map targeting mode (building
    /// placement OR superweapon). Right-click and Esc clear; arming one
    /// kind clears the other.
    pub(crate) targeting_mode: Option<crate::app::types::TargetingMode>,
    /// Current placement preview for the armed building, if any.
    pub(crate) building_placement_preview: Option<crate::sim::production::BuildingPlacementPreview>,
}

impl MatchInputState {
    /// Construct the match input owner with the ordinary startup defaults.
    pub(crate) fn new(hotkey_bindings: crate::app::input::hotkeys::HotkeyBindings) -> Self {
        Self {
            minimap_dragging: false,
            selection_state: crate::sim::selection::SelectionState::new(),
            selection_order: Vec::new(),
            selection_order_pending: false,
            selection_voice_enabled: true,
            queued_order_mode: crate::app::types::OrderMode::Move,
            control_groups: vec![Vec::new(); CONTROL_GROUP_SLOTS],
            last_control_group_press: None,
            follow_target: None,
            targeting_mode: None,
            building_placement_preview: None,
            camera_x: 0.0,
            camera_y: 0.0,
            pending_camera_scroll: Default::default(),
            zoom_level: 1.0,
            zoom_target: 1.0,
            zoom_anchor_world: [0.0, 0.0],
            zoom_anchor_screen: [0.0, 0.0],
            edge_scroll: crate::app::input::camera::EdgeScrollState::default(),
            tactical_mouse: crate::app::input::camera::TacticalMouseState::default(),
            view_bookmarks: crate::app::input::camera::ViewBookmarks::default(),
            cursor_x: 0.0,
            cursor_y: 0.0,
            keys_held: HashSet::new(),
            hotkey_bindings,
            hotkey_modifiers: ModifiersState::empty(),
            type_select: crate::app::types::TypeSelectInputState::default(),
            health_navigation: Default::default(),
            cursor_coordinates: false,
            retail_screenshot_requested: false,
        }
    }

    /// The cursor in whole window pixels, rounded to nearest: the position the
    /// shell screens and the gadget layer hit-test with.
    pub(crate) fn cursor_px(&self) -> (i32, i32) {
        (self.cursor_x.round() as i32, self.cursor_y.round() as i32)
    }

    /// Capture the app-owned portion of a save at the same boundary as its
    /// Simulation. The existing selection owner decides whether its local
    /// order is provisional; this does not drain commands or alter selection.
    pub(crate) fn capture_saved_local_view(
        &self,
        simulation: &crate::sim::world::Simulation,
        rules: &crate::rules::ruleset::RuleSet,
        render_size: (u32, u32),
    ) -> Result<SavedLocalView, SavedLocalViewError> {
        let (order, pending) = crate::app::input::dispatch::selection_order_for_world(
            simulation,
            Some(rules),
            &self.selection_order,
            self.selection_order_pending,
        );
        assert_eq!(self.control_groups.len(), CONTROL_GROUP_SLOTS);
        // Native membership lives on each serialized Techno, not in a list
        // retaining already-deleted objects. Project the existing app lists
        // onto the same entity store being saved, preserving member order.
        let groups = std::array::from_fn(|slot| {
            self.control_groups[slot]
                .iter()
                .copied()
                .filter(|id| simulation.entities().get(*id).is_some())
                .collect()
        });
        SavedLocalView::new(
            crate::app::input::camera::saved_camera_center(self, render_size),
            self.view_bookmarks.saved_cells(),
            order,
            pending,
            groups,
            self.follow_target
                .filter(|id| simulation.entities().get(*id).is_some()),
        )
    }
}

#[cfg(test)]
#[path = "saved_local_view_tests.rs"]
mod saved_local_view_tests;
