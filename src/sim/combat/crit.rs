//! Phobos warhead critical hits.
//!
//! Native identity: `WarheadTypeExt::GetCritChance`/`ApplyCrit`
//! (`src/Ext/WarheadType/Detonate.cpp:525..608`, `735..778`), reached from
//! `WarheadTypeExt::Detonate`'s per-target pass (`DetonateOnOneUnit`, called
//! from the `BulletClass_Detonate @ 0x46920B` and `MapClass_DamageArea
//! @ 0x489286` hooks). The pass runs before the native area damage, so a hit
//! this module kills is already gone when the main warhead dispatches.
//!
//! RESIDUALS:
//! - The pass runs only in the ordinary-damage arm of the bullet detonation
//!   and in `MapClass::DamageArea`; Phobos runs it before the whole bullet arm
//!   chain (`0x46920B`). A warhead with a special arm (IvanBomb, MindControl,
//!   Temporal, Locomotor, ...) therefore skips its crit and the chance draws.
//! - The per-target list reuses the area receiver collection instead of
//!   Phobos' own `getCellSpreadItemsExt` walk, which also reads the
//!   `AircraftTracker` for airborne items; membership and order can differ
//!   (no airborne phase, different cell-list filters). Because that collection
//!   happens after SpawnShrapnel, the crit pass also runs after the shrapnel
//!   draws, where native runs it first.
//! - `Crit.Warhead`'s full detonation creates a projectile with no
//!   `Projectile=` type, so the default bullet type's `Cluster` is not read.
//! - `Crit.ExtraDamage.ApplyFirepowerMult` reads the owner's live multiplier;
//!   native uses the bullet's firepower captured at launch.
//! - A bullet does not retain its firing house (VERA's `firer_house` only
//!   serves `FirersPalette`), so a round whose firer expired loses the
//!   `Crit.AffectsHouse` gate and the crit animations' house; native falls
//!   back to `BulletExt::FirerHouse`.
//! - Crit animations set the owner house but no invoker (VERA stores no anim
//!   invoker on this branch).
//! - `Crit.SuppressWhenIntercepted` is read but VERA tracks no intercepted
//!   bullet, so the bullet's `false` value never suppresses.
//! - `Crit.AffectsTarget` treats every building as a plain building; native's
//!   `IsStrange` split has no VERA reader.
//! - `ApplyPerTargetEffectsOnDetonate=no` selects the per-target dice here but
//!   native moves the whole pass into ReceiveDamage and plays no chance
//!   animations. The per-detonation `ReturnWarhead` buffer Phobos draws before
//!   `Crit.ActiveChanceAnims` does not exist in VERA at all, so a warhead that
//!   sets both keys shifts the scenario stream.

use super::combat_aoe::AreaDamageReceiver;
use super::effects::DeathEffects;
use super::receiver_call::EntityDamageEvent;
use super::world_receiver::{
    ReceiverRun, apply_area_damage_in_run, commit_entities, commit_projectiles,
};
use crate::map::entities::EntityCategory;
use crate::rules::object_type::ObjectType;
use crate::rules::phobos_crit::{AffectedTarget, PhobosAbilityFlags};
use crate::rules::ruleset::RuleSet;
use crate::rules::terrain_rules::LandType;
use crate::rules::warhead_type::WarheadType;
use crate::sim::combat::veterancy::{VeterancyRank, has_weapon_ability};
use crate::sim::intern::InternedId;
use crate::sim::projectile::{
    ProjectileCoord, ProjectileDetonation, ProjectileDetonationReason, ProjectilePayload,
    ProjectileTarget,
};
use crate::sim::world::Simulation;

/// The per-detonation state `WarheadTypeExt::Detonate` sets before its
/// per-target pass: the firer's chance once, and the dice buffer the whole
/// detonation shares unless `Crit.ApplyChancePerTarget` is set.
pub(crate) struct CritRoll {
    chance: f64,
    buffer: f64,
    per_target: bool,
}

