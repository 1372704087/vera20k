//! Selection overlay rendering — highlights selected entities and draws drag rectangle.
//!
//! Renders visual feedback for the selection system:
//! - Green selection brackets around selected entities.
//! - Green drag rectangle outline while the player is box-selecting.
//!
//! Uses procedurally-generated textures rendered via the batch shader.
//! The selection overlay is drawn as a separate batch pass after units/sprites.
//!
//! ## Dependency rules
//! - Part of render/ — reads from sim/ (Selected, Position) but NEVER mutates sim state.

use crate::assets::asset_manager::AssetManager;
use crate::assets::pal_file::Palette;
use crate::assets::shp_file::ShpFile;
use crate::map::terrain::{self, TILE_HEIGHT, TILE_WIDTH};
use crate::render::batch::{BatchRenderer, BatchTexture, SpriteInstance};
use crate::render::gpu::GpuContext;
use crate::sim::production::{BuildingPlacementCell, BuildingPlacementPreview};
use crate::sim::selection::SelectionState;

/// Drag rectangle color — white.
const DRAG_RECT_COLOR: [u8; 4] = [255, 255, 255, 255];
/// Depth for the drag rectangle — always in front of everything.
const DRAG_RECT_DEPTH: f32 = 0.001;

/// Thickness of the drag rectangle outline in pixels (1px = RA2 original style).
const DRAG_RECT_THICKNESS: f32 = 1.0;

/// Number of pip variants packed into the building pip atlas (empty, green, yellow, red).
const PIP_VARIANT_COUNT: u32 = 4;

/// Manages textures and rendering for selection overlays.
pub struct SelectionOverlay {
    /// Solid color texture for drag rectangle lines.
    drag_texture: BatchTexture,
    /// Full physical PLACE frame table in its original palette. Active cells
    /// select frame0 or slope+2 through the common centered SHP owner.
    placement_shape: Option<CenteredShapeStrip>,
    /// Solid white texture reused for order markers/cursor feedback.
    white_texture: BatchTexture,
    /// Diamond outline texture for debug cell grid visualization.
    diamond_outline_texture: BatchTexture,
    /// White filled-diamond texture for debug pathgrid/path overlays.
    diamond_filled_texture: BatchTexture,
    /// Pip atlas: 4 pip frames packed horizontally (empty, green, yellow, red).
    /// Each pip is a small cube-shaped sprite from pips.shp.
    pip_texture: Option<BatchTexture>,
    /// Width of a single pip frame in the atlas (pixels).
    pip_frame_w: u32,
    /// Height of a single pip frame in the atlas (pixels).
    pip_frame_h: u32,
    /// Canvas centering adjustment for building pips (matches DrawSHP flag 0x200).
    pip_canvas_adj: (f32, f32),
    /// Full PIPBRD frame table for selected mobile health backgrounds.
    pipbrd: Option<CenteredShapeStrip>,
    /// Occupant pip atlas: 7 frames from pips.shp packed horizontally.
    /// Frames 6 (empty), 7-12 (PersonGreen..PersonPurple) — used for garrison pips.
    occupant_pip_texture: Option<BatchTexture>,
    occupant_pip_frame_w: u32,
    occupant_pip_frame_h: u32,
    occupant_pip_canvas_adj: (f32, f32),
    /// PIPS2.SHP: all raw frames, shared by cargo and MindControl scales.
    /// Native LoadPipAssets5F76B0 binds AC1480; DrawPipScalePips709AEF
    /// selects this shape for nonbuildings, in PALETTE.PAL/Convert87F6C4.
    scale_pips: Option<CenteredShapeStrip>,
    /// BOMBCURS.SHP, the Crazy Ivan bomb clock.
    bomb_clock: Option<CenteredShapeStrip>,
    /// WRENCH.SHP, the repairing building's wrench.
    repair_wrench: Option<CenteredShapeStrip>,
    /// Full PIPS frame table: mobile health 16..18 and self-heal 13/20.
    /// Native LoadPipAssets 5F76B0 binds AC147C; both use Convert 87F6C4.
    pips: Option<CenteredShapeStrip>,
}

/// Every frame of one SHP packed side by side at its stored size, each with
/// the offset `CC_Draw_Shape @ 0x004AED70` gives a centered draw (flag
/// `0x200`): back by the integer canvas half, then on by the stored frame
/// origin.
pub struct CenteredShapeStrip {
    texture: BatchTexture,
    atlas_w: f32,
    atlas_h: f32,
    frames: Vec<CenteredShapeFrame>,
}

