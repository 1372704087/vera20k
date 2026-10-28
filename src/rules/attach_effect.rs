//! Phobos AttachEffect types, retained by the Rules registry.
//!
//! Native identity: Phobos `AttachEffectTypeClass`
//! (`src/New/Type/AttachEffectTypeClass.{h,cpp}`), an
//! `Enumerable<AttachEffectTypeClass>` whose main section is
//! `[AttachEffectTypes]`. `AttachEffectTypeClass::LoadFromINI` re-reads every
//! retained entry's own section on each reached Rules pass, keeping the
//! previous value for an absent key.
//!
//! This module starts the port with the catalog and the fields the first
//! slices consume: identity, duration, stacking, power gating and the target
//! filters. Discard conditions, animations, multipliers and weapons add their
//! own keys here as their slices land, so the reader stays one owner.

#[cfg(feature = "phobos")]
use crate::rules::ini_parser::IniFile;
#[cfg(feature = "phobos")]
use crate::rules::ini_value::truncate_native_bytes;
use crate::rules::phobos_shields::AffectedHouse;

/// `Enumerable` stores `Name[32]` with `_TRUNCATE`.
#[cfg(feature = "phobos")]
const NAME_PAYLOAD_BYTES: usize = 0x1F;

/// `CCINIClass::GetSection` compares the CRC of the uppercased name, so a
/// catalog entry resolves its own section whatever case the file used.
#[cfg(feature = "phobos")]
fn section_named<'a>(
    pass: &'a IniFile,
    name: &str,
) -> Option<&'a crate::rules::ini_parser::IniSection> {
    pass.section(name).or_else(|| {
        pass.section_names()
            .into_iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(name))
            .and_then(|candidate| pass.section(candidate))
    })
}

/// Phobos `AffectsTarget` techno-type groups. An empty list would let every
/// category through; the reader maps `none` to no bits and `all` to every bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttachEffectTargets {
    pub infantry: bool,
    pub units: bool,
    pub aircraft: bool,
    pub buildings: bool,
}

impl AttachEffectTargets {
    pub const ALL: Self = Self {
        infantry: true,
        units: true,
        aircraft: true,
        buildings: true,
    };
}

impl Default for AttachEffectTargets {
    fn default() -> Self {
        Self::ALL
    }
}

impl AttachEffectTargets {
    /// Phobos `TechnoTypeFlags` enumeration names, in declaration order.
    /// Unknown tokens keep the previous value, as the native list read does.
    #[cfg(feature = "phobos")]
    fn from_tokens(current: Self, tokens: &[&str]) -> Self {
        let mut next = Self {
            infantry: false,
            units: false,
            aircraft: false,
            buildings: false,
        };
        for token in tokens {
            match token.to_ascii_lowercase().as_str() {
                "all" => next = Self::ALL,
                "none" => {}
                "infantry" => next.infantry = true,
                "units" => next.units = true,
                "aircraft" => next.aircraft = true,
                "buildings" => next.buildings = true,
                _ => return current,
            }
        }
        next
    }
}

/// Phobos `AttachedAnimFlag`: what the effect's animation does while the
/// owner is offline or the target is under a temporal freeze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AttachedAnimFlag {
    #[default]
    None,
    Hides,
    Temporal,
    Paused,
    PausedTemporal,
}

impl AttachedAnimFlag {
    #[cfg(feature = "phobos")]
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "hides" => Some(Self::Hides),
            "temporal" => Some(Self::Temporal),
            "paused" => Some(Self::Paused),
            "pausedtemporal" => Some(Self::PausedTemporal),
            _ => None,
        }
    }
}

/// One `Animation.DrawOffsetN` rule. The first absent or `0,0` index ends the
/// list, so the array is always contiguous from index 0.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AnimationDrawOffset {
    pub offset: [i32; 2],
    pub required_types: Vec<String>,
}

