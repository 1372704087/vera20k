//! Retired trigger-link diagnostics.
//!
//! Nothing builds this graph any more: it linked `[CellTags]`, `[Tags]` and
//! `[Triggers]` by upper-cased IDs with a second `[CellTags]` reader, while
//! the runtime reader in `sim::trigger_runtime` resolves them as gamemd does.
//! The empty type stays only because `headless_scenario.rs` binds it and the
//! saved scenario resources serialize it; removing it changes the save layout.

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LinkedTrigger {
    pub trigger_id: String,
    pub tag_ids: Vec<String>,
    pub tagged_cells: Vec<(u16, u16)>,
    pub event_id: Option<String>,
    pub action_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TriggerGraph {
    pub triggers: Vec<LinkedTrigger>,
    pub dangling_cell_tags: Vec<String>,
    pub dangling_tag_trigger_refs: Vec<String>,
    pub untagged_triggers: Vec<String>,
}
