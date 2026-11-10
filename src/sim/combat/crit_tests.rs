//! Phobos warhead critical hits through the production detonation path.

use super::world_receiver::{ReceiverRun, commit_projectiles};
use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::movement::ground_pose::object_get_coords;
use crate::sim::projectile::{
    ProjectileCoord, ProjectileDetonation, ProjectileDetonationReason, ProjectilePayload,
    ProjectileTarget,
};
use crate::sim::world::Simulation;

/// Two rival houses on the flat arena; `sections` appends further type bodies
/// (`[CRITWH]`, ...) and `warheads` the extra `[Warheads]` registry rows.
fn scene(
    warheads: &str,
    wh_ini: &str,
    target_ini: &str,
    sections: &str,
    target_house: &str,
) -> (Simulation, RuleSet, u64, u64) {
    scene_with_firer(warheads, wh_ini, target_ini, sections, "", target_house)
}

#[allow(clippy::too_many_arguments)]
fn scene_with_firer(
    warheads: &str,
    wh_ini: &str,
    target_ini: &str,
    sections: &str,
    firer_ini: &str,
    target_house: &str,
) -> (Simulation, RuleSet, u64, u64) {
    let rules = RuleSet::from_ini(&IniFile::from_str(&format!(
        "[Countries]\n0=Americans\n1=Russians\n\
         [VehicleTypes]\n0=FIRER\n1=TARGET\n\
         [WeaponTypes]\n0=GUN\n\
         [GUN]\nDamage=0\nRange=5\nROF=20\nWarhead=WH\n\
         [Warheads]\n0=WH\n{warheads}\
         [WH]\nCellSpread=0\n{wh_ini}\
         [FIRER]\nStrength=500\nArmor=none\nSpeed=6\nPrimary=GUN\n{firer_ini}\
         [TARGET]\nStrength=500\nArmor=none\nSpeed=6\n{target_ini}\n\
         {sections}"
    )))
    .expect("rules");
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 0x5A_AA_1E);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let firer = sim
        .spawn_object("FIRER", "Americans", 20, 20, 0, &rules)
        .expect("firer");
    let target = sim
        .spawn_object("TARGET", target_house, 22, 20, 0, &rules)
        .expect("target");
    (sim, rules, firer, target)
}

fn health(sim: &Simulation, id: u64) -> i32 {
    sim.entities().get(id).expect("entity").health.current()
}

fn detonation(
    sim: &mut Simulation,
    warhead: &str,
    source: u64,
    target: u64,
    damage: i32,
) -> ProjectileDetonation {
    let coords = {
        let target = sim.entities().get(target).expect("target");
        object_get_coords(target, sim.resolved_terrain.as_ref())
    };
    ProjectileDetonation {
        projectile_id: target,
        source_id: source,
        target: ProjectileTarget::Entity(target),
        impact: ProjectileCoord::new(coords.x, coords.y, coords.z),
        payload: ProjectilePayload::new(damage, sim.interner.intern(warhead), None, false),
        reason: ProjectileDetonationReason::ReachedTarget,
    }
}

fn shoot(
    sim: &mut Simulation,
    rules: &RuleSet,
    warhead: &str,
    source: u64,
    target: u64,
    damage: i32,
) -> super::LogicProjectileCommit {
    let detonation = detonation(sim, warhead, source, target, damage);
    let mut run = ReceiverRun::default();
    let commit = commit_projectiles(sim, &mut run, &[detonation], rules, None);
    run.finish();
    commit
}

/// One `WarheadTypeExt::Detonate` per-target pass of `WH` at `target`'s cell,
/// with `intended` as the bullet target and its own cell-spread collection.
fn detonate_once(
    sim: &mut Simulation,
    rules: &RuleSet,
    firer: u64,
    target: u64,
    intended: Option<u64>,
) {
    let warhead = rules.warhead("WH").expect("warhead");
    let warhead_ref = sim.interner.intern("WH");
    let source_house = sim.entities().get(firer).map(|entity| entity.owner());
    let impact = detonation(sim, "WH", firer, target, 0).impact;
    let mut run = ReceiverRun::default();
    let mut effects = super::effects::DeathEffects::default();
    super::crit::detonate(
        sim,
        &mut run,
        rules,
        None,
        warhead,
        warhead_ref,
        firer,
        source_house,
        impact,
        intended,
        0,
        false,
        &mut effects,
    );
    run.finish();
}

/// The Scenario stream of a plain scene after `doubles` `RandomDouble` draws.
fn after_doubles(doubles: usize) -> crate::sim::rng::SimRngLogicalState {
    let (sim, _rules, _firer, _target) = scene("", "", "", "", "Russians");
    let mut expected = sim.scenario_rng.clone();
    for _ in 0..doubles {
        expected.random_double();
    }
    expected.logical_state()
}

/// `ApplyCrit`'s plain arm calls the target's `ReceiveDamage` with
/// `Crit.ExtraDamage` before the native area damage; the same rules without a
/// crit chance leave the identical packet untouched.
#[test]
fn crit_extra_damage_lands_through_the_bullet_detonation() {
    let (mut crit_sim, crit_rules, firer, target) = scene(
        "",
        "Crit.Chance=1.0\nCrit.ExtraDamage=50\n",
        "",
        "",
        "Russians",
    );
    let (mut plain_sim, plain_rules, plain_firer, plain_target) = scene("", "", "", "", "Russians");
    shoot(&mut crit_sim, &crit_rules, "WH", firer, target, 0);
    shoot(
        &mut plain_sim,
        &plain_rules,
        "WH",
        plain_firer,
        plain_target,
        0,
    );
    assert_eq!(
        health(&plain_sim, plain_target) - health(&crit_sim, target),
        50,
        "the crit adds exactly its ExtraDamage"
    );
}