/// One `[AttachEffectTypes]` entry. Defaults are the constructor's
/// (`AttachEffectTypeClass.h`), never retail values.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AttachEffectType {
    pub name: String,
    /// `Duration`: game frames, negative for an indefinite effect.
    pub duration: i32,
    pub duration_apply_firepower_mult: bool,
    pub duration_apply_armor_mult_on_target: bool,
    pub cumulative: bool,
    /// `Cumulative.MaxCount`; negative means no cap.
    pub cumulative_max_count: i32,
    pub powered: bool,
    pub penetrates_iron_curtain: bool,
    /// `PenetratesForceShield`; `None` inherits `penetrates_iron_curtain`.
    pub penetrates_force_shield: Option<bool>,
    /// `AffectTypes`: whitelist, highest priority when non-empty.
    pub affect_types: Vec<String>,
    /// `IgnoreTypes`: blacklist, checked after `affect_types`.
    pub ignore_types: Vec<String>,
    pub affects_target: AttachEffectTargets,
    /// `Groups`: comma-separated group ids.
    pub groups: Vec<String>,
    /// `AllowTransfer`; `None` defaults to whether the effect is self-owned.
    pub allow_transfer: Option<bool>,
    /// `AllowTransfer.Convert`; `None` defaults to `allow_transfer`.
    pub allow_transfer_convert: Option<bool>,
    /// `Animation`: the looping animation's `[Animations]` name.
    pub animation: Option<String>,
    /// `CumulativeAnimations`: one animation per stacked layer; the last
    /// covers every deeper layer.
    pub cumulative_animations: Vec<String>,
    pub cumulative_animations_restart_on_change: bool,
    pub animation_reset_on_reapply: bool,
    pub animation_offline_action: AttachedAnimFlag,
    pub animation_temporal_action: AttachedAnimFlag,
    pub animation_use_invoker_as_owner: bool,
    pub animation_hide_if_attached_with: Vec<String>,
    pub animation_draw_offsets: Vec<AnimationDrawOffset>,
    /// `Tint.Color`; `None` leaves the target untinted.
    pub tint_color: Option<[u8; 3]>,
    pub tint_intensity: f64,
    pub tint_visible_to_houses: AffectedHouse,
}

impl Default for AttachEffectType {
    /// The constructor's defaults with no catalog name.
    fn default() -> Self {
        Self::named("")
    }
}

impl AttachEffectType {
    fn named(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            duration: 0,
            duration_apply_firepower_mult: false,
            duration_apply_armor_mult_on_target: false,
            cumulative: false,
            cumulative_max_count: -1,
            powered: false,
            penetrates_iron_curtain: false,
            penetrates_force_shield: None,
            affect_types: Vec::new(),
            ignore_types: Vec::new(),
            affects_target: AttachEffectTargets::ALL,
            groups: Vec::new(),
            allow_transfer: None,
            allow_transfer_convert: None,
            animation: None,
            cumulative_animations: Vec::new(),
            cumulative_animations_restart_on_change: true,
            animation_reset_on_reapply: false,
            animation_offline_action: AttachedAnimFlag::Hides,
            animation_temporal_action: AttachedAnimFlag::None,
            animation_use_invoker_as_owner: false,
            animation_hide_if_attached_with: Vec::new(),
            animation_draw_offsets: Vec::new(),
            tint_color: None,
            tint_intensity: 0.0,
            tint_visible_to_houses: AffectedHouse::ALL,
        }
    }

    /// `GetCumulativeAnimation(count)`: the layer count picks the animation,
    /// and every layer past the list keeps the last one.
    pub fn cumulative_animation(&self, count: i32) -> Option<&str> {
        if count < 1 || self.cumulative_animations.is_empty() {
            return None;
        }
        let size = self.cumulative_animations.len() as i32;
        let index = if count >= size { size - 1 } else { count - 1 };
        self.cumulative_animations
            .get(index as usize)
            .map(String::as_str)
    }

    /// `HasAnim()`: a cumulative type shows through its cumulative list, every
    /// other type through its single `Animation`.
    pub fn has_animation(&self) -> bool {
        if self.cumulative {
            !self.cumulative_animations.is_empty() || self.animation.is_some()
        } else {
            self.animation.is_some()
        }
    }

    /// `HasTint()`: a colour or a nonzero intensity.
    pub fn has_tint(&self) -> bool {
        self.tint_color.is_some() || self.tint_intensity != 0.0
    }

    /// `AllowTransfer`: an effect a techno attaches to itself defaults to
    /// carrying across a conversion, one attached from outside does not.
    /// `AllowTransfer.Convert` splits the conversion case out.
    pub fn allows_transfer(&self, self_owned: bool, converting: bool) -> bool {
        let base = self.allow_transfer.unwrap_or(self_owned);
        if converting {
            self.allow_transfer_convert.unwrap_or(base)
        } else {
            base
        }
    }

    /// `PenetratesForceShield` falls back to `PenetratesIronCurtain` when the
    /// key is absent or blank.
    pub fn penetrates_force_shield(&self) -> bool {
        self.penetrates_force_shield
            .unwrap_or(self.penetrates_iron_curtain)
    }
}

