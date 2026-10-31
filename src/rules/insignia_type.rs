//! Phobos `[InsigniaTypes]` catalog.
//!
//! Native identity: Phobos `InsigniaTypeClass`
//! (`src/New/Type/InsigniaTypeClass.{h,cpp}`), an
//! `Enumerable<InsigniaTypeClass>` whose main section is `[InsigniaTypes]`.
//! `InsigniaTypeClass::LoadFromINI` reads `Insignia=` (a
//! `Promotable<SHPStruct*>`, so the common key sets all three ranks and the
//! `.Rookie`/`.Veteran`/`.Elite` keys override one each) and `InsigniaFrame=`
//! (a `Promotable<int>` with the same shape). A techno type referring to an
//! entry through `InsigniaType=` copies these two fields verbatim.
//!
//! Asset loading is deferred: the catalog retains the file names with Phobos'
//! `.shp` suffix rule, and presentation resolves them once per match.

use crate::rules::ini_parser::{IniFile, IniSection};
use crate::rules::ini_value::parse_ares_int;
#[cfg(feature = "phobos")]
use crate::rules::ini_value::truncate_native_bytes;

/// `Enumerable` stores `Name[32]` with `_TRUNCATE`.
#[cfg(feature = "phobos")]
const NAME_PAYLOAD_BYTES: usize = 0x1F;

/// `Promotable<T>` rank order: Rookie, Veteran, Elite.
pub(crate) const RANKS: [&str; 3] = ["Rookie", "Veteran", "Elite"];

/// Phobos `InsigniaTypeClass`: one retained `[InsigniaTypes]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InsigniaType {
    pub name: String,
    /// The common `Insignia=` name, kept so a failed per-rank load can fall
    /// back to it the way Phobos' retained SHP pointer does.
    pub common_image: Option<String>,
    /// `Promotable<SHPStruct*> Insignia`: per-rank file names, `.shp` applied.
    pub images: [Option<String>; 3],
    /// `Promotable<int> InsigniaFrame`: per-rank frame index, -1 = default.
    pub frames: [i32; 3],
}

impl InsigniaType {
    #[cfg(feature = "phobos")]
    fn named(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            common_image: None,
            images: [None, None, None],
            frames: [-1, -1, -1],
        }
    }
}

/// Ordered Phobos `Enumerable<InsigniaTypeClass>::Array`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct InsigniaTypeCatalog {
    types: Vec<InsigniaType>,
}

impl InsigniaTypeCatalog {
    pub fn entry(&self, name: &str) -> Option<&InsigniaType> {
        self.types
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
    }

    pub fn entries(&self) -> &[InsigniaType] {
        &self.types
    }

    /// Read one reached Rules pass: the pass's `[InsigniaTypes]` list appends
    /// entries, then every retained entry re-reads its own section.
    #[cfg(feature = "phobos")]
    pub(crate) fn apply_pass(&mut self, pass: &IniFile) {
        if let Some(list) = pass.section("InsigniaTypes") {
            // Phobos Enumerable::LoadFromINIList reads each physical key
            // through ReadString with Phobos::readBuffer[2048] before lookup.
            for key in list.physical_keys() {
                let Some(name) = list.read_name(key, 2048) else {
                    continue;
                };
                if self.entry(name).is_none() {
                    self.types.push(InsigniaType::named(&truncate_native_bytes(
                        name,
                        NAME_PAYLOAD_BYTES,
                    )));
                }
            }
        }
        for index in 0..self.types.len() {
            let name = self.types[index].name.clone();
            let Some(section) = pass.section(&name) else {
                continue;
            };
            let entry = &mut self.types[index];
            entry.common_image = section
                .read_name("Insignia", 2048)
                .map(normalize_insignia_image);
            entry.images = read_insignia_images(section, entry.images.clone(), "Insignia");
            entry.frames = read_insignia_frames(section, entry.frames, "InsigniaFrame");
        }
    }
}

