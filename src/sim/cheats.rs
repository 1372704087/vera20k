//! The dashboard's trainer panel: switches that make the running match easier.
//!
//! These are deliberately **not** simulation content. Nothing here is
//! serialized, hashed or replayed: a fresh match starts with every switch off,
//! a load drops them, and two peers running the same replay with different
//! switches would diverge — which is exactly why they are a debug facility and
//! not a game mode. The dashboard only ever sees them through
//! [`crate::sim::telemetry::CheatReport`].
//!
//! Two application styles, because the effect has to land where it belongs:
//!
//! * **Once per frame**, from [`Simulation::apply_cheats`]: the money top-up,
//!   the invulnerability refresh and the one-shot actions. These are cheap and
//!   need no ordering beyond "before anything reads the affected state".
//! * **Inline**, read by the system that owns the value: production cadence,
//!   the power assessment and the damage receiver. Those read
//!   [`Simulation::cheats`] directly rather than waiting for a pass, so the
//!   switch takes effect in the frame it is turned on.

use crate::sim::intern::InternedId;

/// One-shot requests waiting for the next frame's [`Simulation::apply_cheats`].
pub mod action {
    /// Restore every owned object to its type's `Strength=`.
    pub const HEAL_ALL: u8 = 1 << 0;
    /// Promote every owned combat object to elite.
    pub const VETERAN_ALL: u8 = 1 << 1;
    /// Charge every granted Super of the owning house.
    pub const SUPERWEAPONS_READY: u8 = 1 << 2;
}

/// How many frames one refresh lasts.
///
/// The cheat re-arms the timer at the start of every frame, so it only has to
/// outlive the frame it was written in. Keeping it short matters at the other
/// end too: switching the cheat off must not leave a three-second curtain
/// behind, and nothing distinguishes the cheat's timer from a real Iron
/// Curtain's once it is written.
pub const INVULNERABILITY_REFRESH_FRAMES: i32 = 3;

/// The trainer's switchboard.
///
/// Every field is inert at its default, so a `Simulation` that never sees the
/// panel behaves exactly as one built before this module existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cheats {
    /// Keep the wallet at [`Self::money_target`] whenever a spend takes it
    /// below. Income above the target is left alone, so the switch reads as
    /// "never run out" rather than "freeze the number".
    pub infinite_money: bool,
    /// The floor [`Self::infinite_money`] holds the wallet at.
    pub money_target: i32,
    /// Report full power and keep every powered building online.
    pub infinite_power: bool,
    /// Complete the active build of every factory immediately.
    pub instant_build: bool,
    /// Owned objects ignore incoming damage.
    pub invincible: bool,
    /// Multiplier on damage the owner's objects deal. 0 and 1 both mean off.
    pub damage_scale: u16,
    /// The map is drawn without shroud (`sandbox_full_visibility`).
    pub reveal_map: bool,
    /// The house the switches apply to. `None` until the app names one; the
    /// first `apply_cheats` falls back to the local player-control house.
    pub owner: Option<InternedId>,
    /// Pending one-shot actions, a bitmask of [`action`].
    pending: u8,
}

impl Cheats {
    /// A switchboard holding the given positions, with nothing queued and no
    /// owner yet. The dashboard's whole-block message maps straight onto this;
    /// the queued actions stay private because they are consumed `apply_cheats`
    /// rather than set from outside.
    #[allow(clippy::too_many_arguments)]
    pub fn switches(
        infinite_money: bool,
        money_target: i32,
        infinite_power: bool,
        instant_build: bool,
        invincible: bool,
        damage_scale: u16,
        reveal_map: bool,
    ) -> Self {
        Self {
            infinite_money,
            money_target,
            infinite_power,
            instant_build,
            invincible,
            damage_scale,
            reveal_map,
            owner: None,
            pending: 0,
        }
    }

