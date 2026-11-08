//! Phobos `AutoDeath`: the timed arm of `TechnoExt::CheckDeathConditions`
//! (`Ext/Techno/Body.Update.cpp:138-184`) and `TechnoExt::KillSelf`'s `Kill`
//! behavior (`:809-821`).
//!
//! ## Scope
//!
//! Only `AutoDeath.Behavior=kill` with `AutoDeath.AfterDelay=` is ported. The
//! `ShouldBeDead` flag, `OnAmmoDepletion`, `OnOwnerChange.*`,
//! `TechnosDontExist.*`, `TechnosExist.*`, `PlayerPowerState`,
//! `PlayerMoney.Max/Min`, the `Vanish`/`Sell` behaviors and
//! `AutoDeath.AllowLimboed` with `KillSelf`'s limbo arm (`:721-766`) stay open;
//! a type that authors any of them keeps that chain's gap entry.
//!
//! Never depends on render/ui/audio/net (sim invariant #1).

use crate::rules::phobos_auto_death::{AutoDeathBehavior, AutoDeathPowerStatus};
use crate::rules::ruleset::RuleSet;
use crate::sim::timer::PAUSED_START_FRAME;
use crate::sim::world::{FrameEffects, Simulation};

impl Simulation {
    /// The timed arm of `TechnoExt::CheckDeathConditions`, called from
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
        let settings = {
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
            object.phobos_auto_death
        };
        // `Vanish` and `Sell` (`:771-807`) are unported; refusing them up front
        // keeps a timer from running that could never fire.
        if settings.behavior != Some(AutoDeathBehavior::Kill) {
            return false;
        }
        // `:165-170`: a type with an `Ammo=` capacity dies at zero or below.
        if settings.on_ammo_depletion {
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
        if settings.after_delay > 0 {
            if self.auto_death_clock_expires(id, settings.after_delay) {
                return self.kill_self(id, rules, registry, effects);
            }
        }
        // `:210-221`: the low-power predicate. Phobos writes
        // `(Full && !isLowPower) || (Low && isLowPower) && !isFirstFrame`; `&&`
        // binds tighter than `||`, so the first-frame guard applies to the
        // `Low` arm only and a `Full` type dies on frame 0 too. Reproduced as
        // written rather than as it reads.
        if settings.player_power_state != AutoDeathPowerStatus::None {
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
            let dies = (settings.player_power_state == AutoDeathPowerStatus::Full && !low_power)
                || (settings.player_power_state == AutoDeathPowerStatus::Low
                    && low_power
                    && !first_frame);
            if dies {
                return self.kill_self(id, rules, registry, effects);
            }
        }
        // `:223-234`: `-1` on either bound means that bound is open.
        if settings.player_money_max != -1 || settings.player_money_min != -1 {
            let Some(owner) = self.substrate.entities.get(id).map(|entity| entity.owner()) else {
                return false;
            };
            let money = self
                .houses()
                .get(&owner)
                .map_or(0, |house| house.economy().available_money());
            let (max, min) = (settings.player_money_max, settings.player_money_min);
            if (max == -1 || money <= max) && (min == -1 || money >= min) {
                return self.kill_self(id, rules, registry, effects);
            }
        }
        false
    }

    /// The `AfterDelay` arm's clock: start it on the first visit, then report
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
