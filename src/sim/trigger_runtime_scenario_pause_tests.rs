use super::*;
use crate::rules::ini_parser::IniFile;
use crate::sim::scenario_session::ScenarioDescriptor;

fn action(sim: &mut Simulation, kind: i32, value: i32) -> Vec<TriggerEffect> {
    let ini = IniFile::from_str(&format!("[Actions]\nA=1,{kind},0,{value},0,0,0,0,Z\n"));
    let entry = crate::map::actions::parse_actions(&ini)["A"].entries[0].clone();
    let action = RuntimeDefinitions::default().resolve_action(entry, None, None);
    let mut effects = Vec::new();
    assert!(sim.apply_live_trigger_action(
        &action,
        TriggerActionContext {
            live_index: None,
            incoming_house: None,
            input: TagEventInput::default()
        },
        &mut effects,
        None,
        None,
        FrameEffects::default(),
    ));
    effects
}

#[test]
fn pause_action_preserves_ordered_raw_duration_and_owns_input_lock() {
    let mut sim = Simulation::with_seed(0);
    let hash = sim.state_hash();
    let draws = sim.scenario_rng.native_state_hex();
    for duration in [0, -1, i32::MIN, i32::MAX, 2] {
        let effects = action(&mut sim, 110, duration);
        assert!(
            matches!(effects[0], TriggerEffect::ScenarioPause { duration: d, .. } if d == duration)
        );
        assert!(sim.session.input_locked());
    }
    assert_ne!(sim.state_hash(), hash);
    assert_eq!(sim.scenario_rng.native_state_hex(), draws);
    assert_eq!(sim.session.tick, 0);
}

#[test]
fn vein_action_saves_and_hashes_the_raw_byte_without_inventing_growth() {
    let mut sim = Simulation::with_seed(0);
    let initial = sim.state_hash();
    assert!(sim.session.vein_growth_enabled());
    for (value, byte) in [(256, 0), (-1, 255), (257, 1), (258, 2)] {
        let rng = sim.scenario_rng.native_state_hex();
        assert!(action(&mut sim, 85, value).is_empty());
        assert_eq!(sim.session.vein_growth_enabled(), byte != 0);
        assert_eq!(sim.scenario_rng.native_state_hex(), rng);
        let hash = sim.state_hash();
        if byte == 1 {
            assert_eq!(hash, initial);
        } else {
            assert_ne!(hash, initial);
        }
        let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "", 0);
        let mut restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
            .unwrap()
            .sim;
        restored.restore_after_snapshot_load().unwrap();
        assert_eq!(restored.state_hash(), hash);
        assert_eq!(restored.session.vein_growth_enabled(), byte != 0);
    }
}

#[test]
fn basic_vein_growth_uses_native_bool_reader_and_fresh_true_default() {
    for (text, expected) in [
        ("", true),
        ("VeinGrowthEnabled=", true),
        ("VeinGrowthEnabled=?", true),
        ("VeinGrowthEnabled=no", false),
        ("VeinGrowthEnabled=yes", true),
        ("veingrowthenabled=no", true),
    ] {
        let basic = crate::map::basic::parse_basic_section(&IniFile::from_str(&format!(
            "[Basic]\n{text}\n"
        )));
        let session =
            crate::sim::scenario_session::ScenarioSession::from_descriptor(&ScenarioDescriptor {
                vein_growth_enabled: basic.vein_growth_enabled,
                ..Default::default()
            });
        assert_eq!(session.vein_growth_enabled(), expected, "{text}");
    }
    for map in ["Hills.mmx", "XMP03T4.MAP"] {
        let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules_for_map(map)
        else {
            return;
        };
        let basic = crate::map::basic::parse_basic_section(&retail.processed_rules);
        assert!(
            !basic.vein_growth_enabled.unwrap_or(true),
            "{map}: authored VeinGrowthEnabled=no"
        );
    }
}