struct CenteredShapeFrame {
    x: u32,
    w: u32,
    h: u32,
    offset: (i32, i32),
}

impl CenteredShapeStrip {
    pub fn texture(&self) -> &BatchTexture {
        &self.texture
    }

    /// `frame` drawn centered on `point`; `None` for a frame the file lacks
    /// or one without pixels.
    pub fn instance(&self, frame: usize, point: (f32, f32), depth: f32) -> Option<SpriteInstance> {
        let f = self.frames.get(frame).filter(|f| f.w > 0 && f.h > 0)?;
        Some(SpriteInstance {
            position: [point.0 + f.offset.0 as f32, point.1 + f.offset.1 as f32],
            size: [f.w as f32, f.h as f32],
            uv_origin: [f.x as f32 / self.atlas_w, 0.0],
            uv_size: [f.w as f32 / self.atlas_w, f.h as f32 / self.atlas_h],
            depth,
            tint: [1.0, 1.0, 1.0],
            alpha: 1.0,
            ..Default::default()
        })
    }
    /// Cell47EC90 active ordinary draw -> CC_Draw_Shape4AED70. Its native
    /// flags20600 pick the plain53-row Convert's RLE row4960A0: opaque stores
    /// without A/Z interaction. Source: placement_render native execution.
    pub(crate) fn building_placement_instance(
        &self,
        cell: BuildingPlacementCell,
    ) -> Option<SpriteInstance> {
        let frame = if cell.valid {
            0
        } else {
            usize::from(cell.slope) + 2
        };
        // Caller supplies the unraised tile point;47EC90 adds30 to X and
        // subtracts15*signed_level+1 from Y before the centered shape draw.
        // iso_to_screen includes that signed level and VERA's shared row bias.
        let (x, y) = terrain::iso_to_screen(cell.coord.0, cell.coord.1, cell.level as u8);
        let mut instance = self.instance(frame, (x + 30.0, y - 1.0), 0.0)?;
        instance.palette_light = crate::render::palette_light::PaletteLight::plain(53, 1000);
        Some(instance)
    }

    /// Decode every raw SHP frame in its named palette. This is the same
    /// resource path for windowed loading and offscreen GPU checks.
    pub(crate) fn from_assets_on_device(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batch: &BatchRenderer,
        assets: &AssetManager,
        shp_name: &str,
        pal_name: &str,
    ) -> Option<Self> {
        let shp = ShpFile::from_bytes(&assets.get(shp_name)?).ok()?;
        let palette = Palette::from_bytes(&assets.get(pal_name)?).ok()?;
        let atlas_w: u32 = shp
            .frames
            .iter()
            .map(|f| u32::from(f.frame_width))
            .sum::<u32>()
            .max(1);
        let atlas_h: u32 = shp
            .frames
            .iter()
            .map(|f| u32::from(f.frame_height))
            .max()
            .unwrap_or(0)
            .max(1);
        let mut rgba = vec![0u8; (atlas_w * atlas_h * 4) as usize];
        // Preserve the original indices for the shared Convert palette
        // resolver. Raw-color icon consumers keep their default PaletteLight;
        // placement supplies its native plain Convert instead.
        let mut indices = vec![0u8; (atlas_w * atlas_h) as usize];
        let mut frames = Vec::with_capacity(shp.frames.len());
        let mut x = 0;
        for frame in &shp.frames {
            let (w, h) = (u32::from(frame.frame_width), u32::from(frame.frame_height));
            for py in 0..h {
                for px in 0..w {
                    let index = frame.pixels[(py * w + px) as usize];
                    indices[(py * atlas_w + x + px) as usize] = index;
                    if index == 0 {
                        continue;
                    }
                    let color = palette.colors[index as usize];
                    let dst = ((py * atlas_w + x + px) * 4) as usize;
                    rgba[dst..dst + 4].copy_from_slice(&[color.r, color.g, color.b, 255]);
                }
            }
            frames.push(CenteredShapeFrame {
                x,
                w,
                h,
                offset: (
                    i32::from(frame.frame_x) - i32::from(shp.width / 2),
                    i32::from(frame.frame_y) - i32::from(shp.height / 2),
                ),
            });
            x += w;
        }
        log::info!(
            "{shp_name} strip in {pal_name}: {atlas_w}x{atlas_h}, {} frames",
            frames.len()
        );
        Some(Self {
            texture: batch.create_texture_on_device(
                device,
                queue,
                &rgba,
                atlas_w,
                atlas_h,
                Some(&indices),
            ),
            atlas_w: atlas_w as f32,
            atlas_h: atlas_h as f32,
            frames,
        })
    }
    /// Active foundation cells in Tactical6D5030's rectangular draw order.
    /// Admission and current Cell ground fields are recomputed with the preview;
    /// this consumer never decides buildability or uses the render height cache.
    pub fn build_building_preview(
        &self,
        preview: &BuildingPlacementPreview,
    ) -> Vec<SpriteInstance> {
        let mut instances = Vec::with_capacity(preview.cells.len());
        // Tactical6D5030 walks rectangular foundation bounds with X outer,
        // Y inner, independently of the row-major admission/sentinel list.
        // Reordering by color changes stores where sloped cells overlap.
        for dx in 0..preview.width {
            for dy in 0..preview.height {
                let index = usize::from(dy) * usize::from(preview.width) + usize::from(dx);
                let Some(cell) = preview.cells.get(index) else {
                    continue;
                };
                if let Some(instance) = self.building_placement_instance(*cell) {
                    instances.push(instance);
                }
            }
        }
        // Preserve existing wall auto-fill admission and order; pending marks
        // and blocked-wall suppression remain explicit separate residuals.
        instances.extend(
            preview
                .wall_autofill_cells
                .iter()
                .filter_map(|cell| self.building_placement_instance(*cell)),
        );
        instances
    }
}

