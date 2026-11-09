//! Phobos `BuildLimitGroup.*` type keys (`Ext/TechnoType/Body.cpp:1033-1041`;
//! fields and defaults `Ext/TechnoType/Body.h:265-273,697-705`) and the two
//! `[General]` fallbacks (`Ext/Rules/Body.cpp:223-224`, `Body.h:130-131,667-668`).
//!
//! The consumer is the House build-limit gate in `sim::production::can_build`
//! (`HouseExt::BuildLimitGroupUpgradeCheck @ Ext/House/Body.cpp:802-1013`,
//! `HouseExt::ReachedBuildLimit @ Ext/House/Body.cpp:1036-1188`). Type names
//! are kept as `Vec<String>` and resolved at use time: a `ValueableVector`
//! has no `<default>` form and the consumer must treat an unresolved name as
//! a group member that contributes nothing (the same rule as
//! `phobos_grinding`'s `grinding_keeps_unresolved_type_names_for_the_consumer`).
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

/// The nine `BuildLimitGroup.*` keys on one TechnoType. Registered on
/// `ObjectType` under `#[cfg(feature = "phobos")]`; this module is gated the
/// same way, so stock builds do not compile any of these fields.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildLimitGroupSettings {
    /// `BuildLimitGroup.Types=`, `ValueableVector<TechnoTypeClass*>`, default
    /// empty. Unresolved names stay in the list for the consumer to resolve.
    pub types: Vec<String>,
    /// `BuildLimitGroup.Nums=`, `ValueableVector<int>`, default empty. Tokens
    /// are kept positionally (see [`read_aligned_ints`]).
    pub nums: Vec<i32>,
    /// `BuildLimitGroup.Factor=`, **`Valueable<int>`** (not double), default 1.
    pub factor: i32,
    /// `BuildLimitGroup.ContentIfAnyMatch=`, **`Nullable<bool>`**: unset falls
    /// back to `[General] BuildLimitGroup.ContentIfAnyMatch`.
    pub content_if_any_match: Option<bool>,
    /// `BuildLimitGroup.NotBuildableIfQueueMatch=`, **`Nullable<bool>`**: unset
    /// falls back to `[General] BuildLimitGroup.NotBuildableIfQueueMatch`.
    pub not_buildable_if_queue_match: Option<bool>,
    /// `BuildLimitGroup.ExtraLimit.Types=`, `ValueableVector<TechnoTypeClass*>`.
    pub extra_limit_types: Vec<String>,
    /// `BuildLimitGroup.ExtraLimit.Nums=`, `ValueableVector<int>` (aligned).
    pub extra_limit_nums: Vec<i32>,
    /// `BuildLimitGroup.ExtraLimit.MaxCount=`, `ValueableVector<int>` (aligned).
    pub extra_limit_max_count: Vec<i32>,
    /// `BuildLimitGroup.ExtraLimit.MaxNum=`, `Valueable<int>`, default 0
    /// (`<= 0` means no cap).
    pub extra_limit_max_num: i32,
}

impl Default for BuildLimitGroupSettings {
    fn default() -> Self {
        Self {
            types: Vec::new(),
            nums: Vec::new(),
            factor: 1,
            content_if_any_match: None,
            not_buildable_if_queue_match: None,
            extra_limit_types: Vec::new(),
            extra_limit_nums: Vec::new(),
            extra_limit_max_count: Vec::new(),
            extra_limit_max_num: 0,
        }
    }
}

/// `ValueableVector<int>` that keeps one slot per comma token.
///
/// `IniSection::read_ares_int_list` (`ini_value.rs:627`) uses `filter_map` and
/// drops tokens that fail Ares `Parser<int>`, which would shift the `Nums` /
/// `MaxCount` pairing against `Types` when a token is unparseable. Instead go
/// through `read_list` (`ini_value.rs:1043`, the same `strtok(",")` tokens a
/// native `ValueableVector` is read from) and parse each token with the
/// existing `parse_ares_int` core (`ini_value.rs:1957`, Ares/Phobos
/// `Parser<int>`), keeping `0` as an aligned placeholder for a rejected token.
/// No `str::parse` is used. An absent key leaves the vector empty.
fn read_aligned_ints(section: &IniSection, key: &str) -> Vec<i32> {
    section
        .read_list(key, 0x800)
        .map(|tokens| {
            tokens
                .into_iter()
                .map(|token| crate::rules::ini_value::parse_ares_int(token).unwrap_or(0))
                .collect()
        })
        .unwrap_or_default()
}

impl BuildLimitGroupSettings {
    pub fn from_section(section: &IniSection) -> Self {
        use crate::rules::native_processing::read_valueable_type_vector;
        // `ValueableVector<TechnoTypeClass*>` resolves at use time; keep every
        // name (phobos_grinding.rs's `grinding_keeps_unresolved_type_names`).
        let keep = |name: &str| Some(name.to_owned());
        Self {
            types: read_valueable_type_vector(section, "BuildLimitGroup.Types", &[], keep),
            nums: read_aligned_ints(section, "BuildLimitGroup.Nums"),
            factor: section.read_ares_int("BuildLimitGroup.Factor", 1),
            content_if_any_match: section
                .read_phobos_nullable_bool("BuildLimitGroup.ContentIfAnyMatch", None),
            not_buildable_if_queue_match: section
                .read_phobos_nullable_bool("BuildLimitGroup.NotBuildableIfQueueMatch", None),
            extra_limit_types: read_valueable_type_vector(
                section,
                "BuildLimitGroup.ExtraLimit.Types",
                &[],
                keep,
            ),
            extra_limit_nums: read_aligned_ints(section, "BuildLimitGroup.ExtraLimit.Nums"),
            extra_limit_max_count: read_aligned_ints(
                section,
                "BuildLimitGroup.ExtraLimit.MaxCount",
            ),
            extra_limit_max_num: section.read_ares_int("BuildLimitGroup.ExtraLimit.MaxNum", 0),
        }
    }
}

#[cfg(test)]
#[path = "phobos_build_limit_group_tests.rs"]
mod tests;