/// The `[AttachEffectTypes]` catalog, retained across Rules passes.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AttachEffectTypeCatalog {
    types: Vec<AttachEffectType>,
}

impl AttachEffectTypeCatalog {
    pub fn types(&self) -> &[AttachEffectType] {
        &self.types
    }

    #[cfg(feature = "phobos")]
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.types.iter().map(|entry| entry.name.as_str())
    }

    pub fn entry(&self, name: &str) -> Option<&AttachEffectType> {
        self.types
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.types
            .iter()
            .position(|entry| entry.name.eq_ignore_ascii_case(name))
    }

    /// Read one reached Rules pass: the pass's `[AttachEffectTypes]` list
    /// appends entries, then every retained entry re-reads its own section.
    #[cfg(feature = "phobos")]
    pub(crate) fn apply_pass(&mut self, pass: &IniFile) {
        if let Some(list) = pass.section("AttachEffectTypes") {
            for key in list.physical_keys() {
                let Some(name) = list.read_name(key, 2048) else {
                    continue;
                };
                if self.entry(name).is_none() {
                    self.types
                        .push(AttachEffectType::named(&truncate_native_bytes(
                            name,
                            NAME_PAYLOAD_BYTES,
                        )));
                }
            }
        }
        for index in 0..self.types.len() {
            let name = self.types[index].name.clone();
            let Some(section) = section_named(pass, &name) else {
                continue;
            };
            let entry = &mut self.types[index];
            entry.duration = section.read_int("Duration", entry.duration);
            entry.duration_apply_firepower_mult = section.read_bool(
                "Duration.ApplyFirepowerMult",
                entry.duration_apply_firepower_mult,
            );
            entry.duration_apply_armor_mult_on_target = section.read_bool(
                "Duration.ApplyArmorMultOnTarget",
                entry.duration_apply_armor_mult_on_target,
            );
            entry.cumulative = section.read_bool("Cumulative", entry.cumulative);
            entry.cumulative_max_count =
                section.read_int("Cumulative.MaxCount", entry.cumulative_max_count);
            entry.powered = section.read_bool("Powered", entry.powered);
            entry.penetrates_iron_curtain =
                section.read_bool("PenetratesIronCurtain", entry.penetrates_iron_curtain);
            entry.penetrates_force_shield = section
                .read_phobos_nullable_bool("PenetratesForceShield", entry.penetrates_force_shield);
            if let Some(tokens) = section.read_list("AffectTypes", 0x800) {
                entry.affect_types = tokens.iter().map(|name| (*name).to_owned()).collect();
            }
            if let Some(tokens) = section.read_list("IgnoreTypes", 0x800) {
                entry.ignore_types = tokens.iter().map(|name| (*name).to_owned()).collect();
            }
            // `AffectTargets` is the deprecated spelling and only warns.
            if let Some(tokens) = section
                .read_list("AffectsTarget", 0x800)
                .or_else(|| section.read_list("AffectTargets", 0x800))
            {
                entry.affects_target =
                    AttachEffectTargets::from_tokens(entry.affects_target, &tokens);
            }
            entry.allow_transfer =
                section.read_phobos_nullable_bool("AllowTransfer", entry.allow_transfer);
            entry.allow_transfer_convert = section
                .read_phobos_nullable_bool("AllowTransfer.Convert", entry.allow_transfer_convert);
            entry.animation = section
                .read_name("Animation", 0x100)
                .filter(|name| !name.is_empty())
                .map(str::to_owned);
            if let Some(tokens) = section.read_list("CumulativeAnimations", 0x800) {
                entry.cumulative_animations =
                    tokens.iter().map(|name| (*name).to_owned()).collect();
            }
            entry.cumulative_animations_restart_on_change = section.read_bool(
                "CumulativeAnimations.RestartOnChange",
                entry.cumulative_animations_restart_on_change,
            );
            entry.animation_reset_on_reapply =
                section.read_bool("Animation.ResetOnReapply", entry.animation_reset_on_reapply);
            entry.animation_offline_action = section.read_string_with(
                "Animation.OfflineAction",
                0x40,
                entry.animation_offline_action,
                |current, raw| AttachedAnimFlag::parse(raw).unwrap_or(current),
            );
            entry.animation_temporal_action = section.read_string_with(
                "Animation.TemporalAction",
                0x40,
                entry.animation_temporal_action,
                |current, raw| AttachedAnimFlag::parse(raw).unwrap_or(current),
            );
            entry.animation_use_invoker_as_owner = section.read_bool(
                "Animation.UseInvokerAsOwner",
                entry.animation_use_invoker_as_owner,
            );
            if let Some(tokens) = section.read_list("Animation.HideIfAttachedWith", 0x800) {
                entry.animation_hide_if_attached_with =
                    tokens.iter().map(|name| (*name).to_owned()).collect();
            }
            entry.tint_color =
                section.read_phobos_nullable_with("Tint.Color", entry.tint_color, |raw| {
                    Some(crate::rules::ini_parser::IniSection::read_color_rgb_value(
                        raw,
                        [0, 0, 0],
                    ))
                });
            entry.tint_intensity =
                section.read_phobos_double("Tint.Intensity", entry.tint_intensity);
            entry.tint_visible_to_houses = section.read_string_with(
                "Tint.VisibleToHouses",
                0x100,
                entry.tint_visible_to_houses,
                |current, raw| AffectedHouse::parse(raw).unwrap_or(current),
            );
            // The native loop appends into the retained vector, which would
            // duplicate every rule on a second Rules pass; rebuild instead.
            entry.animation_draw_offsets.clear();
            for index in 0.. {
                let key = format!("Animation.DrawOffset{index}");
                let offset = section.read_phobos_int_pair(&key, [0, 0]);
                if offset == [0, 0] {
                    break;
                }
                let required_types = section
                    .read_list(&format!("{key}.RequiredTypes"), 0x800)
                    .map(|tokens| tokens.iter().map(|name| (*name).to_owned()).collect())
                    .unwrap_or_default();
                entry.animation_draw_offsets.push(AnimationDrawOffset {
                    offset,
                    required_types,
                });
            }
            if let Some(tokens) = section.read_list("Groups", 0x800) {
                entry.groups = tokens
                    .iter()
                    .map(|token| token.trim_matches(' ').to_owned())
                    .collect();
            }
        }
    }
}