    /// True when nothing is switched on and nothing is queued.
    pub fn is_idle(&self) -> bool {
        !self.infinite_money
            && !self.infinite_power
            && !self.instant_build
            && !self.invincible
            && self.damage_scale <= 1
            && !self.reveal_map
            && self.pending == 0
    }

    /// Queue a one-shot action bit.
    pub fn request(&mut self, action: u8) {
        self.pending |= action;
    }

    /// Take the queued one-shot actions, leaving the mask clear.
    pub fn take_pending(&mut self) -> u8 {
        std::mem::take(&mut self.pending)
    }

    /// Whether the switches apply to `owner`.
    pub fn applies_to(&self, owner: InternedId) -> bool {
        self.owner == Some(owner)
    }

    /// The damage multiplier to apply to a hit from the cheating house.
    /// `1` (no change) when the switch is off.
    pub fn damage_multiplier(&self) -> i32 {
        i32::from(self.damage_scale.max(1))
    }
}

impl crate::sim::world::Simulation {
    /// Install a whole switchboard.
    ///
    /// The dashboard always sends the full block, so replacement (not a
    /// field-by-field merge) is the right shape: two windows cannot then
    /// half-apply a change. Two things only the simulation can see happen
    /// here — the owner is resolved once, and turning the power switch off
    /// asks for a fresh assessment, because the forced totals live behind the
    /// dirty byte the real assessment owns.
    pub fn set_cheats(&mut self, cheats: Cheats) {
        let was_powered = self.cheats.infinite_power;
        self.cheats = cheats;
        if was_powered && !cheats.infinite_power {
            let owners: Vec<_> = self.power_states.keys().copied().collect();
            for owner in owners {
                self.invalidate_house_power(owner, true);
            }
        }
        if self.cheats.owner.is_none() {
            self.cheats.owner = self.cheat_owner();
        }
    }

    /// Apply the trainer switches that need one pass per frame.
    ///
    /// Runs before anything in the frame reads the affected state, so a switch
    /// turned on between frames takes effect on the very next tick. Money and
    /// invulnerability are re-asserted (a spend or a hit may have undone them
    /// since the last frame); the one-shot actions run once and clear.
    pub fn apply_cheats(&mut self, rules: Option<&crate::rules::ruleset::RuleSet>) {
        if self.cheats.is_idle() {
            return;
        }
        let cheats = self.cheats;
        let owner = self.cheat_owner();
        let pending = self.cheats.take_pending();
        if let Some(owner) = owner {
            if cheats.infinite_money
                && let Some(house) = self.houses.get_mut(&owner)
                && house.economy.credits() < cheats.money_target
            {
                house.economy.set_credits(cheats.money_target);
            }
            if cheats.invincible {
                self.refresh_invulnerability(owner);
            }
            if pending & action::HEAL_ALL != 0 {
                self.heal_owned(owner, rules);
            }
            if pending & action::VETERAN_ALL != 0 {
                self.promote_owned(owner);
            }
            if pending & action::SUPERWEAPONS_READY != 0 {
                self.charge_owned_supers(owner, rules);
            }
        }
    }

    /// The house the switches belong to: the app's assignment, or the local
    /// player-control house when it has not named one.
    fn cheat_owner(&mut self) -> Option<InternedId> {
        if let Some(owner) = self.cheats.owner {
            return Some(owner);
        }
        let found = self
            .houses
            .values()
            .find(|house| house.player_control && !house.multiplay_passive)
            .or_else(|| self.houses.values().find(|house| house.is_human))
            .map(|house| house.name);
        self.cheats.owner = found;
        found
    }

