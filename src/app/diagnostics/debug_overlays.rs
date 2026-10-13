//! Debug visualization overlays — pathgrid walkability, terrain costs, etc.
//!
//! Each overlay builds `Vec<SpriteInstance>` quads drawn with the white texture
//! at low opacity using tint colors. Toggled via hotkeys (F9 = pathgrid).
//!
//! ## Dependency rules
//! - Part of the app layer — may depend on everything.

use crate::app::AppState;
use crate::app::presentation::instances::in_view;
use crate::map::terrain::{self, TILE_HEIGHT, TILE_WIDTH};
use crate::render::batch::SpriteInstance;
use crate::rules::locomotor_type::SpeedType;

/// Semi-transparent tint alpha applied to debug overlay cells.
/// Multiplied into the RGB channels since the shader does `color.rgb * tint`.
const OVERLAY_ALPHA: f32 = 0.35;

/// Depth for debug overlays — drawn above terrain but below entities.
/// Between terrain (depth ~0.9+) and entities/overlays (depth ~0.0005-0.0007).
const DEBUG_OVERLAY_DEPTH: f32 = 0.0004;

// -- Terrain cost overlay colors --

/// Terrain cost: blocked (cost=0). Red.
const COST_BLOCKED_TINT: [f32; 3] = [OVERLAY_ALPHA, 0.0, 0.0];
/// Terrain cost: very rough (cost ~60, e.g. wheel on rough). Orange.
const COST_VERY_ROUGH_TINT: [f32; 3] = [OVERLAY_ALPHA, 0.15, 0.0];
/// Terrain cost: rough (cost ~75, e.g. track on rough). Dark yellow.
const COST_ROUGH_TINT: [f32; 3] = [OVERLAY_ALPHA, 0.25, 0.0];
/// Terrain cost: slightly slow (cost ~90, e.g. foot on rough). Yellow.
const COST_SLOW_TINT: [f32; 3] = [OVERLAY_ALPHA, OVERLAY_ALPHA, 0.0];
/// Terrain cost: normal passable (cost=100). Green.
const COST_NORMAL_TINT: [f32; 3] = [0.0, OVERLAY_ALPHA, 0.0];
/// Terrain cost: above normal (cost=106+, e.g. INI speed >100%). Cyan.
const COST_FAST_TINT: [f32; 3] = [0.0, OVERLAY_ALPHA, OVERLAY_ALPHA];

/// Map a terrain cost value to a tint color.
fn cost_to_tint(cost: u8) -> [f32; 3] {
    match cost {
        0 => COST_BLOCKED_TINT,
        1..=65 => COST_VERY_ROUGH_TINT,
        66..=80 => COST_ROUGH_TINT,
        81..=95 => COST_SLOW_TINT,
        96..=105 => COST_NORMAL_TINT,
        _ => COST_FAST_TINT,
    }
}

// -- Foot-path polyline overlay --

/// The route line: the same blue as the reference debug path.
const PATH_LINE_TINT: [f32; 3] = [0.25, 0.44, 0.91];
const PATH_LINE_ALPHA: f32 = 0.95;
/// The route stroke, rasterised the way the rest of the renderer draws lines.
///
/// This renderer has **no rotated-quad concept at all** — the sprite vertex
/// shader only does `position + local * size`, and the one real line path
/// (`render::terrain_surface_lines` + `surface_line::rasterize`) is a CPU DDA
/// walk that emits one quad *per pixel*. So rather than approximate a
/// diagonal with spaced blobs, the stroke follows that same idiom: step one
/// world pixel along the segment and drop a quad there. At native zoom the
/// quads touch and it reads as a true 1px line instead of a dotted staircase.
///
/// The quad is wider than the step so consecutive pixels overlap and leave no
/// seam. Width is the stroke's visible thickness: 1.5px measured as a 2px line
/// on screen and was hard to see against terrain, so it is now 2.5px — still a
/// line, not a slab (7px fused into a ~9px band).
const LINE_STEP_PX: f32 = 1.0;
const LINE_PIXEL_SIZE: f32 = 2.5;
/// Node dot on the waypoint ring — drawn over the stroke, as an iso diamond
/// (nodes mark *cells*, so the diamond cell texture is right for them; the
/// stroke must not use it or its transparent corners make the line faint).
const NODE_SIZE: f32 = 8.0;
/// Immediate next step: bright magenta.
const PATH_NEXT_TINT: [f32; 3] = [0.91, 0.20, 0.91];
/// Final step / arrival cell: bright yellow.
const PATH_GOAL_TINT: [f32; 3] = [0.98, 0.80, 0.08];
/// Step-index label tint. Near-white with a blue cast: a mid-blue was too low
/// a contrast against lit terrain to read at a glance.
const PATH_LABEL_TINT: [f32; 3] = [0.90, 0.96, 1.0];
const PATH_LABEL_HALO_TINT: [f32; 3] = [0.01, 0.02, 0.09];
/// Depth — above the terrain-cost wash, below entities.
const PATH_DEPTH: f32 = 0.00025;
/// Label scale for `BitFont::build_text`. GAME.FNT glyphs are 16 rows tall, so
/// this lands a digit at ~13px. 0.5 (~8px) measured at barely 121 label pixels
/// across a whole route and was unreadable; the reference view's indices are
/// small but plainly legible.
const PATH_LABEL_SCALE: f32 = 0.8;
/// At most this many step indices are labelled. A long route would otherwise
/// bury the map in digits; past the cap the ends are always labelled and the
/// rest is sampled at `label_stride`.
const MAX_PATH_LABELS: usize = 32;

