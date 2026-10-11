//! A Foot's seeks into its own house's buildings (`FootClass` vt+0x33C..
//! +0x34C) and the latches that keep a seek alive across missions
//! (`+0x68F..+0x691`).
//!
//! `FootClass::EnterBioReactor @ 0x004DFB70` (vt+0x340, which Infantry and
//! Unit inherit) sends a Foot into the nearest of its house's absorbers that
//! takes it. Its live callers are `TeamClass::AI`'s script action 62
//! (`0x006E9E83`), `CaptureManagerClass::DecideUnitFate`'s choice 3
//! (`0x0047261F`) and the heads of `FootClass::Mission_Guard`
//! (`0x004D5076`) and `Mission_AreaGuard` (`0x004D6ACC`) while its latch
//! holds; the fifth call (`0x006ECC00`) sits in `0x006ECB50`, which nothing
//! calls or points at. The latch (`+0x68F`) is cleared by the Foot
//! constructor (`0x004D33D2`), written only by this seek (`0x004DFC56`,
//! `0x004DFC6B`) and by Mission_Unload's eject (`0x0044DBEA`), and read by
//! those two heads and `Is_Seeking_Building_Entry @ 0x004E0080`.
//!
//! Native execution: `tools/team_recruit_oracle.py`'s `bio_reactor` rows
//! (the CAN_LOAD answers, the walk order, the distance, ties, the latch and
//! the reissue test) replay in this module's tests.
//!
//! RESIDUALS:
//! - The entry runs through VERA's Boarding adapter
//!   (`passenger::tick_passenger_system`), as a player's Enter order does:
//!   the class setter's Enter arm (radio HELLO and DOCKING,
//!   `0x0051AC25..0x0051AD17`) and InfantryClass::PerCellProcess's DOCK_NOW
//!   to that contact (`0x0051A298..0x0051A2A7`) are not represented, so the
//!   reissue test below never finds the absorber as a radio contact, and a
//!   Foot that arrives at an absorber filled meanwhile is turned away
//!   (DOCK_NOW, which an absorber answers ROGER, would let it in). Trigger:
//!   every seek. Frequency: AI Yuri games with Bio Reactors.
//! - Mission_Unload's eject clears the latch of a human house's passenger
//!   (`0x0044DBEA`); VERA's absorbers do not eject on Unload
//!   (`techno_ai::building_missions`), so nothing clears it there.
//! - `+0x690` and `+0x691` (vt+0x348 `0x004DFCB0`, vt+0x34C `0x004DFE00`,
//!   script actions 63 and 64) and the grinder and tank-bunker seeks
//!   (vt+0x33C `0x004DFA70`, vt+0x344 `0x004DFF40`, actions 60 and 61,
//!   DecideUnitFate's choice 2) are not ported: no VERA code sets the other
//!   two latches, so they read clear.

use crate::map::entities::EntityCategory;
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::components::NavTargetRef;
use crate::sim::game_entity::GameEntity;
use crate::sim::mission::authority::LiveReadyInputProvider;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::radio::{RadioMessage, RadioPayload, RadioResponse, transmit};
use crate::sim::world::{FrameEffects, Simulation};

/// A Foot's building-entry latches. Only this module writes them.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub(crate) struct BuildingEntryLatches {
    /// `+0x68F` ShouldEnterAbsorber.
    should_enter_absorber: bool,
}

impl BuildingEntryLatches {
    /// `+0x68F`, which the Guard and AreaGuard heads test.
    pub(crate) const fn should_enter_absorber(self) -> bool {
        self.should_enter_absorber
    }

    /// `FootClass::Is_Seeking_Building_Entry @ 0x004E0080`: any of
    /// `+0x68F`/`+0x690`/`+0x691` (the last two read clear, module residual).
    pub(crate) const fn seeking(self) -> bool {
        self.should_enter_absorber
    }
}

