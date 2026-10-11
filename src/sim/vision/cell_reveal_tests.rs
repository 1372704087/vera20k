//! Original MapCell/cache controls. Fixture inputs are native supplied storage;
//! expected values are executed bytes, never a second edge-frame algorithm.
use super::*;
use crate::map::resolved_terrain::{ResolvedTerrainGrid, test_clear_cell};
use serde_json::{Value, json};

pub(crate) fn discovery_corpus() -> Value {
    let rows: Vec<Value> = serde_json::from_str(crate::test_fixture::text(
        "tools/spatial_oracle/trigger_action_values.json",
    ))
    .unwrap();
    rows.into_iter()
        .find(|row| row["case_kind"] == "trigger_reveal_discovery")
        .unwrap()
}

pub(crate) fn pair(value: &Value) -> (u16, u16) {
    (
        value[0].as_i64().unwrap() as u16,
        value[1].as_i64().unwrap() as u16,
    )
}

/// Build precisely the supplied native allocated slots, including fixed512-row
/// aliases. Slot location and the Cell's stored+24 coordinate are distinct.
/// Fog indexes the latter; NativeCellQuery owns the former and the Dummy.
pub(crate) fn native_map_cell_fixture(
    topology: &Value,
    viewer: InternedId,
) -> (FogState, ResolvedTerrainGrid) {
    let inputs: Vec<Value> = topology["allocated_cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|input| {
            if input.is_array() {
                let mut row = topology["cell_constructor_prior"].clone();
                row["cell"] = json!([input[0], input[1]]);
                row["level"] = input[2].clone();
                row
            } else {
                input.clone()
            }
        })
        .collect();
    let slots: Vec<_> = inputs
        .iter()
        .map(|cell| {
            let (x, y) = pair(&cell["cell"]);
            let linear =
                crate::map::cell_index::cell_linear_index(i32::from(x as i16), i32::from(y as i16))
                    .unwrap();
            ((linear % 512) as u16, (linear / 512) as u16)
        })
        .collect();
    let width = slots.iter().map(|p| p.0).max().unwrap() + 1;
    let height = slots.iter().map(|p| p.1).max().unwrap() + 1;
    let mut cells = (0..height)
        .flat_map(|y| (0..width).map(move |x| test_clear_cell(x, y)))
        .collect::<Vec<_>>();
    for (input, &(x, y)) in inputs.iter().zip(&slots) {
        let (rx, ry) = pair(&input["cell"]);
        let mut cell = test_clear_cell(rx, ry);
        cell.level = input["level"].as_i64().unwrap() as u8;
        cell.bridge_facts.raw_flags = input["flags"].as_u64().unwrap() as u32 & !0x63;
        cells[usize::from(y) * usize::from(width) + usize::from(x)] = cell;
    }
    let mut terrain = ResolvedTerrainGrid::from_cells(width, height, cells);
    terrain.test_set_native_allocated_cells(&slots);
    let dummy = &topology["dummy_prior"];
    terrain.test_set_dummy_cell_level_slope(dummy["level"].as_i64().unwrap() as i8, 0);
    let (x, y) = pair(&dummy["cell"]);
    terrain.stamp_dummy_cell_requested_coord(i32::from(x as i16), i32::from(y as i16));
    let width = inputs
        .iter()
        .map(|cell| pair(&cell["cell"]).0)
        .max()
        .unwrap()
        + 1;
    let height = inputs
        .iter()
        .map(|cell| pair(&cell["cell"]).1)
        .max()
        .unwrap()
        + 1;
    let mut fog = FogState::new(width, height);
    let mut view = OwnerVisibility::new(width, height);
    for input in &inputs {
        let (x, y) = pair(&input["cell"]);
        let key = VisibilityCell::Real(view.index(x, y).unwrap());
        seed_cell(&mut view, key, input);
    }
    seed_cell(&mut view, VisibilityCell::Dummy, dummy);
    fog.by_owner.insert(viewer, view);
    (fog, terrain)
}

fn seed_cell(view: &mut OwnerVisibility, key: VisibilityCell, input: &Value) {
    let (knowledge, ground, cache) = view.map_cell_parts_mut(key);
    let flags = input["flags"].as_u64().unwrap() as u32;
    *ground = input["alt"].as_u64().unwrap() as u8;
    knowledge.counter = input["counter"].as_i64().unwrap() as i32;
    knowledge.gap_counter = input["gap"].as_i64().unwrap() as i32;
    knowledge.open = *ground & 0x18 == 0x18;
    knowledge.pending = flags & 0x20 != 0;
    *cache = MapCellEdgeCache {
        initialized: flags & 2 != 0,
        fog_open: flags & 1 != 0,
        frames: [
            input["cache"][0].as_i64().unwrap() as i8,
            input["cache"][1].as_i64().unwrap() as i8,
        ],
    };
    if let VisibilityCell::Real(index) = key {
        view.publish_knowledge(index);
    }
}

