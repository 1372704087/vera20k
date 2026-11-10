//! Phobos `SWTypeExt`'s `LimboDelivery.*`/`LimboKill.*` keys.
//!
//! Reader `SWTypeExt::LoadFromINIFile` (`src/Ext/SWType/Body.cpp:159-169`):
//! `LimboDelivery.Types` (`ValueableVector<BuildingTypeClass*>`),
//! `.IDs`/`LimboKill.IDs`/`.Counts` (`ValueableVector<int>`),
//! `.RollChances` (`ValueableVector<float>`), the numbered
//! `LimboDelivery.RandomWeightsN` lists with their unnumbered slot-0 alias,
//! and `LimboKill.AffectsHouse` (`Valueable<AffectedHouse>`, constructor
//! `Owner`) read after the deprecated `LimboKill.Affected` spelling
//! (`Body.cpp:162-166`). Declarations and defaults: `Body.h:65-70`, `:98`,
//! `:164-168`.
//!
//! Consumers, in `SWTypeExt::FireSuperWeaponExt`
//! (`Ext/SWType/FireSuperWeapon.cpp:13-40`): `ApplyLimboDelivery` (`:172-201`)
//! and `ApplyLimboKill` (`:203-262`), run before `SW.Next`.
//!
//! `ValueableVector<BuildingTypeClass*>` resolves each token through
//! `BuildingTypeClass::Find`; an unknown name is dropped at read time and a
//! reached list replaces the field. The int and float vectors keep their
//! tokens in order; an unconvertible token is skipped.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;
use crate::rules::phobos_shields::{AffectedHouse, read_affected_house};
use crate::util::native_x87::NativeF32Bits;

/// One superweapon type's limbo-delivery and limbo-kill settings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct PhobosSuperWeaponLimbo {
    /// `LimboDelivery.Types=`: the buildings delivered on launch.
    types: Vec<String>,
    /// `LimboDelivery.IDs=`: the `LimboID` each delivered building takes,
    /// carried over to later entries when the list is shorter.
    ids: Vec<i32>,
    /// `LimboDelivery.RollChances=`: binary32 chance per weighted roll.
    roll_chances: Vec<NativeF32Bits>,
    /// `LimboDelivery.RandomWeightsN=` / `.RandomWeights`.
    random_weights: Vec<Vec<i32>>,
    /// `LimboKill.IDs=`: the delivered `LimboID`s to remove.
    kill_ids: Vec<i32>,
    /// `LimboKill.Counts=`: per-id removal caps; a missing entry removes all.
    kill_counts: Vec<i32>,
    /// `LimboKill.AffectsHouse=` (legacy `LimboKill.Affected`), constructor
    /// `Owner`: which houses' delivered lists are walked.
    kill_affects_house: AffectedHouse,
}

impl Default for PhobosSuperWeaponLimbo {
    fn default() -> Self {
        Self {
            types: Vec::new(),
            ids: Vec::new(),
            roll_chances: Vec::new(),
            random_weights: Vec::new(),
            kill_ids: Vec::new(),
            kill_counts: Vec::new(),
            kill_affects_house: AffectedHouse::OWNER,
        }
    }
}

impl PhobosSuperWeaponLimbo {
    /// `SWTypeExt::LoadFromINIFile`'s limbo reads. `resolve_building` answers
    /// the canonical registered building name (`BuildingTypeClass::Find`).
    pub(crate) fn from_section(
        section: &IniSection,
        resolve_building: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let defaults = Self::default();
        let types = section
            .read_list("LimboDelivery.Types", 2048)
            .map(|tokens| {
                tokens
                    .into_iter()
                    .filter_map(|token| resolve_building(token.trim()))
                    .collect()
            })
            .unwrap_or_default();
        // `LimboKill.Affected` is the deprecated spelling and is read first;
        // the canonical key then overrides it when both are present.
        let affects =
            read_affected_house(section, "LimboKill.Affected", defaults.kill_affects_house);
        let kill_affects_house = read_affected_house(section, "LimboKill.AffectsHouse", affects);
        Self {
            types,
            ids: read_int_vector(section, "LimboDelivery.IDs"),
            roll_chances: read_roll_chances(section, "LimboDelivery.RollChances"),
            random_weights: read_random_weights(section, "LimboDelivery"),
            kill_ids: read_int_vector(section, "LimboKill.IDs"),
            kill_counts: read_int_vector(section, "LimboKill.Counts"),
            kill_affects_house,
        }
    }

    pub(crate) fn types(&self) -> &[String] {
        &self.types
    }

    pub(crate) fn ids(&self) -> &[i32] {
        &self.ids
    }

    pub(crate) fn roll_chances(&self) -> Vec<f64> {
        self.roll_chances
            .iter()
            .map(|bits| f64::from(f32::from_bits(bits.bits())))
            .collect()
    }

    pub(crate) fn random_weights(&self) -> &[Vec<i32>] {
        &self.random_weights
    }

    pub(crate) fn kill_ids(&self) -> &[i32] {
        &self.kill_ids
    }

    pub(crate) fn kill_counts(&self) -> &[i32] {
        &self.kill_counts
    }

    pub(crate) fn kill_affects_house(&self) -> AffectedHouse {
        self.kill_affects_house
    }
}

fn read_int_vector(section: &IniSection, key: &str) -> Vec<i32> {
    section.read_int_list(key).unwrap_or_default()
}

/// `ValueableVector<float>`: a present list replaces the field; a token that
/// does not convert is skipped.
fn read_roll_chances(section: &IniSection, key: &str) -> Vec<NativeF32Bits> {
    section
        .read_phobos_double_list(key)
        .map(|chances| {
            chances
                .into_iter()
                .map(|chance| NativeF32Bits::from_bits((chance as f32).to_bits()))
                .collect()
        })
        .unwrap_or_default()
}

/// The numbered weight lists, then the unnumbered alias for slot 0; the loop
/// stops at the first empty list (`Body.cpp:222-256`).
fn read_random_weights(section: &IniSection, prefix: &str) -> Vec<Vec<i32>> {
    let mut lists = Vec::new();
    for index in 0.. {
        let key = format!("{prefix}.RandomWeights{index}");
        let Some(weights) = section.read_int_list(&key) else {
            break;
        };
        if weights.is_empty() {
            break;
        }
        lists.push(weights);
    }
    let alias = format!("{prefix}.RandomWeights");
    if let Some(weights) = section.read_int_list(&alias)
        && !weights.is_empty()
    {
        if lists.is_empty() {
            lists.push(weights);
        } else {
            lists[0] = weights;
        }
    }
    lists
}

#[cfg(test)]
#[path = "phobos_limbo_tests.rs"]
mod tests;