/// `Detonate`'s crit prefix: `Crit_CurrentChance = GetCritChance(owner)`, the
/// shared dice buffer and `Crit.ActiveChanceAnims`. `None` when the warhead
/// cannot crit.
#[cfg(feature = "phobos")]
pub(crate) fn begin(
    world: &mut Simulation,
    rules: &RuleSet,
    warhead: &WarheadType,
    source_id: u64,
    source_house: Option<InternedId>,
    impact: ProjectileCoord,
) -> Option<CritRoll> {
    let crit = &warhead.crit;
    if !crit.is_enabled() {
        return None;
    }
    let chance = get_crit_chance(world, rules, warhead, source_id);
    if !(chance > 0.0) {
        return None;
    }
    let globals = rules.crit_globals();
    // `Crit_ApplyChancePerTarget || !ApplyPerTargetEffectsOnDetonate`: the
    // second term is native's per-target ReceiveDamage hook, which rolls a
    // fresh chance for every target.
    let per_target =
        crit.apply_chance_per_target(globals) || !globals.apply_per_target_effects_on_detonate();
    let buffer = if per_target {
        0.0
    } else {
        world.scenario_rng.random_double()
    };
    // One random pick per detonation, even when every target later fails a
    // gate: `CreateRandomAnim` draws before it constructs.
    if !crit.active_chance_anims().is_empty() {
        let index = world.scenario_rng.next_range_i32_inclusive(
            0,
            i32::try_from(crit.active_chance_anims().len() - 1)
                .expect("native anim count fits i32"),
        );
        let name = crit.active_chance_anims()[index as usize].clone();
        spawn_anim_at_world_coord(world, rules, &name, impact, source_house);
    }
    Some(CritRoll {
        chance,
        buffer,
        per_target,
    })
}

/// `WarheadTypeExt::GetCritChance`: the warhead's `Crit.Chance`, the
/// `CRITCHANCE` rank ability's `VeteranCritChance` and (in the AttachEffect
/// chain) the effect list fold.
#[cfg(feature = "phobos")]
fn get_crit_chance(
    world: &Simulation,
    rules: &RuleSet,
    warhead: &WarheadType,
    owner_id: u64,
) -> f64 {
    let mut chance = warhead.crit.chance();
    if owner_id == 0 {
        return chance;
    }
    let Some(owner) = world.substrate.entities.get(owner_id) else {
        return chance;
    };
    let Some(object) = world.interner.object_type(owner.type_ref(), rules) else {
        return chance;
    };
    let rank = owner.veterancy.rank_kind();
    if has_phobos_ability(rank, object, PhobosAbilityFlags::CRIT_CHANCE) {
        let veteran = object
            .veteran_crit_chance()
            .unwrap_or(rules.crit_globals().veteran_crit_chance());
        chance *= veteran.max(0.0);
    }
    // Phobos `GetCritChance`'s AttachEffect fold: every active effect whose
    // block is more than the identity multiplies the chance and adds its
    // extra chance, gated by the Allow/Disallow warhead lists.
    if !owner.phobos_stat_multipliers().has_crit_modifiers {
        return chance;
    }
    let effects = rules.attach_effect_types().types();
    let mut extra_chance = 0.0;
    for (_, effect) in owner.phobos_attach_effects().active_effects(effects) {
        if !effect.has_crit_modifiers() {
            continue;
        }
        if !effect.crit_allow_warheads.is_empty()
            && !effect
                .crit_allow_warheads
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&warhead.id))
        {
            continue;
        }
        if effect
            .crit_disallow_warheads
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&warhead.id))
        {
            continue;
        }
        chance *= effect.crit_multiplier.max(0.0);
        extra_chance += effect.crit_extra_chance;
    }
    chance + extra_chance
}

#[cfg(feature = "phobos")]
fn has_phobos_ability(rank: VeterancyRank, object: &ObjectType, slot: usize) -> bool {
    match rank {
        VeterancyRank::Rookie => false,
        VeterancyRank::Veteran => object.phobos_abilities().has(slot, false),
        VeterancyRank::Elite => object.phobos_abilities().has(slot, true),
    }
}

/// `Detonate`'s per-target pass: `ApplyCrit` on each target in order.
/// Returns whether any target critted (`Crit_Active`), which the detonation's
/// damage-animation selection reads.
#[cfg(feature = "phobos")]
pub(crate) fn apply_targets(
    world: &mut Simulation,
    run: &mut ReceiverRun<'_>,
    rules: &RuleSet,
    overlay_registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
    warhead: &WarheadType,
    warhead_ref: InternedId,
    source_id: u64,
    source_house: Option<InternedId>,
    roll: &CritRoll,
    targets: &[u64],
    effects: &mut DeathEffects,
) -> bool {
    let mut active = false;
    for &target_id in targets {
        if apply_one(
            world,
            run,
            rules,
            overlay_registry,
            warhead,
            warhead_ref,
            source_id,
            source_house,
            roll,
            target_id,
            effects,
        ) {
            active = true;
        }
    }
    active
}