/// Camera offset for `BitFont::build_text` on the **world** camera path.
///
/// Must stay zero. `build_text` adds this to every glyph, and the debug batch
/// is drawn with the world camera in bind group 0, so the shader already
/// subtracts it — passing the live camera double-counts the scroll and throws
/// the glyphs off-screen. Only the UI-camera path (`draw_pooled_ui`, used by
/// tooltips and messages) needs the camera added back.
const NO_CAMERA_OFFSET: [f32; 2] = [0.0, 0.0];

/// Screen centre of a map cell, given `iso_to_screen`'s top-left origin.
fn cell_center(sx: f32, sy: f32) -> [f32; 2] {
    [sx + TILE_WIDTH / 2.0, sy + TILE_HEIGHT / 2.0]
}

/// The halo/digit submission order, as the drawing loop uses it.
///
/// The batch does not blend instances of one texture against each other —
/// later quads overwrite earlier ones. The dark halo copies therefore have
/// to be pushed before the digit, or they bury the glyph they outline.
/// Kept as data rather than inline in the loop so a test can pin the order.
const LABEL_PASSES: [(f32, f32, [f32; 3]); 5] = [
    (-1.0, 0.0, PATH_LABEL_HALO_TINT),
    (1.0, 0.0, PATH_LABEL_HALO_TINT),
    (0.0, -1.0, PATH_LABEL_HALO_TINT),
    (0.0, 1.0, PATH_LABEL_HALO_TINT),
    (0.0, 0.0, PATH_LABEL_TINT),
];

/// Rasterise the stroke covering segment `a -> b`, excluding the endpoint
/// (the caller's next segment or the node ring draws it) so joints never
/// double up.
///
/// One quad per world pixel, stepped along the segment — the same CPU walk
/// `surface_line::rasterize` performs for the native line effects. The endpoint
/// is skipped because the next segment starts there, which also keeps the
/// corner pixel from being emitted twice.
fn push_stroke_along(a: [f32; 2], b: [f32; 2], instances: &mut Vec<SpriteInstance>) {
    let delta = [b[0] - a[0], b[1] - a[1]];
    let dist = (delta[0] * delta[0] + delta[1] * delta[1]).sqrt();
    if dist <= 0.0 {
        return;
    }
    // One step per pixel, minus the endpoint.
    let steps = ((dist / LINE_STEP_PX).ceil() as usize).max(1);
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let p = [a[0] + delta[0] * t, a[1] + delta[1] * t];
        instances.push(SpriteInstance {
            position: [p[0] - LINE_PIXEL_SIZE / 2.0, p[1] - LINE_PIXEL_SIZE / 2.0],
            size: [LINE_PIXEL_SIZE, LINE_PIXEL_SIZE],
            uv_origin: [0.0, 0.0],
            uv_size: [1.0, 1.0],
            depth: PATH_DEPTH,
            tint: PATH_LINE_TINT,
            alpha: PATH_LINE_ALPHA,
            ..Default::default()
        });
    }
}

