//! Optional diagnostic GUI integration — bridges egui, winit, and wgpu.
//!
//! Owns the egui Context, the egui-winit input translator (State),
//! and the egui-wgpu Renderer. Compiled only with the dev-ui feature.
//!
//! ## Dependency rules
//! - Part of render/ — depends on render/gpu for GpuContext.
//! - Does NOT depend on ui/, sim/, or any game logic.
//! - App.rs creates this and passes it to UI draw functions.

use winit::window::Window;

use crate::render::gpu::GpuContext;

const VERDANA_FONT_PATH: &str = "C:/Windows/Fonts/verdana.ttf";
const CALIBRI_FONT_PATH: &str = "C:/Windows/Fonts/calibri.ttf";

/// Candidate CJK fallback fonts, tried in order. The bilingual dev panels
/// render Han glyphs, but Verdana/Calibri and egui's built-in font cover
/// Latin only — without one of these the Chinese strings show up as `◻`
/// replacement squares. epaint resolves a glyph against each font of a
/// family in order, so this is appended AFTER the Latin font and only
/// supplies the glyphs the Latin font lacks.
const CJK_FONT_PATHS: &[&str] = &[
    "C:/Windows/Fonts/msyh.ttc",     // Microsoft YaHei (simplified Chinese)
    "C:/Windows/Fonts/simhei.ttf",   // SimHei (simplified Chinese)
    "C:/Windows/Fonts/simsun.ttc",   // SimSun (simplified Chinese)
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
    "/System/Library/Fonts/PingFang.ttc", // macOS
];

/// All egui state needed for input handling and rendering.
///
/// Created once in App::initialize() alongside the GpuContext.
/// Holds the egui_winit::State (input translator) and egui_wgpu::Renderer
/// (GPU draw pipeline).
pub struct EguiIntegration {
    /// The egui context — shared across all egui frames.
    /// Holds style, fonts, memory, and the current frame's UI state.
    pub ctx: egui::Context,

    /// egui-winit input state. Translates winit WindowEvents into egui RawInput.
    /// Must be fed every event via on_window_event().
    state: egui_winit::State,

    /// egui-wgpu renderer. Manages GPU buffers and textures for egui output.
    /// Does NOT use a depth buffer — egui renders flat UI elements.
    renderer: egui_wgpu::Renderer,
}

impl EguiIntegration {
    /// Create a new EguiIntegration.
    ///
    /// Called once during app initialization after the GPU context is ready.
    /// `window` is needed for egui-winit to query scale factor and set up input.
    pub fn new(gpu: &GpuContext, window: &Window) -> Self {
        let ctx: egui::Context = egui::Context::default();

        // egui-winit State handles all input translation:
        // mouse, keyboard, touch, clipboard, IME, etc.
        let state: egui_winit::State = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        // egui-wgpu Renderer compiles the egui shader and sets up GPU pipelines.
        // No depth format — egui draws flat, no MSAA — egui anti-aliases via feathering.
        let renderer: egui_wgpu::Renderer = egui_wgpu::Renderer::new(
            &gpu.device,
            gpu.surface_format,
            egui_wgpu::RendererOptions {
                depth_stencil_format: None,
                msaa_samples: 1,
                dithering: true,
                ..Default::default()
            },
        );

        // Load Verdana as the proportional font for a cleaner UI look.
        load_system_font(&ctx);

        Self {
            ctx,
            state,
            renderer,
        }
    }

    /// Feed a winit event to egui. Returns whether egui consumed it.
    ///
    /// Call this in window_event() BEFORE handling game-specific input.
    /// If `consumed` is true, the event was used by egui (e.g., mouse click
    /// on a button) and should NOT be passed to game input handling.
    pub fn on_window_event(
        &mut self,
        window: &Window,
        event: &winit::event::WindowEvent,
    ) -> egui_winit::EventResponse {
        self.state.on_window_event(window, event)
    }

    /// A native shell owns these inputs. Keep egui-winit's platform state
    /// current, but do not replay its accumulated events into a later dialog.
    pub(crate) fn discard_pending_input(&mut self, window: &Window) {
        let _ = self.state.take_egui_input(window);
    }

    /// Begin an egui frame. Call once per frame before running UI code.
    ///
    /// Collects accumulated input from on_window_event() calls and
    /// starts the egui frame. After this, call egui UI functions
    /// (panels, windows, etc.), then call end_frame_and_render().
    pub fn begin_frame(&mut self, window: &Window) {
        let raw_input: egui::RawInput = self.state.take_egui_input(window);
        self.ctx.begin_pass(raw_input);
    }