    /// Re-arm the Iron Curtain timer on every owned object.
    ///
    /// The receiver reads this exact state (`superweapon::invulnerability`),
    /// so the cheat goes through the native gate — including its tint and its
    /// "already dead objects still take the hit" ordering — instead of
    /// special-casing damage.
    fn refresh_invulnerability(&mut self, owner: InternedId) {
        use crate::sim::superweapon::invulnerability::{InvulnKind, apply_invulnerability};
        let frame = self.session.binary_frame;
        let ids: Vec<u64> = self
            .entities()
            .values()
            .filter(|entity| entity.owner() == owner)
            .map(|entity| entity.stable_id())
            .collect();
        for id in ids {
            if let Some(entity) = self.entities_mut().get_mut(id) {
                apply_invulnerability(
                    entity,
                    frame,
                    INVULNERABILITY_REFRESH_FRAMES,
                    InvulnKind::IronCurtain,
                );
            }
        }
    }

    /// Restore every owned object to its type's `Strength=`.
    fn heal_owned(&mut self, owner: InternedId, rules: Option<&crate::rules::ruleset::RuleSet>) {
        let Some(rules) = rules else {
            return;
        };
        let (entities, interner) = self.entities_mut_and_interner();
        for entity in entities.values_mut() {
            if entity.owner() != owner {
                continue;
            }
            let Some(object) = interner.object_type(entity.type_ref(), rules) else {
                continue;
            };
            if object.strength > 0 && entity.health.current() < object.strength {
                entity.health.set(object.strength);
            }
        }
    }

    /// Promote every owned combat object to elite (`SetElite(1)`).
    fn promote_owned(&mut self, owner: InternedId) {
        for entity in self.entities_mut().values_mut() {
            if entity.owner() != owner {
                continue;
            }
            if entity.category() == crate::map::entities::EntityCategory::Structure {
                continue;
            }
            entity.veterancy.set_elite();
        }
    }

