//! The sidebar's own surfaces: gamemd's `g_SidebarSurface`, and the command
//! bar along the frame's bottom.
//!
//! Both draw in their own native pixels (`ui::sidebar::frame`): the sidebar
//! into a surface 168 pixels wide, the bar into one 32 pixels tall.
//! `SidebarClass::BlitToScreen` (`6A70E0`) puts the finished sidebar in the
//! frame's right-hand column; here that copy, and the bar's into the bottom
//! strip, also scale by the player's Sidebar size, sharp at the whole sizes.
//! At Normal they are 1:1.
//!
//! Drawing into the surfaces also keeps the sidebar's depth there: the frame's
//! column keeps the cleared depth, so the tooltips and the software cursor
//! drawn after the copy always show over the radar, as gamemd draws the mouse
//! last.

use crate::render::batch::{BatchRenderer, CameraUniform};
use crate::render::scaled_copy::ScaledSurface;
use crate::ui::sidebar::SidebarFrame;

/// A part of the sidebar with a surface of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidebarPart {
    /// The right-hand column.
    Column,
    /// The command bar on the bottom strip.
    Bar,
}

pub(crate) struct SidebarSurface {
    column: PartSurface,
    bar: PartSurface,
    frame: SidebarFrame,
}

/// One part's surface, and the camera the batch pipelines draw into it with.
struct PartSurface {
    surface: ScaledSurface,
    camera_buffer: wgpu::Buffer,
    camera: wgpu::BindGroup,
}

