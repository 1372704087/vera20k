//! The Phobos `AnimTypeExt` keys read at `Ext/AnimType/Body.cpp:93-118`:
//! `AttachedAnimPosition`, `WakeAnim`, `VisibleTo`,
//! `FireAnimDisallowedLandTypes`, `AttachFireAnimsToParent` and
//! `TheaterPalette`.
//!
//! VERA has no anim-runtime consumer for them yet (the fire-anim attach path,
//! the debris wake and the cell-light palette are separate mechanisms); the
//! reads land on [`crate::rules::art_data::AnimTypeRuntimeConfig`] so those
//! paths can consume them without a second parse.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;
use crate::rules::phobos_shields::AffectedHouse;
use crate::rules::terrain_rules::LandType;

/// `Ext/AnimType/Body.h:11-16` `Valueable<AttachedAnimPosition>`; the
/// constructor value is `Default`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AttachedAnimPosition {
    #[default]
    Default,
    Center,
    Ground,
}

impl AttachedAnimPosition {
    /// `EnumParser<AttachedAnimPosition>`'s name table. Anything else keeps
    /// the previous value (`Valueable::Read` returns false).
    pub fn parse(raw: &str) -> Option<Self> {
        let name = raw.trim();
        if name.eq_ignore_ascii_case("default") {
            Some(Self::Default)
        } else if name.eq_ignore_ascii_case("center") {
            Some(Self::Center)
        } else if name.eq_ignore_ascii_case("ground") {
            Some(Self::Ground)
        } else {
            None
        }
    }
}

/// Phobos `LandTypeFlags` (`Utilities/Enum.h:69-87`): one bit per `LandType`,
/// indexed by its `char` value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LandTypeFlags(u16);

impl LandTypeFlags {
    pub const NONE: Self = Self(0);
    /// `LandTypeFlags::DefaultDisallowed = Water | Rock | Ice | Beach`,
    /// the value `Ext/Anim/Body.cpp:379` falls back to for `Scorch=yes`.
    pub const DEFAULT_DISALLOWED: Self = Self::of(&[
        LandType::Water,
        LandType::Rock,
        LandType::Ice,
        LandType::Beach,
    ]);

    const fn of(types: &[LandType]) -> Self {
        let mut bits = 0u16;
        let mut index = 0;
        while index < types.len() {
            bits |= 1 << types[index].as_index();
            index += 1;
        }
        Self(bits)
    }

    /// `IsLandTypeInFlags` (`Utilities/Enum.h:91`).
    pub const fn contains(self, land: LandType) -> bool {
        self.0 & (1 << land.as_index()) != 0
    }
}

/// `Utilities/TemplateDef.h:681-704`'s `read<LandTypeFlags>`. It walks the
/// comma-separated tokens but looks each token's land type up from
/// `parser.value()` — the whole value — so only a value that is itself one
/// land-type name converts; any other non-`none` value fails the read and
/// keeps the previous nullable.
pub fn parse_land_type_flags(raw: &str) -> Option<LandTypeFlags> {
    let mut has_value_token = false;
    for token in raw.split(',') {
        let token = token.trim();
        if token.is_empty() || token.eq_ignore_ascii_case("none") {
            continue;
        }
        has_value_token = true;
    }
    if !has_value_token {
        return Some(LandTypeFlags::NONE);
    }
    let land = land_type_from_name(raw.trim())?;
    Some(LandTypeFlags(1 << land.as_index()))
}

/// `GroundType::GetLandTypeFromName` over the `Clear..=Weeds` range.
fn land_type_from_name(name: &str) -> Option<LandType> {
    LandType::ALL
        .into_iter()
        .find(|land| land.section_name().eq_ignore_ascii_case(name))
}

/// Reads the six keys from an AnimType's ARTMD section.
pub struct PhobosAnimTypeExt;

impl PhobosAnimTypeExt {
    /// `AttachedAnimPosition` (`Valueable`, default `Default`): the attach
    /// anchor for an attached anim (`Ext/Anim/Hooks.cpp:377-409`).
    pub fn attached_anim_position(section: &IniSection) -> AttachedAnimPosition {
        section.read_string_with(
            "AttachedAnimPosition",
            2048,
            AttachedAnimPosition::Default,
            |current, raw| AttachedAnimPosition::parse(raw).unwrap_or(current),
        )
    }

    /// `WakeAnim` (`ValueableVector<AnimType*>`, empty): the anims a debris
    /// impact spawns in water (`Ext/Anim/Body.cpp:278-296`).
    pub fn wake_anim(section: &IniSection) -> Vec<String> {
        crate::rules::native_processing::read_valueable_type_vector(
            section,
            "WakeAnim",
            &[],
            |name| Some(name.to_owned()),
        )
    }

    /// `VisibleTo` (`Nullable<AffectedHouse>`): who may see the anim.
    pub fn visible_to(section: &IniSection) -> Option<AffectedHouse> {
        section.read_phobos_nullable_with("VisibleTo", None, AffectedHouse::parse)
    }

