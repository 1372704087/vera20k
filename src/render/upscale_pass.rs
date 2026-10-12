//! Zoom of the match frame.
//!
//! A zoomed match draws its complete frame (battlefield, sidebar, messages,
//! tooltips, software cursor, and the active-game shells such as the pause
//! menu) into a source target the window size divided by the zoom; this pass
//! then scales it to the window, centred. A whole zoom (200%, 300%) copies
//! each source pixel as a block and stays sharp. A step between (150%) blends
//! the two source pixels a window pixel straddles by how much of it each
//! covers, so only the seams soften and no pixel is drawn wider than its
//! neighbours. A window that doesn't divide evenly keeps a black border
//! narrower than one source pixel. With the original 4:3 shape the frame is
//! the largest 4:3 rectangle that fits, centred between black bars, so the
//! pass runs even at 100%. The front-end menus never come here: they fit the
//! window on their own.
//!
//! Depends on: `render/gpu.rs` (GpuContext)

use super::gpu::GpuContext;

const SHADER_SRC: &str = include_str!("upscale_zoom.wgsl");

/// The match frame's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameShape {
    /// The window's own: a wider window shows more battlefield.
    #[default]
    Window,
    /// gamemd's 4:3 screen, with black bars on a window of any other shape.
    Original,
}

/// Where a zoomed frame sits in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomGeometry {
    /// Window pixels per 100 source pixels, on each axis; at least 100.
    pub percent: u32,
    pub src_width: u32,
    pub src_height: u32,
    /// The window pixel where source pixel (0, 0) lands.
    pub origin: (u32, u32),
}

impl ZoomGeometry {
    pub fn new(window_width: u32, window_height: u32, percent: u32, shape: FrameShape) -> Self {
        let percent = percent.max(100);
        let mut src_width = (window_width * 100 / percent).max(1);
        let mut src_height = (window_height * 100 / percent).max(1);
        if shape == FrameShape::Original {
            if src_width * 3 > src_height * 4 {
                src_width = (src_height * 4 / 3).max(1);
            } else {
                src_height = (src_width * 3 / 4).max(1);
            }
        }
        let zoom = Self {
            percent,
            src_width,
            src_height,
            origin: (0, 0),
        };
        let (frame_width, frame_height) = zoom.frame_size();
        Self {
            origin: (
                window_width.saturating_sub(frame_width) / 2,
                window_height.saturating_sub(frame_height) / 2,
            ),
            ..zoom
        }
    }

    /// Window pixels per source pixel.
    pub fn scale(self) -> f32 {
        self.percent as f32 / 100.0
    }

    /// Whether every source pixel is a whole block of window pixels.
    pub fn is_sharp(self) -> bool {
        self.percent % 100 == 0
    }

    /// The scaled frame, in whole window pixels.
    pub fn frame_size(self) -> (u32, u32) {
        (
            self.src_width * self.percent / 100,
            self.src_height * self.percent / 100,
        )
    }

    /// Whether the frame is the whole window, unscaled: no pass is needed.
    pub fn is_whole_window(self, window_width: u32, window_height: u32) -> bool {
        self.percent == 100 && (self.src_width, self.src_height) == (window_width, window_height)
    }

    /// A window point in source pixels. The border clamps to the frame's edge.
    pub fn window_to_source(self, x: f32, y: f32) -> (f32, f32) {
        let scale = self.scale();
        (
            ((x - self.origin.0 as f32) / scale).clamp(0.0, (self.src_width - 1) as f32),
            ((y - self.origin.1 as f32) / scale).clamp(0.0, (self.src_height - 1) as f32),
        )
    }

    /// A source point in window pixels.
    pub fn source_to_window(self, x: f32, y: f32) -> (f32, f32) {
        let scale = self.scale();
        (
            x * scale + self.origin.0 as f32,
            y * scale + self.origin.1 as f32,
        )
    }
}

/// GPU resources for the zoom pass.
#[allow(dead_code)] // Fields kept alive for GPU resource ownership.
pub struct UpscalePass {
    color_texture: wgpu::Texture,
    depth_texture: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    params_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    geometry: ZoomGeometry,
}