/// `ApplyCrit`'s house, health-threshold and `ImmuneToCrit` gates each stop
/// the same always-crit warhead.
#[test]
fn crit_gates_follow_house_health_and_immunity() {
    let crit_damage =
        |wh_ini: &str, target_ini: &str, target_house: &str, set_health: Option<i32>| {
            let (mut sim, rules, firer, target) = scene("", wh_ini, target_ini, "", target_house);
            let (mut plain, plain_rules, plain_firer, plain_target) =
                scene("", "", target_ini, "", target_house);
            if let Some(value) = set_health {
                for (world, id) in [(&mut sim, target), (&mut plain, plain_target)] {
                    world
                        .substrate
                        .entities
                        .get_mut(id)
                        .unwrap()
                        .health
                        .set_for_test(value);
                }
            }
            shoot(&mut sim, &rules, "WH", firer, target, 0);
            shoot(&mut plain, &plain_rules, "WH", plain_firer, plain_target, 0);
            health(&plain, plain_target) - health(&sim, target)
        };

    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nCrit.AffectsHouse=enemies\n",
            "",
            "Americans",
            None,
        ),
        0,
        "an allied owner fails the enemies flag"
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nCrit.AffectsAbovePercent=0.9\n",
            "",
            "Russians",
            Some(50),
        ),
        0,
        "10% health misses the (0.9, 1.0] interval"
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\n",
            "ImmuneToCrit=yes\n",
            "Russians",
            None,
        ),
        0,
        "ImmuneToCrit refuses the crit"
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nCrit.AffectsAbovePercent=0.9\n",
            "",
            "Russians",
            None,
        ),
        50,
        "full health satisfies the threshold"
    );
}

/// Phobos `WarheadTypeExt::CanAffectTarget`'s trigger gates
/// (`Ext/WarheadType/Body.cpp:37-51`): a warhead whose own
/// `AffectsAbovePercent`/`AffectsBelowPercent`, `AffectsVeterancy` or
/// `AffectsInvokerOnly` refuses the target skips it before the crit pass.
#[test]
fn warhead_trigger_gates_skip_the_per_target_pass() {
    let crit_damage = |wh_ini: &str, set_health: Option<i32>, rank: Option<u16>| {
        let (mut sim, rules, firer, target) = scene("", wh_ini, "", "", "Russians");
        let (mut plain, plain_rules, plain_firer, plain_target) = scene("", "", "", "", "Russians");
        for (world, id) in [(&mut sim, target), (&mut plain, plain_target)] {
            if let Some(value) = set_health {
                world
                    .substrate
                    .entities
                    .get_mut(id)
                    .unwrap()
                    .health
                    .set_for_test(value);
            }
            if let Some(rank) = rank {
                world
                    .substrate
                    .entities
                    .get_mut(id)
                    .unwrap()
                    .veterancy
                    .set_rank_for_test(rank);
            }
        }
        shoot(&mut sim, &rules, "WH", firer, target, 0);
        shoot(&mut plain, &plain_rules, "WH", plain_firer, plain_target, 0);
        health(&plain, plain_target) - health(&sim, target)
    };

    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nAffectsAbovePercent=0.9\n",
            Some(50),
            None
        ),
        0,
        "10% health fails the warhead's own threshold"
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nAffectsAbovePercent=0.9\n",
            None,
            None
        ),
        50
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nAffectsBelowPercent=0.5\n",
            None,
            None
        ),
        0,
        "full health fails (0.0, 0.5]"
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nAffectsBelowPercent=0.5\n",
            Some(100),
            None
        ),
        50,
        "20% health is inside (0.0, 0.5]"
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nAffectsVeterancy=rookie\n",
            None,
            Some(100)
        ),
        0,
        "a veteran target fails the rookie-only mask"
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nAffectsVeterancy=veteran\n",
            None,
            Some(100)
        ),
        50
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nAffectsInvokerOnly=yes\n",
            None,
            None
        ),
        0,
        "the firer is not the target"
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nAffectsInvokerOnly=yes\n\
             AffectsInvokerOnly.Reverse=yes\n",
            None,
            None
        ),
        50,
        "the reversed gate refuses only the invoker itself"
    );
}

/// Phobos `WarheadTypeExt::Detonate`'s `TransactMoney` block
/// (`Ext/WarheadType/Detonate.cpp:102-111`) through the bullet detonation:
/// a positive amount credits the firing house, a negative one debits it and a
/// zero amount moves nothing.
#[test]
fn transact_money_moves_the_firing_house_by_the_signed_amount() {
    // (firing house delta, target house delta) from `start` credits each.
    let run = |amount: i32, start: i32| {
        let (mut sim, rules, firer, target) =
            scene("", &format!("TransactMoney={amount}\n"), "", "", "Russians");
        let owner = sim.entities().get(firer).unwrap().owner();
        let target_owner = sim.entities().get(target).unwrap().owner();
        for house in [owner, target_owner] {
            sim.houses
                .get_mut(&house)
                .unwrap()
                .economy_mut_for_test()
                .set_credits_for_test(start);
        }
        shoot(&mut sim, &rules, "WH", firer, target, 0);
        (
            sim.houses.get(&owner).unwrap().economy().credits() - start,
            sim.houses.get(&target_owner).unwrap().economy().credits() - start,
        )
    };
    assert_eq!(run(25, 0), (25, 0), "a positive value credits the firer");
    assert_eq!(run(-40, 100), (-40, 0), "a negative value debits the firer");
    assert_eq!(
        run(-40, 10),
        (-10, 0),
        "Spend_Money clamps at the balance it covers"
    );
    assert_eq!(run(0, 0), (0, 0), "zero moves nothing");
}

