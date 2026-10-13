//! Retained `VoxelAnimClass` rendering: `VoxelAnimClass::DrawIfVisible @
//! 0x00749B20` -> `ObjectClass::DrawIfVisible @ 0x005F4B10` ->
//! `VoxelAnimClass::DrawIt @ 0x00749B70`.
//!
//! The simulation owns each piece's coordinate, tumble, lifetime and Air-layer
//! Display order. This read-only adapter resolves the floor under a piece,
//! rasterizes its type's model at the bounce drawing matrix on the pose page,
//! and emits the ground shadow and then the body as one Air-layer object.
//!
//! RESIDUAL: DrawIt arms VERA does not draw.
//! - The FogOfWar gate (`0x00749B70..0x00749BB6`): with the scenario's
//!   FogOfWar flag set, a piece over a fogged cell draws nothing. Trigger: a
//!   scenario with FogOfWar on, which skirmish never sets. Effect: VERA still
//!   draws such a piece. Downstream: presentation only.
//! - The unowned `IsTiberium=` Convert (`[0x0087F6BC]`, `0x00749E5A..
//!   0x00749E70`): VERA draws such a piece nothing. Every piece VERA spawns is
//!   death debris, which carries the dying object's house, and every stock
//!   `DebrisTypes=` line names `[TIRE]`, which is not `IsTiberium=`.
//! - A `VoxelIndex=` other than zero, a model with more than one section, and
//!   the `Share*Data=` types (`[SONICTURRET]`, `[4TNKTURRET]`, `[CRYSTAL01]`)
//!   whose model is another type's: VERA draws them nothing. Stock death
//!   debris is `[TIRE]` alone, one section and no `VoxelIndex=`.
//! - ObjectClass::DrawIfVisible's redraw bookkeeping (`+0x80` and the
//!   NeedsRedraw gates), which no VERA object builder models.
//!
//! ## Dependency rules
//! - Part of the app layer — may depend on everything.

use glam::IVec3;

use super::helpers::projection_admitted;
use crate::app::AppState;
use crate::app::presentation::render::draw_plan_lowering::{
    NativeDisplayOrder, ObjectPieceInstance, ObjectTexture, PlannedObjectInstance,
};
use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::render::batch::{DepthAxis, SpriteInstance};
use crate::render::draw_state::{DrawState, FX_SHADOW};
use crate::render::native_z::{ZGradient, pack_z_gradient};
use crate::render::palette_light::PaletteLight;
use crate::render::tactical_draw_plan::{RenderZPolicy, SpriteEncoding};
use crate::render::unit_atlas::UnitSpriteEntry;
use crate::rules::house_colors::{HouseColorIndex, NO_REMAP};
use crate::rules::voxel_anim_type::VoxelAnimType;
use crate::sim::voxel_anim::VoxelAnimObject;
use crate::util::lepton::{BRIDGE_DECK_HEIGHT_LEPTONS, absolute_leptons_to_screen};
use crate::util::native_x87::adjust_for_z_standard;

/// Both blits walk the voxel blitter's vertical gradient: DrawIt blits through
/// `0x004AF2A0`, as TechnoClass's voxel body and uncached shadow do.
const VOXEL_Z_GRADIENT: u32 = pack_z_gradient(ZGradient::Vertical, false);