/// One `ApplyCrit`: dice, immunity/health/house/cell/techno gates in native
/// order, then the crit's animations and damage.
#[cfg(feature = "phobos")]
#[allow(clippy::too_many_arguments)]
fn apply_one(
    world: &mut Simulation,
    run: &mut ReceiverRun<'_>,
    rules: &RuleSet,
    overlay_registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
    warhead: &WarheadType,
    warhead_ref: InternedId,
    source_id: u64,
    source_house: Option<InternedId>,
    roll: &CritRoll,
    target_id: u64,
    effects: &mut DeathEffects,
) -> bool {
    // `InApplyCrit`: a crit whose own damage detonates this same warhead
    // cannot crit again while the outer apply is running.
    if run.crit_warheads.contains(&warhead_ref) {
        return false;
    }
    let crit = &warhead.crit;
    // `ApplyChancePerTarget || !ApplyPerTargetEffectsOnDetonate` picks a fresh
    // draw; the shared buffer otherwise.
    let dice = if roll.per_target {
        world.scenario_rng.random_double()
    } else {
        roll.buffer
    };
    if roll.chance < dice {
        return false;
    }
    let Some(target) = world.substrate.entities.get(target_id) else {
        return false;
    };
    let target_object = world.interner.object_type(target.type_ref(), rules);
    if target_object.is_some_and(|object| object.immune_to_crit()) {
        return false;
    }
    if target_object.is_some_and(|object| {
        has_phobos_ability(
            target.veterancy.rank_kind(),
            object,
            PhobosAbilityFlags::CRIT_IMMUNE,
        )
    }) {
        return false;
    }
    if target.shield.as_ref().is_some_and(|state| {
        state.is_active()
            && rules
                .shield_type(state.type_name())
                .is_some_and(|ty| ty.immune_to_crit())
    }) {
        return false;
    }
    let strength = target_object.map_or(0, |object| object.strength);
    if !super::health_in_threshold(
        target.health.current(),
        strength,
        crit.affects_above_percent(),
        crit.affects_below_percent(),
    ) {
        return false;
    }
    if let Some(source) = source_house {
        let target_owner = target.owner();
        if !super::can_target_house(
            crit.affects_house(),
            source,
            target_owner,
            &world.houses,
            &world.house_alliances,
            &world.interner,
        ) {
            return false;
        }
    }
    let land = world.resolved_terrain.as_ref().and_then(|terrain| {
        terrain
            .cell(target.position.rx(), target.position.ry())
            .and_then(|cell| LandType::from_index(cell.land_type))
    });
    if !cell_eligible(land, crit.affects_target()) {
        return false;
    }
    if !techno_eligible(target.category(), crit.affects_target()) {
        return false;
    }

    run.crit_warheads.push(warhead_ref);
    let applied = crit_effects(
        world,
        run,
        rules,
        overlay_registry,
        warhead,
        warhead_ref,
        source_id,
        source_house,
        target_id,
        effects,
    );
    run.crit_warheads.pop();
    applied
}