    /// `FireAnimDisallowedLandTypes` (`Nullable<LandTypeFlags>`), read with
    /// `<false, true>` so `none` clears rather than fails.
    pub fn fire_anim_disallowed_land_types(section: &IniSection) -> Option<LandTypeFlags> {
        section.read_phobos_nullable_with(
            "FireAnimDisallowedLandTypes",
            None,
            parse_land_type_flags,
        )
    }

    /// `AttachFireAnimsToParent` (`Nullable<bool>`): attach fire anims to the
    /// burning object instead of the cell (`Ext/Anim/Body.cpp:388`).
    pub fn attach_fire_anims_to_parent(section: &IniSection) -> Option<bool> {
        section.read_phobos_nullable_bool("AttachFireAnimsToParent", None)
    }

    /// `TheaterPalette` (`Nullable<bool>`): use the theater/cell light
    /// convert for this anim (`Ext/Anim/Hooks.cpp:252,746`).
    pub fn theater_palette(section: &IniSection) -> Option<bool> {
        section.read_phobos_nullable_bool("TheaterPalette", None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ini_parser::IniFile;

    /// The six keys keep their `Body.h` constructor values when unset.
    #[test]
    fn the_anim_type_ext_keys_default_to_the_constructor_values() {
        let ini = IniFile::from_str("[ANIM]\nStart=0\n");
        let empty = ini.section("ANIM").unwrap();
        assert_eq!(
            PhobosAnimTypeExt::attached_anim_position(empty),
            AttachedAnimPosition::Default
        );
        assert!(PhobosAnimTypeExt::wake_anim(empty).is_empty());
        assert_eq!(PhobosAnimTypeExt::visible_to(empty), None);
        assert_eq!(
            PhobosAnimTypeExt::fire_anim_disallowed_land_types(empty),
            None
        );
        assert_eq!(PhobosAnimTypeExt::attach_fire_anims_to_parent(empty), None);
        assert_eq!(PhobosAnimTypeExt::theater_palette(empty), None);
    }

    #[test]
    fn the_anim_type_ext_keys_read_their_authored_values() {
        let ini = IniFile::from_str(
            "[ANIM]\nAttachedAnimPosition=Ground\nWakeAnim=WAKE1,WAKE2\nVisibleTo=owner,allies\n\
             FireAnimDisallowedLandTypes=Rock\nAttachFireAnimsToParent=yes\n\
             TheaterPalette=no\n",
        );
        let set = ini.section("ANIM").unwrap();
        assert_eq!(
            PhobosAnimTypeExt::attached_anim_position(set),
            AttachedAnimPosition::Ground
        );
        assert_eq!(PhobosAnimTypeExt::wake_anim(set), ["WAKE1", "WAKE2"]);
        assert_eq!(
            PhobosAnimTypeExt::visible_to(set),
            Some(AffectedHouse::TEAM)
        );
        assert_eq!(
            PhobosAnimTypeExt::fire_anim_disallowed_land_types(set),
            Some(LandTypeFlags(1 << LandType::Rock.as_index()))
        );
        assert_eq!(
            PhobosAnimTypeExt::attach_fire_anims_to_parent(set),
            Some(true)
        );
        assert_eq!(PhobosAnimTypeExt::theater_palette(set), Some(false));
    }

    /// `read<LandTypeFlags>` looks the land type up from the whole value, so a
    /// multi-token list fails instead of ORing the tokens. `none` is the
    /// empty mask and succeeds.
    #[test]
    fn the_land_type_flags_reader_matches_the_whole_value_lookup() {
        assert_eq!(
            parse_land_type_flags("rock"),
            Some(LandTypeFlags(1 << LandType::Rock.as_index()))
        );
        assert_eq!(parse_land_type_flags("none"), Some(LandTypeFlags::NONE));
        assert_eq!(parse_land_type_flags("Rock,Water"), None);
        assert_eq!(parse_land_type_flags("Mud"), None);
        assert!(
            LandTypeFlags::DEFAULT_DISALLOWED.contains(LandType::Water)
                && LandTypeFlags::DEFAULT_DISALLOWED.contains(LandType::Rock)
                && LandTypeFlags::DEFAULT_DISALLOWED.contains(LandType::Ice)
                && LandTypeFlags::DEFAULT_DISALLOWED.contains(LandType::Beach)
        );
        assert!(!LandTypeFlags::DEFAULT_DISALLOWED.contains(LandType::Clear));
    }

    /// An unknown position name keeps the current value.
    #[test]
    fn the_attached_anim_position_keeps_the_current_value_on_a_bad_name() {
        assert_eq!(
            AttachedAnimPosition::parse("center"),
            Some(AttachedAnimPosition::Center)
        );
        assert_eq!(AttachedAnimPosition::parse("sideways"), None);
    }
}