/// `TransactMoney.Display`'s label fields: `AtFirer` anchors on the firer's
/// `Location` instead of the impact, `Houses` and `Offset` travel with the
/// event, and the label has no source object (`pSource = nullptr`).
#[test]
fn transact_money_display_uses_at_firer_houses_and_offset() {
    use crate::rules::phobos_shields::AffectedHouse;
    use crate::sim::flying_text::BountyText;
    use crate::sim::movement::ground_pose::object_location;
    use crate::sim::world::FrameEffects;
    use std::cell::RefCell;

    let events = RefCell::new(Vec::new());
    let observe = |_: &Simulation, _: &RuleSet, text: BountyText| {
        events.borrow_mut().push(text);
    };
    let effects = FrameEffects::empty().with_bounty_text(&observe);
    let (mut sim, rules, firer, target) = scene(
        "",
        "TransactMoney=13\nTransactMoney.Display=yes\n\
         TransactMoney.Display.AtFirer=yes\nTransactMoney.Display.Houses=enemies\n\
         TransactMoney.Display.Offset=6,-4\n",
        "",
        "",
        "Russians",
    );
    let owner = sim.entities().get(firer).unwrap().owner();
    let firer_coord = object_location(
        sim.entities().get(firer).unwrap(),
        sim.resolved_terrain.as_ref(),
    );
    let shot = detonation(&mut sim, "WH", firer, target, 0);
    let mut run = ReceiverRun::default().with_effects(effects);
    commit_projectiles(&mut sim, &mut run, &[shot], &rules, None);
    run.finish();
    let events = events.borrow();
    assert_eq!(events.len(), 1, "Display=yes emits one label");
    assert_eq!(events[0].entity_id, None, "FlyingStrings source is nullptr");
    assert_eq!(events[0].amount, 13);
    assert_eq!(
        events[0].coord,
        ProjectileCoord::new(firer_coord.x, firer_coord.y, firer_coord.z),
        "AtFirer anchors on the firer's Location"
    );
    assert_eq!(events[0].display_to, Some(AffectedHouse::ENEMIES));
    assert_eq!(events[0].display_owner, Some(owner));
    assert_eq!(events[0].pixel_offset, [6, -4]);

    // The same warhead without Display emits no label.
    let hidden = RefCell::new(Vec::new());
    let observe_hidden = |_: &Simulation, _: &RuleSet, text: BountyText| {
        hidden.borrow_mut().push(text);
    };
    let hidden_effects = FrameEffects::empty().with_bounty_text(&observe_hidden);
    let (mut sim, rules, firer, target) = scene("", "TransactMoney=13\n", "", "", "Russians");
    let shot = detonation(&mut sim, "WH", firer, target, 0);
    let mut run = ReceiverRun::default().with_effects(hidden_effects);
    commit_projectiles(&mut sim, &mut run, &[shot], &rules, None);
    run.finish();
    assert!(hidden.borrow().is_empty(), "Display=no emits no label");

    // `if (this->TransactMoney)` wraps the Display block too: a zero amount
    // moves nothing and emits no label.
    let zero = RefCell::new(Vec::new());
    let observe_zero = |_: &Simulation, _: &RuleSet, text: BountyText| {
        zero.borrow_mut().push(text);
    };
    let zero_effects = FrameEffects::empty().with_bounty_text(&observe_zero);
    let (mut sim, rules, firer, target) = scene(
        "",
        "TransactMoney=0\nTransactMoney.Display=yes\n",
        "",
        "",
        "Russians",
    );
    let shot = detonation(&mut sim, "WH", firer, target, 0);
    let mut run = ReceiverRun::default().with_effects(zero_effects);
    commit_projectiles(&mut sim, &mut run, &[shot], &rules, None);
    run.finish();
    assert!(zero.borrow().is_empty(), "a zero amount emits no label");
}

/// Phobos `TechnoClass_CanFire @ 0x6FC339`
/// (`Ext/Techno/Hooks.Firing.cpp:325-328`): a negative `TransactMoney`
/// warhead refuses the shot while the firing house's `Available_Money` is
/// below `-amount`; an affordable or non-negative amount fires.
#[cfg(feature = "phobos")]
#[test]
fn transact_money_gates_the_shot_on_the_firing_house_wallet() {
    use crate::sim::combat::fire_error::FireError;
    let error = |amount: i32, credits: i32| {
        let (mut sim, rules, firer, target) =
            scene("", &format!("TransactMoney={amount}\n"), "", "", "Russians");
        let owner = sim.entities().get(firer).unwrap().owner();
        sim.houses
            .get_mut(&owner)
            .unwrap()
            .economy_mut_for_test()
            .set_credits_for_test(credits);
        crate::sim::combat::fire_error_world::FireSubject {
            world: &sim,
            rules: &rules,
            overlay_registry: None,
            fog: None,
            firer: sim.entities().get(firer).unwrap(),
            obj: rules.object("FIRER").unwrap(),
            target: Some(crate::sim::combat::TargetKind::Entity(target)),
            weapon_index: 0,
        }
        .fire_error(true)
    };
    assert_eq!(
        error(-100, 40),
        FireError::Cant,
        "40 credits cannot pay 100"
    );
    assert_ne!(error(-100, 100), FireError::Cant, "an equal balance pays");
    assert_ne!(error(-100, 250), FireError::Cant, "a surplus pays");
    assert_ne!(
        error(100, 0),
        FireError::Cant,
        "a positive amount never gates the shot"
    );
}

