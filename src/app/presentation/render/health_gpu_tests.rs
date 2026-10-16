//! Original ordinary Techno health calls through the actual shared
//! producer, retail resources and production replay. Location, HP, selection,
//! hover and surface planes are consumer inputs, not lifecycle parity claims.
use super::draw_plan_lowering::{
    NativeDisplayOrder, PlannedObjectInstance, lower_ground_object_instances,
};
use crate::app::presentation::ui_overlays::{HealthViewport, health_strip_for_parent};
use crate::assets::asset_manager::{AssetManager, MediaArchiveMode};
use crate::map::entities::EntityCategory;
use crate::render::batch::{BatchRenderer, InstanceBufferPool};
use crate::render::selection_overlay::SelectionOverlay;
use crate::render::shroud_buffer::ShroudBuffer;
use crate::render::tactical_draw_plan::SpriteEncoding;
use crate::render::terrain_draw::TerrainDrawRenderer;
use crate::render::terrain_draw_gpu_tests::{Gpu, camera, clear, encoded};
use crate::sim::{
    combat::Health, components::DriveCoord, game_entity::GameEntity, world::Simulation,
};
use serde_json::Value;

#[test]
#[ignore = "requires retail assets and GPU; original MTNK health strip A/Z and Display replay"]
fn native_mobile_health_strips_preserve_color_a_and_z() {
    native_health_controls(
        "tools/procedural_drawing_oracle/unit_health.json",
        "MTNK",
        EntityCategory::Unit,
    );
}

#[test]
#[ignore = "requires retail assets and GPU; original GAPOWR health colors, phases and unchanged A/Z"]
fn native_building_health_strips_preserve_color_a_and_z() {
    native_health_controls(
        "tools/procedural_drawing_oracle/building_health.json",
        "GAPOWR",
        EntityCategory::Structure,
    );
}