#[cfg(feature = "phobos")]
#[allow(clippy::too_many_arguments)]
fn crit_effects(
    world: &mut Simulation,
    run: &mut ReceiverRun<'_>,
    rules: &RuleSet,
    overlay_registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
    warhead: &WarheadType,
    warhead_ref: InternedId,
    source_id: u64,
    source_house: Option<InternedId>,
    target_id: u64,
    effects: &mut DeathEffects,
) -> bool {
    let crit = &warhead.crit;
    let globals = rules.crit_globals();
    let Some(target) = world.substrate.entities.get(target_id) else {
        return false;
    };
    let coord = crate::sim::movement::ground_pose::object_get_coords(
        target,
        world.resolved_terrain.as_ref(),
    );
    let anim_coord = crate::sim::anim_class::AnimWorldCoord {
        x: coord.x,
        y: coord.y,
        z: coord.z,
    };
    let owner_house = source_house.or_else(|| {
        world
            .substrate
            .entities
            .get(source_id)
            .map(|source| source.owner())
    });

    // `Crit.AnimList` at the affected target; the detonation's damage
    // animation only takes the list when `Crit.AnimOnAffectedTargets` is off.
    if crit.anim_on_affected_targets(globals) && !crit.anim_list().is_empty() {
        if crit.anim_list_create_all() {
            for name in crit.anim_list() {
                spawn_anim_at_coord(world, rules, name, anim_coord, owner_house);
            }
        } else {
            // `EMEffect || Crit_AnimList_PickRandom` selects a random entry;
            // otherwise index 0.
            let index = if warhead.em_effect || crit.anim_list_pick_random() {
                world.scenario_rng.next_range_i32_inclusive(
                    0,
                    i32::try_from(crit.anim_list().len() - 1).expect("native anim count fits i32"),
                )
            } else {
                0
            };
            let name = crit.anim_list()[index as usize].clone();
            spawn_anim_at_coord(world, rules, &name, anim_coord, owner_house);
        }
    }

    let mut damage = crit.extra_damage();
    if crit.extra_damage_apply_firepower_mult(globals)
        && let Some(owner) = world.substrate.entities.get(source_id)
    {
        let firepower = current_firepower_multiplier(world, rules, owner);
        damage = cpp_i32(f64::from(damage) * firepower);
    }

    match crit
        .warhead()
        .map(|name| (name, crit.warhead_full_detonation()))
    {
        Some((name, true)) => {
            let Some(crit_warhead) = rules.warhead(name) else {
                return true;
            };
            let warhead_ref = world.interner.intern(name);
            let impact = ProjectileCoord::new(coord.x, coord.y, coord.z);
            let detonation = ProjectileDetonation {
                projectile_id: target_id,
                source_id,
                target: ProjectileTarget::Entity(target_id),
                impact,
                payload: ProjectilePayload::new(damage, warhead_ref, None, crit_warhead.bright),
                reason: ProjectileDetonationReason::CritWarhead,
            };
            let commit = commit_projectiles(world, run, &[detonation], rules, overlay_registry);
            effects.append(commit.effects);
            effects
                .under_attack_events
                .extend(commit.under_attack_events);
        }
        Some((name, false)) => {
            let Some(crit_warhead) = rules.warhead(name) else {
                return true;
            };
            // `DamageAreaWithTarget`: `MapClass::DamageArea` with the crit
            // warhead; the intended target only affects the nested per-target
            // pass (no CellSpread arm), which this build leaves empty.
            let crit_ref = world.interner.intern(name);
            let impact = ProjectileCoord::new(coord.x, coord.y, coord.z);
            let (nested, mut pings, _crit_active) = apply_area_damage_in_run(
                world,
                run,
                rules,
                overlay_registry,
                impact,
                damage,
                crit_warhead,
                (source_id, source_house, crit_ref),
            );
            effects.append(nested);
            effects.under_attack_events.append(&mut pings);
        }
        None => {
            // `pTarget->ReceiveDamage(&damage, 0, warhead, owner, false,
            // false, house)` with the ORIGINAL warhead's id.
            let event =
                EntityDamageEvent::area(target_id, damage, 0, source_id, source_house, warhead_ref);
            let (nested, mut pings) =
                commit_entities(world, run, &[event], None, rules, overlay_registry);
            effects.append(nested);
            effects.under_attack_events.append(&mut pings);
        }
    }
    true
}

/// `TechnoExt::GetCurrentFirepowerMultiplier`: `Techno+0x158`-adjacent
/// `FirepowerMultiplier`, the house bias, the FIREPOWER rank stage and an
/// occupiable building's `OccupyDamageMultiplier=`.
#[cfg(feature = "phobos")]
fn current_firepower_multiplier(
    world: &Simulation,
    rules: &RuleSet,
    entity: &crate::sim::game_entity::GameEntity,
) -> f64 {
    use crate::rules::object_type::Ability;
    use crate::util::native_x87::NativeF64Bits;
    let f64_bits = |bits: NativeF64Bits| f64::from_bits(bits.bits());
    let mut multiplier = f64_bits(entity.crate_multipliers.firepower());
    multiplier *= world
        .houses
        .get(&entity.owner())
        .map_or(1.0, |house| f64_bits(house.firepower_bias()));
    if let Some(object) = world.interner.object_type(entity.type_ref(), rules) {
        if has_weapon_ability(entity.veterancy.rank_kind(), object, Ability::Firepower) {
            multiplier *= rules.general.veteran_combat;
        }
        if entity.category() == EntityCategory::Structure
            && object.can_be_occupied
            && object.can_occupy_fire
            && object.max_number_occupants > 0
        {
            multiplier *= f64::from(rules.garrison_rules.occupy_damage_multiplier);
        }
    }
    multiplier
}

