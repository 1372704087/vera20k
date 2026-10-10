//! Original-executable observations, never regenerated from the Rust port.
use super::{RightDragPoll, TacticalMouseState, right_drag_pan_step};
use crate::app::match_runtime::sim_tick::{RuntimePassInputs, SessionMode, decide_runtime_pass};
use serde::Deserialize;

#[derive(Deserialize)]
struct Expected {
    crossed: bool,
    engaged: bool,
    motion: [i32; 2],
    band_box_cancel_points: Vec<[i32; 2]>,
}

#[derive(Deserialize)]
struct DragCase {
    name: String,
    anchor: [f32; 2],
    cursor: [f32; 2],
    viewport: [f32; 2],
    drag_metrics: [i32; 2],
    scroll_rate: u32,
    scroll_method: i32,
    crossed_before: bool,
    engaged_before: bool,
    band_box: bool,
    game_active: bool,
    tactical_active: bool,
    map_editor: bool,
    expected: Expected,
}

fn vectors() -> serde_json::Value {
    serde_json::from_str(crate::test_fixture::text(
        "tools/input_oracle/fast_scroll.json",
    ))
    .unwrap()
}

fn mouse_for(case: &DragCase) -> TacticalMouseState {
    TacticalMouseState {
        captured: true,
        right_held: true,
        right_anchor: case.anchor.into(),
        right_threshold_crossed: case.crossed_before,
        right_pan_engaged: case.engaged_before,
        ..Default::default()
    }
}

fn check_poll(mouse: &mut TacticalMouseState, case: &DragCase) {
    let poll = mouse.poll_right_drag(case.cursor.into(), case.drag_metrics.into(), case.band_box);
    let motion = if poll == RightDragPoll::Pan {
        right_drag_pan_step(
            mouse.right_anchor,
            case.cursor.into(),
            case.viewport[0],
            case.viewport[1],
            case.scroll_rate,
        )
    } else {
        (0.0, 0.0)
    };
    assert_eq!(
        motion,
        (
            case.expected.motion[0] as f32,
            case.expected.motion[1] as f32
        ),
        "{}",
        case.name
    );
    assert_eq!(
        mouse.right_threshold_crossed, case.expected.crossed,
        "{}",
        case.name
    );
    assert_eq!(
        mouse.right_pan_engaged, case.expected.engaged,
        "{}",
        case.name
    );
    assert_eq!(
        poll == RightDragPoll::CancelBandBox,
        !case.expected.band_box_cancel_points.is_empty(),
        "{}",
        case.name
    );
}

#[test]
fn stock_right_drag_matches_original_executable() {
    let cases: Vec<DragCase> = serde_json::from_value(vectors()["cases"].clone()).unwrap();
    let mut checked = 0;
    for case in cases {
        // The live stock path is method 0 in an active tactical game. Alternate
        // warp methods/editor admission remain characterization, not Rust parity.
        if case.scroll_method != 0 || !case.game_active || !case.tactical_active || case.map_editor
        {
            continue;
        }
        check_poll(&mut mouse_for(&case), &case);
        checked += 1;
    }
    assert_eq!(checked, 738);
}

#[test]
fn held_pointer_and_return_to_anchor_match_native_sequence() {
    let data = vectors();
    for sequence in data["sequences"].as_array().unwrap() {
        let mut input = sequence["input"].clone();
        input["expected"] = sequence["frames"][0]["expected"].clone();
        let mut case: DragCase = serde_json::from_value(input).unwrap();
        let mut mouse = mouse_for(&case);
        for frame in sequence["frames"].as_array().unwrap() {
            case.cursor = serde_json::from_value(frame["cursor"].clone()).unwrap();
            case.expected = serde_json::from_value(frame["expected"].clone()).unwrap();
            check_poll(&mut mouse, &case);
        }
    }
}