/// Phobos `RulesExt` drawing settings: `[General] EnemyInsignia` /
/// `DisguiseBlinkingVisibility` and the `[AudioVisual] DrawInsignia.*`
/// defaults. The insignia draw consumes all of them; the disguise blink
/// draw still uses its own stock query (#3504).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PhobosDrawGlobals {
    pub enemy_insignia: bool,
    /// `[General] DisguiseBlinkingVisibility` (`AffectedHouse`, default
    /// `Owner`): who can see through a blinking disguise. DrawInsignia asks
    /// it with the viewer as source and the disguised Techno's owner as
    /// target.
    pub disguise_blinking_visibility: crate::rules::phobos_shields::AffectedHouse,
    pub only_on_selected: bool,
    pub use_pixel_selection_bracket_delta: bool,
    pub adjust_infantry: [i32; 2],
    pub adjust_units: [i32; 2],
    pub adjust_buildings: [i32; 2],
    /// Phobos `Nullable<BuildingSelectBracketPosition>`: unset adds the
    /// building offset to the object's own location instead of anchoring to
    /// the selection bracket.
    pub buildings_anchor:
        Option<crate::rules::building_select_bracket::BuildingSelectBracketPosition>,
}

impl Default for PhobosDrawGlobals {
    /// The `RulesExt` constructor values.
    fn default() -> Self {
        Self {
            enemy_insignia: true,
            disguise_blinking_visibility: crate::rules::phobos_shields::AffectedHouse::OWNER,
            only_on_selected: false,
            use_pixel_selection_bracket_delta: false,
            adjust_infantry: [5, 2],
            adjust_units: [10, 6],
            adjust_buildings: [10, 6],
            buildings_anchor: None,
        }
    }
}

impl PhobosDrawGlobals {
    pub fn from_ini(ini: &IniFile) -> Self {
        let mut globals = Self::default();
        let general = ini.section_or_empty("General");
        globals.enemy_insignia = general.read_ares_bool("EnemyInsignia", globals.enemy_insignia);
        globals.disguise_blinking_visibility = general.read_string_with(
            "DisguiseBlinkingVisibility",
            0x40,
            globals.disguise_blinking_visibility,
            |current, raw| {
                crate::rules::phobos_shields::AffectedHouse::parse(raw).unwrap_or(current)
            },
        );
        let audio_visual = ini.section_or_empty("AudioVisual");
        globals.only_on_selected =
            audio_visual.read_ares_bool("DrawInsignia.OnlyOnSelected", globals.only_on_selected);
        globals.use_pixel_selection_bracket_delta = audio_visual.read_ares_bool(
            "DrawInsignia.UsePixelSelectionBracketDelta",
            globals.use_pixel_selection_bracket_delta,
        );
        globals.adjust_infantry = audio_visual
            .read_phobos_int_pair("DrawInsignia.AdjustPos.Infantry", globals.adjust_infantry);
        globals.adjust_units =
            audio_visual.read_phobos_int_pair("DrawInsignia.AdjustPos.Units", globals.adjust_units);
        globals.adjust_buildings = audio_visual
            .read_phobos_int_pair("DrawInsignia.AdjustPos.Buildings", globals.adjust_buildings);
        globals.buildings_anchor = audio_visual.read_phobos_nullable_with(
            "DrawInsignia.AdjustPos.BuildingsAnchor",
            globals.buildings_anchor,
            crate::rules::building_select_bracket::BuildingSelectBracketPosition::parse,
        );
        globals
    }
}

/// One `Promotable<SHPStruct*>/Promotable<int>/Vector3D<int>` slot set: the
/// common-image fallback, the per-rank files, the per-rank single frames and
/// the `InsigniaFrames` vector.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InsigniaVariant {
    pub common_image: Option<String>,
    pub images: [Option<String>; 3],
    pub frames: [i32; 3],
    pub frame_lists: [i32; 3],
}

impl Default for InsigniaVariant {
    fn default() -> Self {
        Self {
            common_image: None,
            images: [None, None, None],
            frames: [-1, -1, -1],
            frame_lists: [-1, -1, -1],
        }
    }
}

/// Phobos `TechnoTypeExt`'s insignia block: the base slot set, the
/// `InsigniaType=`-selected overrides, and the per-weapon/per-passenger
/// variants read from `Insignia.WeaponN`/`Insignia.PassengersN`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct PhobosInsignia {
    pub base: InsigniaVariant,
    pub weapons: Vec<InsigniaVariant>,
    pub passengers: Vec<InsigniaVariant>,
    /// `Insignia.ShowEnemy=`; `None` inherits `[General] EnemyInsignia`.
    pub show_enemy: Option<bool>,
    /// Names kept so the catalog can be consulted after Rules construction.
    pub base_type: Option<String>,
    pub weapon_types: Vec<Option<String>>,
    pub passenger_types: Vec<Option<String>>,
}