impl SelectionOverlay {
    /// Create a new SelectionOverlay with procedurally-generated textures.
    /// If `assets` is provided, loads pips.shp for authentic RA2 health bar rendering.
    pub fn new(gpu: &GpuContext, batch: &BatchRenderer, assets: Option<&AssetManager>) -> Self {
        Self::new_on_device(&gpu.device, &gpu.queue, batch, assets)
    }

    /// The same resource construction for windowed rendering and GPU checks.
    pub(crate) fn new_on_device(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batch: &BatchRenderer,
        assets: Option<&AssetManager>,
    ) -> Self {
        let drag_texture: BatchTexture = batch.create_texture_on_device(
            device,
            queue,
            &generate_solid_color(DRAG_RECT_COLOR),
            2,
            2,
            None,
        );
        // Display4A885B..4A8873 binds PLACE; InitGame52BF26..52BFC1
        // binds PALETTE.PAL's plain Convert87F6C4. Missing retail art has no
        // invented procedural substitute.
        let placement_shape = assets.and_then(|assets| {
            CenteredShapeStrip::from_assets_on_device(
                device,
                queue,
                batch,
                assets,
                "place.shp",
                "palette.pal",
            )
        });
        let white_texture: BatchTexture = batch.create_texture_on_device(
            device,
            queue,
            &generate_solid_color([255, 255, 255, 255]),
            2,
            2,
            None,
        );
        let diamond_outline_texture: BatchTexture = batch.create_texture_on_device(
            device,
            queue,
            &generate_diamond_outline(),
            TILE_WIDTH as u32,
            TILE_HEIGHT as u32,
            None,
        );
        let diamond_filled_texture: BatchTexture = batch.create_texture_on_device(
            device,
            queue,
            &generate_diamond_cell([255, 255, 255, 255]),
            TILE_WIDTH as u32,
            TILE_HEIGHT as u32,
            None,
        );

        // Load pips.shp and pack 4 isometric pip frames into a horizontal strip atlas.
        let (pip_texture, pip_frame_w, pip_frame_h, pip_adj_x, pip_adj_y) =
            load_pip_atlas(device, queue, batch, assets).unwrap_or((None, 0, 0, 0.0, 0.0));

        // LoadPipAssets 5F76B0 binds PIPBRD at AC1478. DrawHealthBar 6F68AE/
        // 6F6947 selects raw frame 1/0; 4AED70 owns the stored centering.
        let pipbrd = assets.and_then(|assets| {
            CenteredShapeStrip::from_assets_on_device(
                device,
                queue,
                batch,
                assets,
                "pipbrd.shp",
                "palette.pal",
            )
        });

        // Load pips.shp frames 6-12 for occupant pips (garrison slots).
        let (
            occupant_pip_texture,
            occupant_pip_frame_w,
            occupant_pip_frame_h,
            occ_adj_x,
            occ_adj_y,
        ) = load_occupant_pip_atlas(device, queue, batch, assets).unwrap_or((None, 0, 0, 0.0, 0.0));

        // Native Init_Game52BF26/52BFC1 builds Convert87F6C4 from the
        // literal PALETTE.PAL; keep the SHP's full raw frame table. The
        // common centered owner applies each stored frame's own origin.
        let scale_pips = assets.and_then(|assets| {
            CenteredShapeStrip::from_assets_on_device(
                device,
                queue,
                batch,
                assets,
                "pips2.shp",
                "palette.pal",
            )
        });

        // `Rules+0xFE0`, BOMBCURS.SHP by name (0x0066C5F4), drawn through the
        // convert `[0x0087F6C8]` that Init_Game builds from MOUSEPAL.PAL
        // (0x0052C13D..0x0052C1DD).
        let bomb_clock = assets.and_then(|assets| {
            CenteredShapeStrip::from_assets_on_device(
                device,
                queue,
                batch,
                assets,
                "bombcurs.shp",
                "mousepal.pal",
            )
        });
        // WRENCH.SHP by name (`0x0045E99E` into `[0x0089DDC8]`), drawn
        // through the same convert.
        let repair_wrench = assets.and_then(|assets| {
            CenteredShapeStrip::from_assets_on_device(
                device,
                queue,
                batch,
                assets,
                "wrench.shp",
                "mousepal.pal",
            )
        });
        // Original 5F76B0 /52BF26..52BFC1: one PIPS.SHP/PALETTE.PAL
        // resource serves health 6F6A4A and the existing self-heal 70A6BB tail.
        // Keep every frame's native origin and original index plane.
        let pips = assets.and_then(|assets| {
            CenteredShapeStrip::from_assets_on_device(
                device,
                queue,
                batch,
                assets,
                "pips.shp",
                "palette.pal",
            )
        });

        Self {
            drag_texture,
            placement_shape,
            white_texture,
            diamond_outline_texture,
            diamond_filled_texture,
            pip_texture,
            pip_frame_w,
            pip_frame_h,
            pip_canvas_adj: (pip_adj_x, pip_adj_y),
            pipbrd,
            occupant_pip_texture,
            occupant_pip_frame_w,
            occupant_pip_frame_h,
            occupant_pip_canvas_adj: (occ_adj_x, occ_adj_y),
            scale_pips,
            bomb_clock,
            repair_wrench,
            pips,
        }
    }

