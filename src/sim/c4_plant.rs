//! A demolition infantryman's C4 plant on a building.
//!
//! Two orders put a `C4=` infantryman on Sabotage (mission 17) with a
//! building as its destination: the player's Sabotage order (the Event's
//! MegaMission, `Command::PlantC4`) and the C4 arm that opens
//! `InfantryClass::Mission_Attack` (`0x0051F3E3..0x0051F455`,
//! `techno_ai::mission_handlers`), which a computer's Tanya team reaches
//! through its script's attack. Sabotage dispatches to
//! `FootClass::Mission_Capture` (`sim::mission::capture`), which heads for a
//! building Target again once it has no destination. The walk enters the
//! building's cells (`InfantryClass::Can_Enter_Cell`, `world::object_entry`),
//! and on arrival `InfantryClass::Per_Cell_Process` plants the charge
//! ([`Simulation::infantry_per_cell_c4`]). `BuildingClass::Update` sets it
//! off when its C4Timer runs out (`Simulation::tick_pending_building_detonation`;
//! a bridge repair hut's collapses its bridge, `tick_bridge_hut_c4`).
//!
//! The arm draws twice from the Scenario stream: the planter's `GetROF(1)`
//! (`RandomRanged(3, 5)` for a retail Sapper, none for an empty slot), then
//! its Scatter's heading (`RandomRanged(0, 4)`). It writes the building's
//! C4Timer and the planter's rearm timer, and detaches nothing.
//!
//! RESIDUAL:
//! - The arm's Tag event 1 ("entered by", `0x0051A507..0x0051A51C`): VERA
//!   attaches no Tags to objects. Trigger: a map building with a Tag that
//!   is planted on. Effect: its trigger does not spring.
//! - The cell arm that follows (`0x0051A667..0x0051A7F2`): a C4 infantryman
//!   on Sabotage standing in its NavCom cell damages the area around it
//!   with `C4Warhead=`, scatters and queues mission 2. Its producer, a
//!   Sabotage order on a cell (a bridge), has no VERA order. Trigger: a
//!   player's C4 order on a bridge. Effect: the bridge cannot be demolished
//!   that way; its repair hut can.
//! - The arm tests the type's `C4=` (`+0xEC2`) only, where Mission_Attack's
//!   arm also takes weapon ability 14: a planter that holds C4 only by rank
//!   walks in and plants nothing. No retail type lists the ability.

use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::TargetKind;
use crate::sim::components::{NavTargetRef, PendingC4Detonation};
use crate::sim::mission::MissionType;
use crate::sim::movement::locomotor::MovementLayer;
use crate::sim::timer::CdTimer;
use crate::sim::world::{FrameAdvanceError, FrameEffects, Simulation};

