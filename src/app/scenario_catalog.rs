//! Scenario catalog (F11): one owner for the skirmish scenario records and
//! the shell map entries projected from them.
//!
//! Before this owner, `skirmish_scenario_records` and `skirmish_shell_maps`
//! were two parallel `AppState` vectors reconciled by hand: the random-map
//! flow mutated the records and then patched the projected list separately by
//! name-position, and nothing else re-projected — any future record mutation
//! silently broke the projection. Here the projection is derived state: reads
//! are borrows, and the only mutable access re-projects when it drops, so the
//! two lists cannot drift.

use crate::app::loading::init::MapMenuEntry;
use crate::map::skirmish_scenarios::SkirmishScenarioRecord;

pub(crate) struct ScenarioCatalog {
    records: Vec<SkirmishScenarioRecord>,
    /// Always exactly `records.iter().map(to_map_menu_entry)`.
    shell_maps: Vec<MapMenuEntry>,
}

impl ScenarioCatalog {
    pub(crate) fn from_records(records: Vec<SkirmishScenarioRecord>) -> Self {
        let mut catalog = Self {
            records,
            shell_maps: Vec::new(),
        };
        catalog.reproject();
        catalog
    }

    pub(crate) fn records(&self) -> &[SkirmishScenarioRecord] {
        &self.records
    }

    pub(crate) fn shell_maps(&self) -> &[MapMenuEntry] {
        &self.shell_maps
    }

    /// Mutable access to the records. The guard re-projects the shell map
    /// entries when it drops, so every mutation path — including the modal's
    /// random-map sentinel upsert — keeps the projection exact.
    pub(crate) fn records_mut(&mut self) -> RecordsMut<'_> {
        RecordsMut { catalog: self }
    }

    fn reproject(&mut self) {
        self.shell_maps = self
            .records
            .iter()
            .map(SkirmishScenarioRecord::to_map_menu_entry)
            .collect();
    }
}

pub(crate) struct RecordsMut<'a> {
    catalog: &'a mut ScenarioCatalog,
}

impl std::ops::Deref for RecordsMut<'_> {
    type Target = Vec<SkirmishScenarioRecord>;
    fn deref(&self) -> &Self::Target {
        &self.catalog.records
    }
}

impl std::ops::DerefMut for RecordsMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.catalog.records
    }
}

impl Drop for RecordsMut<'_> {
    fn drop(&mut self) {
        self.catalog.reproject();
    }
}