    /// The Crazy Ivan bomb clock art (BOMBCURS.SHP), when the assets have it.
    pub fn bomb_clock(&self) -> Option<&CenteredShapeStrip> {
        self.bomb_clock.as_ref()
    }

    /// The repair wrench art (WRENCH.SHP), when the assets have it.
    pub fn repair_wrench(&self) -> Option<&CenteredShapeStrip> {
        self.repair_wrench.as_ref()
    }

    /// Physical PIPS.SHP shared by health frames 16..18 and self-heal 13/20.
    pub fn pips(&self) -> Option<&CenteredShapeStrip> {
        self.pips.as_ref()
    }

    /// Physical PIPBRD.SHP selected-background frames 0/1.
    pub fn pipbrd(&self) -> Option<&CenteredShapeStrip> {
        self.pipbrd.as_ref()
    }

    /// Build sprite instances for the selection drag rectangle outline.
    ///
    /// Returns 4 thin line instances forming the rectangle. Positions are in
    /// screen-space with camera offset added (the shader subtracts camera_pos).
    pub fn build_drag_rect(
        &self,
        selection: &SelectionState,
        camera_x: f32,
        camera_y: f32,
    ) -> Vec<SpriteInstance> {
        let (min_x, min_y, max_x, max_y) = match selection.drag_rect() {
            Some(r) => r,
            None => return Vec::new(),
        };

        let rect_w: f32 = max_x - min_x;
        let rect_h: f32 = max_y - min_y;
        let t: f32 = DRAG_RECT_THICKNESS;

        // Offset by camera so screen-space coords survive the shader's
        // camera_pos subtraction.
        let ox: f32 = camera_x;
        let oy: f32 = camera_y;

        vec![
            // Top edge.
            SpriteInstance {
                position: [ox + min_x, oy + min_y],
                size: [rect_w, t],
                uv_origin: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                depth: DRAG_RECT_DEPTH,
                tint: [1.0, 1.0, 1.0],
                alpha: 1.0,
                ..Default::default()
            },
            // Bottom edge.
            SpriteInstance {
                position: [ox + min_x, oy + max_y - t],
                size: [rect_w, t],
                uv_origin: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                depth: DRAG_RECT_DEPTH,
                tint: [1.0, 1.0, 1.0],
                alpha: 1.0,
                ..Default::default()
            },
            // Left edge.
            SpriteInstance {
                position: [ox + min_x, oy + min_y],
                size: [t, rect_h],
                uv_origin: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                depth: DRAG_RECT_DEPTH,
                tint: [1.0, 1.0, 1.0],
                alpha: 1.0,
                ..Default::default()
            },
            // Right edge.
            SpriteInstance {
                position: [ox + max_x - t, oy + min_y],
                size: [t, rect_h],
                uv_origin: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                depth: DRAG_RECT_DEPTH,
                tint: [1.0, 1.0, 1.0],
                alpha: 1.0,
                ..Default::default()
            },
        ]
    }