/// The `[General]` switches this slice consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct AttachEffectGlobals {
    pub discard_sequences_immediate: bool,
    pub discard_move_based_on_destination: bool,
    pub discard_consider_harvesting_as_stationary: bool,
    pub open_topped_use_transport_range_modifiers: bool,
    pub open_topped_check_transport_disable_weapons: bool,
    pub replace_longer_duration: bool,
    pub attach_on_owner_change: bool,
}

impl AttachEffectGlobals {
    /// Phobos `[General]` defaults. `DiscardOn.Sequences.Immediate` and
    /// `DiscardOn.ConsiderHarvestingAsStationary` default to `true`; the rest
    /// to `false`.
    pub fn from_ini(ini: &crate::rules::ini_parser::IniFile) -> Self {
        let Some(section) = ini.section("General") else {
            return Self {
                discard_sequences_immediate: true,
                discard_consider_harvesting_as_stationary: true,
                ..Self::default()
            };
        };
        let defaults = Self {
            discard_sequences_immediate: true,
            discard_consider_harvesting_as_stationary: true,
            ..Self::default()
        };
        Self {
            discard_sequences_immediate: section.read_bool(
                "DiscardOn.Sequences.Immediate",
                defaults.discard_sequences_immediate,
            ),
            discard_move_based_on_destination: section.read_bool(
                "DiscardOn.MoveBasedOnDestination",
                defaults.discard_move_based_on_destination,
            ),
            discard_consider_harvesting_as_stationary: section.read_bool(
                "DiscardOn.ConsiderHarvestingAsStationary",
                defaults.discard_consider_harvesting_as_stationary,
            ),
            open_topped_use_transport_range_modifiers: section.read_bool(
                "OpenTopped.UseTransportRangeModifiers",
                defaults.open_topped_use_transport_range_modifiers,
            ),
            open_topped_check_transport_disable_weapons: section.read_bool(
                "OpenTopped.CheckTransportDisableWeapons",
                defaults.open_topped_check_transport_disable_weapons,
            ),
            replace_longer_duration: section.read_bool(
                "AttachEffect.ReplaceLongerDuration",
                defaults.replace_longer_duration,
            ),
            attach_on_owner_change: section.read_bool(
                "AttachEffect.AttachOnOwnerChange",
                defaults.attach_on_owner_change,
            ),
        }
    }
}

