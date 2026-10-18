//! Ordinary retail firing and original Techno6FD620 mode0 birth comparisons.
//! The native component supplies declared inputs and independent expected
//! fields; the production FLH/depth owners remain the only implementation.

use crate::rules::ini_parser::IniFile;
use crate::rules::retail_ini_fixture::RetailBattleRules;
use crate::rules::ruleset::RuleSet;
use crate::sim::command::Command;
use crate::sim::components::DriveCoord;
use crate::sim::projectile::{ProjectileCoord, ProjectileTarget};
use crate::sim::rng::SimRng;
use crate::sim::vision::FogState;
use crate::sim::world::{LifecycleOutput, Simulation};
use serde_json::Value;

fn native() -> &'static Value {
    crate::test_fixture::json("tools/procedural_drawing_oracle/rad_beam.json")
}

fn integer(value: &Value) -> i32 {
    i32::try_from(value.as_i64().unwrap()).unwrap()
}

fn xyz(value: &Value) -> [i32; 3] {
    std::array::from_fn(|axis| integer(&value[axis]))
}

fn rgb(value: &Value) -> [u8; 3] {
    std::array::from_fn(|channel| u8::try_from(integer(&value[channel])).unwrap())
}

fn coordinate(value: &Value) -> ProjectileCoord {
    let [x, y, z] = xyz(value);
    ProjectileCoord::new(x, y, z)
}

fn retail() -> Option<RetailBattleRules> {
    let map = crate::test_fixture::checkout_root()
        .join("tools/procedural_drawing_oracle/rad_beam/ordinary-deso-radbeam.map");
    crate::rules::retail_ini_fixture::retail_battle_rules_for_map(map.to_str().unwrap())
}

#[test]
fn omitted_rad_color_uses_the_original_black_constructor_default() {
    // Rules665663 zeroes EBX;6677EF/F5/FB seed the three bytes. Physical
    // stock green is an explicit Radiation/RadColor read, not this default.
    let rules =
        crate::rules::ruleset::RuleSet::from_ini(&IniFile::from_str("")).expect("default rules");
    assert_eq!(rules.radiation.color, (0, 0, 0));
    assert_eq!(
        [
            rules.radiation.color.0,
            rules.radiation.color.1,
            rules.radiation.color.2
        ],
        rgb(&native()["inputs"]["constructor"]["rad_color"]),
        "original Rules constructor"
    );
    // Only the two missing-key controls belong to this constructor-default
    // regression. Ordered retained-value histories remain a separate owner.
    for row in native()["color_controls"].as_array().unwrap() {
        let name = row["name"].as_str().unwrap();
        if !matches!(name, "absent_section" | "present_missing_key") {
            continue;
        }
        let rules = RuleSet::from_ini(&crate::test_fixture::ini_sections(&row["sections"]))
            .expect("declared native color input");
        assert_eq!(
            [
                rules.radiation.color.0,
                rules.radiation.color.1,
                rules.radiation.color.2
            ],
            rgb(&row["rgb"]),
            "{name} native current constructor default"
        );
    }
}