    /// Get the drag rect texture for drawing the selection box.
    pub fn drag_texture(&self) -> &BatchTexture {
        &self.drag_texture
    }

    pub fn placement_shape(&self) -> Option<&CenteredShapeStrip> {
        self.placement_shape.as_ref()
    }

    pub fn white_texture(&self) -> &BatchTexture {
        &self.white_texture
    }

    /// Get the diamond outline texture for debug cell grid visualization.
    pub fn diamond_outline_texture(&self) -> &BatchTexture {
        &self.diamond_outline_texture
    }

    /// Get the white filled-diamond texture for debug pathgrid/path overlays.
    pub fn diamond_filled_texture(&self) -> &BatchTexture {
        &self.diamond_filled_texture
    }

    /// Get the pip atlas texture for health bar rendering.
    /// Returns None if pips.shp failed to load (falls back to white_texture).
    pub fn pip_texture(&self) -> Option<&BatchTexture> {
        self.pip_texture.as_ref()
    }

    /// UV origin for a given pip variant in the horizontal strip atlas.
    /// Variants use pips.shp frame ids: 0=empty, 1=green, 2=yellow, 4=red.
    pub fn pip_uv_origin(&self, variant: u32) -> [f32; 2] {
        let atlas_slot = match variant {
            0..=2 => variant,
            4 => 3,
            _ => 0,
        };
        let u: f32 = atlas_slot as f32 / PIP_VARIANT_COUNT as f32;
        [u, 0.0]
    }

    /// UV size for a single pip frame in the atlas.
    pub fn pip_uv_size(&self) -> [f32; 2] {
        [1.0 / PIP_VARIANT_COUNT as f32, 1.0]
    }

    /// Pixel size of one pip frame.
    pub fn pip_frame_size(&self) -> [f32; 2] {
        [self.pip_frame_w as f32, self.pip_frame_h as f32]
    }

    /// Canvas centering adjustment for building pips.
    /// Matches gamemd DrawSHP flag 0x200: (-canvas_w/2 + frame_x, -canvas_h/2 + frame_y).
    pub fn pip_canvas_adj(&self) -> (f32, f32) {
        self.pip_canvas_adj
    }

    /// Occupant pip atlas texture (pips.shp frames 6-12).
    pub fn occupant_pip_texture(&self) -> Option<&BatchTexture> {
        self.occupant_pip_texture.as_ref()
    }

    /// Number of pip variants in the shared occupant/veterancy atlas.
    const OCCUPANT_PIP_VARIANTS: u32 = OCCUPANT_PIP_FRAMES.len() as u32;

    /// UV origin for a veterancy chevron. `rank` is 0 rookie, 1 veteran, 2 elite.
    pub fn veterancy_pip_uv_origin(&self, rank: u32) -> [f32; 2] {
        let frame: usize = match rank {
            2 => 15,
            1 => 14,
            _ => 19,
        };
        let slot = OCCUPANT_PIP_FRAMES
            .iter()
            .position(|f| *f == frame)
            .unwrap_or(0) as u32;
        [slot as f32 / Self::OCCUPANT_PIP_VARIANTS as f32, 0.0]
    }

    /// UV origin for an occupant pip by pips.shp frame index (6-12).
    /// Frame 6 → slot 0, frame 7 → slot 1, ..., frame 12 → slot 6.
    pub fn occupant_pip_uv_origin(&self, frame_index: u32) -> [f32; 2] {
        let slot: u32 = OCCUPANT_PIP_FRAMES
            .iter()
            .position(|f| *f as u32 == frame_index)
            .unwrap_or(0) as u32;
        [slot as f32 / Self::OCCUPANT_PIP_VARIANTS as f32, 0.0]
    }

