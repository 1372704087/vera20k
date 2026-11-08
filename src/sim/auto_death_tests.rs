//! Phobos `AutoDeath`'s timed arm (`Ext/Techno/Body.Update.cpp:173-184`) driven
//! through the production AI visit, plus the `FootExt::UpdateTypeData` reset
//! (`Ext/Foot/Body.cpp:219-221`).
use crate::rules::ruleset::RuleSet;
use crate::sim::world::Simulation;

const RULES: &str = "\
[InfantryTypes]\n0=E1\n\
[VehicleTypes]\n0=TANK\n1=TANKONLYDELAY\n2=TANKAMMO\n3=TANKPOWER\n4=TANKMONEY\n\
[E1]\nStrength=125\nSight=0\n\
[TANK]\nStrength=100\nSight=0\nSpeed=5\nAutoDeath.Behavior=kill\nAutoDeath.AfterDelay=3\n\
[TANKONLYDELAY]\nStrength=100\nSight=0\nSpeed=5\nAutoDeath.AfterDelay=1\n\
[TANKAMMO]\nStrength=100\nSight=0\nSpeed=5\nAmmo=1\n\
AutoDeath.Behavior=kill\nAutoDeath.OnAmmoDepletion=yes\n\
[TANKPOWER]\nStrength=100\nSight=0\nSpeed=5\n\
AutoDeath.Behavior=kill\nAutoDeath.PlayerPowerState=low\n\
[TANKMONEY]\nStrength=100\nSight=0\nSpeed=5\n\
AutoDeath.Behavior=kill\nAutoDeath.PlayerMoney.Max=50\n\
[Warheads]\n0=C4WH\n\
[C4WH]\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n\
[CombatDamage]\nC4Warhead=C4WH\n\
[Projectiles]\n0=SHOT\n[SHOT]\nInviso=yes\n";

fn arena() -> (Simulation, RuleSet) {
    let rules = RuleSet::from_ini(&crate::sim::arena_fixture::clear_ground_ini(RULES)).unwrap();
    let mut sim = Simulation::new();
    crate::sim::arena_fixture::flat_ground(&mut sim, &rules);
    (sim, rules)
}

fn alive(sim: &Simulation, id: u64) -> bool {
    sim.substrate
        .entities
        .get(id)
        .is_some_and(|entity| entity.lifecycle.object_alive())
}

fn tick(sim: &mut Simulation, rules: &RuleSet, count: usize) {
    for _ in 0..count {
        sim.advance_tick(&[], Some(rules), None, None, 22);
    }
}

/// `:175-183`: the first visit only starts `AutoDeathTimer`, and the object
/// dies once the countdown completes. The C4 warhead carries the object's own
/// Health (`KillSelf` `:820`), so the object is gone by the time it expires.
#[test]
fn a_timed_auto_death_starts_its_clock_then_kills_the_object() {
    let (mut sim, rules) = arena();
    let tank = sim
        .spawn_object("TANK", "Americans", 10, 10, 0, &rules)
        .expect("TANK spawns");
    assert!(alive(&sim, tank));
    tick(&mut sim, &rules, 1);
    assert!(
        alive(&sim, tank),
        "the first visit only starts the countdown"
    );
    // Two more frames are one short of the three-frame countdown.
    tick(&mut sim, &rules, 2);
    assert!(alive(&sim, tank), "the frame before expiry still lives");
    tick(&mut sim, &rules, 1);
    assert!(
        !alive(&sim, tank),
        "AfterDelay=3 completes on the third frame after the clock starts"
    );
}

/// `:165-170`: a type whose `Ammo=` capacity is above zero dies at zero.
#[test]
fn a_type_with_ammo_dies_when_the_ammo_is_gone() {
    let (mut sim, rules) = arena();
    let tank = sim
        .spawn_object("TANKAMMO", "Americans", 10, 10, 0, &rules)
        .expect("TANKAMMO spawns");
    sim.session.set_binary_frame_for_test(5);
    sim.object_ai_stage(Some(&rules));
    assert!(alive(&sim, tank), "Ammo=1 is not depleted yet");
    sim.substrate
        .entities
        .get_mut(tank)
        .unwrap()
        .ammo_mut()
        .expect("Ammo=1 installs a counter")
        .set_current_for_test(0);
    sim.session.set_binary_frame_for_test(6);
    sim.object_ai_stage(Some(&rules));
    assert!(!alive(&sim, tank), "zero ammo triggers the arm");
}

