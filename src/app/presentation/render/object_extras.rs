//! Replay the ordinary health and building-line portion of Tactical6D9789.
//! Other status families retain their existing owners and open parity bounds.
use super::draw_plan_lowering::{ObjectExtrasRun, ObjectLayerPass};
use crate::app::presentation::sidebar_render::begin_main_load_pass;
use crate::render::batch::BatchRenderer;
use crate::render::selection_overlay::SelectionOverlay;
use crate::render::terrain_draw::TerrainDrawRenderer;

/// Native6F64A0/4AED70: per-parent PIPBRD E00 consumes A (493DF0);
/// PIPS600 bypasses A (4912B0). Neither shape reads or writes Z. Commands
/// and ranges belong only to this frame's retained Display traversal.
pub(super) fn draw_object_extras(
    encoder: &mut wgpu::CommandEncoder,
    color: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    terrain: &mut TerrainDrawRenderer,
    clip: [u32; 4],
    batch: &BatchRenderer,
    overlay: Option<&SelectionOverlay>,
    buffer: Option<&wgpu::Buffer>,
    layer: &ObjectLayerPass,
) {
    let mut next = 0;
    while next < layer.extras_runs.len() {
        if let ObjectExtrasRun::Lines { batch, .. } = &layer.extras_runs[next] {
            terrain.draw_surface_lines(encoder, color, *batch);
            next += 1;
            continue;
        }
        // Adjacent health parents share one attachment pass; each keeps its
        // border->fill order. A line parent fences this pass before using Z.
        let start = next;
        while matches!(
            layer.extras_runs.get(next),
            Some(ObjectExtrasRun::Health { .. })
        ) {
            next += 1;
        }
        draw_health_runs(
            encoder,
            color,
            depth,
            terrain,
            clip,
            batch,
            overlay,
            buffer,
            &layer.extras_runs[start..next],
        );
    }
}

/// Shared material owner for initial Building Extras and the late Techno
/// sweep. Native6F64A0/4AED70: border E00 reads A, fills600 bypass A; neither
/// reads or writes Z. The caller retains phase and parent order.
pub(super) fn draw_health_runs(
    encoder: &mut wgpu::CommandEncoder,
    color: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    terrain: &mut TerrainDrawRenderer,
    clip: [u32; 4],
    batch: &BatchRenderer,
    overlay: Option<&SelectionOverlay>,
    buffer: Option<&wgpu::Buffer>,
    runs: &[ObjectExtrasRun],
) {
    let mut pass = begin_main_load_pass(encoder, color, depth);
    pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
    for run in runs {
        let ObjectExtrasRun::Health {
            background, fills, ..
        } = run
        else {
            unreachable!("line parents fence health attachment passes");
        };
        if let (Some(overlay), Some(buffer)) = (overlay, buffer) {
            if let (Some(index), Some(shape)) = (background, overlay.pipbrd()) {
                batch.draw_passthrough_range(&mut pass, shape.texture(), buffer, *index, 1);
            }
            if let Some(shape) = overlay.pips() {
                batch.draw_with_buffer_passthrough_range(
                    &mut pass,
                    shape.texture(),
                    buffer,
                    fills.start,
                    fills.end - fills.start,
                );
            }
        }
    }
    // Account closed passes even when clipped line batches emitted none.
    drop(pass);
    terrain.note_external_passes(encoder, 1);
}
