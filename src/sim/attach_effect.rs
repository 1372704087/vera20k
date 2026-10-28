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
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    /// Who attached it: `IsFromSource` compares this pair.
    source: Option<AttachSource>,
    /// `IsOnline`: only a `Powered` type follows its owner.
    is_online: bool,
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
            source: None,
            is_online: true,
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

    pub fn is_online(&self) -> bool {
        self.is_online
    }

    /// `OnlineCheck`: a `Powered` type follows its owner's activity (not
    /// deactivated, not under EMP, and for a building power-online); every
    /// other type stays online.
    pub fn online_check(&mut self, effect: &AttachEffectType, owner_active: bool) {
        if effect.powered {
            self.is_online = owner_active;
        }
    }

    /// `IsActiveIgnorePowered()`: a re-creating or self-owned effect waits for
    /// its delays to clear; a terminal one is active while its duration runs.
    fn active_ignoring_power(&self, self_owned: bool) -> bool {
        if self_owned || self.delay >= 0 {
            self.initial_delay <= 0
                && self.current_delay == 0
                && self.has_initialized
                && !self.should_refresh
        } else {
            self.remaining > 0
        }
    }

    /// `IsActive()`.
    pub fn is_active(&self, self_owned: bool) -> bool {
        self.is_online && self.active_ignoring_power(self_owned)
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

/// Who handed the effect over: `IsFromSource` compares both the invoker and
/// the source object, so a warhead and the unit that fired it are distinct.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct AttachSource {
    pub invoker: Option<u64>,
    pub source: Option<u64>,
}

/// The attach site's cumulative switches (`AEAttachInfoTypeClass`), separate
/// from the type's own `Cumulative` fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttachPolicy {
    /// `AttachEffect.ReplaceLongerDuration`, defaulted from `[General]`.
    pub replace_longer_duration: bool,
    /// `AttachEffect.CumulativeSourceMaxCount`; negative means no cap.
    pub cumulative_source_max_count: i32,
    pub cumulative_refresh_all: bool,
    pub cumulative_refresh_all_on_attach: bool,
    pub cumulative_refresh_same_source_only: bool,
}

impl Default for AttachPolicy {
    /// `CumulativeRefreshSameSourceOnly` defaults to `true` in Phobos; the
    /// source cap is unbounded and the refresh switches are off.
    fn default() -> Self {
        Self {
            replace_longer_duration: false,
            cumulative_source_max_count: -1,
            cumulative_refresh_all: false,
            cumulative_refresh_all_on_attach: false,
            cumulative_refresh_same_source_only: true,
        }
    }
}

/// What one attach attempt did, mirroring `CreateAndAttach` returning a new
/// instance or refreshing an existing one and returning null.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachOutcome {
    /// Appended at this index.
    Attached(usize),
    /// An existing instance absorbed the attach; nothing was added.
    Refreshed,
}

/// One techno's live effects, in attach order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AttachEffectList {
    instances: Vec<AttachEffectInstance>,
}