/// `:210-221`: the retained low-power flag. The first-frame guard binds to the
/// `Low` arm only, so frame 5 is enough for it to fire.
#[test]
fn a_low_power_house_kills_its_auto_death_type() {
    let (mut sim, rules) = arena();
    let tank = sim
        .spawn_object("TANKPOWER", "Americans", 10, 10, 0, &rules)
        .expect("TANKPOWER spawns");
    let owner = sim.substrate.entities.get(tank).unwrap().owner();
    sim.power_states_mut_for_test()
        .entry(owner)
        .or_default()
        .set_is_low_power_for_test(true);
    sim.session.set_binary_frame_for_test(5);
    sim.object_ai_stage(Some(&rules));
    assert!(!alive(&sim, tank), "a low-power house kills the type");
}

/// `:223-234`: a closed money range kills, and the same type survives once the
/// balance is above `PlayerMoney.Max`.
#[test]
fn the_owner_money_range_kills_or_spares() {
    for (credits, expected_alive) in [(0, false), (100, true)] {
        let (mut sim, rules) = arena();
        let tank = sim
            .spawn_object("TANKMONEY", "Americans", 10, 10, 0, &rules)
            .expect("TANKMONEY spawns");
        let owner = sim.substrate.entities.get(tank).unwrap().owner();
        sim.houses_mut_for_test()
            .register(crate::sim::house_state::HouseState::new(
                owner, 0, None, true, 0, 0,
            ));
        sim.houses_mut_for_test()
            .get_mut(&owner)
            .expect("house")
            .economy_mut_for_test()
            .set_credits_for_test(credits);
        sim.session.set_binary_frame_for_test(5);
        sim.object_ai_stage(Some(&rules));
        assert_eq!(
            alive(&sim, tank),
            expected_alive,
            "PlayerMoney.Max=50 with a balance of {credits}"
        );
    }
}

/// `Ext/Techno/Body.Visuals.cpp:696-717`: the digital display shows the
/// clock's remaining time over its full duration, and a type without the
/// mechanism draws nothing.
#[test]
fn the_digital_display_reports_the_remaining_countdown() {
    let (mut sim, rules) = arena();
    let tank = sim
        .spawn_object("TANK", "Americans", 10, 10, 0, &rules)
        .expect("TANK spawns");
    let delay = sim
        .spawn_object("TANKONLYDELAY", "Americans", 12, 10, 0, &rules)
        .expect("TANKONLYDELAY spawns");
    // Start the three-frame clock at frame 10, then read it two frames later.
    sim.session.set_binary_frame_for_test(10);
    sim.object_ai_stage(Some(&rules));
    sim.session.set_binary_frame_for_test(12);
    let read = |sim: &Simulation, id: u64| {
        let entity = sim.substrate.entities.get(id).unwrap();
        let object = sim
            .interner
            .object_type(entity.type_ref(), &rules)
            .unwrap()
            .clone();
        crate::sim::digital_display::values_for_display(
            sim,
            entity,
            &object,
            &rules,
            crate::rules::digital_display::DigitalInfoType::AutoDeath,
            0,
            None,
        )
    };
    assert_eq!(
        read(&sim, tank),
        Some((1, 3)),
        "GetTimeLeft over TimeLeft two frames into a three-frame countdown"
    );
    assert_eq!(
        read(&sim, delay),
        None,
        "AfterDelay with no Behavior never draws an AutoDeath countdown"
    );
}

