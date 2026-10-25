//! Phobos `SkirmishUnlimitedColors`: skirmish colours indexed into `[Colors]`.
//!
//! Native identity: Phobos `Phobos.INI.cpp`, hook
//! `InitRules_ThingsThatShouldntBeSerailized` (`0x0052D21F`, 6 bytes), reads
//! `[General] SkirmishUnlimitedColors=` with `CCINIClass::ReadBool`, and while
//! the value is set it patches `Game_GetLinkedColor` (`0x0069A310`) with seven
//! bytes:
//!
//! ```text
//! mov eax, [esp+4]   ; the passed colour index
//! shl eax, 1
//! inc eax
//! ```
//!
//! (the function's own `retn 4` stays). The source comment for that patch is
//! "Disable Ares hook at this address so that our logic can run": Ares hooks the
//! same site for its customisable dropdown colours, which is why Phobos lists
//! the two as mutually exclusive.
//!
//! The returned runtime scheme index is therefore `2 * index + 1`. gamemd builds
//! two schemes per `[Colors]` entry and the odd half is the one a player colour
//! selects ([`crate::rules::color_scheme::scheme_for_priority`]), so the entry
//! the index addresses is the index itself: a spawner that sends 6 gets the
//! seventh `[Colors]` entry (`Orange` in stock rulesmd.ini) instead of the
//! third member of YR's eight-priority table.
//!
//! Because the hook replaces the whole conversion, this mode also changes the
//! non-spawner callers that pass a fixed priority (trigger message colours,
//! production and rally tints): Phobos documents the feature as spawner-only
//! and warns that the stock launcher's own priorities then draw other colours.
//!
//! ## Dependency rules
//! - Part of rules/ — depends on rules/color_scheme and rules/ini_parser only.

#[cfg(feature = "phobos")]
use crate::rules::ini_parser::IniFile;

/// `[General] SkirmishUnlimitedColors=` and the priority conversion it selects.
///
/// `Default` is the vanilla conversion, which is also what a build without the
/// `phobos` feature produces: the native reader never runs, so no INI value can
/// turn the mode on.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SkirmishColorMode {
    unlimited: bool,
}

impl SkirmishColorMode {
    /// YR's eight-priority conversion (`PRIORITY_TO_SCHEME_INDEX`).
    pub const VANILLA: Self = Self { unlimited: false };

    /// Phobos `SkirmishUnlimitedColors=true`.
    pub const UNLIMITED: Self = Self { unlimited: true };

    pub fn unlimited(self) -> bool {
        self.unlimited
    }

    /// Native `Game_GetLinkedColor` (`0x0069A310`) as the active build leaves
    /// it: the priority table, or Phobos' `2 * index + 1` pass-through.
    ///
    /// The unhooked conversion and Phobos' hook both return a scheme index, not
    /// an entry: callers divide by two ([`Self::entry_for_priority`]). A
    /// negative priority is the lobby's random/observer marker; the hook would
    /// return a negative index for it, which addresses no `[Colors]` entry, so
    /// VERA clamps it to the first entry.
    pub fn scheme_index_for_priority(self, priority: i32) -> usize {
        if self.unlimited {
            priority.max(0) as usize * 2 + 1
        } else {
            crate::rules::color_scheme::scheme_index_for_priority(priority)
        }
    }

    /// `[Colors]` entry a player colour priority selects under this mode.
    ///
    /// The vanilla answer stays owned by
    /// [`crate::rules::color_scheme::scheme_entry_for_priority`]; only the
    /// unhooked hook's replacement is answered here.
    pub fn entry_for_priority(self, priority: i32) -> usize {
        if self.unlimited {
            priority.max(0) as usize
        } else {
            crate::rules::color_scheme::scheme_entry_for_priority(priority)
        }
    }

    #[cfg(feature = "phobos")]
    pub(crate) fn from_ini(ini: &IniFile) -> Self {
        // Phobos reads the key with the native ReadBool semantics and its
        // constructor default of false.
        let enabled = ini
            .section_or_empty("General")
            .read_bool("SkirmishUnlimitedColors", false);
        if enabled {
            Self::UNLIMITED
        } else {
            Self::VANILLA
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlimited_mode_addresses_the_passed_entry() {
        // Vanilla keeps the SessionClass table: priority 6 is runtime scheme 17,
        // whose doubled pair is [Colors] entry 8.
        assert_eq!(SkirmishColorMode::VANILLA.scheme_index_for_priority(6), 17);
        assert_eq!(SkirmishColorMode::VANILLA.entry_for_priority(6), 8);

        // The hook answers 2 * index + 1, the odd half of the doubled pair, so
        // the entry is the index itself. Oracle: Phobos' own patch bytes
        // (0x69A310) executed for the two indices it names.
        let mode = SkirmishColorMode::UNLIMITED;
        assert_eq!(mode.scheme_index_for_priority(0), 1);
        assert_eq!(mode.scheme_index_for_priority(6), 13);
        assert_eq!(mode.entry_for_priority(0), 0);
        assert_eq!(mode.entry_for_priority(6), 6);
        assert_eq!(mode.entry_for_priority(14), 14);

        // -2 is the lobby's random/observer marker, not an entry: the hook
        // returns -3 there, which addresses no [Colors] entry.
        assert_eq!(mode.entry_for_priority(-2), 0);
    }

    #[cfg(feature = "phobos")]
    #[test]
    fn the_ini_key_defaults_off_and_needs_a_phobos_build() {
        use crate::rules::ini_parser::IniFile;

        let default = SkirmishColorMode::from_ini(&IniFile::from_str("[General]\n"));
        assert_eq!(default, SkirmishColorMode::VANILLA);
        assert!(!default.unlimited());

        let on = SkirmishColorMode::from_ini(&IniFile::from_str(
            "[General]\nSkirmishUnlimitedColors=yes\n",
        ));
        assert_eq!(on, SkirmishColorMode::UNLIMITED);

        // Native ReadBool takes the first character: 'o' is neither a true nor
        // a false token, so `on`/`off` both retain the default.
        let off = SkirmishColorMode::from_ini(&IniFile::from_str(
            "[General]\nSkirmishUnlimitedColors=off\n",
        ));
        assert_eq!(off, SkirmishColorMode::VANILLA);
    }
}
