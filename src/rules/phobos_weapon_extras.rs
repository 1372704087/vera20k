//! Phobos `WeaponTypeExt` keys that no other rules owner reads.
//!
//! Read in `WeaponTypeExt::LoadFromINIFile` order: `FeedbackWeapon`
//! (`Ext/WeaponType/Body.cpp:124`; `Valueable<WeaponTypeClass*>` declared
//! `Ext/WeaponType/Body.h:56`, constructed null `:151`), `VisualScatter`
//! (`Body.cpp:133`; `Valueable<bool>` `Body.h:63`, constructed `no` `:158`) and
//! `AttackNoThreatBuildings` (`Body.cpp:206`; `Nullable<bool>`
//! `Ext/WeaponType/Body.h:114`, constructed unset `:210`).
//!
//! Two of the three carry a `[General]`/`[AudioVisual]` fallback:
//! - `AttackNoThreatBuildings` has none of its own: the consumer supplies
//!   `[General] AutoTarget_NoThreatBuildings` for a human scanner and
//!   `[General] AutoTargetAI_NoThreatBuildings` for a computer one
//!   (`Ext/Techno/Hooks.cpp:1848-1858`, defaults `Ext/Rules/Body.h:931-932`).
//! - `VisualScatter`'s radius is the `[AudioVisual] VisualScatter.Min/Max`
//!   pair (`Ext/Rules/Body.cpp:325-326`, defaults 8/32 leptons
//!   `Ext/Rules/Body.h:758-759`), drawn per fired effect.
//!
//! `FeedbackWeapon` has no fallback: a weapon that does not name one fires
//! nothing extra.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;
use crate::rules::ruleset::RuleSet;

/// The three `WeaponTypeExt` keys owned here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PhobosWeaponExtras {
    /// `FeedbackWeapon=` (`Body.cpp:124`, read `<true>`): the weapon also
    /// detonated at the firer right after a shot
    /// (`Ext/Techno/Hooks.Firing.cpp:802-809`).
    pub feedback_weapon: Option<String>,
    /// `VisualScatter=` (`Body.cpp:133`), default `no`: the drawn laser, bolt
    /// or beam ends at a random point near the bullet target
    /// (`Ext/Techno/Hooks.WeaponEffects.cpp:231-244`).
    pub visual_scatter: bool,
    /// `AttackNoThreatBuildings=` (`Body.cpp:206`), `Nullable`: whether the
    /// scanner may auto-target an enemy building that is unarmed or poses no
    /// threat (`Ext/Techno/Hooks.cpp:1848-1858`).
    pub attack_no_threat_buildings: Option<bool>,
}

impl PhobosWeaponExtras {
    pub fn from_section(section: &IniSection) -> Self {
        Self {
            feedback_weapon: section
                .read_name("FeedbackWeapon", 0x100)
                .map(str::to_owned),
            visual_scatter: section.read_ares_bool("VisualScatter", false),
            attack_no_threat_buildings: section
                .read_phobos_nullable_bool("AttackNoThreatBuildings", None),
        }
    }

    /// `AttackNoThreatBuildings.Get(human ? [General] AutoTarget_NoThreatBuildings
    /// : [General] AutoTargetAI_NoThreatBuildings)`
    /// (`Ext/Techno/Hooks.cpp:1852-1858`).
    pub fn attack_no_threat_buildings(&self, rules: &RuleSet, human: bool) -> bool {
        self.attack_no_threat_buildings.unwrap_or(if human {
            rules.phobos_globals.auto_target_no_threat_buildings
        } else {
            rules.phobos_globals.auto_target_ai_no_threat_buildings
        })
    }
}

#[cfg(test)]
#[path = "phobos_weapon_extras_tests.rs"]
mod tests;
