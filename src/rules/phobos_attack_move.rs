//! Phobos `AttackMove.*` (`Ext/TechnoType/Body.cpp:1086-1087`,
//! `Ext/Rules/Body.cpp:377-378,423`).
//!
//! Two per-type keys refine the native attack-move (MegaMission 29):
//! - `.Aggressive` lets an attack-mover treat an enemy building with no
//!   weapon and no `ThreatPosed=` as a legal auto-target;
//! - `.UpdateTarget` lets it swap a held target it cannot use for the
//!   greatest threat within range.
//!
//! Both are `Nullable<bool>` and fall back to the same-named `[General]`
//! flag, which is `Valueable<bool>` and therefore has no unset state
//! (`Ext/Rules/Body.cpp:377-378`, constructor `Ext/Rules/Body.h:800-801`).
//!
//! The third key of the family, `[General] AttackMove.IgnoreWeaponCheck`
//! (`Ext/Rules/Body.cpp:423`, constructor `Ext/Rules/Body.h:853`), is global
//! only: it drops the native "must own a `Primary=`" test from
//! `TechnoTypeClass::Can_Attack_Move @ 0x00711E90`. It lives on
//! [`crate::rules::phobos_globals::PhobosGlobals`] and is consumed by
//! [`crate::rules::object_type::ObjectType::can_attack_move_with`].
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;
use crate::rules::phobos_globals::PhobosGlobals;

/// `TechnoTypeExt::AttackMove_Aggressive` / `AttackMove_UpdateTarget`
/// (`Ext/TechnoType/Body.h`, constructed unset at `Body.cpp:1086-1087`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PhobosAttackMove {
    /// `AttackMove.Aggressive=`: unarmed enemy buildings become legal
    /// auto-targets while this object is on an attack move. `None` inherits
    /// `[General] AttackMove.Aggressive` (default `false`).
    pub aggressive: Option<bool>,
    /// `AttackMove.UpdateTarget=`: the arrival handler may drop a held target
    /// it cannot use and rescan. `None` inherits
    /// `[General] AttackMove.UpdateTarget` (default `false`).
    pub update_target: Option<bool>,
}

impl PhobosAttackMove {
    /// The two `Nullable<bool>` type keys. `None` stays "unset" so the
    /// consumer can see the `[General]` fallback.
    pub fn from_section(section: &IniSection) -> Self {
        Self {
            aggressive: section.read_phobos_nullable_bool("AttackMove.Aggressive", None),
            update_target: section.read_phobos_nullable_bool("AttackMove.UpdateTarget", None),
        }
    }

    /// `AttackMove_Aggressive.Get(RulesExt::Global()->AttackMove_Aggressive)`
    /// (`Ext/Techno/Hooks.TargetEvaluation.cpp:214`).
    pub fn aggressive(&self, globals: &PhobosGlobals) -> bool {
        self.aggressive.unwrap_or(globals.attack_move_aggressive)
    }

    /// `AttackMove_UpdateTarget.Get(RulesExt::Global()->AttackMove_UpdateTarget)`
    /// (`Ext/Techno/Hooks.TargetEvaluation.cpp:185-186`).
    pub fn update_target(&self, globals: &PhobosGlobals) -> bool {
        self.update_target
            .unwrap_or(globals.attack_move_update_target)
    }
}

#[cfg(test)]
#[path = "phobos_attack_move_tests.rs"]
mod tests;
