//! Source-ordered `[ColorAdd]` rules data retained for later rendering consumers.
//!
//! The three channel values are raw RGB565 magnitudes, not display RGB bytes.
//! This module assigns no consumer meaning to the fixed native slots.

use crate::rules::ini_parser::IniFile;

/// Fixed capacity of the active-YR `[ColorAdd]` table.
pub const COLOR_ADD_SLOT_COUNT: usize = 16;

/// One source entry in the fixed `[ColorAdd]` table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColorAddEntry {
    /// Exact key spelling retained from the source section; `None` marks an unused tail slot.
    pub name: Option<String>,
    /// Raw RGB565 channel magnitudes. These values are deliberately not normalized.
    pub rgb: [u8; 3],
}

/// Rules-owned fixed `[ColorAdd]` table in source declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorAddTable {
    pub slots: [ColorAddEntry; COLOR_ADD_SLOT_COUNT],
}

impl Default for ColorAddTable {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| ColorAddEntry::default()),
        }
    }
}

impl ColorAddTable {
    /// Parse at most 16 source entries, leaving the unused tail zero-filled.
    pub fn from_ini(ini: &IniFile) -> Self {
        let mut table = Self::default();
        let Some(section) = ini.section("ColorAdd") else {
            return table;
        };

        for (slot, key) in table.slots.iter_mut().zip(section.keys()) {
            *slot = ColorAddEntry {
                name: Some(key.to_string()),
                rgb: section.read_color_rgb(key, [0; 3]),
            };
        }
        table
    }

    /// The colour word a draw ORs into its pixels for the entry at `index`, as
    /// `BuildingClass_DrawBody` converts it (`0x0043D44F..0x0043D4E9`; its other
    /// readers repeat the conversion): the RGB565 arm of the pixel format
    /// `0x004BBC90` reports (format 2, the active retail one) falls through the
    /// other two, so the word is `R<<11 | G<<5 | B`, `R<<11 | (G>>1)<<6 | B` and
    /// `R<<10 | (G>>1)<<5 | B` together, of which the blitters OR the low 16
    /// bits (`0x00494CD9`). Executed: `tools/superweapon_oracle.py`
    /// `building_colour_word`.
    ///
    /// RESIDUAL: an index outside the table reads the Rules bytes after it
    /// natively; VERA answers 0. Retail's indexes are all inside it.
    pub fn rgb565_word(&self, index: i32) -> u16 {
        let Some(entry) = usize::try_from(index).ok().and_then(|i| self.slots.get(i)) else {
            return 0;
        };
        let [r, g, b] = entry.rgb.map(u32::from);
        let word =
            (r << 11 | g << 5 | b) | (r << 11 | (g >> 1) << 6 | b) | (r << 10 | (g >> 1) << 5 | b);
        word as u16
    }
}
