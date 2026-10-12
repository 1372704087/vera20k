//! Whole-pixel zoom of the match frame.
//!
//! A zoomed match draws its complete frame (battlefield, sidebar, messages,
//! tooltips, software cursor) into a source target the window size divided by
//! the zoom factor; this pass then copies it to the window with each source
//! pixel as a `factor` x `factor` block, centred. A window that doesn't divide
//! evenly keeps a black border narrower than one block. The menus never come
//! here: they fit the window on their own.
//!
//! Depends on: `render/gpu.rs` (GpuContext)

use super::gpu::GpuContext;

const SHADER_SRC: &str = include_str!("upscale_zoom.wgsl");

/// Where a zoomed frame sits in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomGeometry {
    /// Window pixels per source pixel, on each axis.
    pub factor: u32,
    pub src_width: u32,
    pub src_height: u32,
    /// The window pixel where source pixel (0, 0) lands.
    pub origin: (u32, u32),
}

impl ZoomGeometry {
    pub fn new(window_width: u32, window_height: u32, factor: u32) -> Self {
        let factor = factor.max(1);
        let src_width = (window_width / factor).max(1);
        let src_height = (window_height / factor).max(1);
        Self {
            factor,
            src_width,
            src_height,
            origin: (
                window_width.saturating_sub(src_width * factor) / 2,
                window_height.saturating_sub(src_height * factor) / 2,
            ),
        }
    }

    /// A window point in source pixels. The border clamps to the frame's edge.
    pub fn window_to_source(self, x: f32, y: f32) -> (f32, f32) {
        let factor = self.factor as f32;
        (
            ((x - self.origin.0 as f32) / factor).clamp(0.0, (self.src_width - 1) as f32),
            ((y - self.origin.1 as f32) / factor).clamp(0.0, (self.src_height - 1) as f32),
        )
    }

    /// A source point in window pixels.
    pub fn source_to_window(self, x: f32, y: f32) -> (f32, f32) {
        let factor = self.factor as f32;
        (
            x * factor + self.origin.0 as f32,
            y * factor + self.origin.1 as f32,
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
            size: 16, // vec2f origin + f32 factor + padding (uniform alignment)
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(
            &params_buffer,
            0,
            bytemuck::cast_slice(&[
                geometry.origin.0 as f32,
                geometry.origin.1 as f32,
                geometry.factor as f32,
                0.0f32,
            ]),
        );

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
        let ZoomGeometry {
            factor,
            src_width,
            src_height,
            origin,
        } = self.geometry;
        pass.set_scissor_rect(origin.0, origin.1, src_width * factor, src_height * factor);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

// ---------------------------------------------------------------------------
// Resource creation helpers
// ---------------------------------------------------------------------------

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
        let wide = ZoomGeometry::new(3440, 1440, 2);
        assert_eq!(
            (wide.src_width, wide.src_height, wide.origin),
            (1720, 720, (0, 0))
        );
        // An odd window keeps a border under one block, split evenly.
        let odd = ZoomGeometry::new(1925, 1083, 3);
        assert_eq!(
            (odd.src_width, odd.src_height, odd.origin),
            (641, 361, (1, 0))
        );
        assert_eq!(odd.source_to_window(10.0, 20.0), (31.0, 60.0));
        assert_eq!(odd.window_to_source(31.0, 60.0), (10.0, 20.0));
        // The border clamps to the frame's edge pixels.
        assert_eq!(odd.window_to_source(0.0, 1082.0), (0.0, 360.0));
        // Factor 1 is the identity.
        let plain = ZoomGeometry::new(800, 600, 1);
        assert_eq!(
            (plain.src_width, plain.src_height, plain.origin),
            (800, 600, (0, 0))
        );
        assert_eq!(plain.window_to_source(399.5, 299.5), (399.5, 299.5));
    }
}
