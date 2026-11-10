//! Phobos `Detonate.*` superweapon keys (`Ext/SWType/Body.cpp:274-278`,
//! consumer `SWTypeExt::ApplyDetonation` `Ext/SWType/FireSuperWeapon.cpp:267-303`).
//!
//! `Detonate.Warhead=` and `Detonate.Weapon=` are `Valueable` type pointers
//! read with `FindOrAllocate` (`Read<true>`); this owner keeps the names and
//! resolves them at the call site. `.Damage` is a `Nullable<int>` whose unset
//! value is the weapon's damage (0 on the warhead arm), `.Warhead.Full` a
//! `Valueable<bool>` default true (a bullet explosion instead of
//! `MapClass::DamageArea`) and `.AtFirer` a `Valueable<bool>` default false
//! (detonate at the launching building's centre instead of the target cell).
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct PhobosSuperWeaponDetonate {
    warhead: Option<String>,
    weapon: Option<String>,
    damage: Option<i32>,
    warhead_full: bool,
    at_firer: bool,
}

impl PhobosSuperWeaponDetonate {
    pub(crate) fn from_section(section: &IniSection) -> Self {
        Self {
            warhead: section
                .read_type_name("Detonate.Warhead", 0x100)
                .map(str::to_owned),
            weapon: section
                .read_type_name("Detonate.Weapon", 0x100)
                .map(str::to_owned),
            damage: section.read_phobos_nullable_int("Detonate.Damage", None),
            warhead_full: section.read_bool("Detonate.Warhead.Full", true),
            at_firer: section.read_bool("Detonate.AtFirer", false),
        }
    }

    pub(crate) fn warhead(&self) -> Option<&str> {
        self.warhead.as_deref()
    }

    pub(crate) fn weapon(&self) -> Option<&str> {
        self.weapon.as_deref()
    }

    pub(crate) fn damage(&self) -> Option<i32> {
        self.damage
    }

    pub(crate) fn warhead_full(&self) -> bool {
        self.warhead_full
    }

    pub(crate) fn at_firer(&self) -> bool {
        self.at_firer
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.warhead.is_none() && self.weapon.is_none()
    }
}
