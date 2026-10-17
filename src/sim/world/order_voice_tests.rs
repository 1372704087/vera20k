//! Committed order voices through the production Simulation owner. Native
//! Capture708DC0/Enter709020/Harvest709060 fallback to Move708FC0; the
//! QueueMegaMission6FFCBD enable gate precedes the reached sound function.

use super::Simulation;
use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::rules::sound_ini::SoundRegistry;
use crate::sim::rng::trace_draws;

fn native_voice_corpus() -> serde_json::Value {
    let corpus: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
        "tools/spatial_oracle/allied_object_action.json",
    ))
    .unwrap();
    assert_eq!(
        corpus["native_sha256"],
        "1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c"
    );
    corpus
}

fn scene(body: &str) -> (Simulation, RuleSet, u64) {
    scene_with_input(body, &serde_json::json!({}))
}

fn scene_with_input(body: &str, input: &serde_json::Value) -> (Simulation, RuleSet, u64) {
    use crate::map::entities::EntityCategory;
    use crate::sim::combat::Health;
    use crate::sim::game_entity::GameEntity;
    use crate::sim::house_state::HouseState;

    let ini = IniFile::from_str(&format!(
        "[InfantryTypes]\n0=ENGINEER\n[ENGINEER]\nStrength=100\n{body}"
    ));
    let mut rules = RuleSet::from_ini(&ini).unwrap();
    let sounds = SoundRegistry::from_ini(&IniFile::from_str(
        "[SoundList]\n0=Move\n1=Enter\n2=Capture\n3=Harvest\n4=Attack\n5=Special\n",
    ));
    rules.bind_type_sound_references(&ini, &sounds);
    let mut sim = Simulation::with_seed(input["seed"].as_u64().unwrap_or(1));
    let local = sim.interner.intern("Local");
    let other = sim.interner.intern("Other");
    let actor_owner = match input["actor_owner"].as_str().unwrap_or("local") {
        "local" => local,
        "other" => other,
        other => panic!("unknown native actor owner {other}"),
    };
    let type_ref = sim.interner.intern("ENGINEER");
    sim.session.current_house = Some(local);
    sim.session.game_mode_nonzero = input["mode"].as_u64().unwrap_or(1) != 0;
    sim.houses
        .insert(local, HouseState::new(local, 0, None, true, 0, 10));
    let mut other_house = HouseState::new(
        other,
        1,
        None,
        input["other_human"].as_bool().unwrap_or(false),
        0,
        10,
    );
    other_house.player_control = input["other_player_control"].as_bool().unwrap_or(false);
    sim.houses.insert(other, other_house);
    let id = 1;
    sim.entities_mut()
        .insert(GameEntity::new_at_frame_zero_for_test(
            id,
            0,
            0,
            0,
            0,
            actor_owner,
            Health::new(100),
            type_ref,
            EntityCategory::Infantry,
            0,
            5,
            false,
        ));
    (sim, rules, id)
}

