//! Phobos `AutoDeath`: `TechnoExt::CheckDeathConditions`
//! (`Ext/Techno/Body.Update.cpp:138-255`) and `TechnoExt::KillSelf`'s `Kill`
//! behavior (`:809-821`).
//!
//! ## Scope
//!
//! Ported arms, in native order: `ShouldBeDead` (`:159-163`),
//! `AutoDeath.Behavior=kill` with `OnAmmoDepletion` (`:166`), `AfterDelay`
//! (`:173-184`), `PlayerPowerState` (`:210-221`), `PlayerMoney.Max/Min`
//! (`:223-234`), `TechnosDontExist` (`:236-245`) and `TechnosExist`
//! (`:247-255`). `OnOwnerChange.*` sets the flag at the owner-change hook.
//!
//! Still open: the `Vanish`/`Sell` behaviors with `VanishAnimation` — a type
//! using them gets a flag `check_auto_death` never consumes, where native would
//! sell or vanish — `KillSelf`'s limbo arm (`:721-766`), and the scenario-level
//! `AutoDeathObjects` collection.
//! A type that authors only those keeps that chain's gap entry.
//!
//! Never depends on render/ui/audio/net (sim invariant #1).

use crate::rules::phobos_auto_death::{AutoDeathBehavior, AutoDeathPowerStatus};
use crate::rules::ruleset::RuleSet;
use crate::sim::timer::PAUSED_START_FRAME;
use crate::sim::world::{FrameEffects, Simulation};

