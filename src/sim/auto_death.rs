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

use crate::rules::phobos_auto_death::AutoDeathBehavior;
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
    /// caller therefore continues the AI visit instead of unwinding. Returns
    /// whether a self-kill was issued so a future `ApplyInterceptor` port can
    /// honour the early return.
    ///
    /// RESIDUAL: the caller keeps running this object's `TechnoClass_AI` body
    /// after a fatal self-kill, where native returns from `OnEarlyUpdate` and
    /// then continues `TechnoClass_AI` on the object `ReceiveDamage` already
    /// erased. VERA stops that object's later steps only through the existing
    /// "entity gone" guards; the difference is not player-visible because a
    /// killed object's later AI output is discarded.
    #[cfg(feature = "phobos")]
    pub(crate) fn check_auto_death(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> bool {
        let (behavior, after_delay) = {
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
            let settings = object.phobos_auto_death;
            let Some(behavior) = settings.behavior else {
                return false;
            };
            (behavior, settings.after_delay)
        };
        // `Vanish` and `Sell` (`:771-807`) are unported; refusing them up front
        // keeps a timer from running that could never fire.
        if behavior != AutoDeathBehavior::Kill || after_delay <= 0 {
            return false;
        }
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
        if !timer.expired(frame) {
            return false;
        }
        self.kill_self(id, rules, registry, effects)
    }

    /// `TechnoExt::KillSelf`'s non-limbo `Kill` arm (`:809-821`):
    /// `ReceiveDamage(&pThis->Health, 0, RulesClass::Instance->C4Warhead,
    /// nullptr, true, false, nullptr)` — the object's own Health as damage,
    /// ignoring defenses, with no attacker and no source house. The Ares
    /// `SpawnSurvivors` call ahead of it is an Ares-only step.
    fn kill_self(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> bool {
        let Some(entity) = self.substrate.entities.get(id) else {
            return false;
        };
        let health = entity.health.current();
        let Some(warhead) = rules.warhead(&rules.bridge_warheads.c4_name) else {
            return false;
        };
        let warhead_ref = self.interner.intern(&warhead.id);
        let event = crate::sim::combat::EntityDamageEvent::direct_receiver(
            id,
            health,
            0,
            crate::sim::combat::RAD_NO_ATTACKER,
            None,
            warhead_ref,
            crate::sim::combat::ReceiverCallFlags {
                ignore_defenses: true,
                arg6: false,
            },
        );
        self.commit_direct_damage_receiver(rules, registry, event, effects);
        true
    }
}

#[cfg(all(test, feature = "phobos"))]
#[path = "auto_death_tests.rs"]
mod tests;