#[cfg(all(test, feature = "phobos"))]
mod tests {
    use super::*;

    fn parse(text: &str) -> AttachEffectTypeCatalog {
        let ini = IniFile::from_str(text);
        let mut catalog = AttachEffectTypeCatalog::default();
        catalog.apply_pass(&ini);
        catalog
    }

    /// Animation and tint keys take the constructor's defaults, and the
    /// `Animation.DrawOffsetN` list stops at the first `0,0`.
    #[test]
    fn animation_and_tint_keys_read_with_documented_defaults() {
        let catalog = parse(
            "[AttachEffectTypes]
0=A
[A]
",
        );
        let entry = catalog.entry("A").expect("entry");
        assert_eq!(entry.animation, None);
        assert!(entry.cumulative_animations_restart_on_change);
        assert_eq!(entry.animation_offline_action, AttachedAnimFlag::Hides);
        assert_eq!(entry.animation_temporal_action, AttachedAnimFlag::None);
        assert_eq!(entry.tint_color, None);
        assert_eq!(entry.tint_intensity, 0.0);
        assert_eq!(entry.tint_visible_to_houses, AffectedHouse::ALL);
        assert!(entry.animation_draw_offsets.is_empty());

        let catalog = parse(
            "[AttachEffectTypes]
0=B
[B]
Animation=LOOP
             CumulativeAnimations=ONE,TWO
CumulativeAnimations.RestartOnChange=no
             Animation.ResetOnReapply=yes
             Animation.OfflineAction=Paused
Animation.TemporalAction=PausedTemporal
             Animation.UseInvokerAsOwner=yes
Animation.HideIfAttachedWith=HIDE
             Tint.Color=10,20,30
Tint.Intensity=1.5
Tint.VisibleToHouses=owner
             Animation.DrawOffset0=7,-3
Animation.DrawOffset0.RequiredTypes=REQ
             Animation.DrawOffset1=2,0
Animation.DrawOffset2=0,0
",
        );
        let entry = catalog.entry("B").expect("entry");
        assert_eq!(entry.animation.as_deref(), Some("LOOP"));
        assert_eq!(entry.cumulative_animations, vec!["ONE", "TWO"]);
        assert!(!entry.cumulative_animations_restart_on_change);
        assert!(entry.animation_reset_on_reapply);
        assert_eq!(entry.animation_offline_action, AttachedAnimFlag::Paused);
        assert_eq!(
            entry.animation_temporal_action,
            AttachedAnimFlag::PausedTemporal
        );
        assert!(entry.animation_use_invoker_as_owner);
        assert_eq!(entry.animation_hide_if_attached_with, vec!["HIDE"]);
        assert_eq!(entry.tint_color, Some([10, 20, 30]));
        assert_eq!(entry.tint_intensity, 1.5);
        assert_eq!(entry.tint_visible_to_houses, AffectedHouse::OWNER);
        assert_eq!(entry.animation_draw_offsets.len(), 2, "0,0 ends the list");
        assert_eq!(entry.animation_draw_offsets[0].offset, [7, -3]);
        assert_eq!(entry.animation_draw_offsets[0].required_types, vec!["REQ"]);
        assert_eq!(entry.animation_draw_offsets[1].offset, [2, 0]);
    }

