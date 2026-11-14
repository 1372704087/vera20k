//! Phobos `WeaponTypeExt::LaserZAdjust` / `EBoltZAdjust`
//! (`src/Ext/WeaponType/Body.cpp:130-131`, `Body.h:60-61`, constructor leaves
//! both unset at `Body.cpp:155-156`) and the `[AudioVisual]` defaults they
//! inherit from `RulesExt` (`src/Ext/Rules/Body.cpp:277-278`, `Body.h:195-196`,
//! constructor `0` at `Body.h:718-719`).
//!
//! Both adjust the beam's initial draw depth when it is created:
//! `LaserZAdjust` in the LaserDraw hook (`Misc/Hooks.LaserDraw.cpp:49-57`) and
//! `EBoltZAdjust` in `TechnoClass::FireEBolt`
//! (`Ext/EBolt/Hooks.cpp:208-220`) and in the simulated bolt
//! (`Ext/Bullet/Body.cpp:270-280`).
//!
//! VERA stores the adjustment on the created beam (`sim::combat::laser` /
//! `sim::combat::electric_bolt`), which is the same value the draw would read
//! one frame later; the per-draw hook site is not a separate owner.

use crate::rules::ini_parser::IniFile;

/// `RulesExt`'s `[AudioVisual] LaserZAdjust=` / `EBoltZAdjust=`
/// (`Valueable<int>`, constructor default 0) and
/// `EBoltZAdjust.ClampInitialDepthForBuilding=`
/// (`Valueable<bool>`, constructor default **true**, `Ext/Rules/Body.h:720`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BeamZAdjustGlobals {
    pub laser: i32,
    pub ebolt: i32,
    pub ebolt_clamp_initial_depth_for_building: bool,
}

impl Default for BeamZAdjustGlobals {
    fn default() -> Self {
        Self {
            laser: 0,
            ebolt: 0,
            // `Body.h:720`: the constructor default, not an absent-key fallback.
            ebolt_clamp_initial_depth_for_building: true,
        }
    }
}

impl BeamZAdjustGlobals {
    pub fn from_ini(ini: &IniFile) -> Self {
        let audio_visual = ini.section_or_empty("AudioVisual");
        Self {
            laser: audio_visual.read_int("LaserZAdjust", 0),
            ebolt: audio_visual.read_int("EBoltZAdjust", 0),
            // `Body.cpp:279`, read right after `EBoltZAdjust` (`:278`).
            ebolt_clamp_initial_depth_for_building: audio_visual
                .read_bool("EBoltZAdjust.ClampInitialDepthForBuilding", true),
        }
    }
}

/// `Nullable<int>::Get(global)`: the weapon's own value, else the global one.
pub fn resolved(weapon: Option<i32>, global: i32) -> i32 {
    weapon.unwrap_or(global)
}

/// `Nullable<bool>::Get(global)`: the weapon's own clamp, else the global one.
pub fn resolved_clamp(weapon: Option<bool>, global: bool) -> bool {
    weapon.unwrap_or(global)
}

#[cfg(all(test, feature = "phobos"))]
#[path = "phobos_beam_z_adjust_tests.rs"]
mod tests;