fn native_health_controls(fixture: &'static str, type_name: &str, category: EntityCategory) {
    let native = crate::test_fixture::json(fixture);
    let retail = crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")
        .expect("physical Battle INIs");
    let rules = &retail.rules;
    let root = std::path::PathBuf::from(std::env::var_os("RA2_DIR").expect("retail root"));
    let assets = AssetManager::new(&root, MediaArchiveMode::STOCK_DIGITAL).unwrap();
    for source in native["physical_inputs"].as_array().unwrap() {
        let name = source["name"].as_str().unwrap();
        assert_eq!(
            crate::util::sha256::sha256_hex(assets.get_ref(name).unwrap()),
            source["sha256"].as_str().unwrap(),
            "physical input {name}"
        );
    }
    let gpu = Gpu::new();
    for format in [
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let mut batch = BatchRenderer::new_with_device(&gpu.device, &gpu.queue, format);
        let overlay =
            SelectionOverlay::new_on_device(&gpu.device, &gpu.queue, &batch, Some(&assets));
        let mut pool = InstanceBufferPool::new();
        for row in native["raster_controls"].as_array().unwrap().iter().chain(
            native["composition_controls"]
                .as_array()
                .into_iter()
                .flatten(),
        ) {
            let composed = row.get("before_body_pixels").is_some();
            let input = &row["input"];
            let size = pair(&input["size"]);
            let mut cam = camera(size);
            // The shared projection's world-row origin is +15. Native camera
            // and original Location are supplied independently in the corpus.
            cam.camera_pos = [
                input["camera"][0].as_i64().unwrap() as f32,
                input["camera"][1].as_i64().unwrap() as f32
                    + crate::util::lepton::absolute_leptons_to_screen(0, 0, 0).1,
            ];
            let mut sim = Simulation::new();
            let owner = sim.interner.intern("Owner");
            let type_ref = sim.interner.intern(type_name);
            let mut parents = Vec::new();
            let ids = if composed {
                row["body_admissions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|body| if body["actor"] == 0 { 901 } else { 2 })
                    .collect()
            } else if input["actors"].as_array().unwrap().len() == 1 {
                vec![901]
            } else {
                vec![901, 2]
            };
            let display = NativeDisplayOrder::new(&ids);
            for (index, actor) in input["actors"].as_array().unwrap().iter().enumerate() {
                let id = if index == 0 { 901 } else { 2 };
                let xyz: [i32; 3] =
                    std::array::from_fn(|i| actor["location"][i].as_i64().unwrap() as i32);
                let mut entity = GameEntity::new_at_frame_zero_for_test(
                    id,
                    (xyz[0] / 256) as u16,
                    (xyz[1] / 256) as u16,
                    0,
                    0,
                    owner,
                    Health::new(
                        actor["health"]
                            .as_i64()
                            .unwrap_or(i64::from(rules.object(type_name).unwrap().strength))
                            as i32,
                    ),
                    type_ref,
                    category,
                    0,
                    5,
                    false,
                );
                entity
                    .type_facts
                    .set_foundation_for_test(rules.object(type_name).unwrap().foundation.clone());
                entity.position.put_location(DriveCoord {
                    x: xyz[0],
                    y: xyz[1],
                    z: xyz[2],
                });
                entity.lifecycle.set_in_limbo_for_test(false);
                entity
                    .selection
                    .set_selected_for_test(actor["selected"].as_bool().unwrap());
                sim.entities_mut().insert(entity);
                sim.resubmit_entity_display_for_test(id);
                if actor["sinking"].as_bool().unwrap_or(false) {
                    sim.begin_ship_sinking(id, rules, crate::sim::world::FrameEffects::default());
                }
                let strip = health_strip_for_parent(
                    &sim,
                    sim.entities().get(id).unwrap(),
                    Some(rules),
                    &overlay,
                    actor["hovered"].as_bool().unwrap_or(false).then_some(id),
                    Some("Owner"),
                    HealthViewport {
                        camera: cam.camera_pos,
                        size: size.map(|x| x as f32),
                    },
                );
                let native_pips: Vec<_> = row["draws"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|draw| {
                        draw["asset"] == "PIPS.SHP"
                            && draw["actor"].as_u64() == Some(index as u64)
                            && (!composed || draw["phase"] == "first_extras")
                    })
                    .collect();
                assert_eq!(
                    strip.fills.len(),
                    native_pips.len(),
                    "{} actor{index} pip commands",
                    row["name"]
                );
                for (instance, draw) in strip.fills.iter().zip(native_pips) {
                    assert_eq!(draw["flags"], 0x600);
                    let expected = overlay
                        .pips()
                        .unwrap()
                        .instance(
                            draw["frame"].as_u64().unwrap() as usize,
                            (
                                draw["point"][0].as_i64().unwrap() as f32 + cam.camera_pos[0],
                                draw["point"][1].as_i64().unwrap() as f32 + cam.camera_pos[1],
                            ),
                            instance.depth,
                        )
                        .unwrap();
                    assert_eq!(
                        instance.position, expected.position,
                        "{} native point",
                        row["name"]
                    );
                    assert_eq!(
                        instance.uv_origin, expected.uv_origin,
                        "{} native frame",
                        row["name"]
                    );
                }
                let draw = display.object_draw(id, SpriteEncoding::Plain).unwrap();
                let mut parent = if category == EntityCategory::Structure {
                    PlannedObjectInstance::building(draw, vec![])
                } else {
                    PlannedObjectInstance::object(draw, vec![])
                };
                parent.health = Some(strip);
                parents.push(parent);
            }
            let a_input =
                vec![input["a_value"].as_u64().unwrap() as u8; (size[0] * size[1]) as usize];
            let a = ShroudBuffer::fixture(&gpu.device, &gpu.queue, size, cam.camera_pos, &a_input);
            batch.bind_shroud(&gpu.device, Some(&a));
            batch.write_camera(&gpu.queue, cam);
            // Builders can discover these in ID/atlas order. The exact same
            // lowering used by the app restores retained Display order.
            parents.reverse();
            let layer = lower_ground_object_instances(parents);
            pool.upload_on_device(
                &gpu.device,
                &gpu.queue,
                "object_extras",
                &layer.extras_instances,
            );
            let color = gpu.target(size, format);
            let depth = gpu.target(size, wgpu::TextureFormat::Depth32Float);
            let cv = color.create_view(&Default::default());
            let dv = depth.create_view(&Default::default());
            let background = input["background"].as_u64().unwrap_or(0) as u16;
            let mut initial = encoded(background, format).repeat((size[0] * size[1]) as usize);
            if composed {
                install_pixels(&mut initial, &row["ground_pixels"], size, format);
            }
            write_color(&gpu, &color, size, &initial);
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            let old_z = input["old_z"].as_u64().unwrap_or(u64::from(u16::MAX)) as u16;
            let mut z = vec![old_z; (size[0] * size[1]) as usize];
            if composed {
                install_z(&mut z, &row["ground_z"], size);
                crate::render::terrain_draw_gpu_tests::seed_depth_grid(
                    &gpu,
                    &mut encoder,
                    &dv,
                    &z,
                    size[0],
                );
            } else {
                clear(&mut encoder, &cv, &dv, old_z, wgpu::LoadOp::Load);
            }
            let mut terrain = TerrainDrawRenderer::new(&gpu.device, &gpu.queue, format, &batch);
            terrain.prepare(&gpu.device, &color, &dv, batch.camera_uniform());
            if category == EntityCategory::Structure {
                // Replay the actual initial Building Extras marker with the
                // same instance range used by the app's later Extras sweep.
                super::merge_passes::draw_native_object_pass(
                    &mut encoder,
                    &cv,
                    &dv,
                    &mut terrain,
                    [0, 0, size[0], size[1]],
                    &batch,
                    None,
                    &layer,
                    None,
                    None,
                    None,
                    None,
                    None,
                    batch.default_zshape_bind_group(),
                    overlay_resource(&overlay, &pool),
                );
                let reads = [
                    gpu.read(&mut encoder, &color),
                    gpu.read(&mut encoder, &depth),
                ];
                let output = gpu.finish(encoder, &reads, size);
                assert_pixels(
                    &output[0],
                    &initial,
                    if composed {
                        &row["before_body_pixels"]
                    } else {
                        &row["pixels"]
                    },
                    size,
                    format,
                    composed,
                    row,
                );
                assert_z(&output[1], &z, row);
                encoder = gpu.device.create_command_encoder(&Default::default());
                if composed {
                    // Original body admission, shape and BUILDNGZ execution
                    // supplied these sampled color/Z values. This seam checks
                    // the health consumer across a real body interval; it
                    // makes no GPU-body or whole-frame parity claim.
                    install_pixels(&mut initial, &row["before_late_pixels"], size, format);
                    write_color(&gpu, &color, size, &initial);
                    install_z(&mut z, &row["before_late_z"], size);
                    crate::render::terrain_draw_gpu_tests::seed_depth_grid(
                        &gpu,
                        &mut encoder,
                        &dv,
                        &z,
                        size[0],
                    );
                    assert!(row["body_admissions"].as_array().unwrap().len() > 0);
                }
            }
            super::object_extras::draw_object_extras(
                &mut encoder,
                &cv,
                &dv,
                &mut terrain,
                [0, 0, size[0], size[1]],
                &batch,
                Some(&overlay),
                pool.get("object_extras").map(|(buffer, _)| buffer),
                &layer,
            );
            if category == EntityCategory::Unit {
                if row["name"] == "healthy_a127" {
                    use super::draw_plan_lowering::{ObjectExtrasRun, ObjectLayerPass};
                    let mut fenced = ObjectLayerPass::default();
                    let ObjectExtrasRun::Health {
                        background, fills, ..
                    } = &layer.extras_runs[0]
                    else {
                        panic!("native healthy strip must retain its parent run");
                    };
                    // A clipped building's empty line batch still separates
                    // adjacent health groups. Keep this below Metal's pool limit
                    // so the missing-accounting regression fails without hanging.
                    // These opaque native stores are idempotent; final color/A/Z
                    // must still match the same original healthy control.
                    for parent in 0..129 {
                        fenced.extras_runs.push(ObjectExtrasRun::Health {
                            parent,
                            background: *background,
                            fills: fills.clone(),
                        });
                        fenced.extras_runs.push(ObjectExtrasRun::Lines {
                            parent: parent + 1000,
                            lines: 0..0,
                            batch: Default::default(),
                        });
                    }
                    super::object_extras::draw_object_extras(
                        &mut encoder,
                        &cv,
                        &dv,
                        &mut terrain,
                        [0, 0, size[0], size[1]],
                        &batch,
                        Some(&overlay),
                        pool.get("object_extras").map(|(buffer, _)| buffer),
                        &fenced,
                    );
                    assert!(
                        terrain.submissions() > 0,
                        "closed health passes must reach the shared submission owner before encoder finish"
                    );
                }
            }
            let reads = [
                gpu.read(&mut encoder, &color),
                gpu.read(&mut encoder, &depth),
            ];
            let output = gpu.finish(encoder, &reads, size);
            assert_pixels(
                &output[0],
                &initial,
                &row["pixels"],
                size,
                format,
                composed,
                row,
            );
            assert_z(&output[1], &z, row);
            let a_actual = gpu.read_float_texels(a.gpu_source().0, [0, 0], size);
            for (actual, expected) in a_actual.iter().zip(&a_input) {
                assert_eq!(
                    (actual[0] * 255.0).round() as u8,
                    *expected,
                    "{} A unchanged",
                    row["name"]
                );
            }
        }
    }
}

fn overlay_resource<'a>(
    overlay: &'a SelectionOverlay,
    pool: &'a InstanceBufferPool,
) -> Option<(&'a SelectionOverlay, &'a wgpu::Buffer)> {
    pool.get("object_extras")
        .map(|(buffer, _)| (overlay, buffer))
}