pub(crate) fn assert_native_cell_history(
    fog: &FogState,
    viewer: InternedId,
    row: &Value,
    terrain: &ResolvedTerrainGrid,
    dummy: bool,
) {
    let view = &fog.by_owner[&viewer];
    let cell = pair(&row["cell"]);
    let key = if dummy {
        VisibilityCell::Dummy
    } else {
        VisibilityCell::Real(view.index(cell.0, cell.1).unwrap())
    };
    let (ground, cache) = view.map_cell_parts(key);
    let state = match key {
        VisibilityCell::Real(index) => view.shroud_knowledge[index],
        VisibilityCell::Dummy => view.dummy_visibility.knowledge,
    };
    assert_eq!(
        state.counter,
        row["counter"].as_i64().unwrap() as i32,
        "counter at {cell:?}"
    );
    assert_eq!(
        state.gap_counter,
        row["gap"].as_i64().unwrap() as i32,
        "gap at {cell:?}"
    );
    assert_eq!(
        u64::from(ground & 0x18),
        row["alt"].as_u64().unwrap() & 0x18,
        "ground at {cell:?}"
    );
    assert_eq!(
        state.pending,
        row["flags"].as_u64().unwrap() & 0x20 != 0,
        "pending at {cell:?}"
    );
    assert_eq!(json!(cache.frames), row["cache"], "edge bytes at {cell:?}");
    let flags = u64::from(cache.initialized) * 2 | u64::from(cache.fog_open);
    assert_eq!(
        flags,
        row["flags"].as_u64().unwrap() & 3,
        "map flags at {cell:?}"
    );
    if dummy {
        let actual = terrain.shared_cell_dummy().snapshot();
        assert_eq!(
            json!([actual.coord.0, actual.coord.1]),
            row["cell"],
            "canonical Dummy ordering"
        );
        assert_eq!(i64::from(actual.level), row["level"].as_i64().unwrap());
    }
}

#[test]
fn native_trigger_reveal_edge_channels_match_all_original_masks_and_center_gates() {
    let corpus = discovery_corpus();
    let controls = &corpus["cache_controls"];
    let viewer = crate::sim::intern::test_intern("Player");
    let (mut fog, terrain) = native_map_cell_fixture(&controls["topology"], viewer);
    let cells = NativeCellQuery::canonical(&terrain);
    let center = (9, 10);
    for channel in 0..2usize {
        for (mask, expected) in controls["channel_mask_outputs"][channel]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let view = fog.by_owner.get_mut(&viewer).unwrap();
            let index = view.index(center.0, center.1).unwrap();
            if channel == 0 {
                view.ground_flags[index] = 8;
            } else {
                view.map_cell_edge_cache[index].initialized = true;
            }
            for input in controls["neighbor_masks"].as_array().unwrap() {
                let x = (9i16).wrapping_add(input[0].as_i64().unwrap() as i16) as u16;
                let y = (10i16).wrapping_add(input[1].as_i64().unwrap() as i16) as u16;
                let index = view.index(x, y).unwrap();
                let set = mask & input[2].as_u64().unwrap() as usize == 0;
                if channel == 0 {
                    view.ground_flags[index] = if set { 8 } else { 0 };
                } else {
                    view.map_cell_edge_cache[index].initialized = set;
                }
            }
            assert_eq!(
                i64::from(fog.map_cell_edge_frame(viewer, center, channel != 0, Some(&cells))),
                expected.as_i64().unwrap(),
                "channel{channel} mask{mask}"
            );
        }
    }
    for input in controls["center_gates"].as_array().unwrap() {
        let channel = input["channel"].as_u64().unwrap();
        let value = input["center"].as_u64().unwrap() as u8;
        let view = fog.by_owner.get_mut(&viewer).unwrap();
        let index = view.index(9, 10).unwrap();
        if channel == 0 {
            view.ground_flags[index] = value;
        } else {
            view.map_cell_edge_cache[index].initialized = value & 2 != 0;
            view.map_cell_edge_cache[index].fog_open = value & 1 != 0;
        }
        assert_eq!(
            i64::from(fog.map_cell_edge_frame(viewer, center, channel != 0, Some(&cells))),
            input["returned"].as_i64().unwrap()
        );
    }
}

#[test]
fn native_trigger_reveal_retained_cache_admission_matches_original_histories() {
    let corpus = discovery_corpus();
    let controls = &corpus["cache_controls"];
    let viewer = crate::sim::intern::test_intern("Player");
    let (mut fog, terrain) = native_map_cell_fixture(&controls["topology"], viewer);
    for history in controls["histories"].as_array().unwrap() {
        let cells = NativeCellQuery::canonical(&terrain);
        let notify = fog.reveal_map_cell(
            viewer,
            pair(&history["cell"]),
            history["final"] == 1,
            Some(&cells),
        );
        let expected = history["trace"]
            .as_array()
            .unwrap()
            .iter()
            .find(|event| event["pc"] == "005865F0");
        assert_eq!(
            notify
                .as_ref()
                .map(|event| json!([event.cell.0, event.cell.1, event.fresh])),
            expected.map(|event| json!([event["cell"][0], event["cell"][1], event["fresh"] == 1])),
            "{}",
            history["stage"]
        );
        for row in history["states"].as_array().unwrap() {
            assert_native_cell_history(&fog, viewer, row, &terrain, false);
        }
        let restored: FogState = bincode::deserialize(&bincode::serialize(&fog).unwrap()).unwrap();
        for row in history["states"].as_array().unwrap() {
            assert_native_cell_history(&restored, viewer, row, &terrain, false);
        }
        assert_eq!(
            restored.by_owner[&viewer].map_cell_edge_cache_for_test(pair(&history["cell"])),
            fog.by_owner[&viewer].map_cell_edge_cache_for_test(pair(&history["cell"]))
        );
    }
}