    /// UV size for a single occupant pip frame in the atlas.
    pub fn occupant_pip_uv_size(&self) -> [f32; 2] {
        [1.0 / Self::OCCUPANT_PIP_VARIANTS as f32, 1.0]
    }

    /// Pixel size of one occupant pip frame.
    pub fn occupant_pip_frame_size(&self) -> [f32; 2] {
        [
            self.occupant_pip_frame_w as f32,
            self.occupant_pip_frame_h as f32,
        ]
    }

    /// Canvas centering adjustment for occupant pips.
    pub fn occupant_pip_canvas_adj(&self) -> (f32, f32) {
        self.occupant_pip_canvas_adj
    }

    /// The one PIPS2.SHP strip: raw frame0 empty,1 occupied,3/4 overload,
    /// and2/5 ore/gem. Callers supply native frame IDs, not atlas variants.
    pub fn scale_pips(&self) -> Option<&CenteredShapeStrip> {
        self.scale_pips.as_ref()
    }
}

/// Generate a 2x2 solid color texture (4 identical pixels).
fn generate_solid_color(color: [u8; 4]) -> Vec<u8> {
    let mut rgba: Vec<u8> = Vec::with_capacity(16);
    for _ in 0..4 {
        rgba.extend_from_slice(&color);
    }
    rgba
}

/// Load pips.shp (buildings) and pack 4 health pip frames into a horizontal strip atlas.
///
/// pips.shp frame indices:
///   0 = empty pip (dark/unfilled)
///   1 = green pip (healthy)
///   2 = yellow pip (medium health)
///   4 = red pip (critical health)
///
/// This specialized strip serves building health. Mobile health uses raw frames16..18.
/// Uses palette.pal for coloring.
///
/// Returns (Some(texture), frame_w, frame_h) on success, or (None, 0, 0) on failure.
fn load_pip_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &BatchRenderer,
    assets: Option<&AssetManager>,
) -> Option<(Option<BatchTexture>, u32, u32, f32, f32)> {
    let assets = assets?;
    // pips.shp = building health bar pips.
    let shp_data = assets.get("pips.shp")?;
    let shp = ShpFile::from_bytes(&shp_data).ok()?;
    log::info!(
        "Pip atlas source: pips.shp ({}x{}, {} frames)",
        shp.width,
        shp.height,
        shp.frames.len()
    );
    for (i, f) in shp.frames.iter().enumerate() {
        log::info!(
            "  pip frame {:2}: pos=({},{}) size={}x{} pixels={}",
            i,
            f.frame_x,
            f.frame_y,
            f.frame_width,
            f.frame_height,
            f.pixels.len()
        );
    }
    // Load palette.pal for pip sprites (the general game palette, NOT unittem.pal).
    let pal_data = assets
        .get("palette.pal")
        .or_else(|| assets.get("unittem.pal"))?;
    let palette = Palette::from_bytes(&pal_data).ok()?;

    // pips.shp frame layout (buildings only):
    //   0 = empty pip (dark/unfilled)
    //   1 = green pip (healthy)
    //   2 = yellow pip (medium health)
    //   4 = red pip (critical health)
    // Atlas slot order: [empty, green, yellow, red]
    let frame_indices: [usize; PIP_VARIANT_COUNT as usize] = [0, 1, 2, 4];

    // Find max dimensions across the selected frames.
    let mut max_w: u32 = 0;
    let mut max_h: u32 = 0;
    for &fi in &frame_indices {
        if fi >= shp.frames.len() {
            log::warn!("pips.shp frame {} out of range ({})", fi, shp.frames.len());
            return Some((None, 0, 0, 0.0, 0.0));
        }
        let fw: u32 = shp.frames[fi].frame_width as u32;
        let fh: u32 = shp.frames[fi].frame_height as u32;
        if fw > max_w {
            max_w = fw;
        }
        if fh > max_h {
            max_h = fh;
        }
    }

    // Pack frames side-by-side into atlas (max_w * 4, max_h).
    let atlas_w: u32 = max_w * PIP_VARIANT_COUNT;
    let atlas_h: u32 = max_h;
    let mut rgba: Vec<u8> = vec![0u8; (atlas_w * atlas_h * 4) as usize];

    for (slot, &fi) in frame_indices.iter().enumerate() {
        let frame = &shp.frames[fi];
        let fw: u32 = frame.frame_width as u32;
        let fh: u32 = frame.frame_height as u32;
        let x_off: u32 = slot as u32 * max_w;
        // Center the frame within its slot if smaller than max.
        let cx: u32 = (max_w - fw) / 2;
        let cy: u32 = (max_h - fh) / 2;
        for py in 0..fh {
            for px in 0..fw {
                let src_idx: usize = (py * fw + px) as usize;
                let pal_idx: u8 = frame.pixels[src_idx];
                if pal_idx == 0 {
                    continue; // Transparent.
                }
                let color = palette.colors[pal_idx as usize];
                let dst_x: u32 = x_off + cx + px;
                let dst_y: u32 = cy + py;
                let dst: usize = ((dst_y * atlas_w + dst_x) * 4) as usize;
                rgba[dst] = color.r;
                rgba[dst + 1] = color.g;
                rgba[dst + 2] = color.b;
                rgba[dst + 3] = 255;
            }
        }
    }

    // Canvas centering: match gamemd DrawSHP flag 0x200 integer division.
    // actualX = drawX - (int)canvas_w/2 + frame_x
    // actualY = drawY - (int)canvas_h/2 + frame_y
    let ref_frame = &shp.frames[0]; // use frame 0 (empty pip) as reference
    let adj_x: f32 = ref_frame.frame_x as f32 - (shp.width / 2) as f32;
    let adj_y: f32 = ref_frame.frame_y as f32 - (shp.height / 2) as f32;

    let texture: BatchTexture =
        batch.create_texture_on_device(device, queue, &rgba, atlas_w, atlas_h, None);
    log::info!(
        "Pip atlas: {}x{} (frame {}x{}, {} variants, SHP {}x{} canvas_adj=({:.0},{:.0}))",
        atlas_w,
        atlas_h,
        max_w,
        max_h,
        PIP_VARIANT_COUNT,
        shp.width,
        shp.height,
        adj_x,
        adj_y,
    );
    Some((Some(texture), max_w, max_h, adj_x, adj_y))
}