impl Simulation {
    /// `InfantryClass::Per_Cell_Process`'s C4 arm (`0x0051A4BF..0x0051A664`),
    /// on a reason-2 arrival. An infantryman on Sabotage whose type is `C4=`,
    /// standing on a cell whose first ground building (`0x0047C520`) is its
    /// NavCom:
    /// - When that building is being sold or is Iron Curtained (vt+0x184,
    ///   vt+0x160), nothing is planted.
    /// - When it already holds a charge (`+0x6DF`, which a PostMortem also
    ///   sets), the planter drops its destination (vt+0x480(NULL, 1)), rearms
    ///   and scatters (`0x0051A550..0x0051A5A4`).
    /// - Otherwise the building flashes for [`RuleSet::c4_flash_frames`]
    ///   (vt+0x148), and its C4Timer starts for [`RuleSet::c4_delay_frames`]
    ///   with the planter as C4AppliedBy (`+0x540`; `0x0051A5A7..0x0051A608`).
    ///
    /// Unless it scattered for a charge already there, the planter then stops
    /// (`0x004DF0D0`), uncloaks (vt+0x45C(0)), rearms and scatters
    /// (`0x0051A60B..0x0051A657`). Its rearm is [`Self::rearm_c4_planter`];
    /// its Scatter, from the building's coordinate
    /// ([`Self::scatter_from_building`]), does not Process.
    ///
    /// Answers whether the arm ran; it returns before the Foot body.
    pub(crate) fn infantry_per_cell_c4(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> Result<bool, FrameAdvanceError> {
        let Some(infantry) = self.substrate.entities.get(id) else {
            return Ok(false);
        };
        if infantry.mission.effective().known() != Some(MissionType::Sabotage)
            || !self
                .object_type(infantry.type_ref(), rules)
                .is_some_and(|object| object.c4)
        {
            return Ok(false);
        }
        let heading_for = match infantry.navigation.nav_com {
            Some(
                NavTargetRef::Entity { id }
                | NavTargetRef::Object { id }
                | NavTargetRef::Building { id },
            ) => Some(id),
            _ => None,
        };
        let Some(building) = self
            .substrate
            .occupancy
            .first_building_on_layer(
                infantry.position.rx,
                infantry.position.ry,
                MovementLayer::Ground,
            )
            .filter(|&building| heading_for == Some(building))
        else {
            return Ok(false);
        };
        let frame = self.session.binary_frame;
        let Some(target) = self.substrate.entities.get(building) else {
            return Ok(false);
        };
        let refused = target.mission.effective().known() == Some(MissionType::Selling)
            || crate::sim::superweapon::invulnerability::is_invulnerable(
                target.invulnerability.as_ref(),
                frame,
            );
        if !refused {
            if target.pending_c4_detonation.is_some() {
                self.assign_null_destination(id, Some(rules), registry, effects);
                self.rearm_c4_planter(id, building, rules);
                self.scatter_from_building(id, building, rules, registry, effects)?;
                return Ok(true);
            }
            let (flash, fuse) = (rules.c4_flash_frames(), rules.c4_delay_frames());
            if let Some(target) = self.substrate.entities.get_mut(building) {
                target.flash_building(flash);
                target.pending_c4_detonation = Some(PendingC4Detonation {
                    timer: CdTimer::started(frame as i32, fuse),
                    source_entity_id: Some(id),
                });
            }
        }
        if let Some(infantry) = self.substrate.entities.get_mut(id) {
            crate::sim::movement::foot_stop_moving(infantry);
        }
        crate::sim::combat::world_fire::start_uncloaking_to_fire(self, rules, id);
        self.rearm_c4_planter(id, building, rules);
        self.scatter_from_building(id, building, rules, registry, effects)?;
        Ok(true)
    }

    /// The planter's `RearmTimer.Start(GetROF(1))` (vt+0x318;
    /// `0x0051A55E..0x0051A587`, `0x0051A61E..0x0051A647`): its slot-1
    /// weapon at its own burst index (`+0x3B8`). Outside FireAt that index is
    /// below the retail Sapper's `Burst=`, so GetROF takes its mid-burst arm
    /// and draws `RandomRanged(3, 5)`; PTROOP's empty slot answers 1 without
    /// a draw.
    ///
    /// RESIDUAL: VERA keeps no live particle systems on an object, so
    /// GetROF's spark, fire and railgun arms see none. No retail planter's
    /// slot-1 weapon has those flags.
    fn rearm_c4_planter(&mut self, id: u64, building: u64, rules: &RuleSet) {
        let Some(planter) = self.substrate.entities.get(id) else {
            return;
        };
        let Some(obj) = self.object_type(planter.type_ref(), rules) else {
            return;
        };
        let weapon = crate::sim::combat::combat_weapon::get_weapon(
            planter,
            obj,
            1,
            &self.substrate.entities,
            rules,
            &self.interner,
        );
        let snap = crate::sim::combat::build_attacker_snapshot(
            planter,
            TargetKind::Entity(building),
            None,
        );
        let burst_index = planter.weapon_burst.index();
        let rof = crate::sim::combat::world_fire::snapshot_get_rof(
            self,
            rules,
            &snap,
            obj,
            weapon,
            crate::sim::combat::rof::LiveParticles::default(),
            burst_index,
        );
        let frame = self.session.binary_frame as i32;
        if let Some(planter) = self.substrate.entities.get_mut(id) {
            planter.rearm_timer.start(frame, rof);
        }
    }
}

#[cfg(test)]
#[path = "c4_plant_tests.rs"]
mod tests;
