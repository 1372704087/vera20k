//! Phobos extended `ForbidParallelAIQueues` controls.
//!
//! `[GlobalControls] AllowParallelAIQueues=` is Ares' gate for AI factories
//! producing the same category in parallel. Phobos adds per-category and
//! per-TechnoType forbids that only narrow that feature; with the gate off
//! they do nothing. Phobos' own `RulesExt` default is true, but Ares reads
//! the same key with a false default, so the effective Phobos+Ares behavior
//! requires `=yes`; VERA has no Ares serial mode, so it reads the gate with
//! the Ares default and keeps stock parallel production when it is off.
//!
//! Sources: `RulesExt::ForbidParallelAIQueues_*` reads in
//! `src/Ext/Rules/Body.cpp`, `TechnoTypeExt::ForbidParallelAIQueues` in
//! `src/Ext/TechnoType/Body.cpp`, and the hooks in
//! `src/Ext/Building/Hooks.Production.cpp`.

use crate::rules::ini_parser::IniFile;
use crate::rules::object_type::ObjectCategory;

/// Ares-Phobos parallel AI queue switches, all defaulting to off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParallelAiQueueRules {
    allow: bool,
    forbid_infantry: bool,
    forbid_vehicle: bool,
    forbid_navy: bool,
    forbid_aircraft: bool,
    forbid_building: bool,
}

impl ParallelAiQueueRules {
    pub fn from_ini(ini: &IniFile) -> Self {
        let controls = ini.section("GlobalControls");
        let read = |key: &str| controls.is_some_and(|section| section.read_bool(key, false));
        Self {
            allow: read("AllowParallelAIQueues"),
            forbid_infantry: read("ForbidParallelAIQueues.Infantry"),
            forbid_vehicle: read("ForbidParallelAIQueues.Vehicle"),
            forbid_navy: read("ForbidParallelAIQueues.Navy"),
            forbid_aircraft: read("ForbidParallelAIQueues.Aircraft"),
            forbid_building: read("ForbidParallelAIQueues.Building"),
        }
    }

    /// Whether the Ares gate enables the Phobos restrictions at all.
    pub const fn allows_parallel(&self) -> bool {
        self.allow
    }

    /// The category-wide forbid (`ForbidParallelAIQueues.<Category>=`).
    pub const fn forbids_category(&self, category: ObjectCategory, naval: bool) -> bool {
        match category {
            ObjectCategory::Infantry => self.forbid_infantry,
            ObjectCategory::Vehicle if naval => self.forbid_navy,
            ObjectCategory::Vehicle => self.forbid_vehicle,
            ObjectCategory::Aircraft => self.forbid_aircraft,
            ObjectCategory::Building => self.forbid_building,
        }
    }

    pub(crate) fn hash_into<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        b"phobos-parallel-ai-queues-v1".hash(hasher);
        self.allow.hash(hasher);
        self.forbid_infantry.hash(hasher);
        self.forbid_vehicle.hash(hasher);
        self.forbid_navy.hash(hasher);
        self.forbid_aircraft.hash(hasher);
        self.forbid_building.hash(hasher);
    }
}
