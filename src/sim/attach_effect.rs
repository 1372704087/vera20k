//! One attached AttachEffect instance: its remaining duration and the delays
//! that re-create or end it.
//!
//! Native identity: Phobos `AttachEffectClass`
//! (`src/New/Entity/AttachEffectClass.{h,cpp}`). `AI()` runs one phase per
//! frame in a fixed order: an initial delay, then a pending re-creation delay,
//! then a queued duration refresh, then the duration tick. `RefreshDuration`
//! takes the override, the type's `Duration` and the two duration multipliers.
//!
//! This module owns the timing state only. Stat multipliers, animations, tints
//! and discard conditions read it from their own slices.

use crate::rules::attach_effect::AttachEffectType;

/// `AttachEffectClass`'s constructor arguments, as the attach sites supply
/// them (`AEAttachParams`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct AttachEffectParams {
    /// `DurationOverrides`, paired with the attach list; `0` keeps the type's.
    pub duration_override: i32,
    /// `Delays`: re-creation delay after a natural end. Negative means the
    /// effect ends instead of re-creating.
    pub delay: i32,
    /// `InitialDelays`: frames before the effect becomes active.
    pub initial_delay: i32,
    /// `RecreationDelays`: frames after a full removal before re-attaching.
    pub recreation_delay: i32,
}

/// One live instance on a techno.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttachEffectInstance {
    /// Catalog index of the type, so the instance survives a rules reload
    /// whose types keep their order.
    pub type_index: usize,
    remaining: i32,
    duration_override: i32,
    delay: i32,
    initial_delay: i32,
    recreation_delay: i32,
    current_delay: i32,
    should_refresh: bool,
    has_initialized: bool,
}

impl AttachEffectInstance {
    /// `AttachEffectClass`'s constructor: the duration starts at the override
    /// when present, otherwise the type's, and nothing is active yet.
    pub fn new(type_index: usize, params: AttachEffectParams, effect: &AttachEffectType) -> Self {
        let mut instance = Self {
            type_index,
            remaining: 0,
            duration_override: params.duration_override,
            delay: params.delay,
            initial_delay: params.initial_delay,
            recreation_delay: params.recreation_delay,
            current_delay: 0,
            should_refresh: false,
            has_initialized: false,
        };
        instance.refresh(effect);
        instance
    }

    pub fn remaining(&self) -> i32 {
        self.remaining
    }

    pub fn current_delay(&self) -> i32 {
        self.current_delay
    }

    pub fn should_refresh(&self) -> bool {
        self.should_refresh
    }

    /// `HasExpired()`: a re-creating effect (`Delay >= 0`) never expires; a
    /// terminal one expires once its duration runs out.
    pub fn has_expired(&self) -> bool {
        self.delay < 0 && self.remaining == 0
    }

    /// `ResetIfRecreatable()`: a negative recreation delay refuses the reset.
    /// Otherwise the animation is dropped, the duration cleared and the
    /// re-creation delay armed.
    pub fn reset_if_recreatable(&mut self) -> bool {
        if self.recreation_delay < 0 {
            return false;
        }
        self.remaining = 0;
        self.current_delay = self.recreation_delay;
        self.should_refresh = true;
        true
    }

    /// `RefreshDuration`: the override wins over the type's `Duration`. The
    /// two duration multipliers are applied by their own owners (the firepower
    /// and armor slices), which call [`Self::refresh_scaled`] with the value
    /// they computed.
    fn refresh(&mut self, effect: &AttachEffectType) {
        self.remaining = if self.duration_override != 0 {
            self.duration_override
        } else {
            effect.duration
        };
    }

    /// `RefreshDuration` for a caller that already folded the duration
    /// multipliers into `duration`.
    pub fn refresh_with(&mut self, duration: i32) {
        self.remaining = duration.max(0);
    }

    /// One `AI()` frame. `discarded` is `ShouldBeDiscardedNow()`: a pending
    /// delay or refresh does not advance while the effect is being discarded.
    pub fn tick(&mut self, effect: &AttachEffectType, discarded: bool) {
        if self.initial_delay > 0 {
            self.initial_delay -= 1;
            return;
        }
        if !self.has_initialized {
            self.has_initialized = true;
        }
        if self.current_delay > 0 {
            if !discarded {
                self.current_delay -= 1;
                if self.current_delay == 0 {
                    self.should_refresh = true;
                }
            }
            return;
        }
        if self.should_refresh {
            if !discarded {
                self.refresh(effect);
                self.should_refresh = false;
            }
            return;
        }
        if self.remaining > 0 {
            self.remaining -= 1;
        }
        if self.remaining != 0 {
            return;
        }
        // `Duration` reached zero: a negative delay leaves the effect ended.
        if self.delay < 0 {
            return;
        }
        self.current_delay = self.delay;
        if self.delay > 0 || discarded {
            // A zero delay behind a discard check stays queued instead of
            // refilling this frame, as `AI()`'s shared branch does.
            if self.delay == 0 {
                self.should_refresh = true;
            }
        } else {
            self.refresh(effect);
        }
    }
}