#[test]
fn capture_enter_and_harvest_preserve_scalar_precedence_and_move_fallback() {
    let native = native_voice_corpus();
    let move_draws = &native["voice_controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "move_singleton")
        .unwrap()["output"]["main_draws"];
    for (body, capture, enter, harvest) in [
        (
            "VoiceMove=Move\nVoiceEnter=Enter\nVoiceCapture=Capture\nVoiceHarvest=Harvest\n",
            Some("Capture"),
            Some("Enter"),
            Some("Harvest"),
        ),
        (
            "VoiceMove=Move\nVoiceEnter=Enter\n",
            Some("Enter"),
            Some("Enter"),
            Some("Move"),
        ),
        ("VoiceMove=Move\n", Some("Move"), Some("Move"), Some("Move")),
        ("", None, None, None),
    ] {
        for (field, expected) in [
            ("VoiceCapture", capture),
            ("VoiceEnter", enter),
            ("VoiceHarvest", harvest),
        ] {
            let (mut sim, rules, id) = scene(body);
            let (request, draws) = trace_draws(|| sim.order_voice_request(&rules, id, field, true));
            assert_eq!(request, expected, "{field} with {body:?}");
            let expected_draws = if expected == Some("Move") {
                move_draws.clone()
            } else {
                serde_json::json!([])
            };
            assert_eq!(
                serde_json::json!(draws.iter().map(|draw| &draw["value"]).collect::<Vec<_>>()),
                expected_draws,
                "{field}: only the reached native Move fallback spends Main"
            );
        }
    }
}

#[test]
fn scalar_and_unknown_order_voice_fields_do_not_spend_main_draws() {
    let (mut sim, rules, id) = scene(
        "VoiceMove=Move\nVoiceAttack=Attack\nVoiceCapture=Capture\n\
         VoiceEnter=Enter\nVoiceHarvest=Harvest\n",
    );
    for (field, expected) in [
        ("VoiceAttack", Some("Attack")),
        ("VoiceCapture", Some("Capture")),
        ("VoiceEnter", Some("Enter")),
        ("VoiceHarvest", Some("Harvest")),
        ("unrecognized", None),
    ] {
        let before = sim.main_rng.native_state_hex();
        let (sound, draws) = trace_draws(|| sim.order_voice_request(&rules, id, field, true));
        assert_eq!(sound, expected, "{field}");
        assert!(draws.is_empty(), "{field}");
        assert_eq!(sim.main_rng.native_state_hex(), before, "{field}");
    }
    let (mut sim, rules, id) = scene("VoiceMove=Move\n");
    assert_eq!(
        sim.order_voice_request(&rules, id, "VoiceAttack", true),
        None
    );
}

#[test]
fn disabled_order_voice_rejects_before_move_or_fallback_is_reached() {
    let (mut sim, rules, id) = scene("VoiceMove=Move\n");
    for field in ["VoiceMove", "VoiceCapture", "VoiceEnter", "VoiceHarvest"] {
        let before = sim.main_rng.native_state_hex();
        let (sound, draws) = trace_draws(|| sim.order_voice_request(&rules, id, field, false));
        assert_eq!(sound, None, "{field}");
        assert!(draws.is_empty(), "{field}");
        assert_eq!(sim.main_rng.native_state_hex(), before, "{field}");
    }
}

#[test]
fn allied_object_move_voice_matches_native_rng_and_queue() {
    let corpus = native_voice_corpus();
    let mut singleton = 0;
    let mut empty = 0;
    let mut caller_disabled = 0;
    let mut nonhuman_draw_then_reject = 0;
    let mut direct_disabled = 0;
    let mut list_kernel = 0;
    for row in corpus["voice_controls"].as_array().unwrap() {
        let name = row["name"].as_str().unwrap();
        let input = &row["input"];
        let output = &row["output"];
        let ids = input["voice_ids"].as_array().unwrap();
        let voices: Vec<_> = ids
            .iter()
            .map(|id| match id.as_i64().unwrap() {
                -1 => String::new(),
                id => format!("Sound{id}"),
            })
            .collect();
        let enabled = input["voices_enabled"].as_bool().unwrap();
        let entry = input["caller_entry"].as_str().unwrap();
        let entry = u32::from_str_radix(entry.trim_start_matches("0x"), 16).unwrap();
        assert!(matches!(entry, 0x006f_fbe0 | 0x0070_8fc0), "{name}");
        let is_direct = entry == 0x0070_8fc0;
        let body = voices
            .first()
            .filter(|voice| !voice.is_empty())
            .map_or(String::new(), |voice| format!("VoiceMove={voice}\n"));
        let (mut sim, rules, id) = scene_with_input(&body, input);
        let human = sim.house_is_human_player(sim.entities().get(id).unwrap().owner());
        assert_eq!(
            sim.main_rng.native_state_hex(),
            output["main_before_hex"].as_str().unwrap(),
            "{name}: seeded native caller prior"
        );
        let scenario = sim.scenario_rng.logical_state();
        let mapgen = sim.mapgen_rng.logical_state();
        let world_hash = sim.state_hash();
        // The API consumes its real rules projection for empty/singleton
        // stock-compatible lists. Multi-entry and an explicit unbound -1
        // control instead exercise the existing shared list kernel: those
        // native vector states are outside the scalar rules projection.
        let kernel_only = is_direct || voices.len() > 1 || voices == [String::new()];
        let (request, draws) = trace_draws(|| {
            if kernel_only {
                if enabled || is_direct {
                    sim.voice_request_from_list(&voices, enabled, human)
                } else {
                    None
                }
            } else {
                sim.order_voice_request(&rules, id, "VoiceMove", enabled)
            }
        });
        let queued = request.map_or_else(
            || output["queued_voice_before"].as_i64().unwrap(),
            |sound| sound.strip_prefix("Sound").unwrap().parse::<i64>().unwrap(),
        );
        assert_eq!(
            queued,
            output["queued_voice_after"].as_i64().unwrap(),
            "{name}: native QueueVoice result"
        );
        assert_eq!(
            serde_json::json!(draws.iter().map(|draw| &draw["value"]).collect::<Vec<_>>()),
            output["main_draws"],
            "{name}: actual raw Main draws"
        );
        assert_eq!(
            sim.main_rng.native_state_hex(),
            output["main_after_hex"].as_str().unwrap(),
            "{name}: complete Main continuation"
        );
        assert_eq!(sim.scenario_rng.logical_state(), scenario, "{name}");
        assert_eq!(sim.mapgen_rng.logical_state(), mapgen, "{name}");
        assert_eq!(sim.state_hash(), world_hash, "{name}");
        assert_eq!(output["scenario_unchanged"], true, "{name}");
        assert_eq!(output["mapgen_unchanged"], true, "{name}");
        for expected in output["main_next_four"].as_array().unwrap() {
            assert_eq!(
                u64::from(sim.main_rng.next_u32()),
                expected.as_u64().unwrap()
            );
        }
        if is_direct && !enabled {
            direct_disabled += 1;
        } else if !enabled {
            caller_disabled += 1;
        } else if voices.is_empty() {
            empty += 1;
        } else if kernel_only {
            list_kernel += 1;
        } else if !human {
            nonhuman_draw_then_reject += 1;
        } else {
            singleton += 1;
        }
    }
    for (name, count) in [
        ("singleton production request", singleton),
        ("empty production list", empty),
        ("disabled caller before VoiceMove", caller_disabled),
        (
            "nonhuman caller rejection after draw",
            nonhuman_draw_then_reject,
        ),
        ("disabled direct VoiceMove after draw", direct_disabled),
        ("broader native list kernel controls", list_kernel),
    ] {
        assert!(count > 0, "missing native {name} control");
    }
}