fn write_color(gpu: &Gpu, color: &wgpu::Texture, size: [u32; 2], bytes: &[u8]) {
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: color,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size[0] * 4),
            rows_per_image: Some(size[1]),
        },
        color.size(),
    );
}

fn install_pixels(bytes: &mut [u8], pixels: &Value, size: [u32; 2], format: wgpu::TextureFormat) {
    for pixel in pixels.as_array().unwrap() {
        let offset = (pixel[1].as_u64().unwrap() as usize * size[0] as usize
            + pixel[0].as_u64().unwrap() as usize)
            * 4;
        bytes[offset..offset + 4]
            .copy_from_slice(&encoded(pixel[2].as_u64().unwrap() as u16, format));
    }
}

fn install_z(z: &mut [u16], pixels: &Value, size: [u32; 2]) {
    for pixel in pixels.as_array().unwrap() {
        z[pixel[1].as_u64().unwrap() as usize * size[0] as usize
            + pixel[0].as_u64().unwrap() as usize] = pixel[2].as_u64().unwrap() as u16;
    }
}

fn assert_pixels(
    actual: &[u8],
    initial: &[u8],
    pixels: &Value,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    sampled: bool,
    row: &Value,
) {
    let mut expected = initial.to_vec();
    install_pixels(&mut expected, pixels, size, format);
    if sampled {
        for pixel in pixels.as_array().unwrap() {
            let (x, y) = (
                pixel[0].as_u64().unwrap() as usize,
                pixel[1].as_u64().unwrap() as usize,
            );
            let offset = (y * size[0] as usize + x) * 4;
            assert_eq!(
                &actual[offset..offset + 4],
                &expected[offset..offset + 4],
                "{} {format:?} sample({x},{y})",
                row["name"]
            );
        }
    } else {
        for (index, pixel) in actual.chunks_exact(4).enumerate() {
            assert_eq!(
                pixel,
                &expected[index * 4..index * 4 + 4],
                "{} {format:?} color({},{})",
                row["name"],
                index % size[0] as usize,
                index / size[0] as usize
            );
        }
    }
}