#[test]
fn right_release_cancel_decision_matches_native_messages() {
    let data = vectors();
    let mut checked = 0;
    for row in data["message_cases"].as_array().unwrap() {
        if row["message"] != 0x205 {
            continue;
        }
        let mut mouse = TacticalMouseState {
            captured: row["captured"].as_bool().unwrap(),
            right_held: true,
            right_threshold_crossed: row["crossed_before"].as_bool().unwrap(),
            ..Default::default()
        };
        let expected = &row["expected"];
        let cancel = mouse.end_right_press();
        let calls = expected["calls"].as_array().unwrap();
        assert_eq!(
            cancel.is_some(),
            calls.iter().any(|call| call == "release_capture")
        );
        assert_eq!(
            cancel == Some(true),
            calls.iter().any(|call| call == "cancel_selection_or_mode")
        );
        assert_eq!(mouse.captured, expected["captured"].as_bool().unwrap());
        assert_eq!(
            mouse.right_threshold_crossed,
            expected["crossed"].as_bool().unwrap()
        );
        checked += 1;
    }
    assert_eq!(checked, 4);
}

#[test]
fn capture_and_left_button_priority_match_native_update() {
    let data = vectors();
    let mut checked = 0;
    for row in data["update_cases"].as_array().unwrap() {
        if !matches!(
            row["name"].as_str().unwrap(),
            "held_right" | "held_both" | "held_left" | "no_live_buttons" | "uncaptured_right"
        ) {
            continue;
        }
        let mut input = row.clone();
        input["expected"] = row["expected"]["drag"].clone();
        let case: DragCase = serde_json::from_value(input).unwrap();
        let mut mouse = mouse_for(&case);
        mouse.captured = row["captured"].as_bool().unwrap();
        mouse.left_held = row["left_held"].as_bool().unwrap();
        mouse.right_held = row["right_held"].as_bool().unwrap();
        check_poll(&mut mouse, &case);
        checked += 1;
    }
    assert_eq!(checked, 5);
}

#[test]
fn a_drag_between_offline_polls_remains_a_cancel_click() {
    let data = vectors();
    let mut checked = 0;
    for row in data["throttle_cases"].as_array().unwrap() {
        let mode = row["session_mode"].as_i64().unwrap() as i32;
        if !matches!(mode, 0 | 5) {
            continue;
        }
        let mut mouse = TacticalMouseState {
            right_held: true,
            ..Default::default()
        };
        mouse.begin_right_drag((400.0, 300.0));
        let decision = decide_runtime_pass(RuntimePassInputs {
            exact_step: false,
            window_active: true,
            startup_admitted: true,
            frame_stepping: false,
            paused: false,
            menu_open: false,
            session_mode: SessionMode::from_game_mode(mode),
            pacer_timing_admits: false,
        });
        assert_eq!(
            usize::from(decision.scroll_input),
            row["expected"]["input_calls"].as_u64().unwrap() as usize
        );
        if decision.scroll_input {
            mouse.poll_right_drag((500.0, 300.0), (4, 4), false);
        }
        // Moving far and releasing during the throttle wait cannot cross a
        // latch which only the native admitted poll is allowed to write.
        assert_eq!(mouse.end_right_press(), Some(true), "{}", row["name"]);
        checked += 1;
    }
    assert_eq!(checked, 10);
}

#[derive(Deserialize)]
struct CameraSequence {
    name: String,
    viewport: [f32; 2],
    map_rect_width: i32,
    local_size: [i32; 4],
    initial_center_seed: [f32; 2],
    initial_center: [f32; 2],
    drag_anchor: [f32; 2],
    scroll_rate: u32,
    drag_metrics: [i32; 2],
    steps: Vec<serde_json::Value>,
}

/// Same normalized MapClass geometry owner used by the live camera. Native
/// clamp6D8640 reads these final fields; radar's enumerated tile extent is a
/// separate consumer and must not supply the camera's vertical headroom.
fn camera_fixture_bounds(
    map_rect_width: i32,
    local_size: [i32; 4],
) -> crate::map::terrain::LocalBounds {
    let [left, top, width, height] = local_size;
    let bounds = crate::map::playfield::PlayfieldBounds::from_normalized_local_size(
        map_rect_width,
        left,
        top,
        width,
        height,
    );
    crate::map::terrain::LocalBounds::from_playfield(bounds)
}