impl AttachEffectList {
    pub fn instances(&self) -> &[AttachEffectInstance] {
        &self.instances
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// `CreateAndAttach`'s cumulative decision: a non-cumulative type refreshes
    /// the one existing instance, and a capped cumulative type refreshes the
    /// shortest match (or every match under `CumulativeRefreshAll`) instead of
    /// appending.
    pub fn attach(
        &mut self,
        type_index: usize,
        params: AttachEffectParams,
        effect: &AttachEffectType,
        policy: AttachPolicy,
        source: Option<AttachSource>,
    ) -> AttachOutcome {
        let duration = if params.duration_override != 0 {
            params.duration_override
        } else {
            effect.duration
        };
        let mut current_type_count = 0i32;
        let mut current_source_count = 0i32;
        let mut shortest: Option<usize> = None;
        let mut matches: Vec<usize> = Vec::new();

        for index in 0..self.instances.len() {
            let instance = &self.instances[index];
            if instance.type_index != type_index {
                continue;
            }
            current_type_count += 1;
            if !effect.cumulative {
                if policy.replace_longer_duration || duration >= instance.remaining {
                    self.instances[index].refresh_with(duration);
                }
                return AttachOutcome::Refreshed;
            }
            let same_source = instance.source == source;
            if same_source {
                current_source_count += 1;
            }
            if !policy.cumulative_refresh_same_source_only || same_source {
                matches.push(index);
                let shorter =
                    shortest.is_none_or(|best| instance.remaining < self.instances[best].remaining);
                if shorter {
                    shortest = Some(index);
                }
            }
        }

        if effect.cumulative {
            let type_capped = effect.cumulative_max_count >= 0
                && current_type_count >= effect.cumulative_max_count;
            let source_capped = policy.cumulative_source_max_count >= 0
                && current_source_count >= policy.cumulative_source_max_count;
            if type_capped || source_capped {
                if policy.cumulative_refresh_all {
                    for index in matches {
                        self.instances[index].refresh_with(duration);
                    }
                } else if let Some(index) = shortest
                    && (policy.replace_longer_duration
                        || duration >= self.instances[index].remaining)
                {
                    self.instances[index].refresh_with(duration);
                }
                return AttachOutcome::Refreshed;
            } else if policy.cumulative_refresh_all && policy.cumulative_refresh_all_on_attach {
                // `CumulativeRefreshAll.OnAttach` refreshes under the cap and
                // still appends the new instance.
                for index in matches {
                    self.instances[index].refresh_with(duration);
                }
            }
        }

        let mut instance = AttachEffectInstance::new(type_index, params, effect);
        instance.source = source;
        self.instances.push(instance);
        AttachOutcome::Attached(self.instances.len() - 1)
    }

    /// `OnlineCheck` against one owner state, for every instance.
    pub fn online_check_all(&mut self, effects: &[AttachEffectType], owner_active: bool) {
        for index in 0..self.instances.len() {
            let type_index = self.instances[index].type_index;
            let Some(effect) = effects.get(type_index) else {
                continue;
            };
            self.instances[index].online_check(effect, owner_active);
        }
    }

    /// One frame for every instance, in attach order.
    pub fn tick_all(
        &mut self,
        effects: &[AttachEffectType],
        mut discarded: impl FnMut(&AttachEffectInstance) -> bool,
    ) {
        for index in 0..self.instances.len() {
            let type_index = self.instances[index].type_index;
            let Some(effect) = effects.get(type_index) else {
                continue;
            };
            let discarded = discarded(&self.instances[index]);
            self.instances[index].tick(effect, discarded);
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

    fn cumulative_effect(duration: i32, max_count: i32) -> AttachEffectType {
        let mut effect = effect(duration);
        effect.cumulative = true;
        effect.cumulative_max_count = max_count;
        effect
    }

    fn attach_with(
        list: &mut AttachEffectList,
        effect: &AttachEffectType,
        override_duration: i32,
    ) -> AttachOutcome {
        list.attach(
            0,
            AttachEffectParams {
                duration_override: override_duration,
                delay: -1,
                ..AttachEffectParams::default()
            },
            effect,
            AttachPolicy::default(),
            None,
        )
    }

    /// A re-creating effect is inactive until its initial delay clears, and a
    /// `Powered` type follows its owner's activity; a plain type does not.
    #[test]
    fn active_state_follows_delays_and_the_powered_owner() {
        let plain = effect(3);
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                initial_delay: 1,
                delay: 0,
                ..AttachEffectParams::default()
            },
            &plain,
        );
        instance.tick(&plain, false);
        assert!(
            !instance.is_active(false),
            "the delay frame does not initialize"
        );
        instance.tick(&plain, false);
        assert!(instance.is_active(false), "initialized, no pending delay");
        instance.online_check(&plain, false);
        assert!(instance.is_online(), "a plain type ignores its owner");

        let mut powered = effect(3);
        powered.powered = true;
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                delay: 0,
                ..AttachEffectParams::default()
            },
            &powered,
        );
        instance.tick(&powered, false);
        instance.online_check(&powered, false);
        assert!(
            !instance.is_active(false),
            "a powered type follows its owner"
        );
        instance.online_check(&powered, true);
        assert!(instance.is_active(false));
    }

    /// A terminal effect is active only while its duration runs; a re-creating
    /// one is inactive while its re-creation delay is pending.
    #[test]
    fn a_terminal_effect_follows_its_duration_and_a_recreating_one_its_delay() {
        let terminal = effect(1);
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                delay: -1,
                ..AttachEffectParams::default()
            },
            &terminal,
        );
        assert!(
            instance.is_active(false),
            "a terminal effect reads its duration"
        );
        instance.tick(&terminal, false);
        assert!(!instance.is_active(false), "duration ran out");

        let cycling = effect(1);
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                delay: 2,
                ..AttachEffectParams::default()
            },
            &cycling,
        );
        instance.tick(&cycling, false);
        assert_eq!(instance.current_delay(), 2);
        assert!(
            !instance.is_active(false),
            "the pending re-creation delay keeps it inactive"
        );
        instance.tick(&cycling, false);
        instance.tick(&cycling, false);
        assert!(instance.should_refresh());
        instance.tick(&cycling, false);
        assert!(instance.is_active(false), "refreshed and counting again");
    }

    /// The list round-trips through serde with its instance state intact.
    #[test]
    fn the_list_round_trips_through_serde() {
        let effect = cumulative_effect(4, 2);
        let mut list = AttachEffectList::default();
        attach_with(&mut list, &effect, 0);
        list.tick_all(std::slice::from_ref(&effect), |_| false);
        let json = serde_json::to_string(&list).expect("serialize");
        let back: AttachEffectList = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, list);
    }

    /// A non-cumulative type never stacks: the existing instance takes the
    /// refresh, but only when the new duration wins or the site replaces
    /// longer ones.
    #[test]
    fn reapplying_a_non_cumulative_type_refreshes_by_the_longer_rule() {
        let effect = effect(5);
        let mut list = AttachEffectList::default();
        assert_eq!(
            attach_with(&mut list, &effect, 0),
            AttachOutcome::Attached(0)
        );
        list.tick_all(std::slice::from_ref(&effect), |_| false);
        list.tick_all(std::slice::from_ref(&effect), |_| false);
        assert_eq!(list.instances()[0].remaining(), 3);

        assert_eq!(attach_with(&mut list, &effect, 2), AttachOutcome::Refreshed);
        assert_eq!(list.instances().len(), 1, "no second instance");
        assert_eq!(
            list.instances()[0].remaining(),
            3,
            "a shorter reapply is ignored"
        );

        let mut list = AttachEffectList::default();
        attach_with(&mut list, &effect, 0);
        let replace = AttachPolicy {
            replace_longer_duration: true,
            ..AttachPolicy::default()
        };
        let outcome = list.attach(
            0,
            AttachEffectParams {
                duration_override: 2,
                delay: -1,
                ..AttachEffectParams::default()
            },
            &effect,
            replace,
            None,
        );
        assert_eq!(outcome, AttachOutcome::Refreshed);
        assert_eq!(
            list.instances()[0].remaining(),
            2,
            "replaced even though shorter"
        );
    }

    /// Below `Cumulative.MaxCount` the list stacks; at the cap it refreshes
    /// the shortest match instead of appending.
    #[test]
    fn cumulative_stacks_below_the_cap_and_refreshes_the_shortest_at_it() {
        let effect = cumulative_effect(6, 2);
        let mut list = AttachEffectList::default();
        attach_with(&mut list, &effect, 6);
        attach_with(&mut list, &effect, 2);
        assert_eq!(list.instances().len(), 2, "below the cap both attach");

        assert_eq!(attach_with(&mut list, &effect, 3), AttachOutcome::Refreshed);
        assert_eq!(list.instances().len(), 2, "the cap refuses a third");
        assert_eq!(
            list.instances()[0].remaining(),
            6,
            "the longer match is untouched"
        );
        assert_eq!(
            list.instances()[1].remaining(),
            3,
            "the shortest match refreshed"
        );
    }

    /// `CumulativeRefreshAll` refreshes every match at the cap.
    #[test]
    fn refresh_all_at_the_cap_refreshes_every_match() {
        let effect = cumulative_effect(6, 2);
        let mut list = AttachEffectList::default();
        attach_with(&mut list, &effect, 6);
        attach_with(&mut list, &effect, 2);
        let policy = AttachPolicy {
            cumulative_refresh_all: true,
            ..AttachPolicy::default()
        };
        let outcome = list.attach(
            0,
            AttachEffectParams {
                duration_override: 3,
                delay: -1,
                ..AttachEffectParams::default()
            },
            &effect,
            policy,
            None,
        );
        assert_eq!(outcome, AttachOutcome::Refreshed);
        assert_eq!(list.instances()[0].remaining(), 3);
        assert_eq!(list.instances()[1].remaining(), 3);
    }

    /// `CumulativeRefreshAll.OnAttach` refreshes under the cap and still adds
    /// the new instance.
    #[test]
    fn refresh_all_on_attach_below_the_cap_still_appends() {
        let effect = cumulative_effect(5, 3);
        let mut list = AttachEffectList::default();
        attach_with(&mut list, &effect, 5);
        let policy = AttachPolicy {
            cumulative_refresh_all: true,
            cumulative_refresh_all_on_attach: true,
            ..AttachPolicy::default()
        };
        let outcome = list.attach(
            0,
            AttachEffectParams {
                duration_override: 2,
                delay: -1,
                ..AttachEffectParams::default()
            },
            &effect,
            policy,
            None,
        );
        assert_eq!(outcome, AttachOutcome::Attached(1));
        assert_eq!(list.instances()[0].remaining(), 2, "the match refreshed");
        assert_eq!(
            list.instances()[1].remaining(),
            2,
            "the new instance starts at its own"
        );
    }

    /// `CumulativeRefreshSameSourceOnly` keeps another source's instance out of
    /// the match set, so the cap is judged separately per source.
    #[test]
    fn same_source_only_skips_other_sources() {
        let effect = cumulative_effect(4, 2);
        let mut list = AttachEffectList::default();
        let first = Some(AttachSource {
            invoker: Some(1),
            source: Some(10),
        });
        let second = Some(AttachSource {
            invoker: Some(2),
            source: Some(20),
        });
        let policy = AttachPolicy::default();
        list.attach(
            0,
            AttachEffectParams {
                delay: -1,
                ..AttachEffectParams::default()
            },
            &effect,
            policy,
            first,
        );
        list.attach(
            0,
            AttachEffectParams {
                delay: -1,
                ..AttachEffectParams::default()
            },
            &effect,
            policy,
            second,
        );
        assert_eq!(
            list.instances().len(),
            2,
            "a different source still attaches"
        );
        assert_eq!(
            list.instances()[1].source,
            second,
            "the instance keeps who attached it"
        );
    }

    /// A source cap refuses the new instance even when the type cap allows it.
    #[test]
    fn the_source_cap_refreshes_instead_of_appending() {
        let effect = cumulative_effect(4, -1);
        let mut list = AttachEffectList::default();
        let source = Some(AttachSource {
            invoker: Some(1),
            source: Some(10),
        });
        let policy = AttachPolicy {
            cumulative_source_max_count: 1,
            ..AttachPolicy::default()
        };
        list.attach(
            0,
            AttachEffectParams {
                delay: -1,
                ..AttachEffectParams::default()
            },
            &effect,
            policy,
            source,
        );
        let outcome = list.attach(
            0,
            AttachEffectParams {
                delay: -1,
                ..AttachEffectParams::default()
            },
            &effect,
            policy,
            source,
        );
        assert_eq!(outcome, AttachOutcome::Refreshed);
        assert_eq!(list.instances().len(), 1, "the source cap blocks the copy");
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