/// The merged slot set one draw reads: [`PhobosInsignia::select`] applies the
/// passenger variants, then the weapon variants, over the base.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InsigniaSelection {
    pub images: [Option<String>; 3],
    pub commons: [Option<String>; 3],
    pub frames: [i32; 3],
    pub frame_lists: [i32; 3],
}

impl PhobosInsignia {
    /// Merge the passenger and weapon variants exactly as `DrawInsignia`
    /// does: a variant overrides an image only when it names one for that
    /// rank, a single frame only when it is not -1, and the frame vector only
    /// when it is not the all -1 default.
    pub fn select(&self, weapon: Option<usize>, passengers: Option<usize>) -> InsigniaSelection {
        let mut selection = InsigniaSelection::of(&self.base);
        if let Some(variant) = passengers.and_then(|index| self.passengers.get(index)) {
            selection.merge(variant);
        }
        if let Some(variant) = weapon.and_then(|index| self.weapons.get(index)) {
            selection.merge(variant);
        }
        selection
    }

    /// Copy every resolved `InsigniaType`'s slots into the variants that name
    /// it. An unresolved name keeps the directly authored keys, as Phobos'
    /// nullable type pointer falls back to them.
    pub(crate) fn bind_types(&mut self, catalog: &InsigniaTypeCatalog) {
        if let Some(entry) = self
            .base_type
            .as_deref()
            .and_then(|name| catalog.entry(name))
        {
            apply_type(&mut self.base, entry);
        }
        for (slot, name) in self.weapon_types.iter().enumerate() {
            if let Some(entry) = name.as_deref().and_then(|name| catalog.entry(name)) {
                apply_type(&mut self.weapons[slot], entry);
            }
        }
        for (slot, name) in self.passenger_types.iter().enumerate() {
            if let Some(entry) = name.as_deref().and_then(|name| catalog.entry(name)) {
                apply_type(&mut self.passengers[slot], entry);
            }
        }
    }
}

impl InsigniaSelection {
    fn of(variant: &InsigniaVariant) -> Self {
        Self {
            images: variant.images.clone(),
            commons: std::array::from_fn(|_| variant.common_image.clone()),
            frames: variant.frames,
            frame_lists: variant.frame_lists,
        }
    }

    fn merge(&mut self, variant: &InsigniaVariant) {
        for rank in 0..3 {
            if variant.images[rank].is_some() {
                self.images[rank] = variant.images[rank].clone();
                self.commons[rank] = variant.common_image.clone();
            }
            if variant.frames[rank] != -1 {
                self.frames[rank] = variant.frames[rank];
            }
        }
        if variant.frame_lists != [-1, -1, -1] {
            self.frame_lists = variant.frame_lists;
        }
    }

    /// The frame `DrawInsignia` finally draws for `rank`: the promotable
    /// single frame, else that rank's `InsigniaFrames` entry, else the
    /// custom-shape default 0 or the stock PIPS frame (14 Veteran, 15 Elite;
    /// Rookie draws nothing without an authored frame). `None` means no draw.
    pub fn frame(&self, rank: usize, custom: bool) -> Option<i32> {
        let mut frame = self.frames[rank];
        let mut default_frame = if custom { 0 } else { -1 };
        match rank {
            1 => {
                if !custom {
                    default_frame = 14;
                }
                if frame == -1 {
                    frame = self.frame_lists[1];
                }
            }
            2 => {
                if !custom {
                    default_frame = 15;
                }
                if frame == -1 {
                    frame = self.frame_lists[2];
                }
            }
            _ => {
                if frame == -1 {
                    frame = self.frame_lists[0];
                }
            }
        }
        if frame == -1 {
            frame = default_frame;
        }
        (frame >= 0).then_some(frame)
    }
}

fn apply_type(variant: &mut InsigniaVariant, entry: &InsigniaType) {
    variant.images = entry.images.clone();
    variant.frames = entry.frames;
    variant.frame_lists = [-1, -1, -1];
    variant.common_image = entry.common_image.clone();
}

