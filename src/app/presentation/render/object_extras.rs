//! Replay ordinary rank, health and building lines in Tactical Extras order.
//! Other status families retain their existing owners and open parity bounds.
use super::draw_plan_lowering::{ObjectExtrasRun, ObjectLayerPass};
use crate::app::presentation::sidebar_render::begin_main_load_pass;
use crate::render::batch::BatchRenderer;
use crate::render::selection_overlay::SelectionOverlay;
use crate::render::terrain_draw::TerrainDrawRenderer;

/// Native6F5190/70A990/6F64A0: rank and PIPBRD E00 consume A (493DF0);
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
        // Adjacent status parents share one attachment pass; each keeps its
        // rank->border->fill order. A line parent fences this pass before using Z.
        let start = next;
        while matches!(
            layer.extras_runs.get(next),
            Some(ObjectExtrasRun::Status { .. })
        ) {
            next += 1;
        }
        draw_status_runs(
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
/// sweep. Native70A990/6F64A0/4AED70: rank/border E00 reads A, fills600 bypass A; neither
/// reads or writes Z. The caller retains phase and parent order.
pub(super) fn draw_status_runs(
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
    let mut next = 0;
    while next < runs.len() {
        let end = rank_run_end(runs, next);
        let ObjectExtrasRun::Status {
            rank,
            background,
            fills,
            scale,
            ..
        } = &runs[next]
        else {
            unreachable!("line parents fence status attachment passes");
        };
        if let (Some(overlay), Some(buffer)) = (overlay, buffer) {
            if let (Some(index), Some(shape)) = (rank, overlay.pips()) {
                batch.draw_passthrough_range(
                    &mut pass,
                    shape.texture(),
                    buffer,
                    *index,
                    (end - next) as u32,
                );
            }
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
            if let Some(shape) = overlay.scale_pips() {
                //709A90 uses physical PIPS2 through the same plain600,
                //neutral-A/no-Z store owner as health and garrison fills.
                batch.draw_with_buffer_passthrough_range(
                    &mut pass,
                    shape.texture(),
                    buffer,
                    scale.start,
                    scale.end - scale.start,
                );
            }
        }
        next = end;
    }
    // Account closed passes even when clipped line batches emitted none.
    drop(pass);
    terrain.note_external_passes(encoder, 1);
}

/// Only adjacent rank-only parents with contiguous uploaded instances can
/// share a draw. Health, line, material and phase boundaries remain fences;
/// increasing instance order preserves the retained Display order.
fn rank_run_end(runs: &[ObjectExtrasRun], start: usize) -> usize {
    let ObjectExtrasRun::Status {
        rank: Some(first),
        background: None,
        fills,
        scale,
        ..
    } = &runs[start]
    else {
        return start + 1;
    };
    if !fills.is_empty() || !scale.is_empty() {
        return start + 1;
    }
    let mut end = start + 1;
    while let Some(ObjectExtrasRun::Status {
        rank: Some(index),
        background: None,
        fills,
        scale,
        ..
    }) = runs.get(end)
    {
        if !fills.is_empty() || !scale.is_empty() || *index != *first + (end - start) as u32 {
            break;
        }
        end += 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rank(index: u32) -> ObjectExtrasRun {
        ObjectExtrasRun::Status {
            parent: u64::from(index),
            rank: Some(index),
            background: None,
            fills: index + 1..index + 1,
            scale: index + 1..index + 1,
        }
    }

    #[test]
    fn adjacent_rank_only_parents_share_one_draw_at_the_unit_bound() {
        let runs: Vec<_> = (0..20_000).map(rank).collect();
        assert_eq!(rank_run_end(&runs, 0), runs.len());
    }

    #[test]
    fn rank_batching_preserves_health_line_and_instance_fences() {
        let runs = [
            rank(0),
            rank(1),
            ObjectExtrasRun::Status {
                parent: 2,
                rank: Some(2),
                background: Some(3),
                fills: 4..5,
                scale: 5..5,
            },
            rank(5),
            ObjectExtrasRun::Status {
                parent: 6,
                rank: None,
                background: None,
                fills: 6..7,
                scale: 7..7,
            },
            rank(7),
            ObjectExtrasRun::Lines {
                parent: 8,
                lines: 0..0,
                batch: Default::default(),
            },
            rank(8),
            rank(10),
            ObjectExtrasRun::Status {
                parent: 11,
                rank: Some(11),
                background: None,
                fills: 12..12,
                scale: 12..13,
            },
            rank(13),
        ];
        for (start, end) in [
            (0, 2),
            (2, 3),
            (3, 4),
            (5, 6),
            (7, 8),
            (8, 9),
            (9, 10),
            (10, 11),
        ] {
            assert_eq!(rank_run_end(&runs, start), end, "run {start}");
        }
    }
}
