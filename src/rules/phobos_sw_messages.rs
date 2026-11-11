//! Phobos `Message.Activated.*` / `EVA.Activated.*` and the linked-SW pair.
//!
//! Reader `SWTypeExt::LoadFromINIFile` (`Ext/SWType/Body.cpp:241-250`):
//! `Message.LinkedSWAcquired` and `EVA.LinkedSWAcquired` first, then the six
//! `Message.Activated.{Owner,Allies,Enemies}` / `EVA.Activated.{...}` keys.
//! Declarations and constructor defaults: `Ext/SWType/Body.h:119-126`,
//! `:208-215`.
//!
//! Native types and defaults:
//! - `Message_*` are `Valueable<CSFText>`: constructor-empty; a present key
//!   replaces the label with its first 31 bytes, an empty one clears it
//!   (`detail::read<CSFText>` + `CSFText::operator=`). An empty label prints
//!   nothing (`ApplyActivatedMessage`, `Ext/SWType/FireSuperWeapon.cpp:501`).
//! - `EVA_Activated_*` are `ValueableIdx<VoxClass>`, constructor `-1`: a
//!   blank key stores `-1`, an unknown name keeps the previous value, a known
//!   one stores its `[DialogList]` index. `-1` speaks nothing
//!   (`Ext/SWType/FireSuperWeapon.cpp:522`).
//! - `EVA_LinkedSWAcquired` is `NullableIdx<VoxClass>`, unset by default:
//!   only `isset()` plays (`:484`). An explicit blank still sets `-1`, which
//!   `VoxClass::PlayIndex` treats as silence, so the port keeps `None` for
//!   both unset and explicit none.
//!
//! ## Behavior owners
//! - `sim::superweapon::chaining` emits the launching House for both groups;
//!   the app picks the Owner/Allies/Enemies variant against the client's
//!   current player (`SWTypeExt::ApplyActivatedMessage`/`ApplyActivatedEva`,
//!   `:491-526`) and prints with the *launching* House's colour.
//! - The linked pair is gated on `pHouse->IsCurrentPlayer()` (`:482`) and
//!   prints in the current player's colour (`:487`).
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;
use crate::rules::ini_value::truncate_native_bytes;
use crate::rules::sound_ini::EvaRegistry;

/// Which of the three `Message.Activated.*` / `EVA.Activated.*` columns a
/// launching House selects against the listening client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PhobosActivatedAudience {
    /// `pHouse->IsControlledByCurrentPlayer()`, `Ext/SWType/FireSuperWeapon.cpp:495`.
    Owner,
    /// Else `pHouse->IsAlliedWith(HouseClass::CurrentPlayer)`, `:497`.
    Allies,
    /// Else the enemy column, `:499`.
    Enemies,
}

/// One superweapon type's Phobos activation/link presentation labels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PhobosSuperWeaponMessages {
    /// `Message.LinkedSWAcquired=`, constructor-empty.
    linked_acquired: String,
    /// `EVA.LinkedSWAcquired=`, unset by default (`NullableIdx`).
    eva_linked_acquired: Option<String>,
    /// `Message.Activated.Owner=`, constructor-empty.
    activated_owner: String,
    /// `Message.Activated.Allies=`.
    activated_allies: String,
    /// `Message.Activated.Enemies=`.
    activated_enemies: String,
    /// `EVA.Activated.Owner=`, constructor `-1`.
    eva_activated_owner: Option<String>,
    /// `EVA.Activated.Allies=`.
    eva_activated_allies: Option<String>,
    /// `EVA.Activated.Enemies=`.
    eva_activated_enemies: Option<String>,
}

impl PhobosSuperWeaponMessages {
    /// The ordered reader of `Ext/SWType/Body.cpp:241-250`.
    ///
    /// `Valueable<CSFText>` replaces its label whenever the key is present,
    /// including an empty value; `read_ares_csf_label` cannot express that
    /// (it keeps the previous label on an empty read), so the presence
    /// primitive is used and the 31-byte `FixedString<0x20>` cut applied here.
    pub(crate) fn from_section(section: &IniSection, eva: &EvaRegistry) -> Self {
        Self {
            linked_acquired: read_csf_label(section, "Message.LinkedSWAcquired"),
            eva_linked_acquired: read_eva_index(section, "EVA.LinkedSWAcquired", eva).flatten(),
            activated_owner: read_csf_label(section, "Message.Activated.Owner"),
            activated_allies: read_csf_label(section, "Message.Activated.Allies"),
            activated_enemies: read_csf_label(section, "Message.Activated.Enemies"),
            eva_activated_owner: read_eva_index(section, "EVA.Activated.Owner", eva).flatten(),
            eva_activated_allies: read_eva_index(section, "EVA.Activated.Allies", eva).flatten(),
            eva_activated_enemies: read_eva_index(section, "EVA.Activated.Enemies", eva).flatten(),
        }
    }

    /// `Message.Activated.*` for one audience; empty prints nothing.
    pub fn activated_message(&self, audience: PhobosActivatedAudience) -> &str {
        match audience {
            PhobosActivatedAudience::Owner => &self.activated_owner,
            PhobosActivatedAudience::Allies => &self.activated_allies,
            PhobosActivatedAudience::Enemies => &self.activated_enemies,
        }
    }

    /// `EVA.Activated.*` for one audience; `None` is native `-1`.
    pub fn activated_eva(&self, audience: PhobosActivatedAudience) -> Option<&str> {
        match audience {
            PhobosActivatedAudience::Owner => self.eva_activated_owner.as_deref(),
            PhobosActivatedAudience::Allies => self.eva_activated_allies.as_deref(),
            PhobosActivatedAudience::Enemies => self.eva_activated_enemies.as_deref(),
        }
    }

    /// `Message.LinkedSWAcquired`; empty prints nothing.
    pub fn linked_acquired_message(&self) -> &str {
        &self.linked_acquired
    }

    /// `EVA.LinkedSWAcquired`; `None` is unset or an explicit none.
    pub fn linked_acquired_eva(&self) -> Option<&str> {
        self.eva_linked_acquired.as_deref()
    }
}

/// `Valueable<CSFText>::Read` over `detail::read<CSFText>`: a present key
/// replaces the label with at most 31 bytes (`FixedString<0x20>`), an empty
/// one clears it, an absent one leaves the constructor's empty label. The
/// ordered owner rebuilds the aggregate from the merged section, so an absent
/// key answers the constructor default rather than a previous pass's label.
fn read_csf_label(section: &IniSection, key: &str) -> String {
    section
        .read_string_presence(key, 2048)
        .map_or_else(String::new, |value| {
            truncate_native_bytes(&value, 31).to_owned()
        })
}

/// `ValueableIdx<VoxClass>` / `NullableIdx<VoxClass>::Read`: `None` when the
/// key is absent or names nothing (`Lookuper::FindIndex` failed, so the field
/// keeps its previous value), `Some(None)` when a present blank stores native
/// `-1`, and `Some(Some(name))` for a registered `[DialogList]` identity.
pub(crate) fn read_eva_index(
    section: &IniSection,
    key: &str,
    eva: &EvaRegistry,
) -> Option<Option<String>> {
    let raw = section.read_string_presence(key, 2048)?;
    if raw.is_empty() {
        // Blank: `Lookuper::FindIndex("")` is -1 and `INIClass::IsBlank` lets
        // the write through, so the field becomes -1 (silent).
        return Some(None);
    }
    eva.entry(&raw).map(|entry| Some(entry.name.clone()))
}

#[cfg(test)]
#[path = "phobos_sw_messages_tests.rs"]
mod tests;
