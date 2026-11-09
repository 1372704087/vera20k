//! `Phobos FlyingStrings` value carried from simulation owners to the client
//! display: a consumed signed credit label. Ares' bounty payout and Phobos'
//! `DisplaySoylent` both emit it; display rejection cannot undo the wallet.
//!
//! ## Dependency rules
//! - Part of sim/ — presentation consumes it, never the reverse.

use crate::sim::projectile::ProjectileCoord;

/// A consumed signed credit label, before the client's shroud/visual-character
/// checks. Display rejection cannot undo the wallet or requeue the amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BountyText {
    pub entity_id: u64,
    pub amount: i32,
    /// Unraised virtual+48 point: IsShrouded586360 reads its height too.
    pub coord: ProjectileCoord,
    pub z_offset: i32,
    /// Phobos `FlyingStrings::AddMoneyString`'s `displayToHouses` filter;
    /// `None` keeps the bounty path's unconditional delivery.
    #[cfg(feature = "phobos")]
    pub display_to: Option<crate::rules::phobos_shields::AffectedHouse>,
    /// Screen-pixel offset added after the centred anchor (Phobos
    /// `DisplaySoylentOffset`); zero for the bounty path.
    pub pixel_offset: [i32; 2],
}