    /// The layer count picks a cumulative animation, the last entry covers
    /// deeper layers, and the animation/tint presence checks follow the
    /// constructor's shape.
    #[test]
    fn cumulative_animations_and_presence_follow_the_type() {
        let mut effect = AttachEffectType::default();
        assert_eq!(
            effect.cumulative_animation(1),
            None,
            "no list, no animation"
        );
        effect.cumulative = true;
        effect.cumulative_animations = vec!["A".to_owned(), "B".to_owned()];
        assert_eq!(effect.cumulative_animation(1), Some("A"));
        assert_eq!(effect.cumulative_animation(2), Some("B"));
        assert_eq!(
            effect.cumulative_animation(5),
            Some("B"),
            "the last covers the rest"
        );
        assert!(effect.has_animation(), "a cumulative list counts as one");
        assert!(!effect.has_tint());
        effect.animation = Some("ONE".to_owned());
        assert!(effect.has_animation());
        effect.tint_intensity = 0.25;
        assert!(effect.has_tint(), "a nonzero intensity tints");
    }

    /// `AllowTransfer` defaults to whether the effect is self-owned, and
    /// `.Convert` splits the conversion case out.
    #[test]
    fn allow_transfer_defaults_follow_ownership_and_convert() {
        let mut effect = AttachEffectType::default();
        assert!(effect.allows_transfer(true, false), "self-owned carries");
        assert!(!effect.allows_transfer(false, false), "attached does not");
        assert!(effect.allows_transfer(true, true));
        effect.allow_transfer = Some(false);
        assert!(!effect.allows_transfer(true, false));
        assert!(
            !effect.allows_transfer(true, true),
            "Convert inherits the base"
        );
        effect.allow_transfer_convert = Some(true);
        assert!(!effect.allows_transfer(true, false));
        assert!(effect.allows_transfer(true, true));

        let parsed = parse(
            "[AttachEffectTypes]
0=T
[T]
AllowTransfer=no
AllowTransfer.Convert=yes
",
        );
        let entry = parsed.entry("T").expect("entry");
        assert_eq!(entry.allow_transfer, Some(false));
        assert_eq!(entry.allow_transfer_convert, Some(true));
    }