/// Load pips.shp frames 6-12 for occupant (garrison) pips into a 7-variant atlas.
///
/// pips.shp frame indices:
///   6 = empty occupant slot (gray)
///   7 = PersonGreen, 8 = PersonYellow, 9 = PersonWhite
///   10 = PersonRed, 11 = PersonBlue, 12 = PersonPurple
///
/// Returns (Some(texture), frame_w, frame_h, canvas_adj_x, canvas_adj_y) on success.
/// pips.shp frames packed into the shared occupant/veterancy strip.
///
/// 6 is the empty garrison slot and 7-12 are PersonGreen..PersonPurple. 14, 15
/// and 19 are the veterancy chevrons: `DrawVeterancyPips` 0x0070A990 (TechnoClass
/// vtable +0x454) blits frame 14 for veteran, 15 for elite and 19 for the
/// rookie-flagged variant. They ride the same atlas so they cost no extra
/// texture and no extra draw pass.
const OCCUPANT_PIP_FRAMES: [usize; 10] = [6, 7, 8, 9, 10, 11, 12, 14, 15, 19];

fn load_occupant_pip_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &BatchRenderer,
    assets: Option<&AssetManager>,
) -> Option<(Option<BatchTexture>, u32, u32, f32, f32)> {
    let assets = assets?;
    let shp_data = assets.get("pips.shp")?;
    let shp = ShpFile::from_bytes(&shp_data).ok()?;
    if shp.frames.len() <= OCCUPANT_PIP_FRAMES[OCCUPANT_PIP_FRAMES.len() - 1] {
        log::warn!(
            "pips.shp has too few frames ({}), cannot load the occupant/veterancy pip atlas",
            shp.frames.len()
        );
        return Some((None, 0, 0, 0.0, 0.0));
    }
    let pal_data = assets
        .get("palette.pal")
        .or_else(|| assets.get("unittem.pal"))?;
    let palette = Palette::from_bytes(&pal_data).ok()?;

    const VARIANT_COUNT: u32 = OCCUPANT_PIP_FRAMES.len() as u32;
    let frame_indices = OCCUPANT_PIP_FRAMES;
    let mut max_w: u32 = 0;
    let mut max_h: u32 = 0;
    for &fi in &frame_indices {
        let fw: u32 = shp.frames[fi].frame_width as u32;
        let fh: u32 = shp.frames[fi].frame_height as u32;
        if fw > max_w {
            max_w = fw;
        }
        if fh > max_h {
            max_h = fh;
        }
    }

    let atlas_w: u32 = max_w * VARIANT_COUNT;
    let atlas_h: u32 = max_h;
    let mut rgba: Vec<u8> = vec![0u8; (atlas_w * atlas_h * 4) as usize];

    for (slot, &fi) in frame_indices.iter().enumerate() {
        let frame = &shp.frames[fi];
        let fw: u32 = frame.frame_width as u32;
        let fh: u32 = frame.frame_height as u32;
        let x_off: u32 = slot as u32 * max_w;
        let cx: u32 = (max_w - fw) / 2;
        let cy: u32 = (max_h - fh) / 2;
        for py in 0..fh {
            for px in 0..fw {
                let src_idx: usize = (py * fw + px) as usize;
                let pal_idx: u8 = frame.pixels[src_idx];
                if pal_idx == 0 {
                    continue;
                }
                let color = palette.colors[pal_idx as usize];
                let dst_x: u32 = x_off + cx + px;
                let dst_y: u32 = cy + py;
                let dst: usize = ((dst_y * atlas_w + dst_x) * 4) as usize;
                rgba[dst] = color.r;
                rgba[dst + 1] = color.g;
                rgba[dst + 2] = color.b;
                rgba[dst + 3] = 255;
            }
        }
    }

    let ref_frame = &shp.frames[6];
    let adj_x: f32 = ref_frame.frame_x as f32 - (shp.width / 2) as f32;
    let adj_y: f32 = ref_frame.frame_y as f32 - (shp.height / 2) as f32;

    let texture: BatchTexture =
        batch.create_texture_on_device(device, queue, &rgba, atlas_w, atlas_h, None);
    log::info!(
        "Occupant/veterancy pip atlas: {}x{} (frame {}x{}, {} variants, frames {:?}, adj ({:.0},{:.0}))",
        atlas_w,
        atlas_h,
        max_w,
        max_h,
        VARIANT_COUNT,
        OCCUPANT_PIP_FRAMES,
        adj_x,
        adj_y,
    );
    Some((Some(texture), max_w, max_h, adj_x, adj_y))
}