/// `:142`: an unset `AutoDeath.Behavior` returns before the countdown, so
/// `AutoDeath.AfterDelay=` alone never arms anything.
#[test]
fn after_delay_without_a_behavior_never_arms_the_clock() {
    let (mut sim, rules) = arena();
    let tank = sim
        .spawn_object("TANKONLYDELAY", "Americans", 10, 10, 0, &rules)
        .expect("TANKONLYDELAY spawns");
    tick(&mut sim, &rules, 8);
    assert!(
        alive(&sim, tank),
        "AfterDelay=1 with no Behavior must never start a countdown"
    );
}

/// `Ext/Foot/Body.cpp:219-221` through `TechnoClass::ChangeType`. VERA reaches
/// that owner only from the Ares `Convert.Water=` surface conversion, so the
/// test drives a real shoreline crossing: a timed unit converts to an untimed
/// type and must not carry its countdown across. The stale clock would complete
/// two frames after the crossing and kill the converted unit.
#[cfg(feature = "ares")]
#[test]
fn an_ares_surface_conversion_stops_the_countdown() {
    let ini = crate::sim::arena_fixture::clear_ground_ini(
        "[Countries]\n0=Americans\n1=Russians\n[VehicleTypes]\n0=LAND\n1=WATER\n\
         [LAND]\nStrength=100\nSpeed=5\nSpeedType=Hover\nMovementZone=Amphibious\n\
         Convert.Water=WATER\nAutoDeath.Behavior=kill\nAutoDeath.AfterDelay=5\n\
         Locomotor={4A582742-9839-11d1-B709-00A024DDAFD1}\n\
         [WATER]\nStrength=200\nSpeed=5\nSpeedType=Hover\nMovementZone=Amphibious\n\
         Convert.Land=LAND\nConvert.Water=WATER\n\
         Locomotor={4A582742-9839-11d1-B709-00A024DDAFD1}\n\
         [Warheads]\n0=C4WH\n\
         [C4WH]\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n\
         [CombatDamage]\nC4Warhead=C4WH\n",
    );
    let rules = RuleSet::from_ini(&ini).unwrap();
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 29);
    let id = sim
        .spawn_object("LAND", "Americans", 10, 10, 0, &rules)
        .expect("LAND spawns");
    sim.substrate
        .entities
        .get_mut(id)
        .unwrap()
        .locomotor
        .as_mut()
        .unwrap()
        .power_off();
    // Frames 100..104 on land start the five-frame countdown (start 100,
    // expires 105).
    for frame in 100..104 {
        sim.session.set_binary_frame_for_test(frame);
        sim.object_ai_stage(Some(&rules));
    }
    assert_eq!(
        sim.interner
            .resolve(sim.substrate.entities.get(id).unwrap().type_ref()),
        "LAND",
        "the unit is still on land"
    );
    // Cross to water at 104 and stay well past the countdown: the untimed
    // WATER type never runs a check, so only the conversion reset can clear
    // the clock.
    sim.resolved_terrain
        .as_mut()
        .unwrap()
        .cell_mut_for_test(10, 10)
        .unwrap()
        .land_type = crate::rules::terrain_rules::LandType::Water.as_index();
    for frame in 104..110 {
        sim.session.set_binary_frame_for_test(frame);
        sim.object_ai_stage(Some(&rules));
    }
    assert_eq!(
        sim.interner
            .resolve(sim.substrate.entities.get(id).unwrap().type_ref()),
        "WATER",
        "the crossing converted the unit"
    );
    // Back to land at 110. Without the reset the timer is long expired and the
    // first timed visit kills the unit immediately; a reset one restarts here
    // and leaves four frames.
    sim.resolved_terrain
        .as_mut()
        .unwrap()
        .cell_mut_for_test(10, 10)
        .unwrap()
        .land_type = crate::rules::terrain_rules::LandType::Clear.as_index();
    for frame in 110..114 {
        sim.session.set_binary_frame_for_test(frame);
        sim.object_ai_stage(Some(&rules));
    }
    assert_eq!(
        sim.interner
            .resolve(sim.substrate.entities.get(id).unwrap().type_ref()),
        "LAND",
        "the unit converted back"
    );
    assert!(
        alive(&sim, id),
        "a returning timed type must start a fresh countdown, not inherit a completed one"
    );
}
