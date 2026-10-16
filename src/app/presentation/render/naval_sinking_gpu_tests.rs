//! Original Unit73BEA4 rectangle outputs through actual Ground lowering,
//! instance upload and voxel fragment clipping. Raster geometry is supplied
//! here; the physical AEGIS raster has its own original-instruction packet.
use super::*;
use crate::app::presentation::render::draw_plan_lowering::{
    ObjectPieceInstance, PlannedObjectInstance, lower_ground_object_instances,
};
use crate::render::batch::InstanceBufferPool;
use crate::render::sinking::{SinkingWaterlines, apply_waterline_clip, tests};
use crate::render::tactical_draw_plan::{BlitPolicy, ObjectDraw, SpriteEncoding, TacticalLayer};
use crate::render::terrain_draw::TerrainDrawRenderer;
use crate::render::terrain_draw_gpu_tests::{Gpu, camera, clear, encoded, sprite};

#[test]
#[ignore = "requires GPU; original Unit waterline rectangles through production voxel replay"]
fn native_sinking_clip_survives_camera_scissor_and_ground_upload() {
    let gpu = Gpu::new();
    let size = [640, 480];
    let format = wgpu::TextureFormat::Bgra8UnormSrgb;
    let batch = BatchRenderer::new_with_device(&gpu.device, &gpu.queue, format);
    let color = gpu.target(size, format);
    let cv = color.create_view(&Default::default());
    let depth = gpu.target(size, wgpu::TextureFormat::Depth32Float);
    let dv = depth.create_view(&Default::default());
    let mut terrain = TerrainDrawRenderer::new(&gpu.device, &gpu.queue, format, &batch);
    let units = UnitAtlas::from_test_pages(vec![crate::render::unit_atlas::UnitAtlasPage {
        texture: batch.create_unit_atlas_texture_on_device(&gpu.device, &gpu.queue, 1, 1, &[33]),
    }]);
    let palette = crate::assets::pal_file::Palette {
        colors: [crate::assets::pal_file::Color::rgb(0, 252, 0); 256],
    };
    let ramps = crate::rules::house_colors::HouseColorRamps::from_schemes(&[]);
    let palettes =
        PaletteSet::new_on_device(&gpu.device, &gpu.queue, &palette, &palette, &ramps, &[]);
    let mut pool = InstanceBufferPool::new();
    let mut visits = 0;
    for row in tests::unit_rows() {
        let mut cache = SinkingWaterlines::default();
        cache.restore([(1, row["input"]["waterline"].as_i64().unwrap() as i16)]);
        let steps = row["sequence"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![row.clone()]);
        for step in steps {
            let input = &step["input"];
            let world_y = input["camera_world_y"].as_i64().unwrap() as f32;
            let mut view = camera(size);
            view.camera_pos[1] = world_y;
            batch.write_camera(&gpu.queue, view);
            terrain.prepare(&gpu.device, &color, &dv, batch.camera_uniform());
            // Fill the viewport so every pixel of the native clip rectangle
            // is exercised independently of the raster's opaque coverage.
            let mut body = sprite([0.0, world_y], [640.0, 480.0], -1.0);
            body.z_gradient = crate::render::native_z::pack_voxel_z_gradient(
                crate::render::native_z::ZGradient::Vertical,
                false,
            );
            body.zshape_origin = [world_y, 480.0];
            apply_waterline_clip(&mut body.draw_state, tests::draw_input(&mut cache, input));
            let ground = lower_ground_object_instances(vec![PlannedObjectInstance::object(
                ObjectDraw {
                    id: 1,
                    layer: TacticalLayer(2),
                    display_order: 0,
                    policy: BlitPolicy::z_read(SpriteEncoding::Plain),
                },
                vec![ObjectPieceInstance {
                    target: ObjectTexture::UnitAtlasPage(0),
                    render_z: RenderZPolicy::ReadOnly,
                    instance: body,
                }],
            )]);
            pool.upload_on_device(&gpu.device, &gpu.queue, "ground_objects", &ground.instances);
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            clear(
                &mut encoder,
                &cv,
                &dv,
                65535,
                wgpu::LoadOp::Clear(wgpu::Color::WHITE),
            );
            let caller: [u32; 4] =
                std::array::from_fn(|i| input["caller_rect"][i].as_u64().unwrap() as u32);
            draw_native_object_pass(
                &mut encoder,
                &cv,
                &dv,
                &mut terrain,
                caller,
                &batch,
                pool.get("ground_objects"),
                &ground,
                None,
                Some(&units),
                None,
                &VxlSlopeTransitionCache::default(),
                None,
                Some(&palettes),
                batch.default_zshape_bind_group(),
            );
            let reads = [
                gpu.read(&mut encoder, &color),
                gpu.read(&mut encoder, &depth),
            ];
            let output = gpu.finish(encoder, &reads, size);
            let clip: [i32; 4] =
                std::array::from_fn(|i| step["output"]["clip"][i].as_i64().unwrap() as i32);
            for y in 0..480 {
                for x in 0..640 {
                    let in_clip = x >= clip[0]
                        && x < clip[0] + clip[2]
                        && y >= clip[1]
                        && y < clip[1] + clip[3];
                    let offset = (y as usize * 640 + x as usize) * 4;
                    assert_eq!(
                        &output[0][offset..offset + 4],
                        if in_clip {
                            encoded(0x07e0, format)
                        } else {
                            [255; 4]
                        },
                        "{} visit{visits} pixel{x},{y}",
                        row["name"]
                    );
                    assert_eq!(
                        crate::render::native_z::stored_z(f32::from_le_bytes(
                            output[1][offset..offset + 4].try_into().unwrap()
                        )),
                        65535,
                        "sinking never changes terrain depth"
                    );
                }
            }
            visits += 1;
        }
    }
    assert_eq!(visits, 16);
}