/// Generate a 1px-thick diamond outline texture (TILE_WIDTH x TILE_HEIGHT).
/// White outline on transparent background — tinted per-instance for debug coloring.
fn generate_diamond_outline() -> Vec<u8> {
    let w: u32 = TILE_WIDTH as u32;
    let h: u32 = TILE_HEIGHT as u32;
    let cx: f32 = w as f32 / 2.0;
    let cy: f32 = h as f32 / 2.0;
    let mut rgba: Vec<u8> = vec![0u8; (w * h * 4) as usize];
    for py in 0..h {
        for px in 0..w {
            let dist: f32 = (px as f32 - cx).abs() / cx + (py as f32 - cy).abs() / cy;
            // Draw pixels near the diamond edge (within ~1.5px of dist=1.0).
            let edge_dist: f32 = (dist - 1.0).abs() * cx;
            if edge_dist < 1.5 {
                let alpha: f32 = (1.5 - edge_dist).min(1.0);
                let idx: usize = ((py * w + px) * 4) as usize;
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = (alpha * 255.0) as u8;
            }
        }
    }
    rgba
}

/// Generate an isometric diamond-shaped cell texture (TILE_WIDTH x TILE_HEIGHT).
/// The diamond inscribes the tile rectangle with 1px anti-aliased edges.
fn generate_diamond_cell(color: [u8; 4]) -> Vec<u8> {
    let w: u32 = TILE_WIDTH as u32;
    let h: u32 = TILE_HEIGHT as u32;
    let cx: f32 = (w as f32) / 2.0;
    let cy: f32 = (h as f32) / 2.0;
    let mut rgba: Vec<u8> = vec![0u8; (w * h * 4) as usize];
    for py in 0..h {
        for px in 0..w {
            // Normalized diamond distance: 0 at center, 1 at edge.
            let dist: f32 = (px as f32 - cx).abs() / cx + (py as f32 - cy).abs() / cy;
            if dist <= 1.0 {
                // Anti-alias: fade out over the last ~1px before the edge.
                let edge_fade: f32 = ((1.0 - dist) * cx).min(1.0);
                let alpha: u8 = (color[3] as f32 * edge_fade) as u8;
                let idx: usize = ((py * w + px) * 4) as usize;
                rgba[idx] = color[0];
                rgba[idx + 1] = color[1];
                rgba[idx + 2] = color[2];
                rgba[idx + 3] = alpha;
            }
        }
    }
    rgba
}