    /// End the egui frame, tessellate, and render to the given surface view.
    ///
    /// This:
    /// 1. Ends the egui pass and collects paint jobs.
    /// 2. Handles platform output (cursor changes, clipboard, etc.).
    /// 3. Updates egui GPU buffers with new vertices/textures.
    /// 4. Creates a render pass WITHOUT depth and draws egui on top.
    ///
    /// Must be called AFTER any game render pass so egui draws on top.
    /// Uses LoadOp::Load to preserve whatever was drawn before.
    pub fn end_frame_and_render(
        &mut self,
        gpu: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        window: &Window,
        has_software_cursor: bool,
    ) {
        let full_output: egui::FullOutput = self.ctx.end_pass();

        // Handle platform output (clipboard, open URL, IME, etc.).
        let mut platform_output = full_output.platform_output;
        if has_software_cursor {
            platform_output.cursor_icon = egui::CursorIcon::None;
        }
        self.state.handle_platform_output(window, platform_output);

        // Tessellate egui shapes into GPU-ready triangles.
        let paint_jobs: Vec<egui::epaint::ClippedPrimitive> = self
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let screen_descriptor: egui_wgpu::ScreenDescriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [gpu.config.width, gpu.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        // Upload texture deltas (font atlas changes, user textures).
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(&gpu.device, &gpu.queue, *id, image_delta);
        }

        // Upload vertex/index buffers. Returns extra command buffers for
        // operations that couldn't go through the encoder (rare).
        let extra_cmd_bufs: Vec<wgpu::CommandBuffer> = self.renderer.update_buffers(
            &gpu.device,
            &gpu.queue,
            encoder,
            &paint_jobs,
            &screen_descriptor,
        );
        // Submit extra command buffers immediately if any exist. These contain
        // preparatory work (texture uploads) the render pass will reference.
        if !extra_cmd_bufs.is_empty() {
            gpu.queue.submit(extra_cmd_bufs);
        }

        // Render egui in its own pass — NO depth buffer, LoadOp::Load preserves game content.
        {
            let render_pass: wgpu::RenderPass<'_> =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            // forget_lifetime() is required because Renderer::render expects
            // RenderPass<'static>. Safe — wgpu keeps resources alive internally.
            let mut render_pass: wgpu::RenderPass<'static> = render_pass.forget_lifetime();
            self.renderer
                .render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }

        // Free any textures that egui no longer needs.
        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}

/// Load Verdana (or Calibri fallback) as the default proportional font, plus
/// a CJK fallback for the bilingual dev panels. Falls back silently to egui's
/// built-in font for anything not found.
fn load_system_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut loaded_any = false;

    // Latin UI font: Verdana, then Calibri. Inserted at the FRONT of the
    // Proportional family so it takes priority.
    if let Some(font_path) = preferred_system_font_path(|path| std::path::Path::new(path).exists())
    {
        if let Ok(font_bytes) = std::fs::read(font_path) {
            let byte_length = font_bytes.len();
            log::info!("Loaded system font: {} ({} bytes)", font_path, byte_length);
            fonts.font_data.insert(
                "sidebar_font".to_string(),
                egui::FontData::from_owned(font_bytes).into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "sidebar_font".to_string());
            loaded_any = true;
        } else {
            log::warn!("Failed to read font file: {}", font_path);
        }
    } else {
        log::info!("No Verdana/Calibri found — using egui default font");
    }

    // CJK fallback: appended at the END of both text families so egui only
    // consults it for glyphs the primary fonts lack (per-glyph fallback).
    if let Some(cjk_path) =
        preferred_cjk_font_path(|path| std::path::Path::new(path).exists())
    {
        if let Ok(cjk_bytes) = std::fs::read(cjk_path) {
            let byte_length = cjk_bytes.len();
            log::info!(
                "Loaded CJK fallback font: {} ({} bytes)",
                cjk_path,
                byte_length
            );
            fonts.font_data.insert(
                "cjk_fallback".to_string(),
                egui::FontData::from_owned(cjk_bytes).into(),
            );
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                let list = fonts.families.entry(family).or_default();
                if !list.iter().any(|name| name == "cjk_fallback") {
                    list.push("cjk_fallback".to_string());
                }
            }
            loaded_any = true;
        } else {
            log::warn!("Failed to read CJK font file: {}", cjk_path);
        }
    }

    if loaded_any {
        ctx.set_fonts(fonts);
    }
}

fn preferred_system_font_path(path_exists: impl Fn(&str) -> bool) -> Option<&'static str> {
    if path_exists(VERDANA_FONT_PATH) {
        Some(VERDANA_FONT_PATH)
    } else if path_exists(CALIBRI_FONT_PATH) {
        Some(CALIBRI_FONT_PATH)
    } else {
        None
    }
}

/// First existing CJK candidate, or `None` when the platform ships none.
fn preferred_cjk_font_path(path_exists: impl Fn(&str) -> bool) -> Option<&'static str> {
    CJK_FONT_PATHS
        .iter()
        .copied()
        .find(|path| path_exists(path))
}

#[cfg(test)]
mod tests {
    use super::{
        CALIBRI_FONT_PATH, CJK_FONT_PATHS, VERDANA_FONT_PATH, preferred_cjk_font_path,
        preferred_system_font_path,
    };

    #[test]
    fn system_font_selection_keeps_verdana_then_calibri_precedence() {
        assert_eq!(
            preferred_system_font_path(|_| true),
            Some(VERDANA_FONT_PATH)
        );
        assert_eq!(
            preferred_system_font_path(|path| path == CALIBRI_FONT_PATH),
            Some(CALIBRI_FONT_PATH)
        );
        assert_eq!(preferred_system_font_path(|_| false), None);
    }

    #[test]
    fn cjk_font_selection_prefers_first_existing_candidate() {
        // YaHei wins when present (Windows primary CJK font).
        assert_eq!(
            preferred_cjk_font_path(|path| path == "C:/Windows/Fonts/msyh.ttc"),
            Some("C:/Windows/Fonts/msyh.ttc")
        );
        // Falls through to later candidates when earlier ones are missing.
        assert_eq!(
            preferred_cjk_font_path(|path| path
                == "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
            Some("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc")
        );
        assert_eq!(preferred_cjk_font_path(|_| false), None);
    }

    #[test]
    fn cjk_candidates_cover_major_platforms() {
        assert!(CJK_FONT_PATHS.iter().any(|p| p.starts_with("C:/"))); // Windows
        assert!(CJK_FONT_PATHS.iter().any(|p| p.starts_with("/usr/"))); // Linux
        assert!(CJK_FONT_PATHS.iter().any(|p| p.starts_with("/System/"))); // macOS
    }
}