#[derive(Debug, Clone, Copy, PartialEq)]
struct PieceGeometry {
    point: [f32; 2],
    z_adjust: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DebrisGeometry {
    body: PieceGeometry,
    shadow: PieceGeometry,
}

/// DrawIt's draw points and Z arguments; raw Z is in leptons. `ground_z` is
/// Cell578080's floor height under the piece and `bridge` the Cell's
/// `+0x140 & 0x100` flag. A piece at or above that bridge's deck shadows one
/// lepton above the deck (`0x00749C0E..0x00749C60`). The shadow's Z argument
/// is `-4 - AdjustForZ(surface)` (`0x00749D2A..0x00749D36`) and the body's
/// `-2 - AdjustForZ(GetCoordZ)` (`0x00749EA4..0x00749EB6`); the body point is
/// ObjectClass::DrawIfVisible's projection of the Location.
fn geometry(location: IVec3, ground_z: i32, bridge: bool) -> DebrisGeometry {
    let deck = ground_z.wrapping_add(BRIDGE_DECK_HEIGHT_LEPTONS);
    let (shadow_z, surface_z) = if bridge && location.z >= deck {
        (deck.wrapping_add(1), deck)
    } else {
        (ground_z, ground_z)
    };
    let (x, y) = absolute_leptons_to_screen(location.x, location.y, location.z);
    let (shadow_x, shadow_y) = absolute_leptons_to_screen(location.x, location.y, shadow_z);
    DebrisGeometry {
        body: PieceGeometry {
            point: [x, y],
            z_adjust: (-2i32).wrapping_sub(adjust_for_z_standard(location.z)),
        },
        shadow: PieceGeometry {
            point: [shadow_x, shadow_y],
            z_adjust: (-4i32).wrapping_sub(adjust_for_z_standard(surface_z)),
        },
    }
}

/// DrawIt's body Convert and intensity (`0x00749E05..0x00749E78`). An owned
/// piece takes its house's ColorScheme Convert (`+0x30C`, the one a Techno's
/// voxel body selects at `0x00705D70`) at its cell's intensity, which
/// `owned_light` supplies; an unowned one takes UNITx_PAL's at 1000. None for
/// the unowned `IsTiberium=` arm (module residual).
fn body_palette(
    owner_scheme: Option<HouseColorIndex>,
    is_tiberium: bool,
    owned_light: impl FnOnce() -> PaletteLight,
) -> Option<(u32, PaletteLight)> {
    match owner_scheme {
        Some(scheme) => Some((
            super::units::house_color_to_remap_row(scheme),
            owned_light(),
        )),
        None if is_tiberium => None,
        None => Some((0, PaletteLight::plain(53, 1000))),
    }
}

/// What every piece of one frame shares.
struct DebrisView<'a> {
    terrain: &'a ResolvedTerrainGrid,
    camera: [f32; 2],
    viewport: [f32; 2],
    axis: DepthAxis,
}

pub(crate) fn build_voxel_anim_instances(
    state: &AppState,
    objects: &mut Vec<PlannedObjectInstance>,
    order: &NativeDisplayOrder,
) {
    let (Some(rt), Some(assets)) = (
        state.match_state.sim_runtime.as_ref(),
        state.process_assets.manager(),
    ) else {
        return;
    };
    let sim = &rt.simulation;
    let Some(terrain) = sim.resolved_terrain.as_ref() else {
        return;
    };
    if sim.voxel_anims().is_empty() {
        return;
    }
    let rules = &rt.resources.rules;
    let input = &state.match_state.input;
    let (_, _, width, height) = crate::app::input::camera::tactical_viewport_px(state);
    let view = DebrisView {
        terrain,
        camera: [input.camera_x, input.camera_y],
        viewport: [
            width as f32 / input.zoom_level,
            height as f32 / input.zoom_level,
        ],
        axis: super::helpers::depth_axis(state),
    };
    let presentation = &state.match_state.match_presentation;
    let mut cache = state.renderer.vxl_pose_frame_cache.borrow_mut();
    for (&id, piece) in sim.voxel_anims().iter() {
        let kind = rules.voxel_anim_type(piece.type_id);
        let location = piece.world_coord();
        let cell = [location.x, location.y]
            .map(|lepton| lepton.div_euclid(256).clamp(0, i32::from(u16::MAX)) as u16);
        let owner_scheme = piece.owner_house.map(|owner| {
            presentation
                .house_color_map
                .get(sim.interner.resolve(owner))
                .copied()
                .unwrap_or(NO_REMAP)
        });
        let Some(palette) = body_palette(owner_scheme, kind.is_tiberium, || {
            crate::app::presentation::lighting::body_palette_light(
                presentation.lighting.grid(),
                &sim.session.lighting,
                (cell[0], cell[1]),
                0,
            )
        }) else {
            continue;
        };
        if let Some(object) = voxel_anim_draw_instance(
            id,
            piece,
            kind,
            palette,
            &view,
            order,
            |image, section, matrix| cache.render_tumble(assets, image, section, matrix),
        ) {
            objects.push(object);
        }
    }
}

