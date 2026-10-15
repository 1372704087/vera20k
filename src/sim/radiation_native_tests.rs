//! Original RadSite/Cell operation and store boundaries, including the
//! overlap threshold that changes a stock DOG's first-pulse survival.

use super::*;

fn integer(value: &serde_json::Value) -> i32 {
    i32::try_from(value.as_i64().unwrap()).unwrap()
}

fn cell(value: &serde_json::Value) -> (u16, u16) {
    (
        u16::try_from(value[0].as_u64().unwrap()).unwrap(),
        u16::try_from(value[1].as_u64().unwrap()).unwrap(),
    )
}

#[test]
fn field_operations_match_native_overlap_controls() {
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let native: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
        "tools/spatial_oracle/radiation_overlap.json",
    ))
    .unwrap();
    let scene = crate::sim::arena_fixture::rival_arena_with_size(&fixture.rules, 31, 48);
    let terrain = scene.resolved_terrain.as_ref();
    for row in native["rows"].as_array().unwrap() {
        let name = row["name"].as_str().unwrap();
        let mut rules = fixture.rules.radiation.clone();
        if name.starts_with("zero_steps_") {
            // The corpus explicitly supplies this nonstock reader input to
            // exercise the same shared operation's masked zero divisor.
            rules.level_delay = 600;
        }
        assert_eq!(
            format!("{:016x}", rules.level_factor.to_bits()),
            row["factor_bits"]
        );
        let mut state = RadiationState::default();
        let mut lights = Vec::new();
        let mut frame = 0;
        for stage in row["stages"].as_array().unwrap() {
            let action = &stage["action"];
            match action["kind"].as_str().unwrap() {
                "create" | "merge" => {
                    let (rx, ry) = cell(&action["cell"]);
                    state.apply_detonation(
                        RadDetonation {
                            rx,
                            ry,
                            rad_level: integer(&row["source_inputs"]["rad_level"]),
                            spread: integer(&row["source_inputs"]["spread"]),
                        },
                        frame,
                        &rules,
                        terrain,
                        &mut lights,
                    );
                }
                "tick_to" => {
                    let end = u32::try_from(action["frame"].as_u64().unwrap()).unwrap();
                    while frame < end {
                        frame += 1;
                        state.tick_decay(frame, &rules, terrain, &mut lights);
                    }
                }
                "decay" => state.decay_step(cell(&action["cell"]), terrain),
                "decrease" => state.decrease_site_levels(cell(&action["cell"]), terrain, &rules),
                other => panic!("unknown native action {other}"),
            }
            for sample in stage["samples"].as_array().unwrap() {
                let at = cell(&sample["cell"]);
                assert_eq!(
                    format!("{:016x}", state.cell_level(at).to_bits()),
                    sample["level_bits"],
                    "{name} {action} {at:?}: Cell's stored bits"
                );
                assert_eq!(
                    state.damaging_level(at, rules.level_max),
                    integer(&sample["damaging_level"]),
                    "{name} {action} {at:?}: Cell487CB0 conversion"
                );
            }
            for expected in stage["sites"].as_array().unwrap() {
                let actual = state.site_at(cell(&expected["cell"])).unwrap();
                assert_eq!(
                    actual.radius_leptons,
                    integer(&expected["radius"]),
                    "{name}"
                );
                assert_eq!(actual.level, integer(&expected["level"]), "{name}");
                assert_eq!(
                    actual.level_steps,
                    integer(&expected["level_steps"]),
                    "{name}"
                );
                assert_eq!(actual.duration, integer(&expected["duration"]), "{name}");
                assert_eq!(actual.remaining, integer(&expected["remaining"]), "{name}");
                assert_eq!(
                    RadiationState::current_site_level(actual),
                    integer(&expected["current_level"]),
                    "{name}: Guard65B510 retains its distinct integer query"
                );
                assert_eq!(
                    actual.level_timer.start_frame(),
                    integer(&expected["level_timer_start"]),
                    "{name}"
                );
                assert_eq!(
                    actual.level_timer.duration(),
                    integer(&expected["level_timer_duration"]),
                    "{name}"
                );
            }
            let expected_order: Vec<_> = stage["update_order"]
                .as_array()
                .unwrap()
                .iter()
                .map(cell)
                .collect();
            assert_eq!(
                state
                    .sites_in_update_order()
                    .map(|site| site.center)
                    .collect::<Vec<_>>(),
                expected_order,
                "{name}: native reverse creation order survives each operation"
            );
        }
    }
}

#[test]
fn restore_rejects_missing_or_duplicate_radiation_update_slots() {
    use crate::sim::snapshot::SnapshotRestoreError;
    let mut sim = crate::sim::world::Simulation::new();
    sim.radiation.apply_detonation(
        RadDetonation {
            rx: 10,
            ry: 10,
            rad_level: 500,
            spread: 2,
        },
        0,
        &RadiationRules::default(),
        None,
        &mut Vec::new(),
    );
    sim.radiation.creation_order.push((10, 10));
    assert_eq!(
        sim.restore_after_snapshot_load(),
        Err(SnapshotRestoreError::InvalidRadiationSiteOrder(
            "duplicate radiation site in update order"
        ))
    );
    sim.radiation.creation_order.clear();
    assert_eq!(
        sim.restore_after_snapshot_load(),
        Err(SnapshotRestoreError::InvalidRadiationSiteOrder(
            "radiation site is missing from update order"
        ))
    );
}

#[test]
fn equal_current_fields_with_different_update_order_hash_differently() {
    let mut forward = crate::sim::world::Simulation::new();
    let mut reverse = crate::sim::world::Simulation::new();
    let centers = [(17, 24), (31, 24)];
    for (world, order) in [
        (&mut forward, centers),
        (&mut reverse, [centers[1], centers[0]]),
    ] {
        for (rx, ry) in order {
            world.radiation.apply_detonation(
                RadDetonation {
                    rx,
                    ry,
                    rad_level: 500,
                    spread: 10,
                },
                0,
                &RadiationRules::default(),
                None,
                &mut Vec::new(),
            );
        }
    }
    let bits = |state: &RadiationState| {
        state
            .iter_cells()
            .map(|(cell, level)| (*cell, level.to_bits()))
            .collect::<Vec<_>>()
    };
    assert_eq!(bits(&forward.radiation), bits(&reverse.radiation));
    assert_ne!(
        forward.state_hash(),
        reverse.state_hash(),
        "equal current levels must not hide a different future subtraction order"
    );
}

#[test]
fn serialized_radius_that_changes_future_decay_is_hash_relevant() {
    let mut original = crate::sim::world::Simulation::new();
    original.radiation.apply_detonation(
        RadDetonation {
            rx: 17,
            ry: 24,
            rad_level: 500,
            spread: 10,
        },
        0,
        &RadiationRules::default(),
        None,
        &mut Vec::new(),
    );
    let mut restored = crate::sim::world::Simulation::new();
    restored.radiation =
        bincode::deserialize(&bincode::serialize(&original.radiation).unwrap()).unwrap();
    restored
        .radiation
        .sites
        .get_mut(&(17, 24))
        .unwrap()
        .radius_leptons += 1;
    assert_eq!(original.restore_after_snapshot_load(), Ok(()));
    assert_eq!(restored.restore_after_snapshot_load(), Ok(()));
    assert_eq!(
        original.radiation.cell_level((24, 24)).to_bits(),
        restored.radiation.cell_level((24, 24)).to_bits()
    );
    assert_ne!(
        original.state_hash(),
        restored.state_hash(),
        "admitted saved radii control future decay even with equal current cells"
    );
}
