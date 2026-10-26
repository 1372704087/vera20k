//! Ares 3.0 `HouseTypeExt` paradrop overrides.
//!
//! Ares reads a `Parachute.Anim=`, `ParaDrop.Aircraft=`, `ParaDrop.Types=` and
//! `ParaDrop.Num=` block for every country in
//! `HouseTypeExt::LoadFromRulesFile @ 0x1001C530` (the order is recorded in
//! `tools/rules_oracle/ares_country_loading.json`), and its three resolvers
//! consult them before the SideExt values: `GetParachuteAnim @ 0x1001BBC0`
//! (own field, then the side's getter at `0x1002E0F0`, then the literal
//! `PARACH`), `GetParadropContent @ 0x1001BC20` (own lists while the type list
//! is non-empty, then the side's lists) and `GetParadropPlane @ 0x1001BCB0`
//! (own `+0x154` index unless it is `-1`, then the side's `+0x74`, then
//! `PDPLANE`). Which units a paradrop sends
//! is simulation state, so these fields enter the country extension hash. The
//! reads themselves stay with the Rules owner in `native_processing.rs`, which
//! already owns the Ares value readers.

#[cfg(feature = "ares")]
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AresCountryParadrop {
    /// `Parachute.Anim=` resolved identity (`HouseTypeExt +0x158`); `None`
    /// defers to the side.
    pub parachute_anim: Option<String>,
    /// `ParaDrop.Aircraft=` resolved identity (`+0x154`); a blank or unknown
    /// name is the `-1` index, which also defers to the side.
    pub aircraft: Option<String>,
    /// `ParaDrop.Types=` (`+0x13C`) kept identities, InfantryType and UnitType
    /// only.
    pub types: Vec<String>,
    /// `ParaDrop.Num=` (`+0x148`), paired with [`Self::types`] by index.
    pub counts: Vec<i32>,
}

#[cfg(feature = "ares")]
impl AresCountryParadrop {
    /// `HouseTypeExt::InheritSettings` copies the four resolved fields.
    pub(crate) fn inherit_from(&mut self, parent: &Self) {
        self.parachute_anim = parent.parachute_anim.clone();
        self.aircraft = parent.aircraft.clone();
        self.types = parent.types.clone();
        self.counts = parent.counts.clone();
    }

    /// `GetParadropContent @ 0x1001BC20`. The country lists are taken while the
    /// type list is non-empty, and the resolver only reports content when the
    /// count list has entries too (`return Types && Num`) - so a country that
    /// names types without counts yields nothing at all rather than deferring.
    pub fn resolved(&self) -> CountryParadrop<'_> {
        if self.types.is_empty() {
            return CountryParadrop::NotSet;
        }
        if self.counts.is_empty() {
            return CountryParadrop::Nothing;
        }
        CountryParadrop::Lists {
            types: self.types.as_slice(),
            counts: self.counts.as_slice(),
        }
    }
}

/// The three answers `GetParadropContent @ 0x1001BC20` can give a caller.
#[cfg(feature = "ares")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountryParadrop<'a> {
    /// The country lists are used as they stand.
    Lists {
        types: &'a [String],
        counts: &'a [i32],
    },
    /// Types without counts: the launch sends nothing.
    Nothing,
    /// No country type list: the caller falls back to the side's lists.
    NotSet,
}