#[cfg(all(test, feature = "phobos"))]
mod tests {
    use super::*;

    fn effect(duration: i32) -> AttachEffectType {
        let mut effect = AttachEffectType::default();
        effect.duration = duration;
        effect
    }

    /// The initial delay gates everything, then the duration counts down and a
    /// terminal (`Delay < 0`) effect expires at zero.
    #[test]
    fn initial_delay_gates_the_countdown_and_a_terminal_effect_expires() {
        let effect = effect(3);
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                initial_delay: 2,
                delay: -1,
                ..AttachEffectParams::default()
            },
            &effect,
        );
        assert_eq!(
            instance.remaining(),
            3,
            "constructor takes the type duration"
        );
        instance.tick(&effect, false);
        instance.tick(&effect, false);
        assert_eq!(instance.remaining(), 3, "initial delay holds the duration");
        for _ in 0..3 {
            instance.tick(&effect, false);
        }
        assert_eq!(instance.remaining(), 0);
        assert!(instance.has_expired(), "Delay < 0 ends the effect");
        instance.tick(&effect, false);
        assert_eq!(instance.remaining(), 0, "a terminal effect stays ended");
    }

    /// `Delay = 0` refills the duration in the same frame, and a re-creating
    /// effect never reports expired.
    #[test]
    fn a_zero_delay_recreates_in_the_same_frame() {
        let effect = effect(2);
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                delay: 0,
                ..AttachEffectParams::default()
            },
            &effect,
        );
        instance.tick(&effect, false);
        assert_eq!(instance.remaining(), 1);
        instance.tick(&effect, false);
        assert_eq!(instance.remaining(), 2, "zero delay refreshes on the spot");
        assert!(!instance.should_refresh(), "no deferred refresh was queued");
        assert!(
            !instance.has_expired(),
            "a re-creating effect never expires"
        );
    }

    /// Behind a discard check the same zero delay is deferred instead.
    #[test]
    fn a_zero_delay_under_discard_refreshes_on_a_later_frame() {
        let effect = effect(1);
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                delay: 0,
                ..AttachEffectParams::default()
            },
            &effect,
        );
        instance.tick(&effect, true);
        assert_eq!(instance.remaining(), 0);
        assert!(
            instance.should_refresh(),
            "queued, not refilled while discarded"
        );
        instance.tick(&effect, false);
        assert_eq!(instance.remaining(), 1);
    }

    /// A pending re-creation delay counts down and then refreshes; discarding
    /// freezes it, as `ShouldBeDiscardedNow` gates the native branch.
    #[test]
    fn a_recreation_delay_counts_down_and_discarding_freezes_it() {
        let effect = effect(1);
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                delay: 2,
                ..AttachEffectParams::default()
            },
            &effect,
        );
        instance.tick(&effect, false);
        assert_eq!(
            instance.current_delay(),
            2,
            "duration 1 -> 0 arms the delay"
        );
        instance.tick(&effect, true);
        assert_eq!(instance.current_delay(), 2, "discarded delays do not tick");
        instance.tick(&effect, false);
        assert_eq!(instance.current_delay(), 1);
        instance.tick(&effect, false);
        assert_eq!(instance.current_delay(), 0);
        assert!(instance.should_refresh());
    }

    /// The override wins over the type duration, and a negative recreation
    /// delay refuses `ResetIfRecreatable`.
    #[test]
    fn override_wins_and_a_negative_recreation_delay_refuses_the_reset() {
        let effect = effect(5);
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                duration_override: 7,
                recreation_delay: -1,
                ..AttachEffectParams::default()
            },
            &effect,
        );
        assert_eq!(instance.remaining(), 7);
        assert!(!instance.reset_if_recreatable());
        assert_eq!(instance.remaining(), 7, "a refused reset changes nothing");
    }

    /// A positive re-creation delay clears the duration and arms the delay.
    #[test]
    fn reset_arms_the_recreation_delay() {
        let effect = effect(4);
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                recreation_delay: 3,
                ..AttachEffectParams::default()
            },
            &effect,
        );
        assert!(instance.reset_if_recreatable());
        assert_eq!(instance.remaining(), 0);
        assert_eq!(instance.current_delay(), 3);
        assert!(instance.should_refresh(), "the queued refresh refills it");
    }
}
