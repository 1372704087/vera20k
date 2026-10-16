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
        let mut pass = begin_main_load_pass(encoder, color, depth);
        pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
        while let Some(ObjectExtrasRun::Health {
            background, fills, ..
        }) = layer.extras_runs.get(next)
        {
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
            next += 1;
        }
        // Empty clipped line batches also fence health groups. Account each
        // closed pass even when the line renderer emitted none, using the
        // same bounded submission owner as ordinary object replay.
        drop(pass);
        terrain.note_external_passes(encoder, 1);
    }
}