#[test]
fn pending_requests_commit_once_and_absolute_views_discard_them() {
    use super::{
        PendingCameraScroll, clamp_camera_point_to_local_bounds, tactical_camera_top_left,
    };
    let sequences: Vec<CameraSequence> =
        serde_json::from_value(vectors()["camera_sequences"].clone()).unwrap();
    assert_eq!(sequences.len(), 10);
    let mut commits = 0;
    for sequence in sequences {
        let bounds = camera_fixture_bounds(sequence.map_rect_width, sequence.local_size);
        let viewport = sequence.viewport.into();
        let clamp = |point| {
            clamp_camera_point_to_local_bounds(
                point,
                (
                    bounds.pixel_x,
                    bounds.pixel_y,
                    bounds.pixel_w,
                    bounds.pixel_h,
                ),
                viewport,
                1.0,
            )
        };
        // Original refresh0x6D8B30 converts center to top-left with viewport/2.
        // VERA's shared projection adds its world-row bias; use that owner to
        // obtain the offset, rather than duplicating its literal15 here.
        let world_bias = crate::util::lepton::absolute_leptons_to_screen(0, 0, 0);
        let native_to_world = |center: [f32; 2]| {
            tactical_camera_top_left(
                (center[0] + world_bias.0, center[1] + world_bias.1),
                sequence.viewport[0],
                sequence.viewport[1],
                1.0,
            )
        };
        let mut current = clamp(native_to_world(sequence.initial_center_seed));
        assert_eq!(
            current,
            native_to_world(sequence.initial_center),
            "{} initial clamp",
            sequence.name
        );
        let mut pending = PendingCameraScroll::default();
        let mut mouse_request = (0.0, 0.0);
        let mut mouse = TacticalMouseState {
            right_held: true,
            ..Default::default()
        };
        mouse.begin_right_drag(sequence.drag_anchor.into());
        for step in sequence.steps {
            match step["operation"].as_str().unwrap() {
                "scroll" => {
                    let direction = step["direction"].as_u64().unwrap() as usize;
                    let distance = step["distance"].as_f64().unwrap() as f32;
                    let (dx, dy) = super::OCTANT_DELTA[direction];
                    pending.request((dx * distance, dy * distance));
                }
                "drag" => {
                    let cursor: [f32; 2] = serde_json::from_value(step["cursor"].clone()).unwrap();
                    assert_eq!(
                        mouse.poll_right_drag(cursor.into(), sequence.drag_metrics.into(), false),
                        RightDragPoll::Pan
                    );
                    let delta = right_drag_pan_step(
                        sequence.drag_anchor.into(),
                        cursor.into(),
                        sequence.viewport[0],
                        sequence.viewport[1],
                        sequence.scroll_rate,
                    );
                    mouse_request.0 += delta.0;
                    mouse_request.1 += delta.1;
                }
                "commit" => {
                    current = std::mem::take(&mut pending).commit(current, mouse_request, clamp);
                    mouse_request = (0.0, 0.0);
                    commits += 1;
                }
                "set_view" => {
                    let coord: [i32; 3] = serde_json::from_value(step["coord"].clone()).unwrap();
                    let world = crate::util::lepton::absolute_leptons_to_screen(
                        coord[0], coord[1], coord[2],
                    );
                    current = clamp(tactical_camera_top_left(
                        world,
                        sequence.viewport[0],
                        sequence.viewport[1],
                        1.0,
                    ));
                    pending.clear();
                    mouse_request = (0.0, 0.0);
                }
                operation => panic!("unexpected native camera operation {operation}"),
            }
            let expected: [f32; 2] =
                serde_json::from_value(step["expected"]["committed_center"].clone()).unwrap();
            assert_eq!(
                current,
                native_to_world(expected),
                "{} after {}",
                sequence.name,
                step["operation"]
            );
        }
        assert!(
            pending.is_idle(),
            "{} must leave no stale keyboard request",
            sequence.name
        );
    }
    assert_eq!(commits, 13);
}

