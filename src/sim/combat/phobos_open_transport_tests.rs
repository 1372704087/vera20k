//! Phobos `OpenTransport.*` rider-side values through the production owners:
//! the `0x6FE43B` damage factor and the `0x6F72D2` range bonus.

use super::{in_range, world_fire};
use crate::rules::ruleset::RuleSet;
use crate::sim::world::Simulation;

/// A carrier `CARR` (open-topped) with a `RIDER` inside it. `carrier_key` and
/// `rider_key` are the extra keys under test.
fn scene(carrier_key: &str, rider_key: &str) -> (Simulation, RuleSet, u64) {
    let ini = crate::sim::arena_fixture::clear_ground_ini(&format!(
        "[Countries]\n0=Americans\n1=Russians\n\
         [VehicleTypes]\n0=CARR\n1=RIDER\n\
         [WeaponTypes]\n0=GUN\n\
         [GUN]\nDamage=5\nROF=40\nRange=9.5\nSpeed=100\nProjectile=INVIS\n\
         Warhead=HIT\nFireInTransport=yes\n\
         [INVIS]\nInviso=yes\nAG=yes\n\
         [HIT]\nCellSpread=0\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n\
         [CARR]\nStrength=800\nArmor=heavy\nSpeed=5\nPassengers=1\nOpenTopped=yes\n{carrier_key}\
         [RIDER]\nStrength=200\nArmor=none\nSpeed=4\nPrimary=GUN\nOmniFire=yes\n{rider_key}\
         [CombatDamage]\nOpenToppedDamageMultiplier=4\n"
    ));
    let rules = RuleSet::from_ini(&ini).expect("rules");
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 0x0D_EC_10);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.session.set_binary_frame_for_test(100);
    let carrier = sim
        .spawn_object("CARR", "Americans", 10, 10, 0, &rules)
        .expect("carrier");
    let rider = sim
        .spawn_object("RIDER", "Americans", 12, 10, 0, &rules)
        .expect("rider");
    for id in [carrier, rider] {
        sim.entities_mut_for_test()
            .get_mut(id)
            .expect("placed object")
            .lifecycle
            .set_in_limbo_for_test(false);
    }
    sim.entities_mut_for_test()
        .get_mut(rider)
        .expect("rider")
        .passenger_role
        .set_inside_transport(carrier, true);
    (sim, rules, rider)
}

#[test]
fn rider_damage_multiplier_multiplies_the_carrier_value() {
    let (sim, rules, rider) = scene(
        "OpenTopped.DamageMultiplier=1.5\n",
        "OpenTransport.DamageMultiplier=2\n",
    );
    let entity = sim.entities().get(rider).expect("rider");
    let mult = world_fire::open_topped_damage_multiplier(
        Some(entity),
        &rules,
        &sim.substrate.entities,
        &sim.interner,
    );
    assert_eq!(mult, 3.0, "carrier 1.5 x rider 2");

    let (sim, rules, rider) = scene("", "OpenTransport.DamageMultiplier=2\n");
    let entity = sim.entities().get(rider).expect("rider");
    assert_eq!(
        world_fire::open_topped_damage_multiplier(
            Some(entity),
            &rules,
            &sim.substrate.entities,
            &sim.interner,
        ),
        8.0,
        "the unset carrier takes the YR global 4, then the rider 2"
    );

    let (sim, rules, rider) = scene("", "");
    let entity = sim.entities().get(rider).expect("rider");
    assert_eq!(
        world_fire::open_topped_damage_multiplier(
            Some(entity),
            &rules,
            &sim.substrate.entities,
            &sim.interner,
        ),
        4.0,
        "unset keys keep the YR global"
    );
}

#[test]
fn passenger_fire_while_moving_reads_the_rider_type() {
    let (sim, rules, rider) = scene("", "OpenTransport.FireWhileMoving=no\n");
    let entity = sim.entities().get(rider).expect("rider");
    assert!(
        !super::fire_error_world::passenger_fire_while_moving(entity, &rules, &sim.interner),
        "the rider's own key is read"
    );
    // The carrier's key must not stand in for the passenger's.
    let (sim, rules, rider) = scene("OpenTopped.FireWhileMoving=no\n", "");
    let entity = sim.entities().get(rider).expect("rider");
    assert!(
        super::fire_error_world::passenger_fire_while_moving(entity, &rules, &sim.interner),
        "an unset rider key uses the [General] default, not the carrier's key"
    );
}

#[test]
fn rider_range_bonus_adds_to_the_carrier_bonus() {
    let (sim, rules, rider) = scene("OpenTopped.RangeBonus=2\n", "OpenTransport.RangeBonus=1\n");
    let entity = sim.entities().get(rider).expect("rider");
    assert_eq!(
        in_range::open_topped_range_bonus_leptons(
            entity,
            &rules,
            Some(&sim.substrate.entities),
            &sim.interner,
        ),
        3 << 8,
        "carrier 2 + rider 1 cells"
    );
}