/// One piece's draw. Display membership is admission: storage alone never
/// draws a piece. `render` rasterizes the model `image` names at the drawing
/// matrix and places its body and shadow on the pose page.
fn voxel_anim_draw_instance(
    id: u64,
    piece: &VoxelAnimObject,
    kind: &VoxelAnimType,
    (remap_row, palette_light): (u32, PaletteLight),
    view: &DebrisView<'_>,
    order: &NativeDisplayOrder,
    render: impl FnOnce(&str, i32, glam::Mat4) -> Option<(UnitSpriteEntry, Option<UnitSpriteEntry>)>,
) -> Option<PlannedObjectInstance> {
    let parent = order.object_draw(id, SpriteEncoding::Voxel)?;
    let location = piece.world_coord();
    let (ground_z, bridge) = super::foot_depth::floor_probe(view.terrain, location.x, location.y);
    let geometry = geometry(location, ground_z, bridge);
    // Object6D2140's padded projection admission precedes DrawIt.
    if !projection_admitted(geometry.body.point, view.camera, view.viewport) {
        return None;
    }
    let matrix = piece
        .bounce
        .drawing_matrix()
        .ok()?
        .map(|bits| f32::from_bits(bits.bits()));
    let (body, shadow) = render(
        &kind.image,
        kind.voxel_index,
        crate::render::vxl_raster::hva_to_mat4(&matrix, 1.0),
    )?;
    let depth = crate::render::native_z::depth_for_row(
        geometry.body.point[1] + adjust_for_z_standard(location.z) as f32,
        view.axis.origin_y,
        view.axis.world_height,
    );
    let mut pieces = Vec::with_capacity(2);
    if let Some(shadow) = shadow {
        // `0x2001`: the shadow blitter darkens what lies beneath.
        let draw_state = DrawState {
            fx_flags: FX_SHADOW,
            ..DrawState::default()
        };
        pieces.push(pose_piece(
            shadow,
            geometry.shadow,
            depth,
            draw_state,
            PaletteLight::default(),
        ));
    }
    // `0x2800`, or `0x2804` for a `Translucent=` type (`0x00749DEB..0x00749E00`).
    let draw_state = if kind.translucent {
        DrawState::translucent(remap_row)
    } else {
        DrawState {
            remap_row,
            ..DrawState::default()
        }
    };
    pieces.push(pose_piece(
        body,
        geometry.body,
        depth,
        draw_state,
        palette_light,
    ));
    Some(PlannedObjectInstance::object(parent, pieces))
}

fn pose_piece(
    entry: UnitSpriteEntry,
    at: PieceGeometry,
    depth: f32,
    draw_state: DrawState,
    palette_light: PaletteLight,
) -> ObjectPieceInstance {
    ObjectPieceInstance {
        target: ObjectTexture::UnitPose,
        render_z: RenderZPolicy::ReadOnly,
        instance: SpriteInstance {
            position: [at.point[0] + entry.offset_x, at.point[1] + entry.offset_y],
            size: entry.pixel_size,
            uv_origin: entry.uv_origin,
            uv_size: entry.uv_size,
            depth,
            tint: crate::map::lighting::DEFAULT_TINT,
            palette_light,
            alpha: 1.0,
            draw_state,
            z_adjust: at.z_adjust as f32,
            z_gradient: VOXEL_Z_GRADIENT,
            ..Default::default()
        },
    }
}

#[cfg(test)]
#[path = "voxel_anims_tests.rs"]
mod tests;