/// One waypoint node centred on a screen point.
fn push_node(p: [f32; 2], tint: [f32; 3], instances: &mut Vec<SpriteInstance>) {
    instances.push(SpriteInstance {
        position: [p[0] - NODE_SIZE / 2.0, p[1] - NODE_SIZE / 2.0],
        size: [NODE_SIZE, NODE_SIZE],
        uv_origin: [0.0, 0.0],
        uv_size: [1.0, 1.0],
        depth: PATH_DEPTH,
        tint,
        alpha: 1.0,
        ..Default::default()
    });
}

/// How many steps to skip between printed step indices on a route of `len`
/// steps: 1 labels every step, and longer routes thin out to an even sample so
/// the digit count stays bounded. Always at least 1 (never a divide-by-zero).
fn path_label_stride(len: usize) -> usize {
    if len <= MAX_PATH_LABELS {
        1
    } else {
        len.div_ceil(MAX_PATH_LABELS)
    }
}

/// Resolve which SpeedType the terrain cost overlay should display.
///
/// Priority: manual override → first selected entity's locomotor → Track default.
pub(crate) fn resolve_debug_speed_type(state: &AppState) -> SpeedType {
    if let Some(st) = state.diag.debug_terrain_cost_speed_type {
        return st;
    }
    state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)
        .and_then(|sim| {
            sim.entities()
                .values()
                .find(|e| e.selected)
                .and_then(|e| e.locomotor.as_ref())
                .map(|l| l.speed_type)
        })
        .unwrap_or(SpeedType::Track)
}

/// Build terrain cost overlay instances for all visible cells.
///
/// Each cell gets a colored diamond quad based on the movement cost for the
/// active SpeedType: red=blocked, orange/yellow=rough, green=normal, cyan=road.
pub(crate) fn build_terrain_cost_overlay_instances(
    state: &AppState,
    sw: f32,
    sh: f32,
) -> Vec<SpriteInstance> {
    let Some(sim) = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)
    else {
        log::warn!("Terrain cost overlay: no simulation");
        return Vec::new();
    };
    let speed_type = resolve_debug_speed_type(state);
    let Some(cost_grid) = sim.terrain_costs.get(&speed_type) else {
        log::warn!("Terrain cost overlay: no cost grid for {:?}", speed_type);
        return Vec::new();
    };

    let width: u16 = cost_grid.width();
    let height: u16 = cost_grid.height();

    // One-shot diagnostic on first call.
    use std::sync::atomic::{AtomicBool, Ordering};
    static LOGGED: AtomicBool = AtomicBool::new(false);
    if !LOGGED.swap(true, Ordering::Relaxed) {
        // Sample a few cells to verify the grid has non-zero data.
        let sample_costs: Vec<u8> = (0..width.min(20))
            .map(|x| cost_grid.cost_at(x, height / 2))
            .collect();
        log::info!(
            "Terrain cost overlay: {:?} grid {}x{}, cam=({:.0},{:.0}), screen=({:.0},{:.0}), sample costs={:?}",
            speed_type,
            width,
            height,
            state.match_state.input.camera_x,
            state.match_state.input.camera_y,
            sw,
            sh,
            sample_costs,
        );
    }

    let mut instances: Vec<SpriteInstance> = Vec::with_capacity(2048);
    let cam_x: f32 = state.match_state.input.camera_x;
    let cam_y: f32 = state.match_state.input.camera_y;

    for ry in 0..height {
        for rx in 0..width {
            // Bridge cells use deck height so dots appear ON the bridge.
            let z: u8 = state
                .bridge_height_map()
                .get(&(rx, ry))
                .copied()
                .unwrap_or_else(|| state.height_map().get(&(rx, ry)).copied().unwrap_or(0));
            let (sx, sy) = terrain::iso_to_screen(rx, ry, z);

            if !in_view(sx, sy, TILE_WIDTH, TILE_HEIGHT, cam_x, cam_y, sw, sh, 60.0) {
                continue;
            }

            let cost: u8 = cost_grid.cost_at(rx, ry);
            let tint: [f32; 3] = cost_to_tint(cost);

            instances.push(SpriteInstance {
                position: [sx, sy],
                size: [TILE_WIDTH, TILE_HEIGHT],
                uv_origin: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                depth: DEBUG_OVERLAY_DEPTH,
                tint,
                alpha: 1.0,
                ..Default::default()
            });
        }
    }

    instances
}