impl UpscalePass {
    /// Create a pass that zooms a `geometry.src_width x src_height` frame into
    /// the window it was computed for.
    pub fn new(gpu: &GpuContext, geometry: ZoomGeometry) -> Self {
        let color_texture = create_color_texture(gpu, geometry.src_width, geometry.src_height);
        let depth_texture = create_depth_texture(gpu, geometry.src_width, geometry.src_height);
        let color_view = color_texture.create_view(&Default::default());
        let depth_view = depth_texture.create_view(&Default::default());

        let params_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Zoom Params"),
            size: 16, // vec2f origin + f32 scale + f32 sharp flag
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        write_params(gpu, &params_buffer, geometry);

        let bgl = create_bgl(gpu);
        let bind_group = create_bind_group(&gpu.device, &bgl, &color_texture, &params_buffer);
        let pipeline = create_pipeline(gpu, &bgl);

        Self {
            color_texture,
            depth_texture,
            color_view,
            depth_view,
            params_buffer,
            bind_group,
            bgl,
            pipeline,
            geometry,
        }
    }

    pub fn geometry(&self) -> ZoomGeometry {
        self.geometry
    }

    /// Follow a new window size or zoom. Only the size-dependent targets are
    /// rebuilt, so a live resize never recompiles the shader.
    pub fn set_geometry(&mut self, gpu: &GpuContext, geometry: ZoomGeometry) {
        if geometry == self.geometry {
            return;
        }
        if (geometry.src_width, geometry.src_height)
            != (self.geometry.src_width, self.geometry.src_height)
        {
            self.color_texture = create_color_texture(gpu, geometry.src_width, geometry.src_height);
            self.depth_texture = create_depth_texture(gpu, geometry.src_width, geometry.src_height);
            self.color_view = self.color_texture.create_view(&Default::default());
            self.depth_view = self.depth_texture.create_view(&Default::default());
            self.bind_group = create_bind_group(
                &gpu.device,
                &self.bgl,
                &self.color_texture,
                &self.params_buffer,
            );
        }
        write_params(gpu, &self.params_buffer, geometry);
        self.geometry = geometry;
    }

    /// Cached view for the intermediate color render target.
    pub fn color_view(&self) -> &wgpu::TextureView {
        &self.color_view
    }

    /// Intermediate color texture receiving the completed tactical frame.
    pub fn color_texture(&self) -> &wgpu::Texture {
        &self.color_texture
    }

    /// Cached view for the intermediate depth buffer.
    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    /// Source render width.
    pub fn src_width(&self) -> u32 {
        self.geometry.src_width
    }

    /// Source render height.
    pub fn src_height(&self) -> u32 {
        self.geometry.src_height
    }

    /// Zoom the intermediate texture into `target_view`, a window-sized target.
    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView) {
        self.draw_with_bind_group(encoder, target_view, &self.bind_group);
    }

    /// Draw the same zoom from an explicitly retained source.
    /// Used only for a requested screenshot of the previously presented
    /// cursor-free composition; ordinary frames keep the persistent bind group.
    pub(crate) fn draw_texture(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
        target_view: &wgpu::TextureView,
    ) {
        let bind_group = create_bind_group(device, &self.bgl, source, &self.params_buffer);
        self.draw_with_bind_group(encoder, target_view, &bind_group);
    }

    fn draw_with_bind_group(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        bind_group: &wgpu::BindGroup,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Zoom Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let (x, y) = self.geometry.origin;
        let (width, height) = self.geometry.frame_size();
        pass.set_scissor_rect(x, y, width, height);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

// ---------------------------------------------------------------------------
// Resource creation helpers
// ---------------------------------------------------------------------------

fn write_params(gpu: &GpuContext, params_buffer: &wgpu::Buffer, geometry: ZoomGeometry) {
    gpu.queue.write_buffer(
        params_buffer,
        0,
        bytemuck::cast_slice(&[
            geometry.origin.0 as f32,
            geometry.origin.1 as f32,
            geometry.scale(),
            if geometry.is_sharp() { 1.0f32 } else { 0.0 },
        ]),
    );
}

fn create_color_texture(gpu: &GpuContext, width: u32, height: u32) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Zoom Color RT"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.surface_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn create_depth_texture(gpu: &GpuContext, width: u32, height: u32) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Zoom Depth RT"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn create_bgl(gpu: &GpuContext) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Zoom BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    // Whole texels are loaded, never filtered.
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
}

fn create_bind_group(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    color_texture: &wgpu::Texture,
    params_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    let view = color_texture.create_view(&Default::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Zoom BG"),
        layout: bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    })
}