fn assert_z(actual: &[u8], expected: &[u16], row: &Value) {
    for (index, pixel) in actual.chunks_exact(4).enumerate() {
        assert_eq!(
            crate::render::native_z::stored_z(f32::from_le_bytes(pixel.try_into().unwrap())),
            expected[index],
            "{} Z{index}",
            row["name"]
        );
    }
}

fn pair(value: &Value) -> [u32; 2] {
    std::array::from_fn(|i| value[i].as_u64().unwrap() as u32)
}

#[test]
fn retail_mtnk_health_fields_match_original_layered_readers() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")
    else {
        return;
    };
    let native = crate::test_fixture::json("tools/procedural_drawing_oracle/unit_health.json");
    let result = &native["readers"]["result"];
    let object = retail.rules.object("MTNK").unwrap();
    assert_eq!(object.strength, result["strength"].as_i64().unwrap() as i32);
    assert_eq!(
        object.pixel_selection_bracket_delta,
        result["delta"].as_i64().unwrap() as i32
    );
    assert_eq!(object.pip_scale, crate::rules::object_type::PipScale::None);
    assert_eq!(
        retail.rules.general.condition_red,
        result["condition_red"].as_f64().unwrap()
    );
    assert_eq!(
        retail.rules.general.condition_yellow,
        result["condition_yellow"].as_f64().unwrap()
    );
}

#[test]
fn retail_gapowr_health_fields_match_original_layered_readers() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")
    else {
        return;
    };
    let native = crate::test_fixture::json("tools/procedural_drawing_oracle/building_health.json");
    let result = &native["readers"]["result"];
    let object = retail.rules.object("GAPOWR").unwrap();
    assert_eq!(object.strength, result["strength"].as_i64().unwrap() as i32);
    assert_eq!(
        object.pixel_selection_bracket_delta,
        result["delta"].as_i64().unwrap() as i32
    );
    assert_eq!(object.pip_scale, crate::rules::object_type::PipScale::None);
    assert_eq!(
        retail.rules.general.condition_red,
        result["condition_red"].as_f64().unwrap()
    );
    assert_eq!(
        retail.rules.general.condition_yellow,
        result["condition_yellow"].as_f64().unwrap()
    );
    let dimensions = retail.rules.building_dimensions(object);
    let expected: [i32; 3] =
        std::array::from_fn(|i| native["readers"]["dimensions"][i].as_i64().unwrap() as i32);
    assert_eq!(dimensions, expected);
}
