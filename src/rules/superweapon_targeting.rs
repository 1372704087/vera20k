//! Target-preflight port attributed by contributor baixiong443 to their
//! earlier ra2-mono work, submitted under VERA's GPLv3. The donor source was
//! unavailable for independent review; see the documented provenance below.
//!
//! The decision is pure: commands, AI and cursor queries pass observations
//! from their existing world owners. It never spends charge or changes fog/RNG.
//! Source mapping and intentional adaptations: docs/ares-generic-warhead.md.
//! The target and house masks are read with Ares's own readers, not the
//! donor's alias parser.

#[cfg(feature = "ares")]
use crate::rules::ini_parser::IniSection;
use crate::util::native_x87::NativeF64Bits;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TargetMask(u8);

impl TargetMask {
    pub const NONE: Self = Self(0);
    pub const LAND: Self = Self(1);
    pub const WATER: Self = Self(2);
    pub const EMPTY: Self = Self(4);
    pub const INFANTRY: Self = Self(8);
    pub const UNIT: Self = Self(16);
    pub const BUILDING: Self = Self(32);
    pub const ALL_CELLS: Self = Self(3);
    pub const ALL_TECHNOS: Self = Self(56);
    pub const ALL_CONTENTS: Self = Self(60);
    pub const ALL: Self = Self(255);