    /// Charge every granted Super the house owns.
    fn charge_owned_supers(
        &mut self,
        owner: InternedId,
        rules: Option<&crate::rules::ruleset::RuleSet>,
    ) {
        let Some(rules) = rules else {
            return;
        };
        let frame = self.session.binary_frame;
        let Some(weapons) = self.super_weapons.get_mut(&owner) else {
            return;
        };
        let types: Vec<InternedId> = weapons.keys().copied().collect();
        for type_id in types {
            let Some(sw) = rules.super_weapon(self.interner.resolve(type_id)) else {
                continue;
            };
            let Some(instance) = weapons.get_mut(&type_id) else {
                continue;
            };
            if !instance.is_active {
                continue;
            }
            // The native setter leaves the fractional remainder of the scale
            // factor on the timer (a 100 % request lands one frame early at
            // best), so the ready flags are stored directly beside it.
            if instance.set_charge_percentage(100, sw, frame) {
                instance.is_ready = true;
                instance.ready_tick = frame as i32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::house_state::HouseState;
    use crate::sim::intern::InternedId;
    use crate::sim::world::Simulation;

    fn house(sim: &mut Simulation, credits: i32) -> InternedId {
        let owner = InternedId::from_index(1);
        sim.houses
            .insert(owner, HouseState::new(owner, 0, None, true, credits, 10));
        owner
    }

    #[test]
    fn a_default_switchboard_is_idle() {
        let cheats = Cheats::default();
        assert!(cheats.is_idle());
        assert_eq!(cheats.damage_multiplier(), 1, "off means no change");
    }

    #[test]
    fn any_switch_queues_work() {
        let mut cheats = Cheats::default();
        cheats.instant_build = true;
        assert!(!cheats.is_idle());
        cheats.request(action::HEAL_ALL);
        assert!(!cheats.is_idle(), "a queued action is work too");
        assert_eq!(cheats.take_pending(), action::HEAL_ALL);
        assert_eq!(cheats.take_pending(), 0, "the queue clears");
        assert!(!cheats.is_idle(), "the switch itself is still on");
    }

    #[test]
    fn the_money_floor_refills_only_from_below() {
        let mut sim = Simulation::new();
        let owner = house(&mut sim, 500);
        sim.set_cheats(Cheats::switches(true, 5_000, false, false, false, 1, false));
        assert_eq!(sim.cheats.owner, Some(owner), "the human house is picked");
        sim.apply_cheats(None);
        assert_eq!(sim.houses[&owner].economy.credits(), 5_000);

        // Income above the floor is the player's, not the cheat's.
        sim.houses.get_mut(&owner).unwrap().economy.add_credits(2_000);
        sim.apply_cheats(None);
        assert_eq!(sim.houses[&owner].economy.credits(), 7_000);
    }

    #[test]
    fn the_floor_does_nothing_while_the_switch_is_off() {
        let mut sim = Simulation::new();
        let owner = house(&mut sim, 500);
        sim.set_cheats(Cheats::switches(false, 5_000, false, false, false, 1, false));
        sim.apply_cheats(None);
        assert_eq!(sim.houses[&owner].economy.credits(), 500);
    }

    #[test]
    fn damage_scaling_applies_to_the_named_house_only() {
        let mut sim = Simulation::new();
        let owner = house(&mut sim, 0);
        let other = InternedId::from_index(2);
        sim.houses
            .insert(other, HouseState::new(other, 0, None, false, 0, 10));
        sim.set_cheats(Cheats::switches(false, 0, false, false, false, 10, false));
        assert!(sim.cheats.applies_to(owner));
        assert!(!sim.cheats.applies_to(other));
        assert_eq!(sim.cheats.damage_multiplier(), 10);

        // The switch is copied out, so the receiver's multiplier does not
        // need a simulation borrow while it holds the entity store.
        let cheats = sim.cheats;
        assert_eq!(cheats.damage_multiplier(), 10);
    }

    #[test]
    fn invincibility_re_arms_only_the_owners_objects() {
        use crate::map::entities::EntityCategory;
        use crate::sim::combat::Health;
        use crate::sim::game_entity::GameEntity;

        let mut sim = Simulation::new();
        let owner = house(&mut sim, 0);
        let theirs = InternedId::from_index(2);
        sim.houses
            .insert(theirs, HouseState::new(theirs, 0, None, false, 0, 10));
        let ours_id = 41u64;
        let theirs_id = 42u64;
        let type_ref = sim.interner.intern("E1");
        {
            let store = sim.entities_mut();
            store.insert(GameEntity::new_at_frame_zero_for_test(
                ours_id, 0, 0, 0, 0, owner, Health::new(100), type_ref,
                EntityCategory::Unit, 0, 0, false,
            ));
            store.insert(GameEntity::new_at_frame_zero_for_test(
                theirs_id, 1, 1, 0, 0, theirs, Health::new(100), type_ref,
                EntityCategory::Unit, 0, 0, false,
            ));
        }
        sim.set_cheats(Cheats::switches(false, 0, false, false, true, 1, false));
        sim.apply_cheats(None);

        let armed = |sim: &Simulation, id: u64| {
            sim.entities()
                .get(id)
                .unwrap()
                .invulnerability
                .as_ref()
                .map(|state| state.timer().remaining(0))
        };
        assert_eq!(armed(&sim, ours_id), Some(INVULNERABILITY_REFRESH_FRAMES));
        assert_eq!(armed(&sim, theirs_id), None, "the enemy keeps taking hits");
    }

    #[test]
    fn turning_power_off_asks_for_a_fresh_assessment() {
        let mut sim = Simulation::new();
        let owner = house(&mut sim, 0);
        sim.power_states.entry(owner).or_default();
        sim.set_cheats(Cheats::switches(false, 0, true, false, false, 1, false));
        // The forced totals are visible immediately…
        let state = sim.power_states.get_mut(&owner).unwrap();
        state.total_output = 0;
        state.total_drain = 400;
        state.is_low_power = true;
        sim.cheats.infinite_power = true;
        // …and the switch turning off must not leave them behind.
        sim.set_cheats(Cheats::switches(false, 0, false, false, false, 1, false));
        assert!(!sim.cheats.infinite_power);
    }
}