/// Build debug height map overlay — each cell tinted by elevation.
/// Ground cells use white brightness proportional to z level.
/// Bridge deck cells use blue tint to distinguish them from ground.
pub(crate) fn build_heightmap_overlay_instances(
    state: &AppState,
    sw: f32,
    sh: f32,
) -> Vec<SpriteInstance> {
    if state.height_map().is_empty() {
        return Vec::new();
    }
    let cam_x: f32 = state.match_state.input.camera_x;
    let cam_y: f32 = state.match_state.input.camera_y;
    let mut instances: Vec<SpriteInstance> = Vec::with_capacity(2048);

    // Find max z for normalization.
    let max_z: u8 = state
        .height_map()
        .values()
        .copied()
        .max()
        .unwrap_or(1)
        .max(1);

    // One-shot diagnostic: log bridge_height_map size on first call.
    use std::sync::atomic::{AtomicBool, Ordering};
    static LOGGED: AtomicBool = AtomicBool::new(false);
    if !LOGGED.swap(true, Ordering::Relaxed) {
        let bridge_count = state.bridge_height_map().len();
        let sample: Vec<_> = state.bridge_height_map().iter().take(10).collect();
        log::info!(
            "HeightmapOverlay: bridge_height_map has {} entries, height_map has {} entries. Sample: {:?}",
            bridge_count,
            state.height_map().len(),
            sample,
        );
    }

    for (&(rx, ry), &z) in state.height_map() {
        // Bridge cells: render at deck height so the overlay aligns with the bridge surface.
        let render_z: u8 = state
            .bridge_height_map()
            .get(&(rx, ry))
            .copied()
            .unwrap_or(z);
        let (sx, sy) = terrain::iso_to_screen(rx, ry, render_z);
        if !in_view(sx, sy, TILE_WIDTH, TILE_HEIGHT, cam_x, cam_y, sw, sh, 60.0) {
            continue;
        }

        let is_bridge = state.bridge_height_map().contains_key(&(rx, ry));
        let tint: [f32; 3] = if is_bridge {
            // Solid blue for bridge deck cells — high alpha so the bridge
            // structure doesn't show through the overlay.
            [0.0, 0.15, 0.7]
        } else {
            // White/grey brightness by elevation.
            let intensity: f32 = (z as f32 / max_z as f32) * OVERLAY_ALPHA;
            [intensity, intensity, intensity]
        };

        // Bridge cells need a lower depth to avoid being occluded by bridge
        // overlay sprites that write to the depth buffer in the entity pass.
        let cell_depth: f32 = if is_bridge {
            0.0001
        } else {
            DEBUG_OVERLAY_DEPTH
        };
        instances.push(SpriteInstance {
            position: [sx, sy],
            size: [TILE_WIDTH, TILE_HEIGHT],
            uv_origin: [0.0, 0.0],
            uv_size: [1.0, 1.0],
            depth: cell_depth,
            tint,
            alpha: 1.0,
            ..Default::default()
        });
    }
    instances
}

/// Build debug cell grid overlay — diamond outlines for terrain cells (blue)
/// and overlay cells (yellow) so alignment between the two can be verified visually.
///
/// Both use `iso_to_screen()` with the same height map. If they misalign,
/// the outlines will visually separate — making coordinate bugs obvious.
pub(crate) fn build_cell_grid_overlay_instances(
    state: &AppState,
    sw: f32,
    sh: f32,
) -> Vec<SpriteInstance> {
    let mut instances: Vec<SpriteInstance> = Vec::with_capacity(4096);
    let cam_x: f32 = state.match_state.input.camera_x;
    let cam_y: f32 = state.match_state.input.camera_y;

    /// Terrain cell grid: bright cyan outlines.
    const TERRAIN_TINT: [f32; 3] = [0.0, 0.9, 0.9];
    /// Overlay cell grid: bright yellow outlines.
    const OVERLAY_TINT: [f32; 3] = [1.0, 1.0, 0.0];
    /// Depth — just above terrain but below entities.
    const GRID_DEPTH: f32 = 0.0003;

    // Terrain grid cells.
    if let Some(grid) = &state.match_state.match_presentation.terrain_grid {
        for cell in &grid.cells {
            if !in_view(
                cell.screen_x,
                cell.screen_y,
                TILE_WIDTH,
                TILE_HEIGHT,
                cam_x,
                cam_y,
                sw,
                sh,
                60.0,
            ) {
                continue;
            }
            instances.push(SpriteInstance {
                position: [cell.screen_x, cell.screen_y],
                size: [TILE_WIDTH, TILE_HEIGHT],
                uv_origin: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                depth: GRID_DEPTH,
                tint: TERRAIN_TINT,
                alpha: 1.0,
                ..Default::default()
            });
        }
    }

    // Overlay cells — re-compute iso_to_screen for each overlay entry so the
    // diamond outline uses the exact same formula as overlay rendering.
    for entry in state.match_state.match_presentation.overlays.iter() {
        let z: u8 = state
            .height_map()
            .get(&(entry.rx, entry.ry))
            .copied()
            .unwrap_or(0);
        let (sx, sy) = terrain::iso_to_screen(entry.rx, entry.ry, z);
        if !in_view(sx, sy, TILE_WIDTH, TILE_HEIGHT, cam_x, cam_y, sw, sh, 60.0) {
            continue;
        }
        instances.push(SpriteInstance {
            position: [sx, sy],
            size: [TILE_WIDTH, TILE_HEIGHT],
            uv_origin: [0.0, 0.0],
            uv_size: [1.0, 1.0],
            depth: GRID_DEPTH - 0.0001,
            tint: OVERLAY_TINT,
            alpha: 1.0,
            ..Default::default()
        });
    }

    instances
}