#[test]
fn paused_chrono_clock_finishes_without_a_world_frame_or_rng_draw() {
    let mut sim = Simulation::with_seed(0);
    let mut start = Vec::new();
    sim.start_chrono_screen_effect(3, &mut start);
    let initial = (
        sim.session.tick,
        sim.session.binary_frame,
        sim.scenario_rng.native_state_hex(),
    );
    let mut saw_pause = false;
    let mut saw_finish = false;
    for _ in 0..60 {
        for effect in sim.process_chrono_screen_effect() {
            if let TriggerEffect::ChronoScreen { command } = effect {
                saw_pause |=
                    command == crate::sim::superweapon::chrono_screen::ChronoScreenCommand::Pause;
                saw_finish |=
                    command == crate::sim::superweapon::chrono_screen::ChronoScreenCommand::Finish;
            }
        }
    }
    assert!(saw_pause && saw_finish);
    assert_eq!(
        (
            sim.session.tick,
            sim.session.binary_frame,
            sim.scenario_rng.native_state_hex()
        ),
        initial
    );
    assert!(!sim.session.input_locked());
}

#[test]
fn counted_logic_tag_emits_pause_requests_in_authored_order() {
    let map = crate::map::map_file::MapFile::from_bytes(b"[Map]\nTheater=TEMPERATE\nSize=0,0,2,1\nLocalSize=0,0,2,1\n[IsoMapPack5]\n1=DwALABwBAAIA/////wAAABEAAA==\n[Basic]\nNewINIFormat=4\n[Triggers]\nP=Neutral,<none>,P,0,1,1,1,0\n[Events]\nP=1,13,0,0\n[Actions]\nP=3,85,0,256,0,0,0,0,Z,110,0,2,0,0,0,0,Z,110,0,-1,0,0,0,0,Z\n[Tags]\nPTAG=0,P,P\n").unwrap();
    let mut sim = Simulation::with_seed(0);
    sim.initialize_trigger_definitions(
        &map,
        &crate::rules::team_ai_ini::TeamAiIniRegistry::default(),
        None,
        None,
    );
    sim.classify_map_triggers(None);
    sim.session.set_started(true);
    let effects = sim.advance_triggers(
        TriggerInputs {
            events: &map.events,
            actions: &map.actions,
            rules: None,
        },
        None,
        FrameEffects::default(),
    );
    assert_eq!(
        effects,
        [
            TriggerEffect::ScenarioPause {
                duration: 2,
                outermost: true
            },
            TriggerEffect::InputLockChanged {
                locked: true,
                was_locked: false
            },
            TriggerEffect::ScenarioPause {
                duration: -1,
                outermost: false
            }
        ]
    );
    assert!(!sim.session.vein_growth_enabled());
    assert!(sim.trigger_runtime.logic_tags.is_empty());
}

#[test]
fn pause_lock_is_synchronous_and_explicit_unlock_keeps_native_prior_gate() {
    let mut sim = Simulation::with_seed(0);
    action(&mut sim, 110, 2);
    assert!(sim.session.input_locked());
    action(&mut sim, 47, 0);
    assert!(!sim.session.input_locked());
    sim.expire_scenario_pause();
    assert!(!sim.session.input_locked());

    action(&mut sim, 46, 0);
    action(&mut sim, 110, 2);
    action(&mut sim, 47, 0);
    // B054CC=1 skips the restore write; it does not force35A2 back to1.
    sim.expire_scenario_pause();
    assert!(!sim.session.input_locked());
    assert!(!sim.session.scenario_paused());
}

#[test]
fn chrono_and_timed_trigger_share_one_counter_and_preserve_the_outer_lock() {
    let mut sim = Simulation::with_seed(0);
    action(&mut sim, 110, 10);
    assert!(sim.session.input_locked());
    let mut effects = Vec::new();
    sim.start_chrono_screen_effect(1, &mut effects);
    let mut finish = false;
    for _ in 0..50 {
        finish |= sim.process_chrono_screen_effect().iter().any(|effect| {
            matches!(
                effect,
                TriggerEffect::ChronoScreen {
                    command: crate::sim::superweapon::chrono_screen::ChronoScreenCommand::Finish
                }
            )
        });
    }
    assert!(finish);
    assert!(sim.session.scenario_paused());
    assert!(sim.session.input_locked());
    assert!(
        sim.expire_scenario_pause()
            .contains(&TriggerEffect::ScenarioResume)
    );
    assert!(!sim.session.scenario_paused());
    assert!(!sim.session.input_locked());
}
