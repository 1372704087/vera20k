//! VERA local-view persistence admission and continuation. Native camera and
//! bookmark outputs are consumed by the camera tests and bookmark test below;
//! these entity fixtures test the app/Simulation boundary, not native SAV IO.

use super::{MatchInputState, PreparedSavedLocalView};
use crate::map::entities::EntityCategory;
use crate::rules::{ini_parser::IniFile, ruleset::RuleSet};
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::components::Health;
use crate::sim::game_entity::GameEntity;
use crate::sim::snapshot::saved_local_view::{SavedLocalView, SavedLocalViewError};
use crate::sim::world::Simulation;

fn fixture() -> (Simulation, RuleSet, MatchInputState) {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[VehicleTypes]\n1=UNIT\n[UNIT]\nStrength=100\n",
    ))
    .unwrap();
    let mut sim = Simulation::new();
    let owner = sim.interner.intern("local");
    let type_ref = sim.interner.intern("UNIT");
    sim.session.current_house = Some(owner);
    for id in 1..=3 {
        let mut entity = GameEntity::new_at_frame_zero_for_test(
            id,
            0,
            0,
            0,
            0,
            owner,
            Health { current: 100 },
            type_ref,
            EntityCategory::Unit,
            0,
            5,
            false,
        );
        entity.lifecycle.object_alive = true;
        entity.lifecycle.in_limbo = false;
        entity.selected = id != 2;
        sim.entities_mut().insert(entity);
    }
    let mut input = MatchInputState::new(Default::default());
    input.camera_x = -400.0;
    input.camera_y = 2_000.0;
    input.selection_order = vec![3, 1];
    input.control_groups[0] = vec![2, 3];
    input.control_groups[9] = vec![1];
    input.follow_target = Some(3);
    (sim, rules, input)
}

fn serialized_capture(
    input: &MatchInputState,
    sim: &Simulation,
    rules: &RuleSet,
) -> SavedLocalView {
    let saved = input
        .capture_saved_local_view(sim, rules, (800, 600))
        .unwrap();
    bincode::deserialize(&bincode::serialize(&saved).unwrap()).unwrap()
}

#[test]
fn saved_view_replaces_reused_ids_and_preserves_order_and_raw_bookmarks() {
    let (sim, rules, mut source) = fixture();
    let corpus: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
        "tools/spatial_oracle/native_id_snapshot.json",
    ))
    .unwrap();
    let rows = corpus["scenario_bookmarks"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    let clock_corpus: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
        "tools/storage_oracle/saved_game_layout.json",
    ))
    .unwrap();
    let clocks = clock_corpus["postload_clock_reset"].as_array().unwrap();
    assert_eq!(clocks.len(), 3);
    for clock in clocks {
        assert_eq!(clock["group_last_slot"], -1);
    }
    for row in rows {
        let input_words: [[i16; 2]; 4] = serde_json::from_value(row["input"].clone()).unwrap();
        let expected: [[i16; 2]; 4] = serde_json::from_value(row["loaded"].clone()).unwrap();
        source
            .view_bookmarks
            .restore_saved_cells(input_words.map(|cell| cell.map(|v| v as u16)));
        let saved = serialized_capture(&source, &sim, &rules);
        let center = saved.camera_center_world_px();
        let mut outgoing = MatchInputState::new(Default::default());
        outgoing.selection_order = vec![1, 3];
        outgoing.selection_order_pending = true;
        outgoing.control_groups[4] = vec![1, 2, 3];
        outgoing.follow_target = Some(1);
        outgoing.last_control_group_press = Some((9, std::time::Instant::now()));
        outgoing.view_bookmarks.seed_all(9, 9);
        // This owner restores only established saved facts. A future input
        // lifecycle change must not be smuggled in as blanket initialization.
        outgoing.keys_held.insert(winit::keyboard::KeyCode::KeyA);
        outgoing.zoom_level = 2.0;
        outgoing.zoom_target = 2.0;
        let prepared = PreparedSavedLocalView::new(saved, &sim, &rules).unwrap();
        assert_eq!(prepared.commit_into(&mut outgoing), center);
        assert_eq!(outgoing.selection_order, [3, 1]);
        assert!(!outgoing.selection_order_pending);
        assert_eq!(outgoing.control_groups[0], [2, 3]);
        assert_eq!(outgoing.control_groups[9], [1]);
        assert!(outgoing.control_groups[1..9].iter().all(Vec::is_empty));
        assert_eq!(outgoing.follow_target, Some(3));
        assert_eq!(outgoing.last_control_group_press, None);
        assert_eq!(
            outgoing
                .view_bookmarks
                .saved_cells()
                .map(|cell| cell.map(|v| v as i16)),
            expected,
        );
        assert!(outgoing.keys_held.contains(&winit::keyboard::KeyCode::KeyA));
        assert_eq!(outgoing.zoom_level, 2.0);
        assert_eq!(outgoing.zoom_target, 2.0);
    }
}

