//! Saved local view facts, independent of window, renderer and input devices.
//!
//! Active-retail authorities: TacticalSave6DBE00/Load6DBD20 (committed
//! center), ScenarioSave689310/Load689470 (four cell bookmarks),
//! MiscSave67F82A/Load67FA2A (ordered selection), Techno+214 (one group
//! membership), and MouseLoad5BE117..123 (Follow target). Original executed
//! camera/bookmark controls live in tools/input_oracle/fast_scroll.json and
//! tools/spatial_oracle/native_id_snapshot.json. This record is VERA's local
//! view payload, not a description of the native SAV byte layout.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Ten native team-selection slots; an object belongs to at most one slot.
pub(crate) const CONTROL_GROUP_SLOTS: usize = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SavedLocalView {
    /// Committed tactical center in VERA world pixels, including its existing
    /// projection bias. Window dimensions, pending scroll and zoom are absent.
    camera_center_world_px: [f32; 2],
    /// Raw signed CellStruct words preserved in the app's u16 bit-pattern form.
    view_bookmarks: [[u16; 2]; 4],
    selection_order: Vec<u64>,
    /// App projection of the serialized Simulation Select queue. No command
    /// payload is duplicated here; admission checks the existing queue owner.
    selection_order_pending: bool,
    control_groups: [Vec<u64>; CONTROL_GROUP_SLOTS],
    follow_target: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum SavedLocalViewError {
    #[error("saved tactical camera center is not finite")]
    InvalidCameraCenter,
    #[error("saved selection repeats object {object_id}")]
    DuplicateSelectionReference { object_id: u64 },
    #[error("saved control groups repeat object {object_id}")]
    DuplicateGroupReference { object_id: u64 },
    #[error("saved {field} references absent object {object_id}")]
    MissingEntityReference { field: &'static str, object_id: u64 },
    #[error("saved selection order does not match its simulation selection timeline")]
    SelectionTimelineMismatch,
}

impl SavedLocalView {
    pub(crate) fn new(
        camera_center_world_px: [f32; 2],
        view_bookmarks: [[u16; 2]; 4],
        selection_order: Vec<u64>,
        selection_order_pending: bool,
        control_groups: [Vec<u64>; CONTROL_GROUP_SLOTS],
        follow_target: Option<u64>,
    ) -> Result<Self, SavedLocalViewError> {
        let saved = Self {
            camera_center_world_px,
            view_bookmarks,
            selection_order,
            selection_order_pending,
            control_groups,
            follow_target,
        };
        saved.validate_structure()?;
        Ok(saved)
    }

    /// Also called after deserialization, before any app state is mutated.
    /// Object references and queued selection are validated against the new
    /// world by the input owner, after that world's own restore validation.
    pub(crate) fn validate_structure(&self) -> Result<(), SavedLocalViewError> {
        if !self
            .camera_center_world_px
            .iter()
            .all(|value| value.is_finite())
        {
            return Err(SavedLocalViewError::InvalidCameraCenter);
        }
        let mut selected = BTreeSet::new();
        for &object_id in &self.selection_order {
            if !selected.insert(object_id) {
                return Err(SavedLocalViewError::DuplicateSelectionReference { object_id });
            }
        }
        let mut grouped = BTreeSet::new();
        for &object_id in self.control_groups.iter().flatten() {
            if !grouped.insert(object_id) {
                return Err(SavedLocalViewError::DuplicateGroupReference { object_id });
            }
        }
        Ok(())
    }

    pub(crate) fn camera_center_world_px(&self) -> [f32; 2] {
        self.camera_center_world_px
    }

    pub(crate) fn view_bookmarks(&self) -> [[u16; 2]; 4] {
        self.view_bookmarks
    }

    pub(crate) fn selection_order(&self) -> &[u64] {
        &self.selection_order
    }

    pub(crate) fn selection_order_pending(&self) -> bool {
        self.selection_order_pending
    }

    pub(crate) fn control_groups(&self) -> &[Vec<u64>; CONTROL_GROUP_SLOTS] {
        &self.control_groups
    }

    pub(crate) fn follow_target(&self) -> Option<u64> {
        self.follow_target
    }

    pub(crate) fn into_local_input(
        self,
    ) -> (Vec<u64>, bool, [Vec<u64>; CONTROL_GROUP_SLOTS], Option<u64>) {
        (
            self.selection_order,
            self.selection_order_pending,
            self.control_groups,
            self.follow_target,
        )
    }
}