/// The three batches the route overlay draws into.
///
/// They must stay separate because one batch binds exactly one texture:
/// - `stroke` — solid white. A thin line drawn with the iso **diamond** texture
///   loses its transparent corners and reads as a faint dotted trail.
/// - `nodes` — the filled diamond, because a node marks a *cell*.
/// - `text` — the GAME.FNT atlas, for the step indices.
pub(crate) struct PathOverlayInstances {
    /// Route stroke (solid white texture).
    pub stroke: Vec<SpriteInstance>,
    /// Waypoint nodes (filled-diamond texture).
    pub nodes: Vec<SpriteInstance>,
    /// Step-index glyphs (GAME.FNT atlas).
    pub text: Vec<SpriteInstance>,
}

/// Build the numbered foot-path polyline overlay for selected entities.
///
/// Draws the selected units' remaining route (`navigation.path_replay`, the
/// Foot+558 words every locomotor shares, filtered on a live move target) the
/// way the reference debug view does: a continuous blue line through the cell
/// centres with the step index printed at every waypoint, so both the shape of
/// the route and the order of the steps read at a glance. The immediate next
/// step is magenta and the arrival cell yellow, and the owner's goal is
/// included when it sits past the last queued step.
///
/// Rebuilt every frame, so the line shortens behind the unit as it walks.
///
/// The stroke is rasterised on the CPU one quad per pixel
/// ([`push_stroke_along`]) because this renderer has no rotated quads; the step
/// indices come from `BitFont::build_text`, which emits into this same batch
/// family and shares the terrain's camera uniform.
pub(crate) fn build_path_overlay_instances(state: &AppState) -> PathOverlayInstances {
    let mut stroke: Vec<SpriteInstance> = Vec::with_capacity(1024);
    let mut nodes: Vec<SpriteInstance> = Vec::with_capacity(64);
    let mut text: Vec<SpriteInstance> = Vec::with_capacity(128);
    let Some(sim) = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)
    else {
        return PathOverlayInstances {
            stroke,
            nodes,
            text,
        };
    };

    for entity in sim.entities().values() {
        if !entity.selected || entity.movement_target.is_none() {
            continue;
        }
        // The remaining Foot+558 words, walked from the reference cell.
        let path: Vec<(u16, u16)> = entity
            .navigation
            .path_replay
            .remaining_cells()
            .into_iter()
            .map(|(x, y)| (x as u16, y as u16))
            .collect();
        if path.is_empty() {
            continue;
        }

        // Waypoints in screen space: the queued cells plus the goal when it
        // sits past the last one.
        let mut points: Vec<[f32; 2]> = Vec::with_capacity(path.len() + 1);
        for &(px, py) in &path {
            let z: u8 = state.height_map().get(&(px, py)).copied().unwrap_or(0);
            let (sx, sy) = terrain::iso_to_screen(px, py, z);
            points.push(cell_center(sx, sy));
        }
        if let Some(goal) = crate::sim::movement::movement_goal_cell(entity)
            && goal != *path.last().unwrap()
        {
            let z: u8 = state
                .height_map()
                .get(&(goal.0, goal.1))
                .copied()
                .unwrap_or(0);
            let (sx, sy) = terrain::iso_to_screen(goal.0, goal.1, z);
            points.push(cell_center(sx, sy));
        }

        for pair in points.windows(2) {
            push_stroke_along(pair[0], pair[1], &mut stroke);
        }

        // Node ring + step indices. Index 1 is the immediate next step and the
        // last index is the arrival cell, matching the reference numbering.
        let len = points.len();
        let stride = path_label_stride(len);
        for (i, &p) in points.iter().enumerate() {
            let is_next = i == 0;
            let is_goal = i + 1 == len;
            push_node(
                p,
                if is_goal {
                    PATH_GOAL_TINT
                } else if is_next {
                    PATH_NEXT_TINT
                } else {
                    PATH_LINE_TINT
                },
                &mut nodes,
            );
            // Label the ends always; sample the middle on long routes.
            if !(is_next || is_goal || (i + 1) % stride == 0) {
                continue;
            }
            let label = format!("{}", i + 1);
            // `camera_offset` MUST be zero here.
            //
            // `build_text` adds this to every glyph's position. Callers on the
            // UI-camera path (tooltips, messages) pass the live camera to
            // cancel that out, because `draw_pooled_ui` binds a UI camera with
            // zoom 1 and camera 0. This batch is drawn by
            // `draw_pooled_no_depth`, which keeps the **world** camera in bind
            // group 0 — the shader already subtracts it. Adding the camera here
            // too double-counted it and threw every digit off-screen by
            // `camera * zoom` pixels.
            //
            // Halo first, digit on top. A 1px ring in four directions keeps
            // the number readable over lit and shadowed terrain without a box.
            //
            // ORDER MATTERS: the batch draws instances in submission order, so
            // the dark halo copies must be pushed BEFORE the digit. Listing the
            // digit first let the four halo copies paint over it and every
            // index came out as a black blob (measured 2042 dark px against
            // 282 light ones).
            for (dx, dy, tint) in LABEL_PASSES {
                text.extend(state.renderer.bit_font.build_text(
                    &label,
                    p[0] + NODE_SIZE / 2.0 + dx,
                    p[1] - NODE_SIZE / 2.0 + dy,
                    PATH_LABEL_SCALE,
                    PATH_DEPTH,
                    tint,
                    NO_CAMERA_OFFSET,
                ));
            }
        }
    }

    PathOverlayInstances {
        stroke,
        nodes,
        text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_colors_are_distinct() {
        // Blocked and normal cost tints must differ so the overlay is useful.
        assert_ne!(COST_BLOCKED_TINT, COST_NORMAL_TINT);
    }

    #[test]
    fn overlay_alpha_is_reasonable() {
        // Alpha should be between 0 and 1, and visibly translucent.
        const { assert!(OVERLAY_ALPHA > 0.1) };
        const { assert!(OVERLAY_ALPHA < 0.8) };
    }

    #[test]
    fn stroke_rasterises_one_quad_per_pixel_and_stops_short_of_the_endpoint() {
        // The stroke follows the project's line idiom (CPU walk, one quad per
        // pixel) rather than spaced blobs. A 30px horizontal segment must
        // therefore produce ~30 quads, and must not emit `b` itself — the next
        // segment (or the node ring) owns that pixel.
        let mut out = Vec::new();
        push_stroke_along([0.0, 0.0], [30.0, 0.0], &mut out);
        assert_eq!(out.len(), 30);
        let last = out.last().unwrap();
        assert!(last.position[0] < 30.0 - LINE_PIXEL_SIZE / 2.0);
        assert_eq!(out[0].tint, PATH_LINE_TINT);
        assert_eq!(out[0].alpha, PATH_LINE_ALPHA);
    }

    #[test]
    fn stroke_pixels_overlap_so_the_line_has_no_seams() {
        // Each quad must be at least as wide as the step, otherwise the
        // per-pixel walk degenerates back into a dotted line.
        const { assert!(LINE_PIXEL_SIZE >= LINE_STEP_PX) };
        // 1.5px measured as a 2px line that was hard to see; 7px fused into a
        // ~9px band. 2.5px sits in between.
        const { assert!(LINE_PIXEL_SIZE >= 2.0 && LINE_PIXEL_SIZE <= 3.0) };
    }

    #[test]
    fn stroke_ignores_a_degenerate_zero_length_segment() {
        // Two coincident waypoints must not loop forever or emit NaN.
        let mut out = Vec::new();
        push_stroke_along([5.0, 5.0], [5.0, 5.0], &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn stroke_covers_a_diagonal_without_gaps() {
        // The regression this replaces: a sparse chain along a diagonal read as
        // a visible staircase. One quad per pixel along the hypotenuse gives
        // as many quads as the segment is long.
        let mut out = Vec::new();
        push_stroke_along([0.0, 0.0], [30.0, 40.0], &mut out);
        let len = (30.0f32 * 30.0 + 40.0 * 40.0).sqrt();
        assert_eq!(out.len(), len.ceil() as usize);
    }

    #[test]
    fn node_square_is_centred_on_the_point() {
        let mut out = Vec::new();
        push_node([100.0, 60.0], PATH_GOAL_TINT, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].position,
            [100.0 - NODE_SIZE / 2.0, 60.0 - NODE_SIZE / 2.0]
        );
        assert_eq!(out[0].tint, PATH_GOAL_TINT);
    }

    #[test]
    fn label_digit_is_submitted_after_every_halo_copy() {
        // Regression guard: submitting the digit first let the four dark halo
        // copies paint over it, so every step index rendered as a black blob
        // (2042 dark px against 282 light px in the screenshot that caught it).
        let digit = LABEL_PASSES
            .iter()
            .position(|(_, _, tint)| *tint == PATH_LABEL_TINT)
            .expect("digit pass present");
        assert_eq!(digit, LABEL_PASSES.len() - 1, "digit must be last");
        for tint in LABEL_PASSES.iter().take(digit).map(|(_, _, tint)| tint) {
            assert_eq!(
                *tint, PATH_LABEL_HALO_TINT,
                "every pass before the digit must be halo"
            );
        }
    }

    #[test]
    fn label_halo_ring_covers_all_four_sides() {
        // A two-sided halo left the top and bottom of a 13px glyph fighting the
        // terrain, so the ring has to keep all four offsets.
        let offsets: Vec<(f32, f32)> = LABEL_PASSES
            .iter()
            .filter(|(_, _, tint)| *tint == PATH_LABEL_HALO_TINT)
            .map(|(dx, dy, _)| (*dx, *dy))
            .collect();
        assert_eq!(offsets.len(), 4);
        for side in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            assert!(offsets.contains(&side), "missing halo side {side:?}");
        }
    }

    #[test]
    fn label_stride_thins_long_routes_but_stays_positive() {
        assert_eq!(path_label_stride(1), 1);
        assert_eq!(path_label_stride(MAX_PATH_LABELS), 1);
        // Past the cap the stride grows so the digit count stays bounded.
        assert_eq!(path_label_stride(MAX_PATH_LABELS * 4), 4);
        // Never zero — a zero stride would divide by zero when sampling.
        assert!(path_label_stride(100_000) >= 1);
    }

    #[test]
    fn label_scale_keeps_game_fnt_digits_legible() {
        // GAME.FNT rows are 16px. Scale 0.5 (~8px digits) measured at only 121
        // label pixels across a whole route and was unreadable; this must stay
        // large enough for the step order to actually be readable.
        const { assert!(PATH_LABEL_SCALE >= 0.7) };
        const { assert!(PATH_LABEL_SCALE <= 1.0) };
    }

    #[test]
    fn labels_must_not_re_add_the_camera_on_the_world_camera_path() {
        // Regression guard. The debug batch keeps the WORLD camera in bind
        // group 0, so the shader already subtracts it from every instance.
        // `build_text` *adds* its `camera_offset` argument, so handing it the
        // live camera double-counts the scroll and pushed every step index off
        // screen (digits never appeared at all). Only the UI-camera path needs
        // the camera added back.
        assert_eq!(NO_CAMERA_OFFSET, [0.0, 0.0]);
    }

    #[test]
    fn line_stays_thin_enough_to_trace_rather_than_bury_the_terrain() {
        // At 7px the old dot chain fused into a ~9px solid band. Keep the
        // stroke and the nodes small so the overlay reads as a route across
        // the map instead of a blue slab on top of it.
        const { assert!(LINE_PIXEL_SIZE <= 3.0) };
        const { assert!(NODE_SIZE <= 9.0) };
    }
}
