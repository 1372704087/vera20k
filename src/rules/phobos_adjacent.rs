//! Phobos `Adjacent.*` (`Ext/BuildingType/Body.cpp:235-238`).
//!
//! The placement proximity check (`DisplayClass::PassesProximityCheck`,
//! hooks `0x4A8F3E`/`0x4A8FD7`, `Ext/BuildingType/Hooks.cpp:200-270`) can
//! restrict which building types count as adjacent providers and forbid the
//! placement near disallowed types.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

/// The four `BuildingTypeExt` proximity fields (`Ext/BuildingType/Body.h`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhobosAdjacent {
    /// `Adjacent.Allowed=` (`ValueableVector<BuildingTypeClass*>`): when set,
    /// only these building types count as providers.
    pub allowed: Vec<String>,
    /// `Adjacent.Disallowed=`: same-house providers this list names are
    /// skipped, or fatal with `Adjacent.Disallowed.Prohibit=`.
    pub disallowed: Vec<String>,
    /// `Adjacent.Disallowed.Prohibit=`, default false.
    pub prohibit: bool,
    /// `Adjacent.Disallowed.ProhibitDistance=`, default 0: the larger ring
    /// the prohibit pass scans before the normal check.
    pub prohibit_distance: i32,
}

impl PhobosAdjacent {
    pub fn from_section(section: &IniSection) -> Self {
        use crate::rules::native_processing::read_valueable_type_vector;
        let resolve = |name: &str| Some(name.to_owned());
        Self {
            allowed: read_valueable_type_vector(section, "Adjacent.Allowed", &[], resolve),
            disallowed: read_valueable_type_vector(section, "Adjacent.Disallowed", &[], resolve),
            prohibit: section.read_ares_bool("Adjacent.Disallowed.Prohibit", false),
            prohibit_distance: section.read_ares_int("Adjacent.Disallowed.ProhibitDistance", 0),
        }
    }

    pub fn permits_provider(&self, type_name: &str) -> bool {
        self.allowed.is_empty()
            || self
                .allowed
                .iter()
                .any(|name| name.eq_ignore_ascii_case(type_name))
    }

    pub fn disallows(&self, type_name: &str) -> bool {
        self.disallowed
            .iter()
            .any(|name| name.eq_ignore_ascii_case(type_name))
    }
}

#[cfg(test)]
#[path = "phobos_adjacent_tests.rs"]
mod tests;