/// `EnumFunctions::IsCellEligible` with `explicitEmptyCells = false`: the
/// AllCells pair follows the target cell's water flag; any other flag passes
/// as long as it is not empty. A fixture without terrain reads as land.
#[cfg(feature = "phobos")]
fn cell_eligible(land: Option<LandType>, allowed: AffectedTarget) -> bool {
    if allowed == AffectedTarget::ALL {
        return true;
    }
    if allowed.contains(AffectedTarget::ALL_CELLS) {
        if land == Some(LandType::Water) {
            allowed.contains(AffectedTarget::WATER)
        } else {
            allowed.contains(AffectedTarget::LAND)
        }
    } else {
        allowed != AffectedTarget::NONE
    }
}

/// `EnumFunctions::IsTechnoEligible` with `considerAircraftSeparately =
/// false`: the content flags split by class, aircraft under the Unit bit and
/// buildings under the Building bit.
#[cfg(feature = "phobos")]
fn techno_eligible(category: EntityCategory, allowed: AffectedTarget) -> bool {
    if allowed == AffectedTarget::ALL {
        return true;
    }
    if allowed.contains(AffectedTarget::ALL_CONTENTS) {
        return match category {
            EntityCategory::Infantry => allowed.contains(AffectedTarget::INFANTRY),
            EntityCategory::Unit | EntityCategory::Aircraft => {
                allowed.contains(AffectedTarget::UNIT)
            }
            EntityCategory::Structure => allowed.contains(AffectedTarget::BUILDING),
        };
    }
    allowed != AffectedTarget::NONE
}

/// `AnimExt::CreateRandomAnim` (`Ext/Anim/Body.cpp:407..432`): one random
/// entry constructed at the coordinates, owned by the firing house.
#[cfg(feature = "phobos")]
fn spawn_anim_at_world_coord(
    world: &mut Simulation,
    rules: &RuleSet,
    name: &str,
    coord: ProjectileCoord,
    owner_house: Option<InternedId>,
) {
    let world_coord = crate::sim::anim_class::AnimWorldCoord {
        x: coord.x,
        y: coord.y,
        z: coord.z,
    };
    spawn_anim_at_coord(world, rules, name, world_coord, owner_house);
}

#[cfg(feature = "phobos")]
fn spawn_anim_at_coord(
    world: &mut Simulation,
    rules: &RuleSet,
    name: &str,
    coord: crate::sim::anim_class::AnimWorldCoord,
    owner_house: Option<InternedId>,
) {
    let (rx, ry, sub_x, sub_y, z) = coord.to_cell_sub_z();
    let type_name = world.interner.intern(name);
    let descriptor =
        crate::sim::components::AnimClassSpawnDescriptor::new(type_name, rx, ry, sub_x, sub_y, z);
    match world.spawn_anim_at_world(rules, descriptor, coord) {
        Ok(anim) => {
            if let Some(house) = owner_house {
                world.substrate.set_anim_owner_house(anim, house);
            }
        }
        Err(error) => log::debug!("crit animation [{name}] did not construct: {error}"),
    }
}

/// MSVC `static_cast<int>` (`cvttsd2si`): truncation toward zero, out-of-range
/// and NaN to `i32::MIN`.
#[cfg(feature = "phobos")]
fn cpp_i32(value: f64) -> i32 {
    if value.is_nan() || value < f64::from(i32::MIN) || value >= 2_147_483_648.0 {
        i32::MIN
    } else {
        value.trunc() as i32
    }
}

/// The targets `DetonateOnOneUnit` visits for this detonation: every collected
/// entity receiver in order for a cell-spread warhead, or the bullet's own
/// target within 64 leptons (Phobos' no-spread arm) when it does not spread.
#[cfg(feature = "phobos")]
pub(crate) fn detonation_targets(
    world: &Simulation,
    warhead: &WarheadType,
    detonation: &ProjectileDetonation,
    receivers: &[AreaDamageReceiver],
) -> Vec<u64> {
    let mut targets = Vec::new();
    if warhead.cell_spread_f64 > 0.0 {
        for receiver in receivers {
            if let AreaDamageReceiver::Entity(event) = receiver {
                targets.push(event.target_id);
            }
        }
    } else if let ProjectileTarget::Entity(id) = detonation.target
        && world.substrate.entities.get(id).is_some_and(|target| {
            let coords = crate::sim::movement::ground_pose::object_get_coords(
                target,
                world.resolved_terrain.as_ref(),
            );
            let dx = i64::from(detonation.impact.x) - i64::from(coords.x);
            let dy = i64::from(detonation.impact.y) - i64::from(coords.y);
            let dz = i64::from(detonation.impact.z) - i64::from(coords.z);
            dx * dx + dy * dy + dz * dz <= 4096
        })
    {
        targets.push(id);
    }
    targets
}