#[test]
fn pending_selection_survives_load_until_the_existing_queue_owner_drains_it() {
    let (mut sim, rules, mut input) = fixture();
    input.selection_order = vec![2, 1];
    input.selection_order_pending = true;
    let owner = sim.session.current_house.unwrap();
    sim.queue_command(CommandEnvelope::new(
        owner,
        2,
        Command::Select {
            entity_ids: vec![2, 1],
            additive: false,
        },
    ));
    let queued_before = bincode::serialize(&sim.pending_command_snapshot()).unwrap();
    let saved = serialized_capture(&input, &sim, &rules);
    assert_eq!(saved.selection_order(), [2, 1]);
    assert!(saved.selection_order_pending());
    let prepared = PreparedSavedLocalView::new(saved, &sim, &rules).unwrap();
    let mut restored = MatchInputState::new(Default::default());
    prepared.commit_into(&mut restored);
    assert_eq!(restored.selection_order, [2, 1]);
    assert!(restored.selection_order_pending);
    assert_eq!(
        bincode::serialize(&sim.pending_command_snapshot()).unwrap(),
        queued_before
    );
    assert!(!sim.entities().get(2).unwrap().selected);
    assert!(sim.entities().get(3).unwrap().selected);

    // Model the existing queue receiver draining/refusing this command. The
    // shared reconciliation closes pending mode against committed flags;
    // persistence itself neither applies selection nor invents admission.
    sim.session.tick = 1;
    assert_eq!(sim.take_due_commands().len(), 1);
    let saved = serialized_capture(&restored, &sim, &rules);
    assert!(!saved.selection_order_pending());
    assert_eq!(saved.selection_order(), [1, 3]);
    PreparedSavedLocalView::new(saved, &sim, &rules).unwrap();
}

#[test]
fn malformed_local_view_is_rejected_before_the_outgoing_input_changes() {
    let (sim, rules, input) = fixture();
    let saved = serialized_capture(&input, &sim, &rules);
    let original = serde_json::to_value(&saved).unwrap();
    let cases = [
        ("selection_order", serde_json::json!([3, 3, 1])),
        ("selection_order", serde_json::json!([3, 99])),
        ("selection_order", serde_json::json!([3])),
        ("selection_order_pending", serde_json::json!(true)),
        ("follow_target", serde_json::json!(99)),
        (
            "control_groups",
            serde_json::json!([[2, 3], [], [], [], [], [], [], [], [], [1, 2]]),
        ),
        (
            "control_groups",
            serde_json::json!([[2, 99], [], [], [], [], [], [], [], [], [1]]),
        ),
    ];
    for (key, value) in cases {
        let mut malformed = original.clone();
        malformed[key] = value;
        let malformed: SavedLocalView = serde_json::from_value(malformed).unwrap();
        assert!(
            PreparedSavedLocalView::new(malformed, &sim, &rules).is_err(),
            "{key}"
        );
        assert_eq!(serialized_capture(&input, &sim, &rules), saved);
    }
    let mut wrong_slots = original;
    wrong_slots["control_groups"] = serde_json::json!([[], []]);
    assert!(serde_json::from_value::<SavedLocalView>(wrong_slots).is_err());
    assert_eq!(
        SavedLocalView::new(
            [f32::NAN, 0.0],
            [[0, 0]; 4],
            vec![],
            false,
            std::array::from_fn(|_| Vec::new()),
            None,
        )
        .unwrap_err(),
        SavedLocalViewError::InvalidCameraCenter,
    );
}

#[test]
fn capture_prunes_departed_references_but_preserves_saved_idle_follow() {
    let (sim, rules, mut input) = fixture();
    input.control_groups[0].push(99);
    input.follow_target = Some(99);
    let saved = serialized_capture(&input, &sim, &rules);
    assert_eq!(saved.control_groups()[0], [2, 3]);
    assert_eq!(saved.follow_target(), None);
    assert_eq!(input.follow_target, Some(99), "capture is read-only");
    let prepared = PreparedSavedLocalView::new(saved, &sim, &rules).unwrap();
    input.follow_target = Some(1);
    prepared.commit_into(&mut input);
    assert_eq!(input.follow_target, None);
}