impl Simulation {
    /// `TechnoExt::CheckDeathConditions`, called from
    /// `TechnoExt::OnEarlyUpdate` (`Ext/Techno/Body.Update.cpp:26`) after
    /// `ApplyMindControlRangeLimit` and before `ApplyInterceptor`.
    ///
    /// Native's `if (CheckDeathConditions()) return;` aborts `OnEarlyUpdate`,
    /// whose only remaining step is the unported `ApplyInterceptor`; the
    /// caller therefore continues the AI visit instead of unwinding. Native
    /// likewise keeps running the rest of `TechnoClass_AI` on the object
    /// `ReceiveDamage` already erased, and VERA's later steps carry the same
    /// "entity gone" guards. Returns whether a self-kill was issued so a
    /// future `ApplyInterceptor` port can honour the early return.
    #[cfg(feature = "phobos")]
    pub(crate) fn check_auto_death(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> bool {
        // Only the scalar keys are copied: the type's presence lists are read
        // later, under a borrow that ends before any mutation.
        let (
            behavior,
            after_delay,
            on_ammo_depletion,
            power_state,
            money_max,
            money_min,
            should_be_dead,
        ) = {
            let Some(entity) = self.substrate.entities.get(id) else {
                return false;
            };
            let Some(object) = self.interner.object_type(entity.type_ref(), rules) else {
                return false;
            };
            // `:147-153` asks `AllowLimboed` here; that key and `KillSelf`'s
            // limbo arm are unported, so a limboed object never dies in VERA.
            if entity.lifecycle.in_limbo() {
                return false;
            }
            let settings = &object.phobos_auto_death;
            (
                settings.behavior,
                settings.after_delay,
                settings.on_ammo_depletion,
                settings.player_power_state,
                settings.player_money_max,
                settings.player_money_min,
                entity.should_be_dead(),
            )
        };
        // `Vanish` and `Sell` (`:771-807`) are unported; refusing them up front
        // keeps a timer from running that could never fire.
        if behavior != Some(AutoDeathBehavior::Kill) {
            return false;
        }
        // `:159-163`: a flag another hook set kills the object on this visit.
        if should_be_dead {
            return self.kill_self(id, rules, registry, effects);
        }
        // `:165-170`: a type with an `Ammo=` capacity dies at zero or below.
        if on_ammo_depletion {
            let (capacity, current) = {
                let Some(entity) = self.substrate.entities.get(id) else {
                    return false;
                };
                (
                    self.interner
                        .object_type(entity.type_ref(), rules)
                        .map_or(0, |object| object.ammo),
                    entity.ammo().map_or(0, |ammo| ammo.current()),
                )
            };
            if capacity > 0 && current <= 0 {
                return self.kill_self(id, rules, registry, effects);
            }
        }
        // `:172-184`: the timed arm. Phobos tests `HasStarted()` before
        // `Completed()`, which matters because an unstarted `CDTimerClass`
        // reads as already completed.
        if after_delay > 0 {
            if self.auto_death_clock_expires(id, after_delay) {
                return self.kill_self(id, rules, registry, effects);
            }
        }
        // `:210-221`: the low-power predicate. Phobos writes
        // `(Full && !isLowPower) || (Low && isLowPower) && !isFirstFrame`; `&&`
        // binds tighter than `||`, so the first-frame guard applies to the
        // `Low` arm only and a `Full` type dies on frame 0 too. Reproduced as
        // written rather than as it reads.
        if power_state != AutoDeathPowerStatus::None {
            let low_power = self
                .power_states()
                .get(
                    &self
                        .substrate
                        .entities
                        .get(id)
                        .map_or(crate::sim::intern::InternedId::default(), |entity| {
                            entity.owner()
                        }),
                )
                .is_some_and(crate::sim::power_system::PowerState::is_low_power);
            let first_frame = self.session.binary_frame() == 0;
            let dies = (power_state == AutoDeathPowerStatus::Full && !low_power)
                || (power_state == AutoDeathPowerStatus::Low && low_power && !first_frame);
            if dies {
                return self.kill_self(id, rules, registry, effects);
            }
        }
        // `:223-234`: `-1` on either bound means that bound is open.
        if money_max != -1 || money_min != -1 {
            let Some(owner) = self.substrate.entities.get(id).map(|entity| entity.owner()) else {
                return false;
            };
            let money = self
                .houses()
                .get(&owner)
                .map_or(0, |house| house.economy().available_money());
            let (max, min) = (money_max, money_min);
            if (max == -1 || money <= max) && (min == -1 || money >= min) {
                return self.kill_self(id, rules, registry, effects);
            }
        }
        let Some(owner) = self.substrate.entities.get(id).map(|entity| entity.owner()) else {
            return false;
        };
        // `:236-255`: both presence scans, evaluated under one immutable borrow
        // so no type settings are cloned per visit.
        let kill = {
            let Some(entity) = self.substrate.entities.get(id) else {
                return false;
            };
            let Some(object) = self.interner.object_type(entity.type_ref(), rules) else {
                return false;
            };
            let settings = &object.phobos_auto_death;
            let dont = self.auto_death_resolve_technos(&settings.technos_dont_exist, rules);
            let dont_kills = !dont.is_empty()
                && !self.auto_death_technos_exist(
                    &dont,
                    settings.technos_dont_exist_houses,
                    !settings.technos_dont_exist_any,
                    settings.technos_dont_exist_allow_limboed.unwrap_or(
                        rules
                            .phobos_globals
                            .auto_death_technos_dont_exist_allow_limboed,
                    ),
                    owner,
                );
            if dont_kills {
                true
            } else {
                let exist = self.auto_death_resolve_technos(&settings.technos_exist, rules);
                !exist.is_empty()
                    && self.auto_death_technos_exist(
                        &exist,
                        settings.technos_exist_houses,
                        settings.technos_exist_any,
                        settings
                            .technos_exist_allow_limboed
                            .unwrap_or(rules.phobos_globals.auto_death_technos_exist_allow_limboed),
                        owner,
                    )
            }
        };
        if kill {
            return self.kill_self(id, rules, registry, effects);
        }
        false
    }

    /// The native list is a `ValueableVector<TechnoTypeClass*>`: unknown tokens
    /// never reach it (`Utilities/TemplateDef.h:1727-1742`), so a typo leaves
    /// the arm's emptiness guard in charge rather than changing the decision.
    /// Tokens arrive untrimmed, as `strtok(",")` yields them.
    fn auto_death_resolve_technos(
        &self,
        list: &[String],
        rules: &RuleSet,
    ) -> Vec<(
        Option<crate::sim::intern::InternedId>,
        crate::map::entities::EntityCategory,
    )> {
        let mut resolved = Vec::new();
        for name in list {
            // The type registry is the authority, as native's `TechnoTypeClass::Find`
            // is; the interner only knows names this match has already used, so a
            // known type with no instance resolves to `None` and counts zero.
            let Some(object) = rules.object(name) else {
                continue;
            };
            let category = match object.category {
                crate::rules::object_type::ObjectCategory::Infantry => {
                    crate::map::entities::EntityCategory::Infantry
                }
                crate::rules::object_type::ObjectCategory::Vehicle => {
                    crate::map::entities::EntityCategory::Unit
                }
                crate::rules::object_type::ObjectCategory::Aircraft => {
                    crate::map::entities::EntityCategory::Aircraft
                }
                crate::rules::object_type::ObjectCategory::Building => {
                    crate::map::entities::EntityCategory::Structure
                }
            };
            resolved.push((self.interner.get(name), category));
        }
        resolved
    }

    /// `TechnoClass_Captured_UpdateTracking @ 0x7015C9`'s AutoDeath block
    /// (`Ext/House/Hooks.cpp:231-259`), run before that hook's tracking work.
    /// Sets `ShouldBeDead` from the owner-change direction keys.
    ///
    /// RESIDUAL: `FootExt::IsOwnerChangeFromRevertOnExit` is unported — the
    /// AttachEffect discard chain also reads it as `false` — so the
    /// `IgnoreRevertOnExit` arm and the transporter gate's revert term cannot
    /// fire; the key is read, the guard it needs is not.
    #[cfg(feature = "phobos")]
    pub(crate) fn check_auto_death_owner_change(
        &mut self,
        id: u64,
        new_owner: crate::sim::intern::InternedId,
        rules: &RuleSet,
    ) {
        let (master, to_computer, to_human, allow_limboed, old_owner, has_transporter) = {
            let Some(entity) = self.substrate.entities.get(id) else {
                return;
            };
            let Some(object) = self.interner.object_type(entity.type_ref(), rules) else {
                return;
            };
            let settings = &object.phobos_auto_death;
            if settings.behavior.is_none() {
                return;
            }
            (
                settings.on_owner_change,
                settings.on_owner_change_human_to_computer,
                settings.on_owner_change_computer_to_human,
                settings.allow_limboed,
                entity.owner(),
                entity.passenger_role.transporter().is_some(),
            )
        };
        // `HouseClass::IsControlledByHuman`, the same mode-aware predicate the
        // AttachEffect owner-change hook reads.
        let game_mode_nonzero = self.session.game_mode_nonzero();
        let old_human = self
            .houses()
            .get(&old_owner)
            .is_some_and(|house| house.is_controlled_by_human(game_mode_nonzero));
        let new_human = self
            .houses()
            .get(&new_owner)
            .is_some_and(|house| house.is_controlled_by_human(game_mode_nonzero));
        let to_computer = to_computer.unwrap_or(master);
        let to_human = to_human.unwrap_or(master);
        let mut should = false;
        if to_computer && to_human {
            should = true;
        } else if to_computer || to_human {
            if old_human != new_human && ((old_human && to_computer) || (!old_human && to_human)) {
                should = true;
            }
        }
        // Native ORs into the existing flag, then `:253-258` clears it on a
        // transported object unless `AllowLimboed` (default `yes`).
        let already = self
            .substrate
            .entities
            .get(id)
            .is_some_and(crate::sim::game_entity::GameEntity::should_be_dead);
        let mut value = already || should;
        if value
            && has_transporter
            && !allow_limboed.unwrap_or(rules.phobos_globals.auto_death_allow_limboed)
        {
            value = false;
        }
        if value != already
            && let Some(entity) = self.substrate.entities.get_mut(id)
        {
            entity.set_should_be_dead(value);
        }
    }

    /// `Ext/Techno/Body.Update.cpp:188-208`: whether the listed types exist for
    /// the selected houses. `any` selects `any_of`; `all_of` otherwise. The
    /// non-limbo count is `HouseClass::CountOwnedAndPresent`; VERA reads the
    /// on-map `+0x5550` family (`active_count`). RESIDUAL: the wrapper's body
    /// was not read instruction-level, and `docs/phobos-harvester-counter.md`
    /// records `+0x5500` as the same query, so the counter identity is not
    /// proven here. `allow_limbo` adds the distinct limbo count.
    #[allow(clippy::too_many_arguments)]
    fn auto_death_technos_exist(
        &self,
        list: &[(
            Option<crate::sim::intern::InternedId>,
            crate::map::entities::EntityCategory,
        )],
        houses: crate::rules::phobos_shields::AffectedHouse,
        any: bool,
        allow_limbo: bool,
        owner: crate::sim::intern::InternedId,
    ) -> bool {
        let exist_single = |(type_id, category): &(
            Option<crate::sim::intern::InternedId>,
            crate::map::entities::EntityCategory,
        )| {
            let (type_id, category) = (*type_id, *category);
            // A known type this match has not interned yet has no instances.
            let Some(type_id) = type_id else {
                return false;
            };
            let count_in = |house: &crate::sim::house_state::HouseState| {
                if allow_limbo {
                    house
                        .tracking()
                        .count_owned_present_and_limboed(category, type_id)
                } else {
                    house.tracking().active_count(category, type_id)
                }
            };
            if houses == crate::rules::phobos_shields::AffectedHouse::OWNER {
                return self
                    .houses()
                    .get(&owner)
                    .is_some_and(|house| count_in(house) > 0);
            }
            self.houses().iter().any(|(house_id, house)| {
                crate::sim::combat::can_target_house(
                    houses,
                    owner,
                    *house_id,
                    self.houses(),
                    self.house_alliances(),
                    self.interner(),
                ) && count_in(house) > 0
            })
        };
        if any {
            list.iter().any(|name| exist_single(name))
        } else {
            list.iter().all(|name| exist_single(name))
        }
    }

    /// The `AfterDelay` arm's clock
    /// whether it has completed. `HasStarted()` is the `PAUSED_START_FRAME`
    /// test VERA uses everywhere else for an unstarted `CDTimerClass`.
    fn auto_death_clock_expires(&mut self, id: u64, after_delay: i32) -> bool {
        let frame = self.session.binary_frame() as i32;
        let Some(timer) = self
            .substrate
            .entities
            .get(id)
            .map(crate::sim::game_entity::GameEntity::auto_death_timer)
        else {
            return false;
        };
        // `:175-183`: start on the first visit, then kill once `Completed()`.
        // Phobos checks `HasStarted()` before `Completed()`, which matters
        // because an unstarted `CDTimerClass` reads as already completed.
        if timer.start_frame() == PAUSED_START_FRAME {
            if let Some(entity) = self.substrate.entities.get_mut(id) {
                entity.auto_death_timer_mut().start(frame, after_delay);
            }
            return false;
        }
        timer.expired(frame)
    }

    /// `TechnoExt::KillSelf`'s non-limbo `Kill` arm (`:809-821`):
    /// `[CombatDamage] C4Warhead=` with the object's own Health, no attacker,
    /// no source house and ignoring defenses, through the shared self-kill
    /// owner. arg6 is `false`, unlike the locomotor self-destruct's `true`.
    fn kill_self(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> bool {
        // RESIDUAL: Phobos calls Ares `SpawnSurvivors(pThis, nullptr, false,
        // false)` immediately before this hit `:810-816`, because the hit's
        // `ignoreDefenses` makes the death path's own survivor release refuse
        // (it marks the transport handled and ejects nobody). VERA's
        // `release_ares_vehicle_survivors` assumes the damage path has already
        // lifted the unit (`foot_mark_remove`), so calling it from here places
        // no survivor; trigger: `AutoDeath.Behavior=kill` on a `Crewed=` or
        // driver Unit/Aircraft in an Ares build; effect: the crew is lost where
        // native ejects it. Reproducing it needs the survivor owner taught the
        // pre-damage context, which is its own chain.
        self.receive_own_health_c4(id, rules, registry, effects, false);
        true
    }
}

#[cfg(all(test, feature = "phobos"))]
#[path = "auto_death_tests.rs"]
mod tests;