    /// The catalog appends listed entries only, keeps constructor defaults for
    /// absent keys, and resolves `PenetratesForceShield` from the iron-curtain
    /// flag when the key is absent.
    #[test]
    fn catalog_reads_list_entries_and_keeps_constructor_defaults() {
        let catalog = parse(
            "[AttachEffectTypes]\n0=FIRST\n1=Second\n2=first\n[FIRST]\nDuration=48\n\
             Cumulative=yes\nCumulative.MaxCount=3\nPenetratesIronCurtain=yes\nGroups=tag,other\n\
             [SECOND]\nAffectsTarget=none\n",
        );
        assert_eq!(catalog.types().len(), 2, "duplicate name does not append");
        let first = catalog.entry("first").expect("case-insensitive lookup");
        assert_eq!(first.duration, 48);
        assert!(first.cumulative);
        assert_eq!(first.cumulative_max_count, 3);
        assert!(!first.duration_apply_firepower_mult, "constructor default");
        assert!(first.penetrates_iron_curtain);
        assert!(first.penetrates_force_shield(), "inherits the iron curtain");
        assert_eq!(first.groups, vec!["tag", "other"]);
        assert_eq!(first.affects_target, AttachEffectTargets::ALL);
        let second = catalog.entry("Second").expect("listed entry");
        assert_eq!(
            second.affects_target,
            AttachEffectTargets {
                infantry: false,
                units: false,
                aircraft: false,
                buildings: false,
            }
        );
        assert_eq!(catalog.index_of("SECOND"), Some(1));
    }

    /// A later pass re-reads the same entry and keeps a value whose key it no
    /// longer carries, while writing the keys it does carry.
    #[test]
    fn catalog_retains_absent_keys_across_passes() {
        let mut catalog = AttachEffectTypeCatalog::default();
        catalog.apply_pass(&IniFile::from_str(
            "[AttachEffectTypes]\n0=KEEP\n[KEEP]\nDuration=10\nPowered=yes\n",
        ));
        catalog.apply_pass(&IniFile::from_str("[KEEP]\nDuration=20\n"));
        let entry = catalog.entry("KEEP").expect("retained entry");
        assert_eq!(entry.duration, 20, "present key overwrites");
        assert!(entry.powered, "absent key keeps the previous value");
    }

    /// The catalog, the globals and the country/techno attach lists all
    /// survive the production Rules parse and are reachable from `RuleSet`.
    #[test]
    fn ruleset_retains_the_catalog_and_the_attach_lists() {
        use crate::rules::ruleset::RuleSet;

        let rules = RuleSet::from_ini(&IniFile::from_str(
            "[General]
AttachEffect.ReplaceLongerDuration=yes
             [AttachEffectTypes]
0=FIRST
[FIRST]
Duration=40
             [Countries]
0=Americans
[Americans]
AttachEffect.AttachTypes=NATIONAE
             [VehicleTypes]
0=MTNK
[MTNK]
Name=Apocalypse
             AttachEffect.AttachTypes=FIRST,SECOND
AttachEffect.Delays=5,7
",
        ))
        .expect("rules parse");

        assert_eq!(
            rules
                .attach_effect_types()
                .entry("FIRST")
                .map(|entry| entry.duration),
            Some(40)
        );
        assert!(rules.attach_effect_globals().replace_longer_duration);
        let techno = rules.object("MTNK").expect("techno");
        assert_eq!(
            techno.attach_effect_attach_types(),
            ["FIRST".to_owned(), "SECOND".to_owned()]
        );
        assert_eq!(techno.attach_effect_delays(), [5, 7]);
        assert_eq!(
            rules.country_attach_effect_types("Americans"),
            ["NATIONAE".to_owned()]
        );
    }

    /// `[General]` defaults match the documented values.
    #[test]
    fn globals_default_to_the_documented_switches() {
        let globals = AttachEffectGlobals::from_ini(&IniFile::from_str("[General]\n"));
        assert!(globals.discard_sequences_immediate);
        assert!(globals.discard_consider_harvesting_as_stationary);
        assert!(!globals.discard_move_based_on_destination);
        assert!(!globals.open_topped_use_transport_range_modifiers);
        assert!(!globals.open_topped_check_transport_disable_weapons);
        assert!(!globals.replace_longer_duration);
        assert!(!globals.attach_on_owner_change);
    }
}
