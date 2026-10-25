//! Target-preflight port attributed by contributor baixiong443 to their
//! earlier ra2-mono work, submitted under VERA's GPLv3. The donor source was
//! unavailable for independent review; see the documented provenance below.
//!
//! The decision is pure: commands, AI and cursor queries pass observations
//! from their existing world owners. It never spends charge or changes fog/RNG.
//! Source mapping and intentional adaptations: docs/ares-generic-warhead.md.

#[cfg(feature = "ares")]
use crate::rules::ini_parser::IniSection;
use crate::util::native_x87::NativeF64Bits;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    #[cfg(feature = "ares")]
    pub fn parse(raw: &str) -> Result<Self, String> {
        parse_mask(raw, |token| {
            Some(match token {
                "land" => Self::LAND.0,
                "water" => Self::WATER.0,
                "empty" | "nocontent" | "no-content" | "no_content" => Self::EMPTY.0,
                "infantry" => Self::INFANTRY.0,
                "unit" | "units" | "aircraft" => Self::UNIT.0,
                "building" | "buildings" => Self::BUILDING.0,
                "allcells" | "all-cells" | "all_cells" => Self::ALL_CELLS.0,
                "alltechnos" | "all-technos" | "all_technos" => Self::ALL_TECHNOS.0,
                "allcontents" | "all-contents" | "all_contents" => Self::ALL_CONTENTS.0,
                "all" => Self::ALL.0,
                _ => return None,
            })
        })
        .map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HouseMask(u8);

impl HouseMask {
    pub const NONE: Self = Self(0);
    pub const OWNER: Self = Self(1);
    pub const ALLIES: Self = Self(2);
    pub const ENEMIES: Self = Self(4);

    #[cfg(feature = "ares")]
    fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    #[cfg(feature = "ares")]
    pub fn parse(raw: &str) -> Result<Self, String> {
        parse_mask(raw, |token| {
            Some(match token {
                "owner" | "self" => 1,
                "ally" | "allies" => 2,
                "enemy" | "enemies" => 4,
                "team" => 3,
                "other" | "others" => 6,
                "all" => 7,
                _ => return None,
            })
        })
        .map(Self)
    }
}

// ra2-mono accepts composable aliases as well as the canonical Ares tokens.
// Preserve those aliases, but never turn an unknown token into no restriction.
#[cfg(feature = "ares")]
fn parse_mask(raw: &str, bit: impl Fn(&str) -> Option<u8>) -> Result<u8, String> {
    let mut mask = 0;
    for token in raw
        .trim()
        .trim_matches(['<', '>'])
        .split(|c: char| c.is_ascii_whitespace() || matches!(c, ',' | ';' | '|' | '+'))
        .filter(|s| !s.is_empty())
    {
        let token_lower = token.to_ascii_lowercase();
        if matches!(token_lower.as_str(), "none" | "no" | "false") {
            continue;
        }
        mask |= bit(&token_lower).ok_or_else(|| token.to_owned())?;
    }
    Ok(mask)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetRequirements {
    pub target: Result<TargetMask, String>,
    pub house: Result<HouseMask, String>,
}

#[cfg(feature = "ares")]
impl TargetRequirements {
    pub fn parse(target: &str, house: &str) -> Self {
        Self {
            target: TargetMask::parse(target),
            house: HouseMask::parse(house),
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
    pub fn from_ini(section: &IniSection) -> Self {
        let range = |key| {
            let value = if section.is_present(key) {
                section.read_double(key, f64::NAN)
            } else {
                -1.0
            };
            NativeF64Bits::from_bits(value.to_bits())
        };
        Self {
            manual: TargetRequirements::parse(
                section.read_name("SW.RequiresTarget", 0x100).unwrap_or(""),
                section.read_name("SW.RequiresHouse", 0x100).unwrap_or(""),
            ),
            // GenericWarhead defaults to the Offensive targeting mode. Its
            // automatic requirements are independent of the manual masks.
            automatic: TargetRequirements::parse(
                section
                    .read_name("SW.AIRequiresTarget", 0x100)
                    .unwrap_or("all-technos"),
                section
                    .read_name("SW.AIRequiresHouse", 0x100)
                    .unwrap_or("enemies"),
            ),
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
    UnsupportedTargetRequirement(String),
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

/// Ares `SW.AITargeting` modes. Only the entries present in the Ares 3.0
/// image's `SWTypeExt::AITargetingModes` table carry a default constraint mask.
#[cfg(feature = "ares")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AresAiTargetingMode {
    None,
    Nuke,
    LightningStorm,
    PsychicDominator,
    ParaDrop,
    GeneticMutator,
    ForceShield,
    NoTarget,
    Offensive,
    Stealth,
    SelfTarget,
    Base,
    MultiMissile,
    HunterSeeker,
    EnemyBase,
    /// A mode outside the confirmed 0..=14 table; its default mask is unknown.
    Other,
}

#[cfg(feature = "ares")]
impl AresAiTargetingMode {
    /// One owner for the `SW.AITargeting` key. Case-insensitive; an absent or
    /// unknown value leaves the Ares default (`Offensive`). Modes outside the
    /// confirmed 0..=14 table are `Other` with no default constraints.
    pub fn read(section: &IniSection) -> Self {
        let Some(mode) = section.read_name("SW.AITargeting", 0x40) else {
            return Self::Offensive;
        };
        match mode.to_ascii_lowercase().as_str() {
            "none" => Self::None,
            "nuke" => Self::Nuke,
            "lightningstorm" => Self::LightningStorm,
            "psychicdominator" => Self::PsychicDominator,
            "paradrop" => Self::ParaDrop,
            "geneticmutator" => Self::GeneticMutator,
            "forceshield" => Self::ForceShield,
            "notarget" => Self::NoTarget,
            "offensive" => Self::Offensive,
            "stealth" => Self::Stealth,
            "self" => Self::SelfTarget,
            "base" => Self::Base,
            "multimissile" => Self::MultiMissile,
            "hunterseeker" => Self::HunterSeeker,
            "enemybase" => Self::EnemyBase,
            _ => Self::Other,
        }
    }

    /// `SWTypeExt::AITargetingModes` default constraint masks from the Ares
    /// 3.0 image (`ImageBase 0x10000000`, table `0x1009B8A4`).
    pub fn default_constraints(self) -> u16 {
        match self {
            Self::Nuke
            | Self::Offensive
            | Self::MultiMissile
            | Self::HunterSeeker
            | Self::EnemyBase => CONSTRAINT_ENEMY,
            Self::LightningStorm => CONSTRAINT_ENEMY | CONSTRAINT_LIGHTNING_STORM_INACTIVE,
            Self::PsychicDominator => {
                CONSTRAINT_OFFENSIVE_CELL_CLEAR | CONSTRAINT_ENEMY | CONSTRAINT_DOMINATOR_INACTIVE
            }
            Self::GeneticMutator => CONSTRAINT_OFFENSIVE_CELL_CLEAR,
            _ => 0,
        }
    }
}

#[cfg(feature = "ares")]
const CONSTRAINT_OFFENSIVE_CELL_CLEAR: u16 = 0x0001;
#[cfg(feature = "ares")]
const CONSTRAINT_DEFENSIVE_CELL_CLEAR: u16 = 0x0002;
#[cfg(feature = "ares")]
const CONSTRAINT_ENEMY: u16 = 0x0004;
#[cfg(feature = "ares")]
const CONSTRAINT_LIGHTNING_STORM_INACTIVE: u16 = 0x0008;
#[cfg(feature = "ares")]
const CONSTRAINT_DOMINATOR_INACTIVE: u16 = 0x0010;
#[cfg(feature = "ares")]
const CONSTRAINT_ATTACKED: u16 = 0x0020;
#[cfg(feature = "ares")]
const CONSTRAINT_LOW_POWER: u16 = 0x0040;
#[cfg(feature = "ares")]
const CONSTRAINT_OFFENSIVE_CELL_SET: u16 = 0x0080;
#[cfg(feature = "ares")]
const CONSTRAINT_DEFENSIVE_CELL_SET: u16 = 0x0100;

/// Ares `SW.AITargeting.Constraints`, read by `SWTypeExt::IsTargetConstraintsEligible`
/// (`Ares.dll + 0x32110` for the 3.0 build). The explicit key overrides the
/// mode default.
#[cfg(feature = "ares")]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct AresTargetingConstraints(u16);

#[cfg(feature = "ares")]
impl AresTargetingConstraints {
    pub fn read(section: &IniSection, mode: AresAiTargetingMode) -> Self {
        let Some(raw) = section.read_name("SW.AITargeting.Constraints", 0x100) else {
            return Self(mode.default_constraints());
        };
        match parse_constraints(&raw) {
            Ok(mask) => Self(mask),
            Err(token) => {
                log::warn!("SuperWeapon SW.AITargeting.Constraints: unknown token '{token}'");
                Self(mode.default_constraints())
            }
        }
    }

    /// The `arg3 = true` branch Phobos passes for a human cameo click: only
    /// the cell and enemy constraints apply; the storm, dominator, attacked
    /// and low-power constraints are intentionally skipped.
    pub fn allows_manual_fire(self, house: HouseTargetingObservations) -> bool {
        let mask = self.0;
        let preferred_set = house.preferred_cell != (0, 0);
        let defense_set = house.defense_cell != (0, 0);
        if mask & CONSTRAINT_OFFENSIVE_CELL_CLEAR != 0 && preferred_set {
            return false;
        }
        if mask & CONSTRAINT_OFFENSIVE_CELL_SET != 0 && !preferred_set {
            return false;
        }
        if mask & CONSTRAINT_DEFENSIVE_CELL_CLEAR != 0 && defense_set {
            return false;
        }
        if mask & CONSTRAINT_DEFENSIVE_CELL_SET != 0 && !defense_set {
            return false;
        }
        if mask & CONSTRAINT_ENEMY != 0 && !house.has_enemy {
            return false;
        }
        // Remaining bits are the manual-fire exceptions (arg3 = true).
        let _ = (
            CONSTRAINT_LIGHTNING_STORM_INACTIVE,
            CONSTRAINT_DOMINATOR_INACTIVE,
            CONSTRAINT_ATTACKED,
            CONSTRAINT_LOW_POWER,
        );
        true
    }
}

#[cfg(feature = "ares")]
fn parse_constraints(raw: &str) -> Result<u16, String> {
    let mut mask = 0u16;
    for token in raw
        .trim()
        .trim_matches(['<', '>'])
        .split(|c: char| c.is_ascii_whitespace() || matches!(c, ',' | ';' | '|' | '+'))
        .filter(|token| !token.is_empty())
    {
        let token_lower = token.to_ascii_lowercase();
        if matches!(token_lower.as_str(), "none" | "no" | "false") {
            continue;
        }
        mask |= match token_lower.as_str() {
            "enemy" => CONSTRAINT_ENEMY,
            "offensive_cell_set" | "offensive-cell-set" => CONSTRAINT_OFFENSIVE_CELL_SET,
            "offensive_cell_clear" | "offensive-cell-clear" => CONSTRAINT_OFFENSIVE_CELL_CLEAR,
            "defensive_cell_set" | "defensive-cell-set" => CONSTRAINT_DEFENSIVE_CELL_SET,
            "defensive_cell_clear" | "defensive-cell-clear" => CONSTRAINT_DEFENSIVE_CELL_CLEAR,
            "lightningstorm_inactive" | "lightning-storm-inactive" => {
                CONSTRAINT_LIGHTNING_STORM_INACTIVE
            }
            "dominator_inactive" => CONSTRAINT_DOMINATOR_INACTIVE,
            "attacked" => CONSTRAINT_ATTACKED,
            "lowpower" | "low_power" | "low-power" => CONSTRAINT_LOW_POWER,
            _ => return Err(token.to_owned()),
        };
    }
    Ok(mask)
}

/// The House fields `IsTargetConstraintsEligible` reads.
#[cfg(feature = "ares")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HouseTargetingObservations {
    /// `House+0x5600`, `-1`/`None` when the house has not chosen an enemy.
    pub has_enemy: bool,
    /// `House+0x54F0` scripted offensive cell, `(0, 0)` when unset.
    pub preferred_cell: (u16, u16),
    /// `House+0x54F4` defensive cell, `(0, 0)` when unset.
    pub defense_cell: (u16, u16),
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
    let target_mask = input
        .requirements
        .target
        .as_ref()
        .map_err(|token| TargetReject::UnsupportedTargetRequirement(token.clone()))?;
    let house_mask = input
        .requirements
        .house
        .as_ref()
        .map_err(|token| TargetReject::UnsupportedTargetRequirement(token.clone()))?;
    let constrained_range = input.range.0 >= 0.0
        || input.range.1 >= 0.0
        || input.launch_sites.iter().any(|site| {
            site.range_minimum_override.is_some_and(|v| v >= 0.0)
                || site.range_maximum_override.is_some_and(|v| v >= 0.0)
        });
    let Some(target) = input.target_cell else {
        return if input.requires_target_cell
            || *target_mask != TargetMask::NONE
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
    if constrains_content || *house_mask != HouseMask::NONE {
        let kind_matches =
            |content: &TargetContent| !constrains_content || content_mask.intersects(content.kind);
        let accepted = if input.contents.is_empty() {
            !constrains_content || content_mask.intersects(TargetMask::EMPTY)
        } else {
            input.contents.iter().any(|content| {
                kind_matches(content)
                    && (*house_mask == HouseMask::NONE || house_mask.intersects(content.relation))
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
