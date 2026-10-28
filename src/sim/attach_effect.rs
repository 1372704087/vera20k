//! One attached AttachEffect instance: its remaining duration and the delays
//! that re-create or end it.
//!
//! Native identity: Phobos `AttachEffectClass`
//! (`src/New/Entity/AttachEffectClass.{h,cpp}`). `AI()` runs one phase per
//! frame in a fixed order: an initial delay, then a pending re-creation delay,
//! then a queued duration refresh, then the duration tick. `RefreshDuration`
//! takes the override, the type's `Duration` and the two duration multipliers.
//!
//! This module owns the timing state and the per-frame AI walk, including the
//! animation each effect shows. Stat multipliers, tint rendering and discard
//! conditions read this state from their own owners.

use crate::rules::attach_effect::{AttachEffectType, AttachedAnimFlag};
#[cfg(feature = "phobos")]
use crate::sim::world::Simulation;

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
    /// `Animation`: the live `AnimClass` showing this effect, owned by the
    /// same tree as the entity so a save restores the pair.
    #[cfg(feature = "phobos")]
    #[serde(default)]
    anim: Option<u64>,
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
            #[cfg(feature = "phobos")]
            anim: None,
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

    /// `HasInitialized`: the AI reached its first active frame.
    pub fn has_initialized(&self) -> bool {
        self.has_initialized
    }

    /// `IsSelfOwned`: the attach site said the effect belongs to the type
    /// itself (only the type/country list entry does so today); an effect
    /// attached from outside carries its invoker/source pair instead.
    pub fn is_self_owned(&self) -> bool {
        self.source.is_none()
    }

    /// The frame's `AI()` walk reaches the animation check: not in the initial
    /// delay, the re-creation delay or the queued refresh (those return early),
    /// and the effect still has a duration to show.
    pub fn animation_phase(&self) -> bool {
        self.initial_delay <= 0
            && self.has_initialized
            && self.current_delay == 0
            && !self.should_refresh
            && self.remaining != 0
    }

    /// The live animation this effect shows, if one is attached.
    #[cfg(feature = "phobos")]
    pub fn anim(&self) -> Option<u64> {
        self.anim
    }

    #[cfg(feature = "phobos")]
    pub(crate) fn set_anim(&mut self, anim: Option<u64>) {
        self.anim = anim;
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

    /// `CanShowAnim` with the two flag actions: an offline owner and a temporal
    /// freeze hide, pause or freeze the animation.
    pub fn animation_state(
        &self,
        effect: &AttachEffectType,
        under_temporal: bool,
    ) -> AttachAnimState {
        let mut state = AttachAnimState {
            show: true,
            paused: false,
            temporal: false,
        };
        let mut apply = |action: AttachedAnimFlag| match action {
            AttachedAnimFlag::None => {}
            AttachedAnimFlag::Hides => state.show = false,
            AttachedAnimFlag::Temporal => state.temporal = true,
            AttachedAnimFlag::Paused => state.paused = true,
            AttachedAnimFlag::PausedTemporal => {
                state.paused = true;
                state.temporal = true;
            }
        };
        if effect.powered && !self.is_online {
            apply(effect.animation_offline_action);
        }
        if under_temporal {
            apply(effect.animation_temporal_action);
        }
        state
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

/// What the effect's animation should do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttachAnimState {
    pub show: bool,
    pub paused: bool,
    pub temporal: bool,
}

impl Default for AttachAnimState {
    fn default() -> Self {
        Self {
            show: true,
            paused: false,
            temporal: false,
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

/// One value of a paired list: the index's own entry, else the last, else the
/// key's default. Phobos' `ValueableVector` reads extend the same way.
#[cfg(feature = "phobos")]
fn paired(list: &[i32], index: usize, default: i32) -> i32 {
    list.get(index)
        .or_else(|| list.last())
        .copied()
        .unwrap_or(default)
}

/// Phobos attaches a techno type's `AttachEffect.AttachTypes` once when the
/// entity is created, pairing `DurationOverrides`, `Delays`, `InitialDelays`
/// and `RecreationDelays` by index. `source` is the invoking pair when the
/// attachment came from outside (a warhead or weapon); a type's own list has
/// none.
#[cfg(feature = "phobos")]
pub fn attach_initial_effects(
    list: &mut AttachEffectList,
    object: &crate::rules::object_type::ObjectType,
    rules: &crate::rules::ruleset::RuleSet,
    source: Option<AttachSource>,
) -> usize {
    let names = object.attach_effect_attach_types();
    if names.is_empty() {
        return 0;
    }
    let overrides = object.attach_effect_duration_overrides();
    let delays = object.attach_effect_delays();
    let initial_delays = object.attach_effect_initial_delays();
    let recreation_delays = object.attach_effect_recreation_delays();
    let policy = AttachPolicy {
        replace_longer_duration: rules.attach_effect_globals().replace_longer_duration,
        ..AttachPolicy::default()
    };
    let catalog = rules.attach_effect_types();
    let mut attached = 0;
    for (index, name) in names.iter().enumerate() {
        let Some(type_index) = catalog.index_of(name) else {
            continue;
        };
        let effect = &catalog.types()[type_index];
        let params = AttachEffectParams {
            duration_override: paired(overrides, index, 0),
            delay: paired(delays, index, 0),
            initial_delay: paired(initial_delays, index, 0),
            recreation_delay: paired(recreation_delays, index, -1),
        };
        if let AttachOutcome::Attached(_) = list.attach(type_index, params, effect, policy, source)
        {
            attached += 1;
        }
    }
    attached
}

/// C++ `static_cast<int>(value * multiplier)` as MSVC compiles it: truncation
/// toward zero, and `i32::MIN` for a NaN or out-of-range product. Phobos
/// performs its integer stat hooks in C++ doubles, so each one crosses this
/// boundary; gamemd's own x87 steps stay with `native_x87`.
pub fn scale_i32(value: i32, multiplier: f64) -> i32 {
    cpp_i32(f64::from(value) * multiplier)
}

/// C++ `static_cast<int>(value / divisor)` (MSVC `cvttsd2si`): the armor
/// divide Phobos' receive hook performs on its own.
pub fn divide_i32(value: i32, divisor: f64) -> i32 {
    cpp_i32(f64::from(value) / divisor)
}

fn cpp_i32(value: f64) -> i32 {
    if value.is_nan() || value < f64::from(i32::MIN) || value >= 2_147_483_648.0 {
        i32::MIN
    } else {
        value.trunc() as i32
    }
}

/// Phobos `AttachEffectTechnoProperties`'s cached stat fold
/// (`TechnoExt::RecalculateStatMultipliers`, Body.Update.cpp:1124). The
/// instance list stays the source of truth; this is recomputed from it every
/// AI frame, restored from the save like Phobos' own `TechnoExt::AE`, and
/// never enters the state hash.
///
/// RESIDUALS: `has_restricted_armor` marks a per-hit `CalculateArmorMultipliers`
/// roll (Chance/AffectsHouse/warhead lists/HitAnim) that still needs the
/// Scenario RNG and animation owner threaded into the receiver; the
/// `cloakable`/`force_decloak` consumers need the native `IsReadyToCloak` /
/// `ShouldNotBeCloaked` bodies, `disable_weapons` needs `GetFireError`
/// (`0x006FC0B0`, not ported), `unkillable` needs the Phobos `CanKill` wrapper,
/// `has_range_modifier` needs an entity-aware `GetWeaponRange` fold, and
/// `has_crit_modifiers` needs the Phobos warhead critical-hit mechanism.
#[cfg(feature = "phobos")]
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PhobosStatMultipliers {
    pub firepower: f64,
    pub armor: f64,
    pub speed: f64,
    pub rof: f64,
    pub cloakable: bool,
    pub force_decloak: bool,
    pub disable_weapons: bool,
    pub unkillable: bool,
    pub has_restricted_armor: bool,
    pub has_range_modifier: bool,
    pub has_crit_modifiers: bool,
}

#[cfg(feature = "phobos")]
impl Default for PhobosStatMultipliers {
    fn default() -> Self {
        Self {
            firepower: 1.0,
            armor: 1.0,
            speed: 1.0,
            rof: 1.0,
            cloakable: false,
            force_decloak: false,
            disable_weapons: false,
            unkillable: false,
            has_restricted_armor: false,
            has_range_modifier: false,
            has_crit_modifiers: false,
        }
    }
}

#[cfg(feature = "phobos")]
impl AttachEffectList {
    /// The active effects with their catalog entries, in attach order.
    pub(crate) fn active_effects<'a>(
        &'a self,
        effects: &'a [AttachEffectType],
    ) -> impl Iterator<Item = (&'a AttachEffectInstance, &'a AttachEffectType)> {
        self.instances
            .iter()
            .filter(|instance| instance.is_active(instance.is_self_owned()))
            .filter_map(|instance| {
                effects
                    .get(instance.type_index)
                    .map(|effect| (instance, effect))
            })
    }

    /// `RecalculateStatMultipliers`'s full pass over the active effects: the
    /// four products, the four flags and the three derived "has" flags.
    pub(crate) fn stat_multipliers(&self, effects: &[AttachEffectType]) -> PhobosStatMultipliers {
        let mut stats = PhobosStatMultipliers::default();
        for (_, effect) in self.active_effects(effects) {
            stats.firepower *= effect.firepower_multiplier;
            stats.speed *= effect.speed_multiplier;
            stats.rof *= effect.rof_multiplier;
            if effect.restricted_armor_multiplier() {
                stats.has_restricted_armor = true;
            } else {
                stats.armor *= effect.armor_multiplier;
            }
            stats.cloakable |= effect.cloakable;
            stats.force_decloak |= effect.force_decloak;
            stats.disable_weapons |= effect.disable_weapons;
            stats.unkillable |= effect.unkillable;
            stats.has_range_modifier |=
                effect.weapon_range_multiplier != 1.0 || effect.weapon_range_extra_range != 0.0;
            stats.has_crit_modifiers |= effect.has_crit_modifiers();
        }
        stats
    }

    /// `TechnoExt::ExtData::HasAttachedEffects` with `requireAll=false`: any
    /// list entry with an active instance matches. `require_anims` adds the
    /// effect's own guard: a type that declares an animation only matches
    /// while that instance holds one.
    pub fn has_active_type(
        &self,
        names: &[String],
        effects: &[AttachEffectType],
        require_anims: bool,
    ) -> bool {
        self.instances.iter().any(|instance| {
            let Some(effect) = effects.get(instance.type_index) else {
                return false;
            };
            instance.is_active(instance.is_self_owned())
                && (!require_anims || !effect.has_animation() || instance.anim.is_some())
                && names
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(&effect.name))
        })
    }

    /// `UpdateConditionalAnimDrawingLogic`'s draw offset for the animation of
    /// `instance`: every rule whose `RequiredTypes` is empty or matches at
    /// least one attached effect adds its offset, so the rules accumulate
    /// rather than the last one winning.
    pub fn draw_offset(&self, instance: usize, effects: &[AttachEffectType]) -> [i32; 2] {
        let Some(instance) = self.instances.get(instance) else {
            return [0, 0];
        };
        let Some(effect) = effects.get(instance.type_index) else {
            return [0, 0];
        };
        let mut offset = [0, 0];
        for rule in &effect.animation_draw_offsets {
            if rule.required_types.is_empty()
                || self.has_active_type(&rule.required_types, effects, true)
            {
                offset[0] += rule.offset[0];
                offset[1] += rule.offset[1];
            }
        }
        offset
    }
}

/// Where an effect's animation lives: attached to `owner` at its world point,
/// drawn under `house`, and paused or running with the effect's state.
#[cfg(feature = "phobos")]
struct AttachEffectAnimTarget {
    owner: u64,
    house: crate::sim::intern::InternedId,
    world: crate::sim::anim_class::AnimWorldCoord,
    paused: bool,
}

#[cfg(feature = "phobos")]
impl Simulation {
    /// Phobos `TechnoExt::UpdateAttachEffects`
    /// (`src/Ext/Techno/Body.Update.cpp:895..975`, reached from the
    /// `TechnoClass_AI_0x6F9E50` early update): every effect runs one `AI()`
    /// phase, expired effects are removed, and each surviving effect
    /// reconciles its `AnimClass` with `CanShowAnim`.
    ///
    /// RESIDUALS: `DiscardOn` belongs to its own slice, so nothing is discarded
    /// here yet; native `IsImmobilized` is approximated by the chrono-warp and
    /// transport-interior gates VERA models; `CloakCheck` and the tunnel
    /// animation state are not ported; a cumulative type's animation is
    /// rebuilt when the layer count changes instead of going through
    /// `AnimExt::ChangeAnimType`, so `CumulativeAnimations.RestartOnChange=no`
    /// still restarts; `Animation.ResetOnReapply` does not restart a refreshed
    /// effect's animation.
    pub(crate) fn update_phobos_attach_effects(
        &mut self,
        id: u64,
        rules: &crate::rules::ruleset::RuleSet,
    ) {
        let Some(entity) = self.substrate.entities.get(id) else {
            return;
        };
        // `InLimbo || IsImmobilized`: an object that cannot act keeps its
        // effects untouched. Chrono warp and transport interior stand in for
        // the native immobilised states VERA models.
        if entity.lifecycle.in_limbo()
            || entity.chrono_warp_latch()
            || entity.passenger_role.inside_transport_id().is_some()
            || entity.phobos_attach_effects().is_empty()
        {
            return;
        }
        // `OnlineCheck`: Deactivated, EMP, and a building's power-online latch.
        let owner_active = !entity.deactivated.is_offline()
            && entity.emp_remaining() <= 0
            && (entity.category() != crate::map::entities::EntityCategory::Structure
                || entity.building_online());
        let owner_house = entity.owner();
        let position = crate::sim::movement::ground_pose::position_world_coord(&entity.position);
        let world = crate::sim::anim_class::AnimWorldCoord {
            x: position.x,
            y: position.y,
            z: position.z,
        };
        let mut list = std::mem::take(
            self.substrate
                .entities
                .get_mut(id)
                .expect("the gated entity still exists")
                .phobos_attach_effects_mut(),
        );
        let effects = rules.attach_effect_types().types();
        list.online_check_all(effects, owner_active);
        let initialized_before: Vec<bool> = list
            .instances
            .iter()
            .map(|instance| instance.has_initialized())
            .collect();
        list.tick_all(effects, |_| false);
        // `AI()`'s initialization block: `ROFMultiplier.ApplyOnCurrentTimer`
        // scales the timer already running when the effect first activates.
        let frame = self.session.binary_frame() as i32;
        for (index, instance) in list.instances.iter().enumerate() {
            if initialized_before.get(index).copied().unwrap_or(true) || !instance.has_initialized()
            {
                continue;
            }
            let Some(effect) = effects.get(instance.type_index) else {
                continue;
            };
            if effect.rof_multiplier <= 0.0 || !effect.rof_multiplier_apply_on_current_timer {
                continue;
            }
            let Some(entity) = self.substrate.entities.get_mut(id) else {
                continue;
            };
            let remaining = entity.rearm_timer.remaining(frame);
            if remaining > 0 {
                entity
                    .rearm_timer
                    .start(frame, scale_i32(remaining, effect.rof_multiplier));
            }
        }
        // `HasExpired` removal runs after the AI walk; the effect's destructor
        // unlinks its animation.
        let mut index = 0;
        while index < list.instances.len() {
            if list.instances[index].has_expired() {
                if let Some(anim) = list.instances[index].anim.take() {
                    self.destroy_anim_with_context(anim, rules);
                }
                list.instances.remove(index);
            } else {
                index += 1;
            }
        }
        // `UpdateConditionalAnimDrawingLogic` reacts to other effects on the
        // same object, so every effect re-evaluates it each frame.
        let mut state = Vec::with_capacity(list.instances.len());
        let mut can_show = Vec::with_capacity(list.instances.len());
        for instance in &list.instances {
            let Some(effect) = effects.get(instance.type_index) else {
                state.push(AttachAnimState::default());
                can_show.push(false);
                continue;
            };
            // Temporal state is not tracked yet, so only the power gate applies.
            let anim_state = instance.animation_state(effect, false);
            let hidden =
                list.has_active_type(&effect.animation_hide_if_attached_with, effects, false);
            can_show.push(instance.animation_phase() && anim_state.show && !hidden);
            state.push(anim_state);
        }
        // One cumulative type shows one animation no matter how many layers it
        // has: `UpdateCumulativeAttachEffects` keeps the instance with the
        // longest remaining duration (any negative duration wins, last seen).
        let mut designated = vec![None; effects.len()];
        for (type_index, effect) in effects.iter().enumerate() {
            if !(effect.cumulative && !effect.cumulative_animations.is_empty()) {
                continue;
            }
            let mut best = None;
            let mut duration = 0;
            for (instance_index, instance) in list.instances.iter().enumerate() {
                if instance.type_index != type_index || !can_show[instance_index] {
                    continue;
                }
                let current = instance.remaining();
                if current < 0 || current > duration {
                    best = Some(instance_index);
                    duration = current;
                }
            }
            designated[type_index] = best;
        }
        for instance_index in 0..list.instances.len() {
            let type_index = list.instances[instance_index].type_index;
            let Some(effect) = effects.get(type_index) else {
                continue;
            };
            let desired = if can_show[instance_index] {
                if effect.cumulative && !effect.cumulative_animations.is_empty() {
                    let count = list
                        .instances
                        .iter()
                        .filter(|other| other.type_index == type_index)
                        .filter(|other| other.is_active(other.is_self_owned()))
                        .count() as i32;
                    if designated[type_index] == Some(instance_index) {
                        effect.cumulative_animation(count)
                    } else {
                        None
                    }
                } else {
                    effect.animation.as_deref()
                }
            } else {
                None
            };
            let house = if effect.animation_use_invoker_as_owner {
                list.instances[instance_index]
                    .source
                    .and_then(|source| source.invoker)
                    .and_then(|invoker| self.substrate.entities.get(invoker).map(|e| e.owner()))
                    .unwrap_or(owner_house)
            } else {
                owner_house
            };
            self.maintain_phobos_attach_effect_anim(
                &mut list.instances[instance_index],
                desired,
                AttachEffectAnimTarget {
                    owner: id,
                    house,
                    world,
                    paused: state[instance_index].paused,
                },
                rules,
            );
        }
        let stats = list.stat_multipliers(effects);
        let entity = self
            .substrate
            .entities
            .get_mut(id)
            .expect("the gated entity still exists");
        *entity.phobos_attach_effects_mut() = list;
        entity.set_phobos_stat_multipliers(stats);
    }

    /// `CreateAnim` / `KillAnim`: keep exactly `desired` alive on the effect.
    /// Phobos creates the animation at the owner's location, attaches it with
    /// `SetOwnerObject`, and holds its loops; a changed cumulative type is
    /// rebuilt here because `AnimExt::ChangeAnimType` has no VERA owner yet.
    fn maintain_phobos_attach_effect_anim(
        &mut self,
        instance: &mut AttachEffectInstance,
        desired: Option<&str>,
        target: AttachEffectAnimTarget,
        rules: &crate::rules::ruleset::RuleSet,
    ) {
        let AttachEffectAnimTarget {
            owner,
            house,
            world,
            paused,
        } = target;
        if let Some(anim) = instance.anim {
            let keeps = desired.is_some_and(|name| {
                self.substrate.anims.get(anim).is_some_and(|live| {
                    self.interner
                        .resolve(live.type_id())
                        .eq_ignore_ascii_case(name)
                })
            });
            if !keeps {
                self.destroy_anim_with_context(anim, rules);
                instance.anim = None;
            }
        }
        if instance.anim.is_none()
            && let Some(name) = desired
        {
            let (rx, ry, sub_x, sub_y, z) = world.to_cell_sub_z();
            let type_name = self.interner.intern(name);
            let descriptor = crate::sim::components::AnimClassSpawnDescriptor {
                draw_flags: 0x600,
                owner: Some(owner),
                ..crate::sim::components::AnimClassSpawnDescriptor::new(
                    type_name, rx, ry, sub_x, sub_y, z,
                )
            };
            match self.spawn_anim_at_world(rules, descriptor, world) {
                Ok(anim) => {
                    self.substrate
                        .anims
                        .get_mut(anim)
                        .expect("the new effect animation")
                        .hold_loops();
                    self.substrate.set_anim_owner_house(anim, house);
                    instance.anim = Some(anim);
                }
                Err(error) => log::debug!(
                    "attach effect animation [{name}] on {owner} did not construct: {error}"
                ),
            }
        }
        if let Some(anim) = instance.anim
            && let Some(live) = self.substrate.anims.get_mut(anim)
        {
            if paused {
                live.pause();
            } else {
                live.resume();
            }
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

    /// The first AI frame creates the effect's animation through the animation
    /// owner, attached to the techno with its loops held; the frame that ends
    /// the duration removes the effect and retires the animation.
    #[test]
    fn the_effect_animation_is_created_attached_and_retired() {
        use crate::rules::art_data::ArtRegistry;
        use crate::rules::ini_parser::IniFile;
        let ini = crate::sim::arena_fixture::clear_ground_ini(
            "[Countries]\n0=Americans\n[VehicleTypes]\n0=TARGET\n\
             [TARGET]\nStrength=1000\nSpeed=6\nAttachEffect.AttachTypes=PULSEAE\n\
             AttachEffect.Delays=-1\n\
             [AttachEffectTypes]\n0=PULSEAE\n\
             [PULSEAE]\nDuration=3\nAnimation=PULSE\n",
        );
        let art = IniFile::from_str("[PULSE]\nRate=900\nEnd=4\n");
        let processed = crate::rules::native_processing::RulesLayerStack::new(ini)
            .process_with_fixed_art(&art)
            .expect("rules process");
        let mut rules =
            crate::rules::ruleset::RuleSet::from_processed_rules(&processed).expect("rules");
        let mut registry = ArtRegistry::from_ini(&art);
        registry.bind_anim_frame_count_for_test("PULSE", 4);
        rules.install_art_data(registry);
        let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 17);
        sim.intern_rule_type_ids(&rules);
        sim.resolve_type_handles(&rules);
        let target = sim
            .spawn_object("TARGET", "Americans", 20, 20, 0, &rules)
            .expect("spawn");

        let instances = sim
            .entities()
            .get(target)
            .unwrap()
            .phobos_attach_effects()
            .instances();
        assert_eq!(instances.len(), 1);
        assert!(
            instances[0].anim().is_none(),
            "the constructor only attaches; AI creates the animation"
        );

        sim.update_phobos_attach_effects(target, &rules);
        let anim = sim
            .entities()
            .get(target)
            .unwrap()
            .phobos_attach_effects()
            .instances()[0]
            .anim()
            .expect("the first AI frame creates the animation");
        assert_eq!(
            sim.anim(anim).unwrap().owner_entity(),
            Some(target),
            "SetOwnerObject binds the animation to its techno"
        );
        assert_eq!(
            sim.anim(anim).unwrap().runtime().loop_remaining,
            u8::MAX,
            "the effect holds its animation loops"
        );
        assert!(
            sim.interner
                .resolve(sim.anim(anim).unwrap().type_id())
                .eq_ignore_ascii_case("PULSE")
        );
        let position = crate::sim::movement::ground_pose::position_world_coord(
            &sim.entities().get(target).unwrap().position,
        );
        assert_eq!(
            sim.anim_absolute_coord(anim),
            Some(crate::sim::anim_class::AnimWorldCoord {
                x: position.x,
                y: position.y,
                z: position.z,
            }),
            "an attached animation follows its owner's coordinate"
        );

        sim.update_phobos_attach_effects(target, &rules);
        assert_eq!(
            sim.entities()
                .get(target)
                .unwrap()
                .phobos_attach_effects()
                .instances()[0]
                .remaining(),
            1
        );
        sim.update_phobos_attach_effects(target, &rules);
        assert!(
            sim.entities()
                .get(target)
                .unwrap()
                .phobos_attach_effects()
                .is_empty(),
            "the frame that ends the duration removes the effect"
        );
        assert!(
            sim.substrate.pending_delete.contains_object(&anim),
            "the removed effect retires its animation"
        );
    }

    /// `Animation.HideIfAttachedWith`: a listed type being attached keeps the
    /// effect's animation away; when that type expires the next frame
    /// recreates it.
    #[test]
    fn hide_if_attached_with_hides_and_restores_the_animation() {
        use crate::rules::art_data::ArtRegistry;
        use crate::rules::ini_parser::IniFile;
        let ini = crate::sim::arena_fixture::clear_ground_ini(
            "[Countries]\n0=Americans\n[VehicleTypes]\n0=TARGET\n\
             [TARGET]\nStrength=1000\nSpeed=6\nAttachEffect.AttachTypes=SHOOTER,HIDER\n\
             AttachEffect.Delays=-1\n\
             [AttachEffectTypes]\n0=SHOOTER\n1=HIDER\n\
             [SHOOTER]\nDuration=100\nAnimation=PULSE\n\
             Animation.HideIfAttachedWith=HIDER\n\
             [HIDER]\nDuration=3\n",
        );
        let art = IniFile::from_str("[PULSE]\nRate=900\nEnd=4\n");
        let processed = crate::rules::native_processing::RulesLayerStack::new(ini)
            .process_with_fixed_art(&art)
            .expect("rules process");
        let mut rules =
            crate::rules::ruleset::RuleSet::from_processed_rules(&processed).expect("rules");
        let mut registry = ArtRegistry::from_ini(&art);
        registry.bind_anim_frame_count_for_test("PULSE", 4);
        rules.install_art_data(registry);
        let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 17);
        sim.intern_rule_type_ids(&rules);
        sim.resolve_type_handles(&rules);
        let target = sim
            .spawn_object("TARGET", "Americans", 20, 20, 0, &rules)
            .expect("spawn");
        let shooter_anim = |sim: &Simulation| {
            sim.entities()
                .get(target)
                .unwrap()
                .phobos_attach_effects()
                .instances()
                .iter()
                .find(|instance| instance.type_index == 0)
                .and_then(|instance| instance.anim())
        };

        sim.update_phobos_attach_effects(target, &rules);
        assert!(
            shooter_anim(&sim).is_none(),
            "HIDER is attached, so the animation stays hidden"
        );
        sim.update_phobos_attach_effects(target, &rules);
        assert!(shooter_anim(&sim).is_none());
        sim.update_phobos_attach_effects(target, &rules);
        assert!(
            shooter_anim(&sim).is_some(),
            "the expired HIDER no longer hides the animation"
        );
    }

    /// Phobos' C++ int casts: truncation toward zero, `i32::MIN` for a NaN or
    /// out-of-range product.
    #[test]
    fn scale_i32_matches_the_cpp_cast() {
        assert_eq!(scale_i32(7, 1.0), 7);
        assert_eq!(scale_i32(7, 1.5), 10);
        assert_eq!(scale_i32(7, -0.5), -3);
        assert_eq!(scale_i32(7, f64::INFINITY), i32::MIN);
        assert_eq!(scale_i32(7, f64::NAN), i32::MIN);
        assert_eq!(scale_i32(i32::MAX, 2.0), i32::MIN);
        assert_eq!(scale_i32(i32::MIN, 1.0), i32::MIN);
    }

    /// `ROFMultiplier.ApplyOnCurrentTimer`: the rearm timer already running
    /// when the effect first initializes is scaled; the key can turn it off.
    #[test]
    fn rof_multiplier_scales_the_running_rearm_timer_on_initialization() {
        use crate::rules::ini_parser::IniFile;

        for (apply, expected) in [(false, 30), (true, 60)] {
            let rules = crate::rules::ruleset::RuleSet::from_ini(&IniFile::from_str(&format!(
                "[Countries]\n0=Americans\n[VehicleTypes]\n0=TARGET\n\
                 [TARGET]\nStrength=1000\nSpeed=6\nAttachEffect.AttachTypes=ROFAE\n\
                 AttachEffect.InitialDelays=1\n\
                 [AttachEffectTypes]\n0=ROFAE\n\
                 [ROFAE]\nDuration=10\nROFMultiplier=2\n\
                 ROFMultiplier.ApplyOnCurrentTimer={}\n",
                if apply { "yes" } else { "no" }
            )))
            .expect("rules");
            let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 17);
            sim.intern_rule_type_ids(&rules);
            sim.resolve_type_handles(&rules);
            let target = sim
                .spawn_object("TARGET", "Americans", 20, 20, 0, &rules)
                .expect("spawn");
            let frame = sim.session().binary_frame() as i32;
            sim.substrate
                .entities
                .get_mut(target)
                .unwrap()
                .rearm_timer
                .start(frame, 30);

            sim.update_phobos_attach_effects(target, &rules);
            assert_eq!(
                sim.entities()
                    .get(target)
                    .unwrap()
                    .rearm_timer
                    .remaining(frame),
                30,
                "the initial-delay frame never initializes the effect"
            );
            sim.update_phobos_attach_effects(target, &rules);
            assert_eq!(
                sim.entities()
                    .get(target)
                    .unwrap()
                    .rearm_timer
                    .remaining(frame),
                expected,
                "ApplyOnCurrentTimer={apply}"
            );
        }
    }

    /// `RecalculateStatMultipliers` folds every active armor multiplier into
    /// the cached factor, and the receiver's extra divide consumes it after
    /// the first AI frame.
    #[test]
    fn active_effects_fold_into_the_receiver_armor_divider() {
        use crate::rules::ini_parser::IniFile;
        let rules = crate::rules::ruleset::RuleSet::from_ini(&IniFile::from_str(
            "[Countries]\n0=Americans\n[VehicleTypes]\n0=TARGET\n\
             [TARGET]\nStrength=1000\nSpeed=6\nAttachEffect.AttachTypes=ARMORA,ARMORB\n\
             [AttachEffectTypes]\n0=ARMORA\n1=ARMORB\n\
             [ARMORA]\nDuration=10\nArmorMultiplier=0.5\n\
             [ARMORB]\nDuration=10\nArmorMultiplier=0.5\n",
        ))
        .expect("rules");
        let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 17);
        sim.intern_rule_type_ids(&rules);
        sim.resolve_type_handles(&rules);
        let target = sim
            .spawn_object("TARGET", "Americans", 20, 20, 0, &rules)
            .expect("spawn");
        assert_eq!(
            sim.entities()
                .get(target)
                .unwrap()
                .phobos_armor_multiplier(),
            1.0,
            "the fold starts at identity until the first AI frame"
        );
        sim.update_phobos_attach_effects(target, &rules);
        assert_eq!(
            sim.entities()
                .get(target)
                .unwrap()
                .phobos_armor_multiplier(),
            0.25,
            "two active effects stack"
        );
        let divisors = crate::sim::combat::receiver_divisors(
            sim.entities().get(target).unwrap(),
            rules.object("TARGET"),
            &rules,
            sim.interner(),
            sim.houses(),
        );
        assert_eq!(divisors.ae_armor, 0.25);
        assert_eq!(
            crate::sim::combat::damage::receive::defence_divides(100, &divisors),
            400,
            "the stock divide then the effect divide"
        );
    }

    /// `TechnoExt::ExtData::HasAttachedEffects(..., requireAll=false,
    /// requireAnims=true)`: a required set matches on any active attached
    /// type, and a type that declares an animation only counts while its
    /// instance holds one.
    #[test]
    fn draw_offsets_match_any_active_required_type_and_need_their_animation() {
        use crate::rules::attach_effect::AnimationDrawOffset;

        let mut host = AttachEffectType::default();
        host.name = "HOST".to_owned();
        host.duration = 10;
        host.animation_draw_offsets = vec![
            AnimationDrawOffset {
                offset: [2, -1],
                required_types: Vec::new(),
            },
            AnimationDrawOffset {
                offset: [5, 5],
                required_types: vec!["NEEDED".to_owned(), "OTHER".to_owned()],
            },
        ];
        let mut needed = AttachEffectType::default();
        needed.name = "NEEDED".to_owned();
        needed.duration = 10;
        needed.animation = Some("LOOP".to_owned());
        let mut other = AttachEffectType::default();
        other.name = "OTHER".to_owned();
        other.duration = 10;
        let effects = vec![host, needed, other];
        let attach = |list: &mut AttachEffectList, type_index| {
            list.attach(
                type_index,
                AttachEffectParams {
                    delay: -1,
                    ..AttachEffectParams::default()
                },
                &effects[type_index],
                AttachPolicy::default(),
                None,
            )
        };

        let mut list = AttachEffectList::default();
        attach(&mut list, 0);
        attach(&mut list, 1);
        list.tick_all(&effects, |_| false);
        assert_eq!(
            list.draw_offset(0, &effects),
            [2, -1],
            "the unconditional rule adds on its own; a type that declares an \
             animation does not match without one"
        );
        list.instances[1].set_anim(Some(9));
        assert_eq!(
            list.draw_offset(0, &effects),
            [7, 4],
            "the animated instance satisfies the rule"
        );
        // `RequiredTypes` is any-of, and a second match must not add twice.
        list.instances[1].set_anim(None);
        attach(&mut list, 2);
        list.tick_all(&effects, |_| false);
        assert_eq!(
            list.draw_offset(0, &effects),
            [7, 4],
            "OTHER declares no animation, so it matches on its own"
        );
        list.instances[1].set_anim(Some(9));
        assert_eq!(
            list.draw_offset(0, &effects),
            [7, 4],
            "a rule adds once however many listed types match"
        );

        // An ended terminal effect no longer satisfies a required type.
        let mut host = AttachEffectType::default();
        host.name = "HOST".to_owned();
        host.duration = 10;
        host.animation_draw_offsets = vec![AnimationDrawOffset {
            offset: [7, 4],
            required_types: vec!["OTHER".to_owned()],
        }];
        let mut short = AttachEffectType::default();
        short.name = "OTHER".to_owned();
        short.duration = 2;
        let short_effects = vec![host, short];
        let mut list = AttachEffectList::default();
        list.attach(
            0,
            AttachEffectParams {
                delay: -1,
                ..AttachEffectParams::default()
            },
            &short_effects[0],
            AttachPolicy::default(),
            None,
        );
        // Attached from outside: activity follows the remaining duration, so a
        // run-out effect leaves the required set even before removal.
        list.attach(
            1,
            AttachEffectParams {
                delay: -1,
                ..AttachEffectParams::default()
            },
            &short_effects[1],
            AttachPolicy::default(),
            Some(AttachSource::default()),
        );
        list.tick_all(&short_effects, |_| false);
        assert_eq!(list.draw_offset(0, &short_effects), [7, 4]);
        list.tick_all(&short_effects, |_| false);
        assert_eq!(
            list.draw_offset(0, &short_effects),
            [0, 0],
            "an expired effect is not active"
        );
    }

    /// A creation attach reads the type's list with each paired value and
    /// keeps an unknown catalog name out.
    #[test]
    fn creation_attaches_the_type_list_with_paired_values() {
        use crate::rules::ruleset::RuleSet;
        let rules = RuleSet::from_ini(&crate::rules::ini_parser::IniFile::from_str(
            "[AttachEffectTypes]
0=FIRST
[FIRST]
Duration=40
             [VehicleTypes]
0=MTNK
[MTNK]
Name=Apocalypse
             AttachEffect.AttachTypes=FIRST,MISSING
AttachEffect.InitialDelays=3
",
        ))
        .expect("rules");
        let object = rules.object("MTNK").expect("techno");
        let mut list = AttachEffectList::default();
        assert_eq!(
            attach_initial_effects(&mut list, object, &rules, None),
            1,
            "only the catalog name attaches"
        );
        assert_eq!(list.instances().len(), 1);
        assert_eq!(list.instances()[0].remaining(), 40);
    }

    /// A paired list uses its own index, else the last value, else the key's
    /// documented default.
    #[test]
    fn paired_lists_take_the_last_value_for_the_remaining_types() {
        assert_eq!(paired(&[5, 7], 0, 0), 5);
        assert_eq!(paired(&[5, 7], 1, 0), 7);
        assert_eq!(paired(&[5, 7], 4, 0), 7, "the last covers the rest");
        assert_eq!(paired(&[], 0, -1), -1, "an absent list keeps the default");
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

    /// The offline and temporal actions hide, pause or freeze the animation.
    #[test]
    fn animation_state_maps_the_offline_and_temporal_actions() {
        let mut powered = effect(3);
        powered.powered = true;
        let mut instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                delay: -1,
                ..AttachEffectParams::default()
            },
            &powered,
        );
        instance.tick(&powered, false);
        let online = instance.animation_state(&powered, false);
        assert!(online.show && !online.paused && !online.temporal);

        instance.online_check(&powered, false);
        let offline = instance.animation_state(&powered, false);
        assert!(!offline.show, "the default OfflineAction hides");

        powered.animation_offline_action = AttachedAnimFlag::Paused;
        let offline = instance.animation_state(&powered, false);
        assert!(offline.show && offline.paused && !offline.temporal);

        powered.animation_temporal_action = AttachedAnimFlag::PausedTemporal;
        let frozen = instance.animation_state(&powered, true);
        assert!(frozen.show && frozen.paused && frozen.temporal);

        let plain = effect(3);
        let instance = AttachEffectInstance::new(
            0,
            AttachEffectParams {
                delay: -1,
                ..AttachEffectParams::default()
            },
            &plain,
        );
        let untouched = instance.animation_state(&plain, true);
        assert!(untouched.show && !untouched.paused && !untouched.temporal);
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