impl SidebarSurface {
    /// Surfaces for `frame` that the batch pipelines draw in `format`, copied
    /// into a frame pass with a Depth32Float attachment.
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        batch: &BatchRenderer,
        frame: SidebarFrame,
    ) -> Self {
        let part = |label, size| {
            let (camera_buffer, camera) = batch.create_ui_camera(device);
            PartSurface {
                surface: ScaledSurface::new(
                    device,
                    label,
                    format,
                    size,
                    Some(wgpu::TextureFormat::Depth32Float),
                ),
                camera_buffer,
                camera,
            }
        };
        let surface = Self {
            column: part("Sidebar Surface", frame.surface_size()),
            bar: part("Command Bar Surface", frame.bar_surface_size()),
            frame,
        };
        surface.place(queue);
        surface
    }

    /// Follow a new frame size or Sidebar size.
    pub(crate) fn set_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: SidebarFrame,
    ) {
        if frame == self.frame {
            return;
        }
        self.column.surface.resize(device, frame.surface_size());
        self.bar.surface.resize(device, frame.bar_surface_size());
        self.frame = frame;
        self.place(queue);
    }

    fn place(&self, queue: &wgpu::Queue) {
        let frame = self.frame;
        let (scale, sharp) = (frame.scale(), frame.is_sharp());
        self.column
            .surface
            .place(queue, (frame.frame_left(), 0.0), scale, sharp);
        self.bar
            .surface
            .place(queue, frame.bar_origin(), scale, sharp);
    }

    /// Point each part's camera at its surface: the UI camera `ui`, sized to
    /// the surface, with its pixel (0, 0) at the part's corner in layout
    /// space. Write them after this frame's camera and Z origin. The bar's
    /// corner also shifts the camera row `vs_depth` reads; that moves all of
    /// the bar's layers alike, so their order stands.
    pub(crate) fn write_cameras(&self, queue: &wgpu::Queue, ui: CameraUniform) {
        let frame = self.frame;
        for (part, size, corner) in [
            (
                &self.column,
                frame.surface_size(),
                [frame.layout_left(), 0.0],
            ),
            (
                &self.bar,
                frame.bar_surface_size(),
                [0.0, frame.strip_layout_top()],
            ),
        ] {
            let uniform = CameraUniform {
                screen_size: [size.0 as f32, size.1 as f32],
                camera_pos: [ui.camera_pos[0] + corner[0], ui.camera_pos[1] + corner[1]],
                ..ui
            };
            queue.write_buffer(&part.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        }
    }

    fn part(&self, part: SidebarPart) -> &PartSurface {
        match part {
            SidebarPart::Column => &self.column,
            SidebarPart::Bar => &self.bar,
        }
    }

    /// The camera bind group that draws into `part`'s surface.
    pub(crate) fn camera(&self, part: SidebarPart) -> &wgpu::BindGroup {
        &self.part(part).camera
    }

    /// Begin the pass that draws `part`: its whole surface cleared to `clear`,
    /// depth to empty.
    pub(crate) fn begin_pass<'e>(
        &self,
        part: SidebarPart,
        encoder: &'e mut wgpu::CommandEncoder,
        clear: wgpu::Color,
    ) -> wgpu::RenderPass<'e> {
        let surface = &self.part(part).surface;
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sidebar Surface Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface.color_view(),
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: surface.depth_view(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(crate::render::native_z::STORED_DEPTH_CLEAR),
                    // Depth orders the part's own layers only.
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    /// Copy both surfaces into a frame `pass`: the sidebar into its column,
    /// the bar into the strip left of it. The scissor is left on the strip.
    pub(crate) fn composite(&self, pass: &mut wgpu::RenderPass<'_>) {
        let (x, y, width, height) = self.frame.column();
        self.column.surface.draw(pass, None, [x, y, width, height]);
        let (x, y, width, height) = self.frame.strip();
        self.bar.surface.draw(pass, None, [x, y, width, height]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wgpu::util::DeviceExt;

    // Unorm, so a blend is the plain average the box filter names.
    const UNORM: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
    const BACKDROP: [u8; 4] = [9, 9, 9, 255];

    struct Gpu {
        device: wgpu::Device,
        queue: wgpu::Queue,
    }

    fn gpu() -> Gpu {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("explicit sidebar surface GPU test requires adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        Gpu { device, queue }
    }

    fn extent((width, height): (u32, u32)) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        }
    }

    /// Write `texel(x, y)` into every pixel of `part`'s surface.
    fn fill(
        gpu: &Gpu,
        surface: &SidebarSurface,
        part: SidebarPart,
        texel: impl Fn(u32, u32) -> [u8; 4],
    ) {
        let (width, height) = match part {
            SidebarPart::Column => surface.frame.surface_size(),
            SidebarPart::Bar => surface.frame.bar_surface_size(),
        };
        let texels: Vec<u8> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| texel(x, y))
            .collect();
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: surface.part(part).surface.color_texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            extent((width, height)),
        );
    }

    /// Run `draw`, then composite the surfaces into a frame pass with a depth
    /// attachment, as the match does, and read the frame back as BGRA.
    fn composite(
        gpu: &Gpu,
        surface: &SidebarSurface,
        format: wgpu::TextureFormat,
        frame_size: (u32, u32),
        draw: impl FnOnce(&mut wgpu::CommandEncoder),
    ) -> Vec<u8> {
        let target = |format: wgpu::TextureFormat, usage: wgpu::TextureUsages| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: extent(frame_size),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | usage,
                view_formats: &[],
            })
        };
        let color = target(format, wgpu::TextureUsages::COPY_SRC);
        let depth = target(
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::empty(),
        );
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        draw(&mut encoder);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 9.0 / 255.0,
                            g: 9.0 / 255.0,
                            b: 9.0 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            surface.composite(&mut pass);
        }
        let readback = crate::render::frame_readback::PendingBgra8Readback::encode(
            &gpu.device,
            &mut encoder,
            &color,
            format,
            frame_size.0,
            frame_size.1,
        )
        .unwrap();
        let submission = gpu.queue.submit([encoder.finish()]);
        readback
            .finish(&gpu.device, submission, Duration::from_secs(20))
            .unwrap()
    }

    /// Surfaces for `frame` whose texel (x, y) is BGRA [x, y, part, 255]:
    /// part 7 for the column, 8 for the bar.
    fn tagged(gpu: &Gpu, frame: SidebarFrame) -> SidebarSurface {
        let batch = BatchRenderer::new_with_device(&gpu.device, &gpu.queue, UNORM);
        let surface = SidebarSurface::new(&gpu.device, &gpu.queue, UNORM, &batch, frame);
        fill(gpu, &surface, SidebarPart::Column, |x, y| {
            [x as u8, y as u8, 7, 255]
        });
        fill(gpu, &surface, SidebarPart::Bar, |x, y| {
            [x as u8, y as u8, 8, 255]
        });
        surface
    }

    fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * width + x) * 4) as usize;
        pixels[at..at + 4].try_into().unwrap()
    }

    #[test]
    #[ignore = "requires a wgpu adapter; executes the production scaled copy"]
    fn xl_shows_each_sidebar_pixel_as_a_2x2_block_in_the_right_column() {
        let gpu = gpu();
        let frame = SidebarFrame::new(400, 120, 200);
        assert_eq!(frame.column(), (64, 0, 336, 120));
        assert_eq!(frame.strip(), (0, 56, 64, 64));
        let surface = tagged(&gpu, frame);
        let pixels = composite(&gpu, &surface, UNORM, (400, 120), |_| {});
        let width = 400;
        for (x, y) in [(0, 0), (1, 0), (0, 1), (100, 30), (167, 59)] {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                assert_eq!(
                    pixel(&pixels, width, 64 + 2 * x + dx, 2 * y + dy),
                    [x as u8, y as u8, 7, 255],
                    "sidebar pixel ({x}, {y})"
                );
            }
        }
        // The bar's 32 sidebar pixels make the strip's 64, flush with the
        // column: its last pixel meets the sidebar's first.
        assert_eq!(frame.bar_surface_size(), (32, 32));
        assert_eq!(pixel(&pixels, width, 63, 56), [31, 0, 8, 255]);
        assert_eq!(pixel(&pixels, width, 0, 119), [0, 31, 8, 255]);
        // The battlefield's part of the frame is left alone.
        assert_eq!(pixel(&pixels, width, 63, 55), BACKDROP);
        assert_eq!(pixel(&pixels, width, 0, 0), BACKDROP);
    }

    #[test]
    #[ignore = "requires a wgpu adapter; executes the production scaled copy"]
    fn large_keeps_whole_pixels_and_blends_only_the_seams() {
        let gpu = gpu();
        let frame = SidebarFrame::new(400, 120, 150);
        assert_eq!(frame.column(), (148, 0, 252, 120));
        let surface = tagged(&gpu, frame);
        let pixels = composite(&gpu, &surface, UNORM, (400, 120), |_| {});
        let width = 400;
        // Frame pixel 148 lies inside sidebar pixel 0 and 150 inside pixel 1;
        // 149 straddles them half and half.
        assert_eq!(pixel(&pixels, width, 148, 0), [0, 0, 7, 255]);
        assert_eq!(pixel(&pixels, width, 150, 0), [1, 0, 7, 255]);
        let seam = pixel(&pixels, width, 149, 0);
        assert!(seam[0] <= 1 && seam[1..] == [0, 7, 255], "{seam:?}");
        // The last column pixel is the sidebar's last pixel.
        assert_eq!(pixel(&pixels, width, 399, 0), [167, 0, 7, 255]);
        assert_eq!(pixel(&pixels, width, 147, 0), BACKDROP);
        // The bar's strip is 48 pixels tall and ends at the column.
        assert_eq!(frame.strip(), (0, 72, 148, 48));
        assert_eq!(pixel(&pixels, width, 147, 119)[2], 8);
        assert_eq!(pixel(&pixels, width, 147, 71), BACKDROP);
    }

    /// At Normal the copy is 1:1 in the production sRGB format: every byte
    /// comes through unchanged, so Normal frames are the ones drawn before
    /// the surfaces existed.
    #[test]
    #[ignore = "requires a wgpu adapter; executes the production scaled copy"]
    fn normal_copies_every_byte_unchanged() {
        const SRGB: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
        let gpu = gpu();
        let frame = SidebarFrame::native(400, 300);
        let batch = BatchRenderer::new_with_device(&gpu.device, &gpu.queue, SRGB);
        let surface = SidebarSurface::new(&gpu.device, &gpu.queue, SRGB, &batch, frame);
        // Each channel runs through all 256 values across the column.
        let texel = |x: u32, y: u32| {
            let i = x + y * 168;
            [i as u8, (i * 7 + 3) as u8, (255 - i % 256) as u8, 255]
        };
        fill(&gpu, &surface, SidebarPart::Column, texel);
        fill(&gpu, &surface, SidebarPart::Bar, |x, y| texel(x % 168, y));
        let pixels = composite(&gpu, &surface, SRGB, (400, 300), |_| {});
        for y in 0..300 {
            for x in 0..168 {
                assert_eq!(pixel(&pixels, 400, 232 + x, y), texel(x, y), "({x}, {y})");
            }
        }
        for y in 0..32 {
            for x in 0..232 {
                assert_eq!(pixel(&pixels, 400, x, 268 + y), texel(x % 168, y));
            }
        }
    }

    /// The parts' cameras put a layer at the frame pixels input converts its
    /// layout rect to (`SidebarFrame::rect_to_frame`, `bar_rect_to_frame`),
    /// through the production overlay pipeline.
    #[test]
    #[ignore = "requires a wgpu adapter; executes the production batch shader"]
    fn layers_land_where_their_input_rects_are() {
        let gpu = gpu();
        let frame = SidebarFrame::new(400, 120, 200);
        let batch = BatchRenderer::new_with_device(&gpu.device, &gpu.queue, UNORM);
        let surface = SidebarSurface::new(&gpu.device, &gpu.queue, UNORM, &batch, frame);
        // A scrolled world camera: UI layers carry it in their positions.
        let mut world = batch.camera_uniform();
        world.camera_pos = [1000.0, 500.0];
        world.zoom = 1.5;
        batch.write_camera(&gpu.queue, world);
        surface.write_cameras(&gpu.queue, batch.ui_camera_uniform());
        let white = batch.create_texture_on_device(
            &gpu.device,
            &gpu.queue,
            &[255, 255, 255, 255],
            1,
            1,
            None,
        );
        let column_rect = crate::ui::sidebar::Rect {
            x: frame.layout_left() + 10.0,
            y: 20.0,
            w: 4.0,
            h: 3.0,
        };
        let bar_rect = crate::ui::sidebar::Rect {
            x: 5.0,
            y: frame.strip_layout_top() + 3.0,
            w: 2.0,
            h: 2.0,
        };
        let quad = |rect: crate::ui::sidebar::Rect| crate::render::batch::SpriteInstance {
            position: [1000.0 + rect.x, 500.0 + rect.y],
            size: [rect.w, rect.h],
            uv_size: [1.0; 2],
            tint: [1.0; 3],
            alpha: 1.0,
            depth: 0.00048,
            ..Default::default()
        };
        let pixels = composite(&gpu, &surface, UNORM, (400, 120), |encoder| {
            for (part, rect) in [
                (SidebarPart::Column, column_rect),
                (SidebarPart::Bar, bar_rect),
            ] {
                let instances = gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::bytes_of(&quad(rect)),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                let mut pass = surface.begin_pass(part, encoder, wgpu::Color::BLACK);
                pass.set_pipeline(batch.overlay_pipeline());
                pass.set_bind_group(0, surface.camera(part), &[]);
                pass.set_bind_group(1, &white.bind_group, &[]);
                pass.set_vertex_buffer(0, instances.slice(..));
                pass.draw(0..6, 0..1);
            }
        });
        for rect in [
            frame.rect_to_frame(column_rect),
            frame.bar_rect_to_frame(bar_rect),
        ] {
            let (x, y) = (rect.x as u32, rect.y as u32);
            let (right, bottom) = (x + rect.w as u32, y + rect.h as u32);
            assert_eq!(pixel(&pixels, 400, x, y), [255; 4], "{rect:?}");
            assert_eq!(pixel(&pixels, 400, right - 1, bottom - 1), [255; 4]);
            assert_eq!(pixel(&pixels, 400, x - 1, y), [0, 0, 0, 255]);
            assert_eq!(pixel(&pixels, 400, right, bottom), [0, 0, 0, 255]);
        }
    }
}