/// Phobos `WarheadTypeExt::ApplyConvert`
/// (`Ext/WarheadType/Detonate.cpp:215,669-678`): the per-target pass converts
/// a Foot through the first applicable `Convert{i}` pair, with `All` as a
/// warhead's default source-house filter.
#[cfg(feature = "phobos")]
#[test]
fn warhead_convert_pairs_convert_the_target_foot() {
    let converted = |wh_ini: &str| {
        let (mut sim, rules, firer, target) = scene("", wh_ini, "", "", "Russians");
        shoot(&mut sim, &rules, "WH", firer, target, 0);
        sim.interner
            .resolve(sim.entities().get(target).unwrap().type_ref())
            .to_owned()
    };
    assert_eq!(
        converted("Convert0.From=TARGET\nConvert0.To=FIRER\n"),
        "FIRER"
    );
    assert_eq!(
        converted("Convert.From=TARGET\nConvert.To=FIRER\n"),
        "FIRER",
        "the unnumbered pair fills an empty list"
    );
    assert_eq!(
        converted("Convert0.From=TARGET\nConvert0.To=FIRER\nConvert0.AffectsHouse=owner\n"),
        "TARGET",
        "owner-only refuses the enemy target"
    );
    assert_eq!(
        converted("Convert0.From=TARGET\nConvert0.To=FIRER\nConvert0.AffectsHouse=enemies\n"),
        "FIRER",
        "enemies accepts the enemy target"
    );
    assert_eq!(
        converted("Convert0.From=FIRER\nConvert0.To=FIRER\n"),
        "TARGET",
        "a known non-matching From skips the pair"
    );
    assert_eq!(
        converted("Convert0.From=NOSUCH\nConvert0.To=FIRER\n"),
        "FIRER",
        "an all-unknown From is an empty list and converts any Foot"
    );
    assert_eq!(
        converted("Convert0.To=NOSUCH\nConvert1.To=FIRER\n"),
        "TARGET",
        "an unknown To ends the numbered list instead of skipping the row"
    );
    assert_eq!(
        converted(
            "Convert0.From=TARGET\nConvert0.To=FIRER\nConvert.From=TARGET\nConvert.To=TARGET\n"
        ),
        "TARGET",
        "the unnumbered pair replaces row 0, so no FIRER row applies"
    );
    assert_eq!(
        converted("Convert0.From=TARGET\nConvert0.To=FIRER\nConvert0.AffectedHouses=owner\n"),
        "TARGET",
        "the deprecated AffectedHouses key is read"
    );
    assert_eq!(
        converted(
            "Convert0.From=TARGET\nConvert0.To=FIRER\nConvert0.AffectedHouses=owner\nConvert0.AffectsHouse=enemies\n"
        ),
        "FIRER",
        "a set AffectsHouse overrides AffectedHouses"
    );
    assert_eq!(
        converted("Convert0.To=FIRER\nConvert1.From=TARGET\nConvert1.To=TARGET\n"),
        "FIRER",
        "the first converting row stops the list"
    );
    assert_eq!(converted(""), "TARGET", "no pairs, no conversion");
    let after = |wh_ini: &str| {
        let (mut sim, rules, firer, target) = scene("", wh_ini, "", "", "Russians");
        sim.substrate
            .entities
            .get_mut(target)
            .unwrap()
            .health
            .set_for_test(100);
        shoot(&mut sim, &rules, "WH", firer, target, 0);
        health(&sim, target)
    };
    assert_eq!(
        after("Convert0.From=TARGET\nConvert0.To=TARGET\n"),
        after(""),
        "a same-type pair is a no-op, not a full-health reset"
    );
}

/// Phobos `WarheadTypeExt::ApplyReturnWarhead`
/// (`Ext/WarheadType/Detonate.cpp:610-636`): the per-target pass sends the
/// `ReturnWarhead=` warhead back at the techno that fired the bullet, gated
/// by the shared chance buffer, the source house and the victim filters.
#[cfg(feature = "phobos")]
#[test]
fn return_warhead_strikes_the_firer_back() {
    let outcome = |wh_ini: &str| {
        let (mut sim, rules, firer, target) =
            scene("1=RH\n", wh_ini, "", "[RH]\nCellSpread=0\n", "Russians");
        let mut expected = sim.scenario_rng.clone();
        // The per-detonation crit and ReturnWarhead buffers.
        expected.random_double();
        expected.random_double();
        let before = health(&sim, firer);
        shoot(&mut sim, &rules, "WH", firer, target, 10);
        (
            before - health(&sim, firer),
            sim.scenario_rng.logical_state() == expected.logical_state(),
        )
    };
    let (damage, _) = outcome("ReturnWarhead=RH\nReturnWarhead.Damage=30\n");
    assert_eq!(damage, 30, "the firer takes the return warhead");
    let (damage, one_draw) = outcome(
        "ReturnWarhead=RH\nReturnWarhead.Damage=30\nReturnWarhead.AffectsTarget=infantry\n",
    );
    assert_eq!(damage, 0, "the victim class filter refuses the vehicle");
    assert!(one_draw, "a refused return only draws the shared buffer");
    let (damage, one_draw) =
        outcome("ReturnWarhead=RH\nReturnWarhead.Damage=30\nReturnWarhead.AffectsHouse=owner\n");
    assert_eq!(damage, 0, "owner-only refuses the enemy source house");
    assert!(one_draw, "a refused return only draws the shared buffer");
    let (damage, _) =
        outcome("ReturnWarhead=RH\nReturnWarhead.Damage=30\nReturnWarhead.FullDetonation=no\n");
    assert_eq!(
        damage, 30,
        "the direct ReceiveDamage arm also hits the firer"
    );
}