#[test]
fn saved_camera_center_and_new_viewport_match_original_load_and_resize() {
    use super::{
        clamp_camera_point_to_local_bounds, saved_camera_center, set_camera_position,
        tactical_camera_top_left,
    };
    let data = vectors();
    let cases = data["camera_save_load"].as_array().unwrap();
    assert_eq!(cases.len(), 13);
    let world_bias = crate::util::lepton::absolute_leptons_to_screen(0, 0, 0);
    let mut checked = 0;
    for row in cases {
        let source = &row["input"];
        // The map-editor no-clamp gate remains characterization: ordinary
        // production VERA has no editor-mode saved-game route.
        if source["map_editor"].as_u64().unwrap() != 0 {
            continue;
        }
        let name = source["name"].as_str().unwrap();
        let viewport: [u32; 2] = serde_json::from_value(source["viewport"].clone()).unwrap();
        let saved_center: [f32; 2] =
            serde_json::from_value(source["saved_center"].clone()).unwrap();
        let pending: [f32; 2] = serde_json::from_value(source["saved_requested"].clone()).unwrap();
        let local_size = serde_json::from_value(source["local_size"].clone()).unwrap();
        let bounds = camera_fixture_bounds(
            source["map_rect_width"].as_i64().unwrap() as i32,
            local_size,
        );
        let area = (
            bounds.pixel_x,
            bounds.pixel_y,
            bounds.pixel_w,
            bounds.pixel_h,
        );
        let center = (
            saved_center[0] + world_bias.0,
            saved_center[1] + world_bias.1,
        );
        let point = tactical_camera_top_left(center, viewport[0] as f32, viewport[1] as f32, 1.0);
        let mut input = crate::app::input::state::MatchInputState::new(Default::default());
        input.pending_camera_scroll.request(pending.into());
        let clamped = clamp_camera_point_to_local_bounds(
            point,
            area,
            (viewport[0] as f32, viewport[1] as f32),
            1.0,
        );
        set_camera_position(&mut input, clamped);
        for (expected, dimensions) in [
            (&row["loaded"], viewport),
            (
                &row["resized"],
                serde_json::from_value(source["resize"].clone()).unwrap_or(viewport),
            ),
        ] {
            if expected.is_null() {
                continue;
            }
            let center = saved_camera_center(&input, (viewport[0] + 168, viewport[1] + 32));
            if dimensions != viewport {
                let point = tactical_camera_top_left(
                    center.into(),
                    dimensions[0] as f32,
                    dimensions[1] as f32,
                    1.0,
                );
                set_camera_position(
                    &mut input,
                    clamp_camera_point_to_local_bounds(
                        point,
                        area,
                        (dimensions[0] as f32, dimensions[1] as f32),
                        1.0,
                    ),
                );
            }
            let native_top_left: [f32; 2] =
                serde_json::from_value(expected["top_left"].clone()).unwrap();
            assert_eq!(
                (input.camera_x, input.camera_y),
                (
                    native_top_left[0] + world_bias.0,
                    native_top_left[1] + world_bias.1
                ),
                "{name}"
            );
            let native_center: [f32; 2] =
                serde_json::from_value(expected["committed"].clone()).unwrap();
            assert_eq!(
                saved_camera_center(&input, (dimensions[0] + 168, dimensions[1] + 32)),
                [
                    native_center[0] + world_bias.0,
                    native_center[1] + world_bias.1
                ],
                "{name}"
            );
            assert_eq!(expected["committed"], expected["requested"], "{name}");
            assert!(input.pending_camera_scroll.is_idle(), "{name}");
            checked += 1;
        }
    }
    assert_eq!(checked, 13);
}

#[test]
fn saved_world_center_is_independent_of_the_current_vera_zoom_and_viewport() {
    let mut input = crate::app::input::state::MatchInputState::new(Default::default());
    input.camera_x = 100.25;
    input.camera_y = 2_000.5;
    input.zoom_level = 2.0;
    let saved = super::saved_camera_center(&input, (809, 513));
    let (width, height) = super::tactical_viewport_size_px(1_193, 801);
    input.zoom_level = 0.5;
    let point = super::tactical_camera_top_left(
        saved.into(),
        width as f32,
        height as f32,
        input.zoom_level,
    );
    super::set_camera_position(&mut input, point);
    assert_eq!(super::saved_camera_center(&input, (1_193, 801)), saved);
}