#[test]
fn retail_deso_attack_emits_a_copied_straight_radiation_beam() {
    let Some(retail) = retail() else {
        return;
    };
    let rules = &retail.rules;
    let deso = rules.object("DESO").expect("retail DESO");
    assert_eq!(deso.primary(), Some("RadBeamWeapon"));
    let weapon = rules.weapon("RadBeamWeapon").unwrap();
    assert!(weapon.is_rad_beam);
    assert!(!weapon.is_laser);
    assert!(!weapon.is_electric_bolt);
    assert!(
        !rules
            .warhead(weapon.warhead.as_deref().unwrap())
            .unwrap()
            .temporal
    );
    assert_eq!(rules.radiation.color, (0, 255, 0));

    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 0xC29D_E500);
    sim.fog = FogState::new(32, 32);
    let owner = sim.interner.intern("Americans");
    sim.session.current_house = Some(owner);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    let source = sim
        .spawn_object("DESO", "Americans", 16, 16, 64, rules)
        .unwrap();
    let target = sim
        .spawn_object("MTNK", "Russians", 20, 16, 192, rules)
        .unwrap();
    sim.fog.mark_visible_for_owner(owner, 20, 16);
    assert_eq!(
        sim.substrate.entities.get(source).unwrap().health.current(),
        deso.strength
    );
    assert!(sim.apply_command(
        "Americans",
        &Command::Attack {
            attacker_id: source,
            target_id: target
        },
        Some(rules),
    ));
    sim.lifecycle_outputs.clear();
    let grid = sim.path_grid_snapshot().unwrap();
    for _ in 0..32 {
        sim.advance_tick(&[], Some(rules), Some(&grid), None, 67);
        if let Some(shot) = sim
            .fire_events
            .iter()
            .find(|shot| shot.attacker_id == source)
        {
            assert_eq!(sim.interner.resolve(shot.weapon_id), "RadBeamWeapon");
            let beams: Vec<_> = sim
                .lifecycle_outputs
                .iter()
                .filter_map(|output| {
                    if let LifecycleOutput::RadiationBeamCreated(birth) = output {
                        Some(birth)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(
                beams.len(),
                1,
                "native FireAt6FF5C6 must create its straight beam on the ordinary shot"
            );
            let beam = beams[0];
            assert_ne!(beam.from, beam.to);
            assert_eq!(beam.color, [0, 255, 0]);
            assert_eq!(beam.duration, 15);
            assert_eq!(beam.amplitude.bits(), 40.0_f64.to_bits());
            return;
        }
    }
    panic!("ordinary in-range retail DESO did not fire within32 ticks");
}

#[test]
fn retail_deso_selected_readers_match_the_native_mode0_inputs() {
    let Some(retail) = retail() else {
        return;
    };
    let rules = &retail.rules;
    let expected = &native()["inputs"]["final"];
    let deso = rules.object("DESO").unwrap();
    assert_eq!(deso.primary(), expected["deso"]["primary"].as_str());
    assert_eq!(
        deso.elite_primary(),
        expected["deso"]["elite_primary"].as_str()
    );
    assert_eq!(deso.strength, integer(&expected["deso"]["strength"]));
    let art = super::fire_coord::firer_art(rules, deso).unwrap();
    assert_eq!(
        <[i32; 3]>::from(art.primary_fire_flh),
        xyz(&expected["deso"]["primary_flh"])
    );
    assert_eq!(
        art.turret_offset,
        integer(&expected["deso"]["turret_offset"])
    );
    assert_eq!(
        [
            rules.radiation.color.0,
            rules.radiation.color.1,
            rules.radiation.color.2
        ],
        rgb(&expected["rad_color"])
    );
    for (name, expected) in expected["weapons"].as_object().unwrap() {
        let weapon = rules.weapon(name).unwrap();
        assert_eq!(weapon.damage, integer(&expected["damage"]), "{name} Damage");
        assert_eq!(
            weapon.range_leptons,
            integer(&expected["range"]),
            "{name} Range"
        );
        assert_eq!(
            weapon.projectile.as_deref(),
            expected["projectile"].as_str(),
            "{name} Projectile"
        );
        assert_eq!(
            weapon.warhead.as_deref(),
            expected["warhead"].as_str(),
            "{name} Warhead"
        );
        assert_eq!(
            weapon.is_laser,
            expected["is_laser"].as_bool().unwrap(),
            "{name} IsLaser"
        );
        assert_eq!(
            weapon.is_electric_bolt,
            expected["is_electric_bolt"].as_bool().unwrap(),
            "{name} IsElectricBolt"
        );
        assert_eq!(
            weapon.is_rad_beam,
            expected["is_rad_beam"].as_bool().unwrap(),
            "{name} IsRadBeam"
        );
        assert_eq!(
            weapon.is_rad_eruption,
            expected["is_rad_eruption"].as_bool().unwrap(),
            "{name} IsRadEruption"
        );
    }
    for (name, expected) in expected["warheads"].as_object().unwrap() {
        let warhead = rules.warhead(name).unwrap();
        assert_eq!(
            warhead.temporal,
            expected["temporal"].as_bool().unwrap(),
            "{name} Temporal"
        );
        assert_eq!(
            warhead.radiation,
            expected["radiation"].as_bool().unwrap(),
            "{name} Radiation"
        );
    }
}

fn rules_for_birth(retail: &RetailBattleRules, input: &Value) -> RuleSet {
    let mut rules = retail.rules.clone();
    if let Some(control) = input["art_flh_control"].as_str() {
        let mut art = (*retail.fixed_art).clone();
        art.merge(&IniFile::from_str(&format!(
            "[DESO]\nPrimaryFireFLH={control}\n"
        )));
        // Parse the declared ART string through the production owner. Never
        // install row.physical_flh or row.beam.source as an implementation input.
        rules.install_art_data(crate::rules::retail_ini_fixture::art_registry(&art));
    }
    rules
}

fn place_native_actor(sim: &mut Simulation, rules: &RuleSet, owner: &str, at: &Value) -> u64 {
    let [x, y, z] = xyz(at);
    let id = sim
        .spawn_object("DESO", owner, (x / 256) as u16, (y / 256) as u16, 0, rules)
        .unwrap();
    sim.substrate
        .entities
        .get_mut(id)
        .unwrap()
        .position
        .put_location(DriveCoord { x, y, z });
    id
}

#[test]
fn six_native_deso_births_use_the_shared_flh_and_effect_depth_owners() {
    let Some(retail) = retail() else {
        return;
    };
    let rows = native()["birth_rows"].as_array().unwrap();
    assert_eq!(
        rows.len(),
        6,
        "four cardinal headings plus positive depth/equal raw Y"
    );
    for row in rows {
        let input = &row["input"];
        let expected = &row["beam"];
        let name = input["name"].as_str().unwrap();
        let rules = rules_for_birth(&retail, input);
        assert_eq!(
            <[i32; 3]>::from(
                super::fire_coord::firer_art(&rules, rules.object("DESO").unwrap())
                    .unwrap()
                    .primary_fire_flh
            ),
            xyz(&row["physical_flh"]),
            "{name} original ART reader"
        );
        let mut sim = crate::sim::arena_fixture::rival_arena_with_size(&rules, 0xC29D_E500, 64);
        let source = place_native_actor(&mut sim, &rules, "Americans", &input["location"]);
        let target = place_native_actor(&mut sim, &rules, "Russians", &input["target"]);
        sim.resolve_type_handles(&rules);
        // Explicit native component frame (rad_beam.py PhysicalBirth setup),
        // not a simulated scenario clock or timing-event history.
        sim.session.binary_frame = 200;
        let source_entity = sim.substrate.entities.get_mut(source).unwrap();
        source_entity
            .body_facing
            .snap(u16::try_from(integer(&input["heading"])).unwrap(), 200);
        assert_eq!(source_entity.weapon_burst.index(), integer(&input["burst"]));
        assert_eq!(source_entity.veterancy.rank(), 0, "{name} rookie input");
        let facing_before = source_entity.body_facing;
        let rng_before = &native()["birth_rng_before"];
        sim.main_rng =
            SimRng::from_native_state_hex_for_test(rng_before["main"].as_str().unwrap()).into();
        sim.scenario_rng =
            SimRng::from_native_state_hex_for_test(rng_before["scenario"].as_str().unwrap());
        sim.mapgen_rng =
            SimRng::from_native_state_hex_for_test(rng_before["mapgen"].as_str().unwrap());
        let id_before = sim.native_unique_ids.as_ref().unwrap().current_raw();
        let stable_before = sim.substrate.next_stable_object_id;
        let logic_before = sim.live_object_order_snapshot();
        sim.lifecycle_outputs.clear();
        super::radiation_beam::create(&mut sim, &rules, source, ProjectileTarget::Entity(target));
        let [LifecycleOutput::RadiationBeamCreated(birth)] = sim.lifecycle_outputs.as_slice()
        else {
            panic!(
                "{name} expected exactly one copied mode0 beam: {:?}",
                sim.lifecycle_outputs
            );
        };
        assert_eq!(birth.frame, 200, "{name} birth frame input");
        assert_eq!(
            birth.from,
            coordinate(&expected["source"]),
            "{name} native GetFLH index0"
        );
        assert_eq!(
            birth.to,
            coordinate(&expected["target"]),
            "{name} native nonbuilding target"
        );
        assert_eq!(
            birth.z_adjust,
            integer(&expected["z_adjust"]),
            "{name} raw-Y effect adjustment"
        );
        assert_eq!(birth.color, rgb(&expected["rgb"]), "{name} native RadColor");
        assert_eq!(
            birth.duration,
            integer(&expected["duration"]),
            "{name} native duration"
        );
        assert_eq!(
            birth.amplitude.bits(),
            u64::from_str_radix(expected["amplitude_f64_le_hex"].as_str().unwrap(), 16)
                .unwrap()
                .swap_bytes(),
            "{name} original binary64 amplitude"
        );
        let rng_after = &native()["birth_rng_after"];
        assert_eq!(
            sim.main_rng.native_state_hex(),
            rng_after["main"].as_str().unwrap(),
            "{name} full Main RNG"
        );
        assert_eq!(
            sim.scenario_rng.native_state_hex(),
            rng_after["scenario"].as_str().unwrap(),
            "{name} full Scenario RNG"
        );
        assert_eq!(
            sim.mapgen_rng.native_state_hex(),
            rng_after["mapgen"].as_str().unwrap(),
            "{name} full Mapgen RNG"
        );
        assert_eq!(
            sim.native_unique_ids.as_ref().unwrap().current_raw(),
            id_before,
            "{name} no Abstract ID"
        );
        assert_eq!(
            sim.substrate.next_stable_object_id, stable_before,
            "{name} no simulation object allocation"
        );
        assert_eq!(
            sim.live_object_order_snapshot(),
            logic_before,
            "{name} no Logic registration"
        );
        let source_entity = sim.substrate.entities.get(source).unwrap();
        assert_eq!(
            source_entity.body_facing, facing_before,
            "{name} facing timer unchanged"
        );
        assert_eq!(
            source_entity.weapon_burst.index(),
            integer(&input["burst"]),
            "{name} burst unchanged"
        );
    }
}