/// Phobos `TechnoClass_ReceiveDamage_Shield @ 0x701900`'s first block
/// (`Ext/Techno/Hooks.ReceiveDamage.cpp:26-34`) zeroes the whole packet when
/// the warhead's own triggers refuse the victim. The gate does not set
/// `SkipLowDamageCheck`, so native's `0x7019D8` floor turns that zero packet
/// into one point of damage.
#[test]
fn warhead_trigger_gates_zero_the_received_damage() {
    let damage = |wh_ini: &str, no_invoker: bool, set_health: Option<i32>| {
        let (mut sim, rules, firer, target) = scene("", wh_ini, "", "", "Russians");
        if let Some(value) = set_health {
            sim.substrate
                .entities
                .get_mut(target)
                .unwrap()
                .health
                .set_for_test(value);
        }
        let source = if no_invoker { 0 } else { firer };
        let before = health(&sim, target);
        shoot(&mut sim, &rules, "WH", source, target, 100);
        before - health(&sim, target)
    };

    assert_eq!(damage("", false, None), 100, "control");
    assert_eq!(
        damage("AffectsAbovePercent=0.9\n", false, Some(50)),
        1,
        "10% health refuses the packet, leaving the low-damage floor"
    );
    assert_eq!(
        damage("AffectsAbovePercent=0.9\n", false, None),
        100,
        "full health takes the damage"
    );
    assert_eq!(
        damage("AffectsInvokerOnly=yes\n", false, None),
        1,
        "the firer is not the victim"
    );
    assert_eq!(
        damage(
            "AffectsInvokerOnly=yes\nAffectsInvokerOnly.Reverse=yes\n",
            false,
            None
        ),
        100
    );
    assert_eq!(
        damage("AffectsInvokerOnly=yes\n", true, None),
        1,
        "no invoker and the default global true refuses"
    );
    assert_eq!(
        damage(
            "AffectsInvokerOnly=yes\nAffectsInvokerOnly.IgnoreInvokerState=no\n",
            true,
            None
        ),
        100,
        "IgnoreInvokerState=no admits an invoker-less packet"
    );
}

/// `Crit.Warhead` with `Crit.Warhead.FullDetonation` runs a real detonation of
/// the named warhead at the target, whose own `AnimList` shows in the tail.
#[test]
fn crit_warhead_detonates_at_the_target() {
    let sections = "[CRITWH]\nCellSpread=1\nAnimList=CRITANIM\n";
    let (mut sim, rules, firer, target) = scene(
        "1=CRITWH\n",
        "Crit.Chance=1.0\nCrit.ExtraDamage=30\nCrit.Warhead=CRITWH\n",
        "",
        sections,
        "Russians",
    );
    let (mut plain, plain_rules, plain_firer, plain_target) = scene(
        "1=CRITWH\n",
        "Crit.Warhead=CRITWH\n",
        "",
        sections,
        "Russians",
    );
    assert_eq!(
        rules.warhead("WH").unwrap().crit.warhead(),
        Some("CRITWH"),
        "the crit warhead key reads"
    );
    assert!(
        rules.warhead("CRITWH").is_some(),
        "the crit warhead resolves"
    );
    shoot(&mut sim, &rules, "WH", firer, target, 0);
    shoot(&mut plain, &plain_rules, "WH", plain_firer, plain_target, 0);
    assert_eq!(
        health(&plain, plain_target) - health(&sim, target),
        30,
        "the crit detonated the crit warhead on the target"
    );
}

/// `SelectDamageAnimation_CritAnim`: an active crit replaces the detonation's
/// damage animation with the `Crit.AnimList` entry while
/// `Crit.AnimOnAffectedTargets` is off.
#[test]
fn a_crit_replaces_the_detonation_damage_animation() {
    let (mut sim, rules, _firer, _target) = scene(
        "",
        "Crit.Chance=1.0
Crit.AnimList=CRITICAL
AnimList=NORMAL
",
        "",
        "",
        "Russians",
    );
    let warhead = rules.warhead("WH").expect("warhead");
    let coordinate = ProjectileCoord::new(22 * 256 + 128, 20 * 256 + 128, 0);
    let selected = |sim: &mut Simulation, crit_active: bool| {
        super::detonation_anim::select(sim, &rules, warhead, 10, 0, coordinate, crit_active)
            .map(|id| sim.interner.resolve(id).to_owned())
    };
    assert_eq!(
        selected(&mut sim, false),
        Some("NORMAL".to_owned()),
        "no crit keeps the ordinary damage animation"
    );
    assert_eq!(
        selected(&mut sim, true),
        Some("CRITICAL".to_owned()),
        "Crit_Active swaps in the crit animation"
    );
}

/// `Detonate`'s prefix (`Detonate.cpp:162..170`): a warhead that can crit or
/// carries an AttachEffect list draws the crit buffer (unless
/// `Crit.ApplyChancePerTarget`) and the `ReturnWarhead` buffer, even with no
/// target; `Crit.Chance` alone is not `CanCrit` and draws nothing.
#[test]
fn detonate_prefix_draws_both_buffers_for_crit_or_attach_effect_warheads() {
    let draws = |wh_ini: &str| {
        let (mut sim, rules, firer, target) = scene(
            "",
            wh_ini,
            "",
            "[AttachEffectTypes]\n0=MARK\n[MARK]\nDuration=100\n",
            "Russians",
        );
        detonate_once(&mut sim, &rules, firer, target, None);
        sim.scenario_rng.logical_state()
    };
    assert_eq!(
        draws("Crit.Chance=1.0\nCrit.AnimList=X\n"),
        after_doubles(2),
        "crit buffer, then the ReturnWarhead buffer"
    );
    assert_eq!(
        draws("Crit.Chance=1.0\nCrit.AnimList=X\nCrit.ApplyChancePerTarget=yes\n"),
        after_doubles(1),
        "per-target dice leave only the ReturnWarhead buffer"
    );
    assert_eq!(
        draws("AttachEffect.AttachTypes=MARK\n"),
        after_doubles(2),
        "an AttachEffect list is PossibleCellSpreadDetonate"
    );
    assert_eq!(
        draws("Crit.Chance=1.0\n"),
        after_doubles(0),
        "a chance without damage, warhead or animation cannot crit"
    );
}