/// EnterBioReactor's decision (`0x004DFB87..0x004DFC81`) over its walk:
/// each absorber from the house list's last entry as `(id, answered ROGER,
/// distance)`. Of those answering ROGER the first strictly nearest is the
/// pick; it is reissued (Enter and destination) unless it is the Foot's
/// first radio contact or its pending entry already.
fn pick_bio_reactor(
    walk: impl IntoIterator<Item = (u64, bool, i32)>,
    contact: Option<u64>,
    pending: Option<u64>,
) -> (Option<u64>, bool) {
    let mut best = None;
    let mut best_distance = i32::MAX;
    for (id, roger, distance) in walk {
        if roger && distance < best_distance {
            best_distance = distance;
            best = Some(id);
        }
    }
    let reissue = best.is_some_and(|best| contact != Some(best) && pending != Some(best));
    (best, reissue)
}

/// The `(dx²+dz²)+dy²` Sqrt_Approx distance from the Foot's GetCoords
/// (vt+0x48) to the absorber's, each `[x, y, z]` (`0x004DFBC4..0x004DFC30`).
fn bio_reactor_distance(foot: [i32; 3], absorber: [i32; 3]) -> i32 {
    let [fx, fy, fz] = foot;
    let [ax, ay, az] = absorber;
    crate::util::native_x87::distance_3d_leptons([fx, fz, fy], [ax, az, ay])
}

fn get_coords(sim: &Simulation, entity: &GameEntity) -> [i32; 3] {
    let coords =
        crate::sim::movement::ground_pose::object_get_coords(entity, sim.resolved_terrain.as_ref());
    [coords.x, coords.y, coords.z]
}

impl Simulation {
    /// `FootClass::EnterBioReactor @ 0x004DFB70`. Each of the house's
    /// absorbers (House+0xB0, [`Self::house_absorbers`]), from the last, is
    /// asked CAN_LOAD (vt+0x278, `0x004DFBB9`); of those answering ROGER the
    /// first strictly nearest is taken. With none the latch clears and the
    /// answer is false. With one the latch sets and, unless the Foot's first
    /// radio contact or its pending entry (`+0x500`) is that absorber
    /// already, it queues Enter (vt+0x1E8(7, 0)) and takes the absorber as
    /// its destination (vt+0x480(absorber, 1)); the answer is true.
    pub(crate) fn foot_enter_bio_reactor(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> bool {
        let Some(owner) = self.substrate.entities.get(id).map(GameEntity::owner) else {
            return false;
        };
        let absorbers = self.house_absorbers(owner, rules);
        let mut candidates = Vec::with_capacity(absorbers.len());
        for &absorber in absorbers.iter().rev() {
            let roger = transmit(
                self,
                id,
                absorber,
                RadioMessage::CanEnter,
                RadioPayload::default(),
                Some(rules),
                effects,
            ) == RadioResponse::Roger;
            let distance = match (
                self.substrate.entities.get(id),
                self.substrate.entities.get(absorber),
            ) {
                (Some(foot), Some(building)) if roger => {
                    bio_reactor_distance(get_coords(self, foot), get_coords(self, building))
                }
                _ => i32::MAX,
            };
            candidates.push((absorber, roger, distance));
        }
        let Some(foot) = self.substrate.entities.get_mut(id) else {
            return false;
        };
        let (best, reissue) = pick_bio_reactor(
            candidates,
            foot.radio_contacts.slot(0),
            foot.pending_entry(),
        );
        foot.building_entry.should_enter_absorber = best.is_some();
        let Some(best) = best else {
            return false;
        };
        if !reissue {
            return true;
        }
        let now = self.session.binary_frame;
        let _ = self.mission_queue_exact(
            id,
            MissionId::from_known(MissionType::Enter),
            0,
            now,
            &LiveReadyInputProvider { rules },
        );
        let _ = self.assign_destination_represented(
            id,
            Some(NavTargetRef::object(best)),
            Some(rules),
            registry,
            effects,
        );
        // The Boarding adapter carries the entry to the absorber (module
        // residual).
        if let Some(foot) = self.substrate.entities.get_mut(id)
            && matches!(
                foot.category,
                EntityCategory::Infantry | EntityCategory::Unit
            )
        {
            foot.passenger_role = crate::sim::passenger::PassengerRole::Boarding {
                target_transport_id: best,
            };
        }
        true
    }
}

/// Sets `+0x68F` as a seek that found an absorber would.
#[cfg(test)]
pub(crate) fn set_should_enter_absorber_for_test(entity: &mut GameEntity, value: bool) {
    entity.building_entry.should_enter_absorber = value;
}

#[cfg(test)]
#[path = "building_entry_tests.rs"]
mod tests;