/// Read a TechnoType's whole insignia block. `weapon_count` and
/// `passengers` are the type's own `WeaponCount=`/`Passengers=`, which size
/// the variant vectors at load time.
pub(crate) fn parse_insignia(
    section: &IniSection,
    weapon_count: i32,
    passengers: i32,
) -> PhobosInsignia {
    let type_name = |key: &str| {
        section
            .read_name(key, 0x80)
            .filter(|name| !crate::rules::ini_parser::is_native_none_type_name(name))
            .map(str::to_owned)
    };
    let mut insignia = PhobosInsignia::default();
    insignia.base_type = type_name("InsigniaType");
    // Both sources are read. Phobos skips the direct keys only when the
    // nullable type pointer resolved; an unknown name falls back to them, so
    // [`PhobosInsignia::bind_types`] picks the resolved type and keeps these
    // values otherwise.
    insignia.base = read_variant(section, "Insignia", "InsigniaFrame", "InsigniaFrames");
    insignia.show_enemy = section.read_phobos_nullable_bool("Insignia.ShowEnemy", None);
    if weapon_count > 0 {
        for index in 0..weapon_count {
            let number = index + 1;
            insignia
                .weapon_types
                .push(type_name(&format!("InsigniaType.Weapon{number}")));
            insignia.weapons.push(read_variant(
                section,
                &format!("Insignia.Weapon{number}"),
                &format!("InsigniaFrame.Weapon{number}"),
                &format!("InsigniaFrames.Weapon{number}"),
            ));
        }
    }
    if passengers > 0 {
        for count in 0..=passengers {
            insignia
                .passenger_types
                .push(type_name(&format!("InsigniaType.Passengers{count}")));
            insignia.passengers.push(read_variant(
                section,
                &format!("Insignia.Passengers{count}"),
                &format!("InsigniaFrame.Passengers{count}"),
                &format!("InsigniaFrames.Passengers{count}"),
            ));
        }
    }
    insignia
}

fn read_variant(
    section: &IniSection,
    image_base: &str,
    frame_base: &str,
    list_base: &str,
) -> InsigniaVariant {
    InsigniaVariant {
        common_image: section
            .read_name(image_base, 2048)
            .map(normalize_insignia_image),
        images: read_insignia_images(section, [None, None, None], image_base),
        frames: read_insignia_frames(section, [-1, -1, -1], frame_base),
        frame_lists: section.read_int_fields::<3>(list_base, 2048, [-1, -1, -1]),
    }
}

/// Phobos `Promotable<SHPStruct*>::Read` with the base flag `base` (`%s` is
/// the rank): the common key sets all three, then each `.Rookie`/`.Veteran`/
/// `.Elite` key replaces one. A name without a `.shp` suffix gains one, as the
/// Phobos SHP reader does. Asset load failures are presentation's concern.
pub(crate) fn read_insignia_images(
    section: &IniSection,
    mut current: [Option<String>; 3],
    base: &str,
) -> [Option<String>; 3] {
    if let Some(all) = section.read_name(base, 2048) {
        let name = Some(normalize_insignia_image(all));
        current = [name.clone(), name.clone(), name];
    }
    for (slot, rank) in RANKS.iter().enumerate() {
        if let Some(value) = section.read_name(&format!("{base}.{rank}"), 2048) {
            current[slot] = Some(normalize_insignia_image(value));
        }
    }
    current
}

/// Phobos `Promotable<int>::Read`: the common key's successful parse sets all
/// three ranks, then the rank keys replace one; a rejected value keeps its
/// slot.
pub(crate) fn read_insignia_frames(
    section: &IniSection,
    mut current: [i32; 3],
    base: &str,
) -> [i32; 3] {
    if let Some(value) = read_int_key(section, base) {
        current = [value; 3];
    }
    for (slot, rank) in RANKS.iter().enumerate() {
        if let Some(value) = read_int_key(section, &format!("{base}.{rank}")) {
            current[slot] = value;
        }
    }
    current
}

/// Phobos' SHP name rule: append `.shp` unless the name already ends with it,
/// case-insensitively (`TemplateDef.h`'s `read<SHPStruct*>`).
pub(crate) fn normalize_insignia_image(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".shp") {
        name.to_owned()
    } else {
        format!("{name}.shp")
    }
}

fn read_int_key(section: &IniSection, key: &str) -> Option<i32> {
    section.read_name(key, 2048).and_then(parse_ares_int)
}

#[cfg(all(test, feature = "phobos"))]
#[path = "insignia_type_tests.rs"]
mod tests;