/// `CanCrit` does not read `Crit.Chance`: a firer's AttachEffect
/// `Crit.ExtraChance` crits a warhead that leaves the chance unset.
#[test]
fn an_attach_effect_extra_chance_crits_a_zero_chance_warhead() {
    let (mut sim, rules, firer, target) = scene_with_firer(
        "",
        "Crit.ExtraDamage=50\n",
        "",
        "[AttachEffectTypes]\n0=CRIT\n[CRIT]\nDuration=100\nCrit.ExtraChance=1.0\n",
        "AttachEffect.AttachTypes=CRIT\n",
        "Russians",
    );
    sim.update_phobos_attach_effects(firer, &rules);
    shoot(&mut sim, &rules, "WH", firer, target, 0);
    assert_eq!(health(&sim, target), 450, "the folded chance crits");
}

/// `CanAffectTarget`: under `EffectsRequireVerses` (default) a target whose
/// armor the warhead's Verses zero skips the whole per-target pass, so not
/// even a crit warhead with its own Verses reaches it.
#[test]
fn zero_verses_targets_skip_the_per_target_pass() {
    let crit_damage = |extra: &str| {
        let (mut sim, rules, firer, target) = scene(
            "1=CRITWH\n",
            &format!(
                "Verses=0%,0%,0%,0%,0%,0%,0%,0%,0%,0%,0%\nCrit.Chance=1.0\n\
                 Crit.ExtraDamage=30\nCrit.Warhead=CRITWH\n{extra}"
            ),
            "",
            "[CRITWH]\nCellSpread=1\n",
            "Russians",
        );
        shoot(&mut sim, &rules, "WH", firer, target, 0);
        500 - health(&sim, target)
    };
    assert_eq!(crit_damage(""), 0, "0% Verses refuses the target");
    assert_eq!(
        crit_damage("EffectsRequireVerses=no\n"),
        30,
        "without the key the crit warhead lands"
    );
}

/// `BulletClass_Detonate @ 0x46920B` hooks the routine's entry, so a warhead
/// that claims the impact with a special arm (`IvanBomb=yes`) still runs the
/// per-target AttachEffect pass before its arm body, for both the intended
/// target and a cell-spread collection.
#[test]
fn special_arm_warheads_still_run_the_per_target_pass() {
    for cell_spread in ["CellSpread=0", "CellSpread=1"] {
        let (mut sim, rules, firer, target) = scene(
            "",
            &format!("IvanBomb=yes\n{cell_spread}\nAttachEffect.AttachTypes=MARK\n"),
            "",
            "[AttachEffectTypes]\n0=MARK\n[MARK]\nDuration=100\n",
            "Russians",
        );
        shoot(&mut sim, &rules, "WH", firer, target, 0);
        let attached = sim
            .entities()
            .get(target)
            .expect("target")
            .phobos_attach_effects()
            .instances()
            .len();
        assert_eq!(
            attached, 1,
            "the special arm still visited the target ({cell_spread})"
        );
    }
}

/// `MapClass_DamageArea @ 0x489286`: a direct area damage runs the same
/// per-target pass, so a spreading warhead's AttachEffect list reaches its
/// receivers.
#[test]
fn direct_area_damage_applies_the_warhead_attach_effects() {
    let (mut sim, rules, firer, target) = scene(
        "1=AEWH\n",
        "",
        "",
        "[AEWH]\nCellSpread=1\nAttachEffect.AttachTypes=MARK\n\
         [AttachEffectTypes]\n0=MARK\n[MARK]\nDuration=100\n",
        "Russians",
    );
    let warhead = rules.warhead("AEWH").expect("warhead");
    let warhead_ref = sim.interner.intern("AEWH");
    let impact = detonation(&mut sim, "AEWH", firer, target, 0).impact;
    super::world_receiver::apply_area_damage(
        &mut sim,
        &rules,
        None,
        impact,
        10,
        warhead,
        false,
        (firer, None, warhead_ref),
        crate::sim::world::FrameEffects::empty(),
    );
    let attached = sim
        .entities()
        .get(target)
        .expect("target")
        .phobos_attach_effects()
        .instances()
        .len();
    assert_eq!(attached, 1, "the area's receiver got the effect");
}

/// `SelectDamageAnimation_CritAnim` (`0x48A5B3`) runs before the `AnimList`
/// empty test, so a warhead with only `Crit.AnimList` still shows it; the
/// `0x48A4F3` hook selects a negative packet by its magnitude.
#[test]
fn crit_anim_list_shows_without_an_anim_list() {
    let (mut sim, rules, _firer, _target) = scene(
        "",
        "Crit.Chance=1.0\nCrit.AnimList=LOW,HIGH\n",
        "",
        "",
        "Russians",
    );
    let warhead = rules.warhead("WH").expect("warhead");
    let coordinate = ProjectileCoord::new(22 * 256 + 128, 20 * 256 + 128, 0);
    let mut selected = |damage: i32| {
        super::detonation_anim::select(&mut sim, &rules, warhead, damage, 0, coordinate, true)
            .map(|id| sim.interner.resolve(id).to_owned())
    };
    assert_eq!(
        selected(30),
        Some("HIGH".to_owned()),
        "30 is the second band"
    );
    assert_eq!(
        selected(-10),
        Some("LOW".to_owned()),
        "-10 selects as 10, not as a huge unsigned value"
    );
}