fn create_pipeline(gpu: &GpuContext, bgl: &wgpu::BindGroupLayout) -> wgpu::RenderPipeline {
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Zoom Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

    let layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Zoom Pipeline Layout"),
            bind_group_layouts: &[bgl],
            push_constant_ranges: &[],
        });

    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Zoom Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: gpu.surface_format,
                    blend: None, // opaque overwrite
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None, // own render pass, no depth
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_centres_whole_blocks_and_maps_points_both_ways() {
        // An even window: the frame covers it exactly.
        let wide = ZoomGeometry::new(3440, 1440, 200, FrameShape::Window);
        assert_eq!(
            (wide.src_width, wide.src_height, wide.origin),
            (1720, 720, (0, 0))
        );
        // An odd window keeps a border under one block, split evenly.
        let odd = ZoomGeometry::new(1925, 1083, 300, FrameShape::Window);
        assert_eq!(
            (odd.src_width, odd.src_height, odd.origin),
            (641, 361, (1, 0))
        );
        assert_eq!(odd.source_to_window(10.0, 20.0), (31.0, 60.0));
        assert_eq!(odd.window_to_source(31.0, 60.0), (10.0, 20.0));
        // The border clamps to the frame's edge pixels.
        assert_eq!(odd.window_to_source(0.0, 1082.0), (0.0, 360.0));
        // 100% is the identity.
        let plain = ZoomGeometry::new(800, 600, 100, FrameShape::Window);
        assert_eq!(
            (plain.src_width, plain.src_height, plain.origin),
            (800, 600, (0, 0))
        );
        assert_eq!(plain.window_to_source(399.5, 299.5), (399.5, 299.5));
        assert!(plain.is_whole_window(800, 600));
        assert!(!wide.is_whole_window(3440, 1440));
        assert!(wide.is_sharp() && odd.is_sharp());
    }

    #[test]
    fn zoom_shader_compiles() {
        use wgpu::naga;
        let module = naga::front::wgsl::parse_str(SHADER_SRC).expect("the zoom shader parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("the zoom shader validates");
    }

    #[test]
    fn a_step_between_whole_zooms_scales_smoothly_from_the_centre() {
        let half = ZoomGeometry::new(1920, 1080, 150, FrameShape::Window);
        assert_eq!(
            (half.src_width, half.src_height, half.origin),
            (1280, 720, (0, 0))
        );
        assert!(!half.is_sharp());
        assert_eq!(half.frame_size(), (1920, 1080));
        assert_eq!(half.source_to_window(10.0, 20.0), (15.0, 30.0));
        assert_eq!(half.window_to_source(15.0, 30.0), (10.0, 20.0));
        // A window the step doesn't divide keeps a border under one source
        // pixel.
        let odd = ZoomGeometry::new(1366, 768, 150, FrameShape::Window);
        assert_eq!((odd.src_width, odd.src_height), (910, 512));
        assert_eq!((odd.frame_size(), odd.origin), ((1365, 768), (0, 0)));
        assert!(!odd.is_whole_window(1366, 768));
    }

    #[test]
    fn original_shape_keeps_a_4_3_frame_between_centred_bars() {
        let frame = |w, h, percent| {
            let zoom = ZoomGeometry::new(w, h, percent, FrameShape::Original);
            (zoom.src_width, zoom.src_height, zoom.origin)
        };
        // A widescreen gets bars at the sides, at any zoom.
        assert_eq!(frame(1920, 1080, 100), (1440, 1080, (240, 0)));
        assert_eq!(frame(1920, 1080, 200), (720, 540, (240, 0)));
        assert_eq!(frame(1920, 1080, 150), (960, 720, (240, 0)));
        // A taller screen gets them above and below.
        assert_eq!(frame(1280, 1024, 100), (1280, 960, (0, 32)));
        // A 4:3 window is unchanged, and at 100% needs no pass.
        assert_eq!(frame(1600, 1200, 200), (800, 600, (0, 0)));
        let whole = ZoomGeometry::new(1024, 768, 100, FrameShape::Original);
        assert!(whole.is_whole_window(1024, 768));
        let barred = ZoomGeometry::new(1920, 1080, 100, FrameShape::Original);
        assert!(!barred.is_whole_window(1920, 1080));
        // The bars clamp to the frame's edge, like the zoom's own border.
        assert_eq!(barred.window_to_source(100.0, 500.0), (0.0, 500.0));
        assert_eq!(barred.source_to_window(0.0, 0.0), (240.0, 0.0));
    }
}