    #[cfg(feature = "ares")]
    fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Ares SuperWeaponTarget reader10034430, tokens1009BC40; same
    /// comma-only list and keep-on-failure rules as [`HouseMask::read_ares`].
    #[cfg(feature = "ares")]
    pub(crate) fn read_ares(section: &IniSection, key: &str) -> Option<Self> {
        read_native_mask(section, key, &TARGET_TOKENS).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct HouseMask(u8);

impl HouseMask {
    pub const NONE: Self = Self(0);
    pub const OWNER: Self = Self(1);
    pub const ALLIES: Self = Self(2);
    pub const ENEMIES: Self = Self(4);
    pub const TEAM: Self = Self(3);
    pub const ALL: Self = Self(7);

    #[cfg(feature = "ares")]
    pub(crate) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Ares household-mask reader100341D0, tokens1009BCE0: successful
    /// native comma-only lists replace; absent or unknown keeps the prior.
    #[cfg(feature = "ares")]
    pub(crate) fn read_ares(section: &IniSection, key: &str) -> Option<Self> {
        read_native_mask(section, key, &HOUSE_TOKENS).map(Self)
    }
}

/// Ares AffectedHouse names1009BCE0..1009BD20, read by100341D0.
#[cfg(feature = "ares")]
const HOUSE_TOKENS: [(&str, u8); 8] = [
    ("owner", 1),
    ("allies", 2),
    ("enemies", 4),
    ("team", 3),
    ("trump", 5),
    ("others", 6),
    ("all", 7),
    ("none", 0),
];

/// Ares SuperWeaponTarget names1009BC40..1009BC80, read by10034430.
#[cfg(feature = "ares")]
const TARGET_TOKENS: [(&str, u8); 8] = [
    ("land", 1),
    ("water", 2),
    ("empty", 4),
    ("infantry", 8),
    ("units", 0x10),
    ("buildings", 0x20),
    ("all", 0xFF),
    ("none", 0),
];

/// Both Ares mask readers: ReadString into the shared 0x800 buffer
/// (10007860), then a comma-only strtok (10007920) whose tokens keep their
/// spaces and must each `_strcmpi` (7C8D20) a table name. An empty read or
/// any unknown token writes nothing (100341E8/10034231), so the caller keeps
/// its prior mask; a present list without tokens stores 0.
#[cfg(feature = "ares")]
fn read_native_mask(section: &IniSection, key: &str, tokens: &[(&str, u8)]) -> Option<u8> {
    section
        .read_list(key, 0x800)?
        .into_iter()
        .try_fold(0, |bits, name| {
            tokens
                .iter()
                .find_map(|(token, value)| token.eq_ignore_ascii_case(name).then_some(bits | value))
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TargetRequirements {
    pub target: TargetMask,
    pub house: HouseMask,
}

/// SWTypeExt+0x241/+0x23D `SW.RequiresTarget`/`SW.RequiresHouse` and the
/// nullable +0x242/+0x23E `SW.AIRequiresTarget`/`SW.AIRequiresHouse`, read
/// by every ordered rules pass.
#[cfg(feature = "ares")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct AresSuperWeaponTargetMasks {
    manual: TargetRequirements,
    ai_target: Option<TargetMask>,
    ai_house: Option<HouseMask>,
}

#[cfg(feature = "ares")]
impl Default for AresSuperWeaponTargetMasks {
    fn default() -> Self {
        // SWTypeExt ctor100317AE/100317B8: the dword stores of AffectsHouse 7
        // and AffectsTarget 0xFF zero both manual masks and both AI flags.
        Self {
            manual: TargetRequirements {
                target: TargetMask::NONE,
                house: HouseMask::NONE,
            },
            ai_target: None,
            ai_house: None,
        }
    }
}

#[cfg(feature = "ares")]
impl AresSuperWeaponTargetMasks {
    /// SWTypeExt::LoadFromINI10033370, reads1003362B..1003367D in this order.
    /// The AI keys are Nullable: their wrappers (10033CC0/10033C60) set the
    /// flag only when the read succeeds, and a set flag is never cleared.
    pub(crate) fn read_pass(&mut self, section: &IniSection) {
        if let Some(mask) = TargetMask::read_ares(section, "SW.RequiresTarget") {
            self.manual.target = mask;
        }
        if let Some(mask) = HouseMask::read_ares(section, "SW.RequiresHouse") {
            self.manual.house = mask;
        }
        if let Some(mask) = TargetMask::read_ares(section, "SW.AIRequiresTarget") {
            self.ai_target = Some(mask);
        }
        if let Some(mask) = HouseMask::read_ares(section, "SW.AIRequiresHouse") {
            self.ai_house = Some(mask);
        }
    }

    pub(crate) fn manual(self) -> TargetRequirements {
        self.manual
    }

    /// 10032270..10032322 and100328D0: an unset AI mask falls back to the
    /// SW.AITargeting mode's row, which the caller supplies.
    pub(crate) fn automatic(self, mode_default: TargetRequirements) -> TargetRequirements {
        TargetRequirements {
            target: self.ai_target.unwrap_or(mode_default.target),
            house: self.ai_house.unwrap_or(mode_default.house),
        }
    }
}

/// Immutable configuration. Only explicitly opted-in extension types read it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SuperWeaponTargeting {
    pub manual: TargetRequirements,
    pub automatic: TargetRequirements,
    pub fire_into_shroud: bool,
    pub range_minimum: NativeF64Bits,
    pub range_maximum: NativeF64Bits,
}

#[cfg(feature = "ares")]
impl SuperWeaponTargeting {
    /// The target masks start at the ordered reader's defaults; RuleSet
    /// replaces them with the processed [`AresSuperWeaponTargetMasks`].
    pub fn from_ini(section: &IniSection, mode_default: TargetRequirements) -> Self {
        let range = |key| {
            let value = if section.is_present(key) {
                section.read_double(key, f64::NAN)
            } else {
                -1.0
            };
            NativeF64Bits::from_bits(value.to_bits())
        };
        let masks = AresSuperWeaponTargetMasks::default();
        Self {
            manual: masks.manual(),
            automatic: masks.automatic(mode_default),
            fire_into_shroud: section.read_bool("SW.FireIntoShroud", true),
            range_minimum: range("SW.RangeMinimum"),
            range_maximum: range("SW.RangeMaximum"),
        }
    }

    pub fn range(&self) -> (f64, f64) {
        (
            f64::from_bits(self.range_minimum.bits()),
            f64::from_bits(self.range_maximum.bits()),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetReject {
    InstanceNotFound,
    InvalidPayload,
    TargetRequired,
    TargetOutOfBounds,
    TargetRequiresLand,
    TargetRequiresWater,
    TargetContentMismatch,
    TargetHouseMismatch,
    TargetShrouded,
    InvalidRangeConfiguration,
    NoEligibleLaunchSite,
}

#[derive(Debug, Clone, Copy)]
pub struct TargetContent {
    pub kind: TargetMask,
    pub relation: HouseMask,
}

#[derive(Debug, Clone, Copy)]
pub struct LaunchSite {
    pub center: (i32, i32),
    pub eligible: bool,
    pub range_minimum_override: Option<f64>,
    pub range_maximum_override: Option<f64>,
}

#[cfg(feature = "ares")]
impl LaunchSite {
    pub fn admits(self, target: (i32, i32), range: (f64, f64)) -> bool {
        let minimum = self.range_minimum_override.unwrap_or(range.0);
        let maximum = self.range_maximum_override.unwrap_or(range.1);
        if !self.eligible || !valid_range((minimum, maximum)) {
            return false;
        }
        let dx = i64::from(target.0) - i64::from(self.center.0);
        let dy = i64::from(target.1) - i64::from(self.center.1);
        // Coordinates supplied by the world are 16-bit cells. f64 exactly
        // represents their squared distance; no sqrt, FMA or RNG is involved.
        let distance_squared = (dx as f64) * (dx as f64) + (dy as f64) * (dy as f64);
        (minimum < 0.0 || distance_squared >= minimum * minimum)
            && (maximum < 0.0 || distance_squared <= maximum * maximum)
    }
}

#[cfg(feature = "ares")]
fn valid_range((minimum, maximum): (f64, f64)) -> bool {
    minimum.is_finite()
        && maximum.is_finite()
        && (minimum < 0.0 || maximum < 0.0 || minimum <= maximum)
}

pub struct TargetPreflightInput<'a> {
    pub target_cell: Option<(i32, i32)>,
    pub requires_target_cell: bool,
    pub requirements: &'a TargetRequirements,
    pub contents: &'a [TargetContent],
    pub target_in_bounds: bool,
    pub terrain_is_water: Option<bool>,
    pub fire_into_shroud: bool,
    pub shrouded: Option<bool>,
    pub range: (f64, f64),
    pub requires_launch_site: bool,
    pub launch_sites: &'a [LaunchSite],
}

/// Port of Evaluate: same ordered gates and rejection categories. A missing
/// terrain/visibility observation is not invented as water or shroud.
#[cfg(feature = "ares")]
pub fn evaluate_target(input: &TargetPreflightInput<'_>) -> Result<(), TargetReject> {
    let TargetRequirements {
        target: target_mask,
        house: house_mask,
    } = *input.requirements;
    let constrained_range = input.range.0 >= 0.0
        || input.range.1 >= 0.0
        || input.launch_sites.iter().any(|site| {
            site.range_minimum_override.is_some_and(|v| v >= 0.0)
                || site.range_maximum_override.is_some_and(|v| v >= 0.0)
        });
    let Some(target) = input.target_cell else {
        return if input.requires_target_cell
            || target_mask != TargetMask::NONE
            || !input.fire_into_shroud
            || constrained_range
        {
            Err(TargetReject::TargetRequired)
        } else {
            Ok(())
        };
    };
    if !input.target_in_bounds {
        return Err(TargetReject::TargetOutOfBounds);
    }
    if target_mask.intersects(TargetMask::ALL_CELLS)
        && let Some(water) = input.terrain_is_water
    {
        if water && !target_mask.intersects(TargetMask::WATER) {
            return Err(TargetReject::TargetRequiresLand);
        }
        if !water && !target_mask.intersects(TargetMask::LAND) {
            return Err(TargetReject::TargetRequiresWater);
        }
    }
    let content_mask = TargetMask(target_mask.0 & TargetMask::ALL_CONTENTS.0);
    let constrains_content = content_mask != TargetMask::NONE;
    if constrains_content || house_mask != HouseMask::NONE {
        let kind_matches =
            |content: &TargetContent| !constrains_content || content_mask.intersects(content.kind);
        let accepted = if input.contents.is_empty() {
            !constrains_content || content_mask.intersects(TargetMask::EMPTY)
        } else {
            input.contents.iter().any(|content| {
                kind_matches(content)
                    && (house_mask == HouseMask::NONE || house_mask.intersects(content.relation))
            })
        };
        if !accepted {
            return Err(
                if !constrains_content || input.contents.iter().any(kind_matches) {
                    TargetReject::TargetHouseMismatch
                } else {
                    TargetReject::TargetContentMismatch
                },
            );
        }
    }
    if !input.fire_into_shroud && input.shrouded == Some(true) {
        return Err(TargetReject::TargetShrouded);
    }
    if !valid_range(input.range) {
        return Err(TargetReject::InvalidRangeConfiguration);
    }
    if input.requires_launch_site && !input.launch_sites.iter().any(|site| site.eligible) {
        return Err(TargetReject::NoEligibleLaunchSite);
    }
    if constrained_range
        && !input
            .launch_sites
            .iter()
            .any(|site| site.admits(target, input.range))
    {
        return Err(TargetReject::NoEligibleLaunchSite);
    }
    Ok(())
}

#[cfg(all(test, feature = "ares"))]
#[path = "superweapon_targeting_tests.rs"]
mod tests;