/// `Crit.Warhead.FullDetonation=no`: `DamageAreaWithTarget` runs the crit
/// warhead's area damage without a bullet, so the target takes its damage and
/// no detonation animation is selected.
#[test]
fn crit_warhead_area_arm_damages_the_target() {
    let sections = "[CRITWH]\nCellSpread=1\n";
    let (mut sim, rules, firer, target) = scene(
        "1=CRITWH\n",
        "Crit.Chance=1.0\nCrit.ExtraDamage=30\nCrit.Warhead=CRITWH\n\
         Crit.Warhead.FullDetonation=no\n",
        "",
        sections,
        "Russians",
    );
    let (mut plain, plain_rules, plain_firer, plain_target) = scene(
        "1=CRITWH\n",
        "Crit.Warhead=CRITWH\nCrit.Warhead.FullDetonation=no\n",
        "",
        sections,
        "Russians",
    );
    shoot(&mut sim, &rules, "WH", firer, target, 0);
    shoot(&mut plain, &plain_rules, "WH", plain_firer, plain_target, 0);
    assert_eq!(
        health(&plain, plain_target) - health(&sim, target),
        30,
        "the crit warhead's area damage reached the target"
    );
}

/// The `CRITIMMUNE` Phobos rank ability and a `Crit.AffectsTarget` that does
/// not list the target's class each stop the same always-crit warhead.
#[test]
fn crit_immune_ability_and_target_filter_stop_the_crit() {
    let crit_damage = |wh_ini: &str, target_ini: &str, veteran: bool| {
        let (mut sim, rules, firer, target) = scene("", wh_ini, target_ini, "", "Russians");
        let (mut plain, plain_rules, plain_firer, plain_target) =
            scene("", "", target_ini, "", "Russians");
        if veteran {
            for (world, id) in [(&mut sim, target), (&mut plain, plain_target)] {
                world
                    .substrate
                    .entities
                    .get_mut(id)
                    .unwrap()
                    .veterancy
                    .set_veteran();
            }
        }
        shoot(&mut sim, &rules, "WH", firer, target, 0);
        shoot(&mut plain, &plain_rules, "WH", plain_firer, plain_target, 0);
        health(&plain, plain_target) - health(&sim, target)
    };

    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\n",
            "VeteranAbilities=CRITIMMUNE\n",
            true,
        ),
        0,
        "a veteran with CRITIMMUNE refuses the crit"
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\nCrit.AffectsTarget=aircraft\n",
            "",
            false,
        ),
        0,
        "an aircraft-only filter skips a ground unit"
    );
    assert_eq!(
        crit_damage(
            "Crit.Chance=1.0\nCrit.ExtraDamage=50\n",
            "VeteranAbilities=CRITIMMUNE\n",
            false,
        ),
        50,
        "a rookie with the same list is not immune"
    );
}

/// `Crit.ActiveChanceAnims` picks one entry with a `RandomRanged(0, len-1)`
/// draw after the crit and `ReturnWarhead` buffers.
#[test]
fn crit_active_chance_anims_draw_after_the_buffers() {
    let (mut sim, rules, firer, target) = scene(
        "",
        "Crit.Chance=1.0\nCrit.ExtraDamage=0\nCrit.ActiveChanceAnims=ONE,TWO\n",
        "",
        "",
        "Russians",
    );
    detonate_once(&mut sim, &rules, firer, target, None);
    let mut expected = {
        let (sim, _rules, _firer, _target) = scene("", "", "", "", "Russians");
        sim.scenario_rng.clone()
    };
    expected.random_double();
    expected.random_double();
    expected.next_range_i32_inclusive(0, 1);
    assert_eq!(
        sim.scenario_rng.logical_state(),
        expected.logical_state(),
        "two buffers, then one anim pick"
    );
}

/// `CRITCHANCE` + `VeteranCritChance` scale the warhead chance; the same
/// rules without the rank ability keep the base chance.
#[test]
fn crit_chance_rank_ability_scales_the_warhead_chance() {
    let scene_for = |firer_ini: &str, veteran: bool| {
        let (mut sim, rules, firer, target) = scene_with_firer(
            "",
            "Crit.Chance=0.05
Crit.ExtraDamage=50
",
            "",
            "",
            firer_ini,
            "Russians",
        );
        if veteran {
            sim.substrate
                .entities
                .get_mut(firer)
                .unwrap()
                .veterancy
                .set_veteran();
        }
        // The shot's dice must miss the base 0.05 chance.
        let mut probe = sim.scenario_rng.clone();
        assert!(probe.random_double() > 0.05);
        (sim, rules, firer, target)
    };
    let control_health = |firer_ini: &str, veteran: bool| {
        let (mut sim, rules, firer, target) = scene_for(firer_ini, veteran);
        shoot(&mut sim, &rules, "WH", firer, target, 0);
        health(&sim, target)
    };

    let control = control_health("", false);
    let (mut sim, rules, firer, target) = scene_for(
        "VeteranAbilities=CRITCHANCE
VeteranCritChance=20
",
        true,
    );
    shoot(&mut sim, &rules, "WH", firer, target, 0);
    assert_eq!(
        control - health(&sim, target),
        50,
        "0.05 x 20 reaches a certain crit"
    );

    let (mut sim, rules, firer, target) = scene_for(
        "VeteranCritChance=20
",
        true,
    );
    shoot(&mut sim, &rules, "WH", firer, target, 0);
    assert_eq!(
        control - health(&sim, target),
        0,
        "without the ability the chance stays at 0.05"
    );
}

/// `Crit.ExtraDamage.ApplyFirepowerMult` scales the extra damage by the
/// firer's current firepower multiplier (here its FIREPOWER rank stage).
#[test]
fn crit_extra_damage_can_apply_the_firepower_multiplier() {
    let (mut sim, rules, firer, target) = scene_with_firer(
        "",
        "Crit.Chance=1.0\nCrit.ExtraDamage=50\nCrit.ExtraDamage.ApplyFirepowerMult=yes\n",
        "",
        "[General]\nVeteranCombat=2\n",
        "VeteranAbilities=FIREPOWER\n",
        "Russians",
    );
    sim.substrate
        .entities
        .get_mut(firer)
        .unwrap()
        .veterancy
        .set_veteran();
    let (mut plain, plain_rules, plain_firer, plain_target) = scene("", "", "", "", "Russians");
    shoot(&mut sim, &rules, "WH", firer, target, 0);
    shoot(&mut plain, &plain_rules, "WH", plain_firer, plain_target, 0);
    assert_eq!(
        health(&plain, plain_target) - health(&sim, target),
        100,
        "50 extra damage times the veteran FIREPOWER 2"
    );
}

/// `Crit.AnimOnAffectedTargets` keeps the ordinary damage animation even when
/// the crit applies its own animation at the target.
#[test]
fn crit_affected_target_anims_keep_the_ordinary_damage_animation() {
    let (mut sim, rules, _firer, _target) = scene(
        "",
        "Crit.Chance=1.0\nCrit.AnimList=CRITICAL\nAnimList=NORMAL\n\
         Crit.AnimOnAffectedTargets=yes\n",
        "",
        "",
        "Russians",
    );
    let warhead = rules.warhead("WH").expect("warhead");
    let coordinate = ProjectileCoord::new(22 * 256 + 128, 20 * 256 + 128, 0);
    assert_eq!(
        super::detonation_anim::select(&mut sim, &rules, warhead, 10, 0, coordinate, true)
            .map(|id| sim.interner.resolve(id).to_owned()),
        Some("NORMAL".to_owned()),
        "the crit list belongs to the affected targets"
    );
}

/// `Crit.Warhead` is read with `Read<true>`: a warhead only the key names is
/// still allocated and detonates.
#[test]
fn crit_warhead_allocates_an_unlisted_type() {
    let sections = "[CRITWH]\nCellSpread=1\n";
    let (mut sim, rules, firer, target) = scene(
        "",
        "Crit.Chance=1.0\nCrit.ExtraDamage=30\nCrit.Warhead=CRITWH\n",
        "",
        sections,
        "Russians",
    );
    assert!(
        rules.warhead("CRITWH").is_some(),
        "the key allocates the type"
    );
    let (mut plain, plain_rules, plain_firer, plain_target) = scene("", "", "", "", "Russians");
    shoot(&mut sim, &rules, "WH", firer, target, 0);
    shoot(&mut plain, &plain_rules, "WH", plain_firer, plain_target, 0);
    assert_eq!(
        health(&plain, plain_target) - health(&sim, target),
        30,
        "the allocated crit warhead detonated"
    );
}

/// Phobos `ApplyPenetratesTransport` (`Ext/WarheadType/Detonate.cpp:780-930`):
/// a warhead level above the transport's reaches its passengers with the
/// bullet's Health; `DamageAll=yes` hits both, `no` picks one, and a level at
/// or below the transport's skips the arm.
fn penetrates_scene(
    transport_level: i32,
    damage_all: bool,
) -> (Simulation, RuleSet, u64, u64, u64, u64) {
    let rules = RuleSet::from_ini(&IniFile::from_str(&format!(
        "[Countries]\n0=Americans\n1=Russians\n\
         [VehicleTypes]\n0=FIRER\n1=APC\n\
         [InfantryTypes]\n0=GI\n\
         [WeaponTypes]\n0=GUN\n\
         [GUN]\nDamage=0\nRange=5\nROF=20\nWarhead=WH\n\
         [Warheads]\n0=WH\n\
         [WH]\nCellSpread=0\nPenetratesTransport.Level=15\n\
         PenetratesTransport.DamageAll={}\n\
         [FIRER]\nStrength=500\nArmor=none\nSpeed=6\nPrimary=GUN\n\
         [APC]\nStrength=500\nArmor=none\nSpeed=6\nPassengers=2\n\
         PenetratesTransport.Level={transport_level}\n\
         [GI]\nStrength=100\nArmor=none\nSpeed=4\n",
        if damage_all { "yes" } else { "no" }
    )))
    .expect("penetrates rules");
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 0x5A_AA_1E);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let firer = sim
        .spawn_object("FIRER", "Americans", 20, 20, 0, &rules)
        .expect("firer");
    let apc = sim
        .spawn_object("APC", "Russians", 22, 20, 0, &rules)
        .expect("transport");
    let gi1 = sim
        .spawn_object("GI", "Russians", 22, 21, 0, &rules)
        .expect("passenger");
    let gi2 = sim
        .spawn_object("GI", "Russians", 22, 19, 0, &rules)
        .expect("passenger");
    sim.substrate
        .entities
        .get_mut(apc)
        .expect("transport")
        .passenger_role
        .cargo_mut()
        .expect("transport cargo")
        .set_passengers_for_test(vec![gi1, gi2]);
    (sim, rules, firer, apc, gi1, gi2)
}

#[test]
fn penetrates_transport_damages_the_selected_passengers() {
    let (mut sim, rules, firer, apc, gi1, gi2) = penetrates_scene(1, true);
    shoot(&mut sim, &rules, "WH", firer, apc, 20);
    assert_eq!(health(&sim, gi1), 80, "DamageAll damages every passenger");
    assert_eq!(health(&sim, gi2), 80);

    let (mut sim, rules, firer, apc, gi1, gi2) = penetrates_scene(1, false);
    shoot(&mut sim, &rules, "WH", firer, apc, 20);
    let hits = usize::from(health(&sim, gi1) == 80) + usize::from(health(&sim, gi2) == 80);
    assert_eq!(hits, 1, "DamageAll=no picks exactly one passenger");

    let (mut sim, rules, firer, apc, gi1, gi2) = penetrates_scene(15, true);
    shoot(&mut sim, &rules, "WH", firer, apc, 20);
    assert_eq!(
        (health(&sim, gi1), health(&sim, gi2)),
        (100, 100),
        "a transport at the warhead's level is not penetrated"
    );
}
