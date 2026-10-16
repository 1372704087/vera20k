//! UI overlay instance builders — status bars, software cursor.
//!
//! Split from the presentation render path. Part of the app layer — may depend on everything.

use crate::app::AppState;
use crate::app::input::commands::preferred_local_owner_name;
use crate::app::input::cursor::{
    current_cursor_feedback_kind, cursor_id_for_feedback, software_cursor_frame_for,
};
use crate::app::presentation::instances::in_view;
use crate::app::types::{CursorId, HoverTargetKind, SoftwareCursorSequence};
use crate::map::entities::EntityCategory;
use crate::map::houses::HouseColorMap;
use crate::render::batch::{BatchTexture, SpriteInstance};
use crate::render::bit_font::BitFont;
use crate::rules::house_colors::{HouseColorRamps, NO_REMAP};
use crate::rules::object_type::ObjectType;
use crate::rules::ruleset::RuleSet;
use crate::sim::components::Position;
use std::collections::HashMap;
/// Authentic pip counts for unit health bars.
const UNIT_PIPS_VEHICLE: u32 = 17; // vehicles + aircraft
const UNIT_PIPS_INFANTRY: u32 = 8;

/// Horizontal pixel spacing between unit health pips (each pip drawn 2px apart).
const UNIT_PIP_STEP_X: f32 = 2.0;

/// The object under the cursor whose health bar the original draws on hover.
///
/// The per-object overlay pass calls the health-bar slot from exactly two arms:
/// the object is selected, or the object carries the cursor's hover flag and is
/// *not* selected. The hover flag is set from the cursor's resolved action
/// target, so a shrouded enemy — which is never an action target — is excluded.
fn health_bar_hover_target(
    state: &AppState,
    local_owner: Option<&str>,
) -> Option<(u64, HoverTargetKind)> {
    let sim = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)?;
    let local_owner = local_owner?;
    let (world_x, world_y) = crate::app::match_runtime::sim_tick::screen_point_to_world(
        state,
        state.match_state.input.cursor_x,
        state.match_state.input.cursor_y,
    );
    let hover = crate::app::input::entity_pick::hover_target_at_point(
        sim,
        world_x,
        world_y,
        local_owner,
        state.match_state.sandbox_full_visibility,
        state.rules(),
        state.height_map(),
        crate::app::match_runtime::sim_tick::tactical_bridge_cells(sim),
    )?;
    match hover.kind {
        HoverTargetKind::FriendlyStructure
        | HoverTargetKind::EnemyStructure
        | HoverTargetKind::FriendlyUnit
        | HoverTargetKind::EnemyUnit => Some((hover.stable_id, hover.kind)),
        HoverTargetKind::HiddenEnemy => None,
    }
}

/// The existing resolved hover target for either Techno health branch.
pub(crate) fn health_hover_target(state: &AppState, local_owner: Option<&str>) -> Option<u64> {
    health_bar_hover_target(state, local_owner).map(|(id, _)| id)
}

/// The hovered *structure*, for the building pip pass.
fn building_health_hover_target(state: &AppState, local_owner: Option<&str>) -> Option<u64> {
    health_bar_hover_target(state, local_owner).and_then(|(id, kind)| {
        matches!(
            kind,
            HoverTargetKind::FriendlyStructure | HoverTargetKind::EnemyStructure
        )
        .then_some(id)
    })
}

/// The hovered *unit or infantry*, for the non-building health-bar pass.
pub(crate) fn unit_health_hover_target(state: &AppState, local_owner: Option<&str>) -> Option<u64> {
    health_bar_hover_target(state, local_owner).and_then(|(id, kind)| {
        matches!(
            kind,
            HoverTargetKind::FriendlyUnit | HoverTargetKind::EnemyUnit
        )
        .then_some(id)
    })
}

/// Whether a status-producing object draws its bracket background and pips.
///
/// Two separate rules in the original, and damage is not part of either. The
/// health bar itself is drawn when the object is selected *or* hovered, while
/// the PIPBRD bracket behind it is drawn only when the object is selected — so
/// a hovered-but-unselected unit shows a bare pip strip with no bracket, and an
/// unselected, unhovered unit shows nothing however badly damaged it is.
///
/// Returns `(draw_bracket_background, draw_health_pips)`.
pub(crate) fn status_visibility(selected: bool, hovered: bool) -> (bool, bool) {
    (selected, selected || hovered)
}

/// Health-bar admission shared by building pips, mobile pips and their extras.
///
/// Steam gamemd SHA3e81a617: Tactical6D907E/6D9111 marks a Display member for
/// Extras before drawing its body; 6D95D2..6D9789 dispatches that later sweep.
/// Techno6F5E27/6F5E37..6F5E87 then admits selected / ordinary hovered objects
/// to health-bar slot+44C (6F64A0). Neither arm retests the raw anchor's shroud
/// or discovery+41B. Hover already passed ProcessClickCoords' projected query.
/// Reuse the body admission owner for lifecycle/cloak instead of adding another
/// fog interpretation here. Native dispatch controls: health_hover.json.
fn health_bar_drawable(
    sim: &crate::sim::world::Simulation,
    entity: &crate::sim::game_entity::GameEntity,
    hovered_id: Option<u64>,
    local_owner: Option<&str>,
    rules: Option<&RuleSet>,
) -> bool {
    let (_, draw_pips) = status_visibility(
        entity.selection.is_selected(),
        hovered_id == Some(entity.stable_id()),
    );
    if !draw_pips {
        return false;
    }
    admitted_techno_observer(sim, entity, local_owner, rules).is_some()
}

/// Retained Display membership and the shared tactical draw admission.
/// Return the already derived observer facts for native Extras queries.
fn admitted_techno_observer(
    sim: &crate::sim::world::Simulation,
    entity: &crate::sim::game_entity::GameEntity,
    local_owner: Option<&str>,
    rules: Option<&RuleSet>,
) -> Option<crate::render::draw_state::ObserverDrawContext> {
    sim.display_layers().layer_of(entity.stable_id())?;
    let local_owner_id = local_owner.and_then(|owner| sim.interner.get(owner));
    let observer = crate::app::presentation::instances::observer_draw_context(
        sim,
        entity,
        local_owner,
        local_owner_id,
        rules,
    );
    crate::app::presentation::instances::tactical_entity_admission(
        crate::app::presentation::instances::TacticalEntityPurpose::Drawing,
        entity,
        sim.interner.resolve(entity.owner()),
        local_owner,
        local_owner_id,
        &sim.fog,
        false,
        sim.session.binary_frame,
        0,
        observer,
    )?;
    Some(observer)
}

/// Native `DrawHealthBar @0x006F64A0` guards its class `vt+0x450` pip/status
/// tail after the health strip. It admits CurrentPlayer's allies directly; the
/// other arm tests the object's current-player display bit at Techno+0x210.
/// That bit has no Rust owner yet, so this covers the common current-owned /-
/// allied path and leaves the non-allied display-bit arm as a bounded residual.
/// Keep this gate separate from health-strip admission.
fn entity_status_pips_drawable(
    sim: &crate::sim::world::Simulation,
    entity: &crate::sim::game_entity::GameEntity,
    hovered_entity_id: Option<u64>,
    current_house_id: Option<crate::sim::intern::InternedId>,
    rules: Option<&RuleSet>,
) -> bool {
    let Some(current_house_id) = current_house_id else {
        return false;
    };
    // House4F9A50 reads the ASKING owner's ally bits (6F6A58..8C).
    health_bar_drawable(
        sim,
        entity,
        hovered_entity_id,
        Some(sim.interner.resolve(current_house_id)),
        rules,
    ) && crate::map::houses::is_allied_with(
        &sim.house_alliances,
        sim.interner.resolve(entity.owner()),
        sim.interner.resolve(current_house_id),
    )
}

/// Project the input-owned control-group membership onto the live objects that
/// reach the status-pip draw. The membership remains owned by
/// `MatchInputState`; this transient index only makes each frame's lookup
/// linear in the retained Display list plus the ten group lists.
///
/// Native `DrawHealthBar @0x006F64A0` calls the class pip-scale slot
/// (`vtable +0x450`), whose tail reads Techno `+0x214` at
/// `DrawPipScalePips @0x00709A90`. Native `+0x214` is a zero-based slot; Rust's
/// input-owned control groups are indexed by the registered hotkey digit
/// (`TeamCreate_1` stores slot 1 and `TeamCreate_10` stores slot 0), so the
/// displayed digit is the Rust slot itself. The native caller's selected/hovered
/// admission is shared with the health strip for units, infantry, and buildings.
/// The relationship gate is pip-only; mobile labels reuse retained body
/// admission. Building labels retain their separate existing plain visibility
/// policy. Stale stable IDs and concealed bodies naturally produce no label.
pub(crate) fn control_group_status_labels(
    sim: &crate::sim::world::Simulation,
    control_groups: &[Vec<u64>],
    hovered_entity_id: Option<u64>,
    current_house_id: Option<crate::sim::intern::InternedId>,
    ignore_visibility: bool,
    rules: Option<&RuleSet>,
) -> Vec<(u64, char)> {
    let mut slot_by_id = HashMap::new();
    for (slot, members) in control_groups.iter().take(10).enumerate() {
        for &stable_id in members {
            // Match the single native Techno field if malformed/restored input
            // ever contains duplicate membership: the last slot is the one
            // whose live assignment most recently wins.
            slot_by_id.insert(stable_id, slot);
        }
    }

    crate::app::presentation::instances::tactical_entity_encounter_order(sim)
        .into_iter()
        .filter_map(|stable_id| {
            let entity = sim.entities().get(stable_id)?;
            if entity.sinking.is_active()
                || entity.passenger_role.is_inside_transport()
                || !entity_status_pips_drawable(
                    sim,
                    entity,
                    hovered_entity_id,
                    current_house_id,
                    rules,
                )
                || (entity.category() == EntityCategory::Structure
                    && !status_entity_visible_plain(
                        current_house_id,
                        &sim.fog,
                        &entity.position,
                        entity.owner(),
                        ignore_visibility,
                    ))
            {
                return None;
            }
            let slot = *slot_by_id.get(&stable_id)?;
            let digit = char::from(b'0' + slot as u8);
            Some((stable_id, digit))
        })
        .collect()
}

/// Paired background and glyph instances for the native control-group label.
/// The background is emitted in its own earlier layer so the glyph renderer
/// remains on the existing GAME.FNT atlas/pass.
pub(crate) struct ControlGroupStatusOverlayInstances {
    pub background: Vec<SpriteInstance>,
    pub outline: Vec<SpriteInstance>,
    pub text: Vec<SpriteInstance>,
}

/// Native `DrawHealthBar @0x006F64A0` adds the projected building-foundation
/// offset before calling the shared pip/status slot at `+0x450`.
/// `BuildingTypeClass::Dimension2 @0x00464AF0` supplies the foundation width
/// and height in 256-lepton cells; the caller halves each with signed
/// truncation, negates X, and uses the shared native coordinate projection.
fn building_control_group_status_anchor_offset(foundation: &str) -> (i32, i32) {
    let [width_leptons, height_leptons] =
        crate::rules::foundation::foundation_dimensions_leptons(foundation);
    let half_width = width_leptons / 2;
    let half_height = height_leptons / 2;
    crate::util::lepton::project_absolute_lepton_xy(
        -(width_leptons - half_width),
        height_leptons - half_height,
    )
}

/// Emit the selected/hovered unit's control-group digit with the shared GAME.FNT
/// atlas. Membership is read from the input owner; the per-frame output is only
/// draw data.
///
/// For the regular non-building call at native `DrawPipScalePips @ 0x00709A90`,
/// `DrawHealthBar @ 0x006F64A0` passes the screen point after `x-10,y+10` at
/// `0x006F6AAB`. The group tail then uses `x-4,y-0x27` only for Infantry
/// (`WhatAmI()==0xF`), and `x-4,y-0x24` for other classes. Relative to the
/// `0x006F6AB0` caller's input point, the final text origins are Unit
/// `(-14,-0x1A)` and Infantry `(-14,-0x1D)`. Native builds that point from the type vtable `+0x7C`
/// coordinate through `0x0041C230` and `0x006D1F10`; Rust uses the shared
/// tactical entity screen-position owner. Buildings enter the same slot with
/// the plain screen point plus the projected half-foundation offset from
/// `Dimension2`; the non-Infantry tail then applies `(-4,-0x24)`. The
/// selection-bracket delta is not part of the native group-number tail.
pub(crate) fn build_control_group_status_render_instances(
    sim: &crate::sim::world::Simulation,
    control_groups: &[Vec<u64>],
    hovered_entity_id: Option<u64>,
    current_house_id: Option<crate::sim::intern::InternedId>,
    ignore_visibility: bool,
    house_color_map: &HouseColorMap,
    house_color_ramps: &HouseColorRamps,
    font: &BitFont,
    camera: [f32; 2],
    sw: f32,
    sh: f32,
    tactical_clip: [i32; 4],
    rules: Option<&RuleSet>,
) -> ControlGroupStatusOverlayInstances {
    let labels = control_group_status_labels(
        sim,
        control_groups,
        hovered_entity_id,
        current_house_id,
        ignore_visibility,
        rules,
    );
    let mut background = Vec::with_capacity(labels.len());
    let mut outline = Vec::with_capacity(labels.len() * 4);
    let mut text_instances = Vec::with_capacity(labels.len());
    for (stable_id, digit) in labels {
        let Some(entity) = sim.entities().get(stable_id) else {
            continue;
        };
        let (mut screen_x, mut screen_y) =
            crate::app::presentation::instances::interpolated_screen_position_entity(entity);
        let building = entity.category() == EntityCategory::Structure;
        if building {
            let (foundation_x, foundation_y) =
                building_control_group_status_anchor_offset(entity.type_facts.foundation());
            screen_x += foundation_x as f32;
            screen_y += foundation_y as f32;
        }
        // `DrawHealthBar` pre-adjusts non-buildings by (-10,+10); the shared
        // tail subtracts X=4 and only gives Infantry its additional -3 Y.
        let (caller_offset_x, caller_offset_y) = if building { (0.0, 0.0) } else { (-10.0, 10.0) };
        let tail_offset_y = if entity.category() == EntityCategory::Infantry {
            -0x27 as f32
        } else {
            -0x24 as f32
        };
        let text = digit.to_string();
        let (x, y) = (
            screen_x + caller_offset_x - 4.0,
            screen_y + caller_offset_y + tail_offset_y,
        );
        let width = font.text_width(&text) as f32;
        let height = font.glyph_height();
        if !in_view(x, y, width, height, camera[0], camera[1], sw, sh, 48.0) {
            continue;
        }
        let owner = sim.interner.resolve(entity.owner());
        let color = house_color_map.get(owner).copied().unwrap_or(NO_REMAP);
        // Native `DrawPipScalePips @0x00709A90` reads House+0x56F9 directly.
        let tint = crate::render::palette_light::house_color_rgb(house_color_ramps, color)
            .map(|channel| f32::from(channel) / 255.0);
        // The original measures the text rectangle, clips it to the tactical
        // surface, then expands the clipped result one pixel to the left for
        // its opaque black fill and House-color outline. All rectangles here
        // use the shared xywh owner; the GPU scissor remains the final clip.
        let measure_rect = [
            (x - camera[0]).round() as i32 - 2,
            (y - camera[1]).round() as i32 + 2,
            font.text_width(&text) as i32 + 4,
            13,
        ];
        let clipped = crate::util::rect::clip_rect(measure_rect, tactical_clip);
        if clipped[2] > 0 && clipped[3] > 0 {
            let rect_x = clipped[0] - 1;
            let rect_y = clipped[1];
            let rect_width = clipped[2] + 1;
            let rect_height = clipped[3];
            let rect_x = rect_x as f32 + camera[0];
            let rect_y = rect_y as f32 + camera[1];
            let rect_width = rect_width as f32;
            let rect_height = rect_height as f32;
            background.push(SpriteInstance {
                position: [rect_x, rect_y],
                size: [rect_width, rect_height],
                uv_origin: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                tint: [0.0, 0.0, 0.0],
                alpha: 1.0,
                ..Default::default()
            });
            // Native 0x7BADC0 draws inclusive endpoints (origin+size-1).
            // Avoid zero-area quads when clipping leaves a one-pixel edge.
            let mut edges = vec![
                ([rect_x, rect_y], [rect_width, 1.0]),
                ([rect_x, rect_y + rect_height - 1.0], [rect_width, 1.0]),
            ];
            if rect_height > 2.0 {
                edges.extend([
                    ([rect_x, rect_y + 1.0], [1.0, rect_height - 2.0]),
                    (
                        [rect_x + rect_width - 1.0, rect_y + 1.0],
                        [1.0, rect_height - 2.0],
                    ),
                ]);
            }
            outline.extend(edges.into_iter().map(|(position, size)| SpriteInstance {
                position,
                size,
                uv_origin: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                tint,
                alpha: 1.0,
                ..Default::default()
            }));
        }
        // These are world-pixel coordinates; the tactical surface camera
        // applies the camera offset at draw time, like the neighboring pips.
        text_instances.extend(font.build_text(&text, x, y, 1.0, 0.0, tint, [0.0, 0.0]));
    }
    ControlGroupStatusOverlayInstances {
        background,
        outline,
        text: text_instances,
    }
}

/// AppState adapter for the production tactical UI build.
pub(crate) fn build_control_group_status_instances(
    state: &AppState,
    sw: f32,
    sh: f32,
) -> ControlGroupStatusOverlayInstances {
    let (Some(sim), Some(rules)) = (
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|runtime| &runtime.simulation),
        state.rules(),
    ) else {
        return ControlGroupStatusOverlayInstances {
            background: Vec::new(),
            outline: Vec::new(),
            text: Vec::new(),
        };
    };
    let local_owner = preferred_local_owner_name(state);
    let local_owner_id = local_owner
        .as_deref()
        .and_then(|name| sim.interner.get(name));
    let hovered_entity_id =
        health_bar_hover_target(state, local_owner.as_deref()).map(|(stable_id, _)| stable_id);
    let (tactical_width, tactical_height) =
        crate::app::input::camera::tactical_viewport_size(state);
    let zoom = state.match_state.input.zoom_level;
    let tactical_clip = [
        0,
        0,
        (tactical_width as f32 / zoom).round() as i32,
        (tactical_height as f32 / zoom).round() as i32,
    ];
    build_control_group_status_render_instances(
        sim,
        &state.match_state.input.control_groups,
        hovered_entity_id,
        local_owner_id,
        state.match_state.sandbox_full_visibility,
        &state.match_state.match_presentation.house_color_map,
        &rules.house_color_ramps,
        &state.renderer.bit_font,
        [
            state.match_state.input.camera_x,
            state.match_state.input.camera_y,
        ],
        sw,
        sh,
        tactical_clip,
        Some(rules),
    )
}

/// Which PIPS.SHP frame a self-healing non-building's health strip carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelfHealPip {
    /// Frame `0x0D`, the red cross: an Infantry, or a Unit whose type is
    /// `Organic`.
    Organic,
    /// Frame `0x14`, the white wrench: a Unit whose type is not `Organic`.
    Units,
}

impl SelfHealPip {
    /// The PIPS.SHP frame index this arm draws (`ebx` at `0x0070A573` /
    /// `0x0070A5DF`).
    pub(crate) const fn frame(self) -> usize {
        match self {
            Self::Organic => 0x0D,
            Self::Units => 0x14,
        }
    }

    /// The draw-point offset from the object's screen position plus bracket
    /// delta — the `[esp+0x50]` / `[esp+0x68]` base the health strip uses,
    /// which `(+0x26, -0x20)` and `(+0x13, -0x23)` are added to at
    /// `0x0070A5F1` and `0x0070A60A`.
    pub(crate) const fn offset(self) -> (f32, f32) {
        match self {
            Self::Organic => (0x13 as f32, -0x23 as f32),
            Self::Units => (0x26 as f32, -0x20 as f32),
        }
    }
}

/// The self-heal status pip an object draws, or `None`.
///
/// Native `DrawPipScalePips` (`0x0070A50C..0x0070A6BB`): an Infantry
/// (`WhatAmI()==0xF`) takes the organic arm directly, and a Unit
/// (`WhatAmI()==1`) whose type byte `+0xD97` (`Organic`) is set falls through
/// into the same arm; a Unit whose type is not organic takes the units arm;
/// every other class passes both tests and draws nothing. Each arm first
/// requires its owner house's counter — `HasInfSelfHeal @ 0x0050D9C0` on
/// `House+0x164`, `HasUnitSelfHeal @ 0x0050D9D0` on `House+0x168`.
pub(crate) fn self_heal_pip(
    category: EntityCategory,
    organic: bool,
    self_heal_infantry: i32,
    self_heal_units: i32,
) -> Option<SelfHealPip> {
    let (arm, count) = match category {
        EntityCategory::Infantry => (SelfHealPip::Organic, self_heal_infantry),
        EntityCategory::Unit if organic => (SelfHealPip::Organic, self_heal_infantry),
        EntityCategory::Unit => (SelfHealPip::Units, self_heal_units),
        EntityCategory::Structure | EntityCategory::Aircraft => return None,
    };
    (count > 0).then_some(arm)
}

// DEFERRED members of this row's overlay set, recorded with what a follow-up
// needs. None of them requires new texture infrastructure — pips.shp and
// pips2.shp are both already loaded, and `DrawPipScalePips` swaps the shape
// handle from `DAT_00AC147C` (PIPS.SHP) to `DAT_00AC1480` (PIPS2.SHP) at
// 0x00709AEF for every non-building.
//
// * **`PipScale=Passengers` and `PipScale=Ammo`.** `build_scale_pip_instances`
//   handles Tiberium cargo and MindControl count/overload feedback; these
//   separate Ammo/Passengers arms remain unported. 38
//   stock rulesmd.ini sections set one of the two (34 Passengers, 4 Ammo).
//   Their separate ammo/passenger display branches remain required work;
//   MindControl is native case5 (manager+2BC), handled by the shared strip.
//   Trigger:
//   selecting a loaded IFV, Flak Track, Battle Fortress, Nighthawk or any
//   transport. Player effect: no passenger or ammo readout at all. Frequency:
//   common — transports are ordinary play. Downstream risk: none.
//
// * **Spawn pips**, from `TypeClass+0xD5C` via
//   `SpawnManagerClass__CountDockedSpawns`. Trigger: a selected Kirov/carrier
//   with docked spawns. Player effect: spawned aircraft count is absent.
//   Frequency: rare. Downstream risk: none. The group-number tail is projected
//   through the live input membership in `build_control_group_status_instances`
//   above; the self-heal status pip is ported below.
//
/// Occupant pips for garrisoned buildings (pips.shp frames 6-12).
///
/// Drawn for every visible building with `ShowOccupantPips=yes` and `MaxNumberOccupants > 0`.
/// One pip per slot: filled slots use the infantry's `OccupyPip` color, empty slots use
/// frame 6 (gray). Starts at (screen_x+6, screen_y-1), step (+4, +2) per pip along
/// the isometric NW edge.
pub(crate) fn build_occupant_pip_instances(
    state: &AppState,
    sw: f32,
    sh: f32,
) -> Vec<SpriteInstance> {
    let (Some(sim), Some(overlay)) = (
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
        &state.match_state.match_presentation.selection_overlay,
    ) else {
        return Vec::new();
    };
    let has_tex = overlay.occupant_pip_texture().is_some();
    if !has_tex {
        return Vec::new();
    }
    let pip_size: [f32; 2] = overlay.occupant_pip_frame_size();
    let pip_uv_size: [f32; 2] = overlay.occupant_pip_uv_size();
    let (adj_x, adj_y) = overlay.occupant_pip_canvas_adj();
    let local_owner = preferred_local_owner_name(state);
    let rules = state.rules();
    // Occupant pips are NOT free-standing: `DrawPipScalePips` 0x00709A90 has
    // exactly two callsites, 0x006F682C and 0x006F6AB0, and both are inside
    // `TechnoClass__DrawHealthBar`. So they are reachable only through the same
    // selected-or-hovered arms that gate the health bar — the same rule that
    // stopped VERA drawing a bar over every damaged unit. Without it every
    // garrisoned building on a city map wears its pip strip permanently.
    let hovered_structure_id = building_health_hover_target(state, local_owner.as_deref());
    let mut instances = Vec::new();

    for e in sim.entities().values() {
        if e.category() != EntityCategory::Structure {
            continue;
        }
        if !health_bar_drawable(
            sim,
            e,
            hovered_structure_id,
            local_owner.as_deref(),
            state.rules(),
        ) {
            continue;
        }
        let type_str = sim.interner.resolve(e.type_ref());
        let Some(obj) = rules.and_then(|r| r.object(type_str)) else {
            continue;
        };
        if !obj.show_occupant_pips || obj.max_number_occupants == 0 {
            continue;
        }
        let cargo = match e.passenger_role.cargo() {
            Some(c) => c,
            None => continue,
        };
        let (sx, sy) = crate::render::locomotor_visual::screen_position(e);
        // The (+6, -1) offset is applied to the point `DrawHealthBar`'s BUILDING
        // branch hands the pip routine, which is the entity anchor plus the
        // projected foundation edge — not the raw anchor. Dropping that term put
        // the strip on the building's art instead of at its west corner, a miss
        // of (-60, +15) px on a 2-deep footprint and (-90, +30) on a 3-deep one.
        // Same pair the health pips above already use.
        let (fw, fh) = {
            let (w, h) = crate::rules::foundation::foundation_dimensions(&obj.foundation);
            (w as f32, h as f32)
        };
        let anchor_x = sx + (fw - fh) * 15.0 - (fw + fh) * 15.0;
        let anchor_y = sy + (fw + fh) * 7.5 - 15.0 + (fh - fw) * 7.5;
        let start_x: f32 = anchor_x + 6.0 + adj_x;
        let start_y: f32 = anchor_y - 1.0 + adj_y;
        let count: u32 = obj.max_number_occupants;
        // Occupant pip step: +4 right, +2 down (isometric NW edge, same as health pips but positive X).
        const STEP_X: f32 = 4.0;
        const STEP_Y: f32 = 2.0;

        // Bounding box culling.
        let end_x: f32 = start_x + (count.saturating_sub(1)) as f32 * STEP_X;
        let end_y: f32 = start_y + (count.saturating_sub(1)) as f32 * STEP_Y;
        let min_x: f32 = start_x.min(end_x);
        let min_y: f32 = start_y.min(end_y);
        let total_w: f32 = (end_x - start_x).abs() + pip_size[0];
        let total_h: f32 = (end_y - start_y).abs() + pip_size[1];
        if !in_view(
            min_x,
            min_y,
            total_w,
            total_h,
            state.match_state.input.camera_x,
            state.match_state.input.camera_y,
            sw,
            sh,
            48.0,
        ) {
            continue;
        }

        for i in 0..count {
            // Determine pip frame: occupied slot → occupant's OccupyPip, empty → frame 6.
            let frame_index: u32 = if (i as usize) < cargo.passengers().len() {
                let pax_id = cargo.passengers()[i as usize];
                sim.entities()
                    .get(pax_id)
                    .and_then(|pax| rules.and_then(|r| sim.interner.object_type(pax.type_ref(), r)))
                    .map(|pax_obj| pax_obj.occupy_pip)
                    .unwrap_or(7) // default PersonGreen
            } else {
                6 // empty slot
            };
            let px: f32 = start_x + i as f32 * STEP_X;
            let py: f32 = start_y + i as f32 * STEP_Y;
            let uv_origin: [f32; 2] = overlay.occupant_pip_uv_origin(frame_index);
            instances.push(SpriteInstance {
                position: [px, py],
                size: [pip_size[0], pip_size[1]],
                uv_origin,
                uv_size: pip_uv_size,
                depth: 0.0006,
                tint: [1.0, 1.0, 1.0],
                alpha: 1.0,
                ..Default::default()
            });
        }
    }

    instances
}

/// The Crazy Ivan bomb clock, `TechnoClass::DrawExtras @ 0x006F5190`
/// (0x006F519B..0x006F524E): over a carrier being drawn (DrawExtras is reached
/// only from the tactical render loop, `0x006D9153`, so never in limbo or a
/// transport) that is not sinking (`+0x3CD`), whose
/// BombVisible (`+0x68`) holds for the local player and whose cell is not
/// shrouded (`0x00487950`), BOMBCURS.SHP frame `BombClass::GetClockFrame`
/// centered (flags `0xE00`) on the render point (`vt+0xAC`: a building's art
/// anchor, `0x00459EF0`, else its coordinate; `CoordsToClient2 @ 0x006D2140`).
/// Drawn before the veterancy chevrons (0x006F5382), as there.
pub(crate) fn build_bomb_clock_instances(
    state: &AppState,
    sw: f32,
    sh: f32,
) -> Vec<SpriteInstance> {
    let (Some(sim), Some(overlay), Some(rules)) = (
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
        &state.match_state.match_presentation.selection_overlay,
        state.rules(),
    ) else {
        return Vec::new();
    };
    let Some(clock) = overlay.bomb_clock() else {
        return Vec::new();
    };
    let ignore_visibility = state.match_state.sandbox_full_visibility;
    let local = preferred_local_owner_name(state).and_then(|name| sim.interner.get(&name));
    // `GetClockFrame` reads the frame the draw sees: the next one to run.
    let frame = sim.session.binary_frame as i32;
    let mut instances = Vec::new();
    for &carrier in sim.bomb_carriers() {
        let Some(entity) = sim.entities().get(carrier) else {
            continue;
        };
        let Some(bomb) = entity.bomb else {
            continue;
        };
        if entity.lifecycle.in_limbo()
            || entity.passenger_role.is_inside_transport()
            || entity.sinking.is_active()
        {
            continue;
        }
        if !ignore_visibility {
            let Some(local) = local else {
                continue;
            };
            let (rx, ry) = (entity.position.rx(), entity.position.ry());
            if !sim.bomb_seen_by(carrier, local)
                || !sim.fog.is_cell_revealed(local, rx, ry)
                || sim.fog.is_cell_gap_covered(local, rx, ry)
            {
                continue;
            }
        }
        let Some(index) = bomb
            .clock_frame(
                frame,
                rules.combat_damage.ivan_timed_delay,
                rules.combat_damage.ivan_icon_flicker_rate,
            )
            .and_then(|index| usize::try_from(index).ok())
        else {
            continue;
        };
        let point = crate::render::locomotor_visual::screen_position(entity);
        let point = if entity.category() == EntityCategory::Structure {
            crate::render::locomotor_visual::building_art_anchor(point.0, point.1)
        } else {
            point
        };
        let Some(instance) = clock.instance(index, point, 0.0006) else {
            continue;
        };
        if in_view(
            instance.position[0],
            instance.position[1],
            instance.size[0],
            instance.size[1],
            state.match_state.input.camera_x,
            state.match_state.input.camera_y,
            sw,
            sh,
            48.0,
        ) {
            instances.push(instance);
        }
    }
    instances
}

/// The repair wrench's WRENCH.SHP frame at `frame`
/// (`TechnoClass::DrawExtras 0x006F52D8..0x006F532B`): a cycle of
/// `SpeedNormalize(14) / 4` frames (at least 2) spread over frames 0..=6,
/// both signed IDIVs.
fn repair_wrench_frame(frame: i32, options: &crate::sim::game_options::GameOptions) -> i32 {
    let cycle = (options.speed_normalize(14) / 4).max(2);
    frame % cycle * 6 / (cycle - 1)
}

/// The repair wrench, `TechnoClass::DrawExtras @ 0x006F5190`
/// (0x006F5253..0x006F5345): over a building being drawn whose repair byte
/// (`+0x6E8`) is set, whoever owns it, and whose cell is not shrouded
/// (`0x00487950`), WRENCH.SHP frame [`repair_wrench_frame`] of the frame the
/// draw sees, centered (flags `0xE00`) on the render point (the building's
/// art anchor, as the bomb clock's). Drawn after the bomb clock and before
/// the veterancy chevrons (0x006F5382), as there.
pub(crate) fn build_repair_wrench_instances(
    state: &AppState,
    sw: f32,
    sh: f32,
) -> Vec<SpriteInstance> {
    let (Some(sim), Some(overlay)) = (
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
        &state.match_state.match_presentation.selection_overlay,
    ) else {
        return Vec::new();
    };
    let Some(wrench) = overlay.repair_wrench() else {
        return Vec::new();
    };
    let Ok(index) = usize::try_from(repair_wrench_frame(
        sim.session.binary_frame as i32,
        &sim.session.game_options,
    )) else {
        return Vec::new();
    };
    let ignore_visibility = state.match_state.sandbox_full_visibility;
    let local = preferred_local_owner_name(state).and_then(|name| sim.interner.get(&name));
    let mut instances = Vec::new();
    for entity in sim.entities().values() {
        if entity.category() != EntityCategory::Structure
            || !entity.repairing.is_on()
            || entity.lifecycle.in_limbo()
        {
            continue;
        }
        if !ignore_visibility {
            let Some(local) = local else {
                continue;
            };
            let (rx, ry) = (entity.position.rx(), entity.position.ry());
            if !sim.fog.is_cell_revealed(local, rx, ry)
                || sim.fog.is_cell_gap_covered(local, rx, ry)
            {
                continue;
            }
        }
        let point = crate::render::locomotor_visual::screen_position(entity);
        let point = crate::render::locomotor_visual::building_art_anchor(point.0, point.1);
        let Some(instance) = wrench.instance(index, point, 0.0006) else {
            continue;
        };
        if in_view(
            instance.position[0],
            instance.position[1],
            instance.size[0],
            instance.size[1],
            state.match_state.input.camera_x,
            state.match_state.input.camera_y,
            sw,
            sh,
            48.0,
        ) {
            instances.push(instance);
        }
    }
    instances
}

/// Frame-local screen viewport for Techno status emission.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StatusViewport {
    pub camera: [f32; 2],
    pub size: [f32; 2],
}

/// One Display parent's rank and health commands, in native Extras order.
#[derive(Debug, Default)]
pub(crate) struct TechnoExtras {
    pub rank: Option<SpriteInstance>,
    pub background: Option<SpriteInstance>,
    /// Raw PIPS commands, including the Building branch's empty slots.
    pub fills: Vec<SpriteInstance>,
}

/// One Techno DrawExtras6F5190 parent, rank prefix then health6F64A0.
/// Rank70A990 precedes the selected/hovered health gate at6F5388. It uses
/// the shared cloak/disguise and raw Veterancy owners, with opaque status art.
/// Ordinary MTNK is executed in procedural_drawing_oracle/unit_rank.json;
/// other locomotor +68 overrides and promoted-Building lifecycle remain open.
///
/// Building6F64A0..6839 projects Dimension2's upper edge and draws raw PIPS
/// frames1/2/4 followed by frame0. Infantry6F6852..68E3 uses8 pips and PIPBRD1:
/// pip(-5,delta-24), background(11,delta-25). Other mobile6F68E8..6976 uses17
/// and PIPBRD0: pip(-15,delta-25), background(1,delta-26). The common centered
/// resource owns every frame's original origin and index plane (4AED70).
/// Source/executable controls: procedural_drawing_oracle/building_health.json
/// and unit_health.json. Stock selected/hovered GAPOWR and mobile coverage;
/// conditional pip/status tails remain with their existing owners.
/// No RNG, timer, detach or retained presentation state changes here.
pub(crate) fn techno_extras_for_parent(
    sim: &crate::sim::world::Simulation,
    entity: &crate::sim::game_entity::GameEntity,
    rules: Option<&RuleSet>,
    overlay: &crate::render::selection_overlay::SelectionOverlay,
    hovered_id: Option<u64>,
    local_owner: Option<&str>,
    viewport: StatusViewport,
) -> TechnoExtras {
    let mut strip = TechnoExtras::default();
    // DrawExtras6F51AA precedes every status family. Display/lifecycle/body
    // admission remains the existing shared owner, independent of selection.
    if entity.sinking.is_active() || entity.passenger_role.is_inside_transport() {
        return strip;
    }
    let Some(observer) = admitted_techno_observer(sim, entity, local_owner, rules) else {
        return strip;
    };
    // Tactical6D9676 dispatches Object5F65A0/Building447AC0 GetCoords;
    //6D9691..9726 projects integer XYZ before subtracting the camera.
    let coord =
        crate::sim::movement::ground_pose::object_get_coords(entity, sim.resolved_terrain.as_ref());
    let (sx, sy) = crate::util::lepton::absolute_leptons_to_screen(coord.x, coord.y, coord.z);
    let object = rules.and_then(|rules| sim.interner.object_type(entity.type_ref(), rules));
    let instance = |shape: &crate::render::selection_overlay::CenteredShapeStrip,
                    frame: usize,
                    point: (f32, f32),
                    depth: f32| {
        let mut instance = shape.instance(frame, point, depth)?;
        // E00 rank/background reads tactical A; 600 fills bypass A. The renderer
        // chooses those bindings; both use the same plain N53 Convert.
        instance.palette_light = crate::render::palette_light::PaletteLight::plain(53, 1000);
        in_view(
            instance.position[0],
            instance.position[1],
            instance.size[0],
            instance.size[1],
            viewport.camera[0],
            viewport.camera[1],
            viewport.size[0],
            viewport.size[1],
            48.0,
        )
        .then_some(instance)
    };
    //6F534E..5382: +C8's canonical observer-disguise query is distinct
    // from drawing disguise art; +68 uses VisualCharacter's screen(0,NULL)
    // query. The first screen argument means graphical client, not force_sensor.
    let observer_id = local_owner.and_then(|owner| sim.interner.get(owner));
    let disguised = observer_id.is_some_and(|viewer| {
        entity.is_disguised_to(
            viewer,
            Some(&sim.fog),
            Some(&sim.house_alliances),
            &sim.interner,
        )
    });
    let character = entity.visual_character(
        observer.cloaking_stages,
        observer.invisible,
        crate::sim::cloak_disguise::VisualCharacterQuery::screen(
            true,
            observer.observer_present,
            observer.detects_cloak,
            observer.owner_is_mutually_allied,
            observer.is_campaign,
            false,
        ),
    );
    if !disguised && character != 5 {
        use crate::sim::combat::veterancy::{raw_is_elite, raw_is_negative, raw_is_veteran};
        //70A999's +88 getter is discarded (Unit741490, Building459EE0);
        // there is no Trainable gate.70A9B2/C2/D2 tests the raw accumulator,
        // not the cached/quantized rank. A negative value selects frame19.
        let raw = entity.veterancy.raw();
        let frame = if raw_is_veteran(raw) {
            Some(14)
        } else if raw_is_elite(raw) {
            Some(15)
        } else if raw_is_negative(raw) {
            Some(19)
        } else {
            None
        };
        if let (Some(frame), Some(shape)) = (frame, overlay.pips()) {
            let (dx, dy) = if entity.category() == EntityCategory::Infantry {
                (5.0, 2.0)
            } else {
                (10.0, 6.0)
            };
            //70AA20..AA4F: opaque PIPS E00, plain53/1000, A-consuming/no-Z.
            // Its -2 Z-adjust is unused by this leaf; do not inherit body FX.
            strip.rank = instance(shape, frame, (sx + dx, sy + dy), 0.0006).map(|mut rank| {
                rank.z_adjust = -2.0;
                rank
            });
        }
    }
    let (draw_background, draw_health) = status_visibility(
        entity.selection.is_selected(),
        hovered_id == Some(entity.stable_id()),
    );
    if !draw_health {
        return strip;
    }
    // Both6F65D7 and6F697C use Object5F5C60 then multiply/ftol7C5F00.
    // Keep the shared health/numeric owners and the signed [1,N] clamp.
    let filled_count = |strength: i32, count: i32| {
        use crate::util::native_x87::MaskedX87Chop53 as X87;
        X87::ftol_i32_low_masked(X87::mul(
            entity.health.ratio(strength),
            X87::load_i32(count),
        ))
        .clamp(1, count)
    };
    if entity.category() == EntityCategory::Structure {
        let (Some(rules), Some(object), Some(shape)) = (rules, object, overlay.pips()) else {
            return strip;
        };
        // BuildingType464AF0 owns Foundation*256 and fixed ART Height*104.
        //6F6536/656E project local offsets through Tactical6D1F10, without
        // the absolute projection's camera or +15 world-row origin.
        let [width, height, z] = rules.building_dimensions(object);
        let left = (width - width / 2).wrapping_neg();
        let back = height - height / 2;
        let (ax, ay) = crate::util::lepton::project_absolute_lepton_xy(left, back);
        let (_, by) = crate::util::lepton::project_absolute_lepton_xy(left, back.wrapping_neg());
        let upper_y = ay - crate::util::native_x87::adjust_for_z_standard(z);
        // Both upper projections carry the same Z; it cancels in6F65BC..CF.
        let num_pips = (ay - by) / 2;
        if num_pips <= 0 {
            return strip;
        }
        let filled = filled_count(object.strength, num_pips);
        //6F660A/661F use the existing health-category predicates; zero HP
        // does not satisfy IsRed. Raw frame selection is green1/yellow2/red4.
        let frame = match crate::sim::combat::health_category(
            entity.health.current(),
            object.strength,
            rules.general.condition_red,
            rules.general.condition_yellow,
        ) {
            0 => 4,
            1 => 2,
            _ => 1,
        };
        let start = (
            sx + (ax + 4 * num_pips + 3) as f32,
            sy + (upper_y + 4 - 2 * num_pips) as f32,
        );
        strip.fills.reserve(num_pips as usize);
        //6F6658..6774: filled first, then empty frame0, walking [-4,+2].
        // Building PIPS600 bypasses A and Z; the replay owner preserves both
        // original first-Extras and late-Extras positions around the body.
        for index in 0..num_pips {
            strip.fills.extend(instance(
                shape,
                if index < filled { frame } else { 0 },
                (start.0 - (index * 4) as f32, start.1 + (index * 2) as f32),
                0.0006,
            ));
        }
        return strip;
    }
    let delta = object.map_or(0.0, |object| object.pixel_selection_bracket_delta as f32);
    let infantry = entity.category() == EntityCategory::Infantry;
    let (num_pips, background_frame, background_offset, pip_offset) = if infantry {
        (UNIT_PIPS_INFANTRY, 1, [11.0, -25.0], [-5.0, -24.0])
    } else {
        (UNIT_PIPS_VEHICLE, 0, [1.0, -26.0], [-15.0, -25.0])
    };
    if draw_background && let Some(shape) = overlay.pipbrd() {
        strip.background = instance(
            shape,
            background_frame,
            (sx + background_offset[0], sy + delta + background_offset[1]),
            0.0006,
        );
    }
    let (Some(rules), Some(object), Some(shape)) = (rules, object, overlay.pips()) else {
        return strip;
    };
    let filled = filled_count(object.strength, num_pips as i32) as u32;
    //6F69B1..69F3 compares independently: <=red overrides <=yellow. These
    // are direct ratio comparisons, unlike Building's positive-HP red gate.
    use crate::util::native_x87::MaskedX87Ordering::Greater;
    let frame = if entity
        .health
        .compare_ratio(object.strength, rules.general.condition_red)
        != Greater
    {
        18
    } else if entity
        .health
        .compare_ratio(object.strength, rules.general.condition_yellow)
        != Greater
    {
        17
    } else {
        16
    };
    strip.fills.reserve(filled as usize);
    for index in 0..filled {
        strip.fills.extend(instance(
            shape,
            frame,
            (
                sx + pip_offset[0] + index as f32 * UNIT_PIP_STEP_X,
                sy + delta + pip_offset[1],
            ),
            0.0005,
        ));
    }
    strip
}

/// The self-heal status pip a non-building's health strip carries.
///
/// Native tail of `DrawPipScalePips` (`0x0070A614..0x0070A6BB`), which the
/// object-render loop reaches straight after the strip: PIPS.SHP frame `0x0D`
/// (the red cross) or `0x14` (the white wrench), centered (flags `0x600`) on
/// the same draw point the strip uses plus [`SelfHealPip::offset`]. The gate is
/// [`self_heal_pip`]'s, so the pip appears exactly when the strip does.
///
/// Residual: for an object whose `Health` (`+0x6C`) is below its type's
/// `Strength` (`Type+0xA0`) and whose frame lands in the first six of the
/// heal period, native re-issues the draw with bit 0 set (flags `0x601`,
/// `0x0070A628..0x0070A667`). That variant's draw effect is not established,
/// so every pip here uses `0x600`, which is also what the native draws at full
/// health and outside the window.
pub(crate) fn build_self_heal_pip_instances(
    state: &AppState,
    sw: f32,
    sh: f32,
) -> Vec<SpriteInstance> {
    let (Some(sim), Some(overlay)) = (
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
        &state.match_state.match_presentation.selection_overlay,
    ) else {
        return Vec::new();
    };
    let Some(pip_art) = overlay.pips() else {
        return Vec::new();
    };
    let local_owner = preferred_local_owner_name(state);
    let hovered_unit_id = unit_health_hover_target(state, local_owner.as_deref());
    let mut instances = Vec::new();
    for e in sim.entities().values() {
        if e.category() == EntityCategory::Structure || e.sinking.is_active() {
            continue;
        }
        if e.passenger_role.is_inside_transport() {
            continue;
        }
        if !entity_status_pips_drawable(
            sim,
            e,
            hovered_unit_id,
            local_owner
                .as_deref()
                .and_then(|owner| sim.interner.get(owner)),
            state.rules(),
        ) {
            continue;
        }
        let Some(obj) = state
            .rules()
            .and_then(|rules| sim.interner.object_type(e.type_ref(), rules))
        else {
            continue;
        };
        let (infantry, units) = sim.houses.get(&e.owner()).map_or((0, 0), |house| {
            (house.self_heal_infantry(), house.self_heal_units())
        });
        let Some(arm) = self_heal_pip(e.category(), obj.organic, infantry, units) else {
            continue;
        };
        let (sx, sy) = crate::app::presentation::instances::interpolated_screen_position_entity(e);
        let (off_x, off_y) = arm.offset();
        let point: (f32, f32) = (
            sx + off_x,
            sy + obj.pixel_selection_bracket_delta as f32 + off_y,
        );
        let Some(instance) = pip_art.instance(arm.frame(), point, 0.0004) else {
            continue;
        };
        if in_view(
            instance.position[0],
            instance.position[1],
            instance.size[0],
            instance.size[1],
            state.match_state.input.camera_x,
            state.match_state.input.camera_y,
            sw,
            sh,
            48.0,
        ) {
            instances.push(instance);
        }
    }
    instances
}

/// Raw PIPS2 frames after selected/hovered body admission and the shared
/// native pip-tail relationship gate. DrawHealthBar6F6A58 ->709A90.
/// Rendering only reads retained state; it never advances timers or RNG.
pub(crate) fn scale_pip_frames(
    sim: &crate::sim::world::Simulation,
    entity: &crate::sim::game_entity::GameEntity,
    hovered_id: Option<u64>,
    local_owner: Option<&str>,
    rules: &RuleSet,
) -> Vec<usize> {
    if entity.category() == EntityCategory::Structure
        || entity.sinking.is_active()
        || entity.passenger_role.is_inside_transport()
        || !entity_status_pips_drawable(
            sim,
            entity,
            hovered_id,
            local_owner.and_then(|owner| sim.interner.get(owner)),
            Some(rules),
        )
    {
        return Vec::new();
    }
    let Some(object) = sim.interner.object_type(entity.type_ref(), rules) else {
        return Vec::new();
    };
    if object.pip_scale == crate::rules::object_type::PipScale::MindControl {
        // GetPipMax716290 case5 -> current primary Weapon.Damage+A4,
        // independent of the manager's retained admission capacity.
        let maximum = object
            .weapon_at(0)
            .and_then(|weapon| rules.weapon(weapon))
            .map_or(0, |weapon| weapon.damage);
        let slots = usize::try_from(maximum).unwrap_or(0);
        let manager = entity.capture_manager.as_ref();
        let count = manager.map_or(0, |manager| manager.count());
        // Fill708C7A..708D46 scales count/max by the SAME positive max:
        // stock count0..4/weaponDamage3 rounds back to count. Draw70A0B5
        // uses frame1 while index<fill and frame0 for the remaining slots.
        let mut frames: Vec<_> = (0..slots)
            .map(|index| usize::from((index as i64) < i64::from(count)))
            .collect();
        // IsOverloading4726C0 owns infinite/capacity/counter policy;
        //70A17F..1C2 adds exactly one frame3 (positive) or4 (idle).
        if let Some(active) = manager.and_then(|manager| manager.overload_pip_state()) {
            frames.push(if active { 3 } else { 4 });
        }
        return frames;
    }
    if object.pip_scale != crate::rules::object_type::PipScale::Tiberium {
        return Vec::new();
    }
    let Some(miner) = &entity.miner else {
        return Vec::new();
    };
    let filled = usize::from(miner.cargo_pips());
    let total = miner.cargo().len();
    let gems = miner
        .cargo()
        .iter()
        .filter(|bale| bale.resource_type == crate::sim::miner::ResourceType::Gem)
        .count();
    let gem_pips = if total > 0 {
        (gems * filled).div_ceil(total).min(filled)
    } else {
        0
    };
    let mut frames = Vec::with_capacity(5);
    frames.extend(std::iter::repeat_n(5, gem_pips));
    frames.extend(std::iter::repeat_n(2, filled - gem_pips));
    frames.resize(5, 0);
    frames
}

/// Native nonbuilding scale strip709B0F: anchor(-15,+10), step(4,0).
/// CenteredShapeStrip owns the physical SHP origin/canvas adjustment.
pub(crate) fn build_scale_pip_instances(state: &AppState, sw: f32, sh: f32) -> Vec<SpriteInstance> {
    let (Some(sim), Some(overlay), Some(rules)) = (
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
        &state.match_state.match_presentation.selection_overlay,
        state.rules(),
    ) else {
        return Vec::new();
    };
    let Some(art) = overlay.scale_pips() else {
        return Vec::new();
    };
    let local_owner = preferred_local_owner_name(state);
    let hovered = unit_health_hover_target(state, local_owner.as_deref());
    let mut instances = Vec::new();
    for stable_id in crate::app::presentation::instances::tactical_entity_encounter_order(sim) {
        let Some(entity) = sim.entities().get(stable_id) else {
            continue;
        };
        let frames = scale_pip_frames(sim, entity, hovered, local_owner.as_deref(), rules);
        if frames.is_empty() {
            continue;
        }
        let object = sim.interner.object_type(entity.type_ref(), rules).unwrap();
        let (sx, sy) =
            crate::app::presentation::instances::interpolated_screen_position_entity(entity);
        for (slot, frame) in frames.into_iter().enumerate() {
            let point = (
                sx - 15.0 + slot as f32 * 4.0,
                sy + object.pixel_selection_bracket_delta as f32 + 10.0,
            );
            let Some(instance) = art.instance(frame, point, 0.0004) else {
                continue;
            };
            if in_view(
                instance.position[0],
                instance.position[1],
                instance.size[0],
                instance.size[1],
                state.match_state.input.camera_x,
                state.match_state.input.camera_y,
                sw,
                sh,
                48.0,
            ) {
                instances.push(instance);
            }
        }
    }
    instances
}

/// Radius rings for selected sensor/gap buildings. This is Tactical action-visual
/// rendering, not part of selection brackets or health pips.
pub(crate) fn build_building_radius_ring_instances(
    state: &AppState,
    sw: f32,
    sh: f32,
) -> Vec<SpriteInstance> {
    let (Some(sim), Some(rules)) = (
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
        state.rules(),
    ) else {
        return Vec::new();
    };
    let local_owner = preferred_local_owner_name(state);
    let local_owner_id = local_owner.as_deref().and_then(|n| sim.interner.get(n));
    let ignore_visibility = state.match_state.sandbox_full_visibility;
    let mut instances = Vec::new();

    for e in sim.entities().values() {
        if e.category() != EntityCategory::Structure || !e.selection.is_selected() {
            continue;
        }
        if !status_entity_visible_plain(
            local_owner_id,
            &sim.fog,
            &e.position,
            e.owner(),
            ignore_visibility,
        ) {
            continue;
        }
        let type_str = sim.interner.resolve(e.type_ref());
        let Some(obj) = rules.object(type_str) else {
            continue;
        };
        let Some(radius_cells) = building_sensor_range_cells(obj, rules) else {
            continue;
        };
        if radius_cells == 0 {
            continue;
        }

        let (fw, fh) = crate::rules::foundation::foundation_dimensions(&obj.foundation);
        let (base_x, base_y) = crate::render::locomotor_visual::screen_position(e);
        let center_x = base_x + (fw as f32 - fh as f32) * 15.0;
        let center_y = base_y + (fw as f32 + fh as f32) * 7.5 - 15.0;
        let radius_x = radius_cells as f32 * 30.0;
        let radius_y = radius_cells as f32 * 15.0;
        if !in_view(
            center_x - radius_x,
            center_y - radius_y,
            radius_x * 2.0,
            radius_y * 2.0,
            state.match_state.input.camera_x,
            state.match_state.input.camera_y,
            sw,
            sh,
            16.0,
        ) {
            continue;
        }
        emit_ellipse_ring(
            &mut instances,
            center_x,
            center_y,
            radius_x,
            radius_y,
            [0.15, 1.0, 0.25],
        );
    }

    instances
}

fn building_sensor_range_cells(obj: &ObjectType, rules: &RuleSet) -> Option<u32> {
    if obj.psychic_detection_radius > 0 {
        return Some(u32::from(obj.psychic_detection_radius));
    }
    if obj.gap_generator {
        if obj.super_gap_radius_in_cells > 0 {
            return Some(u32::from(obj.super_gap_radius_in_cells));
        }
        if obj.gap_radius_in_cells > 0 {
            return Some(u32::from(obj.gap_radius_in_cells));
        }
        return Some(rules.general.gap_radius.max(0) as u32);
    }
    if (obj.sensor_array || obj.cloak_generator || obj.sensors) && obj.sensors_sight > 0 {
        return Some(u32::from(obj.sensors_sight));
    }
    None
}

fn emit_ellipse_ring(
    instances: &mut Vec<SpriteInstance>,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    tint: [f32; 3],
) {
    let segments = ((rx.max(ry) / 2.0).ceil() as usize).clamp(96, 384);
    let mut prev = ellipse_point(cx, cy, rx, ry, 0.0);
    for i in 1..=segments {
        let angle = std::f32::consts::TAU * i as f32 / segments as f32;
        let next = ellipse_point(cx, cy, rx, ry, angle);
        emit_colored_line(instances, prev, next, tint);
        prev = next;
    }
}

fn ellipse_point(cx: f32, cy: f32, rx: f32, ry: f32, angle: f32) -> [f32; 2] {
    [cx + angle.cos() * rx, cy + angle.sin() * ry]
}

fn emit_colored_line(
    instances: &mut Vec<SpriteInstance>,
    a: [f32; 2],
    b: [f32; 2],
    tint: [f32; 3],
) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let steps = dx.abs().max(dy.abs()).ceil() as i32;
    if steps <= 0 {
        return;
    }
    let step_x = dx / steps as f32;
    let step_y = dy / steps as f32;
    for i in 0..steps {
        instances.push(SpriteInstance {
            position: [
                (a[0] + step_x * i as f32).round(),
                (a[1] + step_y * i as f32).round(),
            ],
            size: [1.0, 1.0],
            uv_origin: [0.0, 0.0],
            uv_size: [1.0, 1.0],
            tint,
            alpha: 1.0,
            depth: 0.00055,
            ..Default::default()
        });
    }
}

/// Resolve the active cursor id and sequence for the current game state.
/// Maps game-state intent → CursorId → loaded sequence via HashMap lookup.
/// The id travels with the sequence because the animation phase is keyed on it —
/// changing shape restarts the sequence at frame 0.
fn active_cursor_sequence(state: &AppState) -> Option<(CursorId, &SoftwareCursorSequence)> {
    let cursor = state
        .match_state
        .match_presentation
        .software_cursor
        .as_ref()?;
    let id: CursorId = current_cursor_feedback_kind(state)
        .and_then(cursor_id_for_feedback)
        .unwrap_or(CursorId::Default);
    cursor.get(id).map(|sequence| (id, sequence))
}

/// Observation of the same cursor selection used by the production draw.
/// Map captures admit only a static arrow, so cursor wall time cannot alter it.
pub(crate) fn static_default_cursor(state: &AppState) -> bool {
    state.use_software_cursor()
        && active_cursor_sequence(state)
            .is_some_and(|(id, sequence)| id == CursorId::Default && sequence.interval_ms == 0)
}

pub(crate) fn build_software_cursor_instances(state: &AppState) -> Vec<SpriteInstance> {
    if !state.use_software_cursor() || !state.match_state.input.cursor_visibility.is_visible() {
        return Vec::new();
    }
    let Some((id, sequence)) = active_cursor_sequence(state) else {
        return Vec::new();
    };
    let frame = if id == CursorId::Default
        && crate::app::input::planning_mode::node_at_cursor(state).is_some()
    {
        //63AC41..4E selects row0's static small frame when only hovering.
        sequence.frames.get(1)
    } else {
        software_cursor_frame_for(id, sequence)
    };
    let Some(frame) = frame else {
        return Vec::new();
    };
    // Cursor rendering: the hotspot pixel must sit exactly at the OS cursor position.
    // Scroll cursors use edge-aligned hotspots, so cursor_x/y represents the hotspot
    // location — subtract the hotspot offset to find the top-left of the sprite.
    // Note: cursor_x/y are in screen space; camera offset is NOT applied (cursor is UI).
    vec![SpriteInstance {
        position: [
            state.match_state.input.cursor_x + state.match_state.input.camera_x
                - sequence.hotspot[0],
            state.match_state.input.cursor_y + state.match_state.input.camera_y
                - sequence.hotspot[1],
        ],
        size: [frame.width, frame.height],
        uv_origin: [0.0, 0.0],
        uv_size: [1.0, 1.0],
        depth: 0.0001,
        tint: [1.0, 1.0, 1.0],
        alpha: 1.0,
        ..Default::default()
    }]
}

pub(crate) fn current_software_cursor_texture(state: &AppState) -> Option<&BatchTexture> {
    if !state.match_state.input.cursor_visibility.is_visible() {
        return None;
    }
    let (id, sequence) = active_cursor_sequence(state)?;
    Some(&software_cursor_frame_for(id, sequence)?.texture)
}

fn status_entity_visible_plain(
    local_owner: Option<crate::sim::intern::InternedId>,
    fog: &crate::sim::vision::FogState,
    pos: &Position,
    entity_owner: crate::sim::intern::InternedId,
    ignore_visibility: bool,
) -> bool {
    if ignore_visibility {
        return true;
    }
    let Some(local_owner) = local_owner else {
        return true;
    };
    if local_owner == entity_owner {
        return true;
    }
    fog.is_cell_revealed(local_owner, pos.rx(), pos.ry())
        && !fog.is_cell_gap_covered(local_owner, pos.rx(), pos.ry())
}

#[cfg(test)]
#[path = "mind_control_pip_tests.rs"]
mod mind_control_pip_tests;

#[cfg(test)]
mod tests {
    /// `tools/spatial_oracle/building_repair.json` `wrench` rows: the frame
    /// DrawExtras picks over every stored game speed.
    #[test]
    fn the_repair_wrench_frame_matches_the_original() {
        let corpus: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
            "tools/spatial_oracle/building_repair.json",
        ))
        .unwrap();
        let mut compared = 0;
        for row in corpus["wrench"].as_array().unwrap() {
            let input = &row["input"];
            let options = crate::sim::game_options::GameOptions {
                game_speed: input["speed"].as_i64().unwrap() as i32,
                ..Default::default()
            };
            assert_eq!(
                i64::from(super::repair_wrench_frame(
                    input["frame"].as_i64().unwrap() as i32,
                    &options
                )),
                row["frame_index"].as_i64().unwrap(),
                "{}",
                input["name"]
            );
            compared += 1;
        }
        assert_eq!(compared, 144);
    }

    use super::*;

    #[test]
    fn status_tail_asks_owner_alliance_and_shares_selected_hover_body_admission() {
        use crate::sim::{combat::Health, game_entity::GameEntity, world::Simulation};
        let mut sim = Simulation::new();
        let owner = sim.interner.intern("Owner");
        let viewer = sim.interner.intern("Viewer");
        let type_ref = sim.interner.intern("MIND");
        let mut actor = GameEntity::new_at_frame_zero_for_test(
            1,
            12,
            12,
            0,
            0,
            owner,
            Health::new(500),
            type_ref,
            EntityCategory::Unit,
            0,
            5,
            false,
        );
        actor.lifecycle.set_in_limbo_for_test(false);
        actor.selection.set_selected_for_test(true);
        sim.entities_mut().insert(actor);
        sim.resubmit_entity_display_for_test(1);
        let admitted = |sim: &Simulation, hover| {
            entity_status_pips_drawable(
                sim,
                sim.entities().get(1).unwrap(),
                hover,
                Some(viewer),
                None,
            )
        };
        assert!(
            health_bar_drawable(
                &sim,
                sim.entities().get(1).unwrap(),
                None,
                Some("Viewer"),
                None
            ),
            "enemy health strip has independent admission"
        );
        assert!(!admitted(&sim, None));
        sim.house_alliances
            .entry("VIEWER".into())
            .or_default()
            .insert("OWNER".into());
        assert!(
            !admitted(&sim, None),
            "viewer's one-way alliance does not admit owner status"
        );
        sim.house_alliances.clear();
        sim.house_alliances
            .entry("OWNER".into())
            .or_default()
            .insert("VIEWER".into());
        assert!(
            admitted(&sim, None),
            "owner's one-way alliance admits shared pip tail"
        );
        sim.entities_mut()
            .get_mut(1)
            .unwrap()
            .selection
            .set_selected_for_test(false);
        assert!(!admitted(&sim, None));
        assert!(
            admitted(&sim, Some(1)),
            "ordinary hover also admits count/self-heal/group tail"
        );
        sim.entities_mut()
            .get_mut(1)
            .unwrap()
            .lifecycle
            .set_in_limbo_for_test(true);
        assert!(
            !admitted(&sim, Some(1)),
            "retained Display cannot resurrect limbo feedback"
        );
    }

    /// Original Building DrawExtras selected/hover dispatch, with raw shroud
    /// and +41B contrasts. The same health admission owns mobile pips too.
    #[test]
    fn ordinary_health_hover_matches_native_without_raw_anchor_gate() {
        use crate::sim::{combat::Health, game_entity::GameEntity, world::Simulation};
        let corpus: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
            "tools/procedural_drawing_oracle/health_hover.json",
        ))
        .unwrap();
        let mut compared = 0;
        for row in corpus["controls"].as_array().unwrap() {
            let input = &row["input"];
            // The disguised hover branch is a separate existing residual;
            // this comparison is ordinary buildings and the shared admission.
            if input["disguised"].as_bool().unwrap() {
                continue;
            }
            for category in [
                EntityCategory::Structure,
                EntityCategory::Unit,
                EntityCategory::Infantry,
                EntityCategory::Aircraft,
            ] {
                let mut sim = Simulation::new();
                let player = sim.interner.intern("Player");
                let owner = sim.interner.intern("Neutral");
                let type_ref = sim.interner.intern("CATIME01");
                sim.fog = crate::sim::vision::FogState::new(32, 32);
                if input["raw_mapped"].as_bool().unwrap() {
                    sim.fog.reveal_cells_for_owner(player, [(20, 20)]);
                }
                let mut entity = GameEntity::new_at_frame_zero_for_test(
                    1,
                    20,
                    20,
                    4,
                    0,
                    owner,
                    Health::new(100),
                    type_ref,
                    category,
                    0,
                    5,
                    false,
                );
                entity.lifecycle.set_in_limbo_for_test(false);
                entity
                    .selection
                    .set_selected_for_test(input["selected"].as_bool().unwrap());
                entity.discovery.set_discovered_by_current_house_for_test(
                    input["discovered"].as_bool().unwrap(),
                );
                sim.entities_mut().insert(entity);
                sim.resubmit_entity_display_for_test(1);
                let hovered = input["hovered"].as_bool().unwrap().then_some(1);
                let expected = row["draw_health_bar"].as_bool().unwrap();
                assert_eq!(
                    health_bar_drawable(
                        &sim,
                        sim.entities().get(1).unwrap(),
                        hovered,
                        Some("Player"),
                        None
                    ),
                    expected,
                    "{} {category:?}",
                    row["name"]
                );
                if expected {
                    // Being hovered cannot resurrect a limbo member, even
                    // while a retained Display registration still exists.
                    sim.entities_mut()
                        .get_mut(1)
                        .unwrap()
                        .lifecycle
                        .set_in_limbo_for_test(true);
                    assert!(!health_bar_drawable(
                        &sim,
                        sim.entities().get(1).unwrap(),
                        hovered,
                        Some("Player"),
                        None
                    ));
                }
                compared += 1;
            }
        }
        assert!(
            compared >= 16,
            "ordinary selected/hover and anchor contrasts"
        );
    }

    #[test]
    #[ignore = "requires retail SOV01 assets"]
    fn sov01_trigger_revealed_time_machine_hover_admits_health_bar() {
        let (retail, _) = crate::rules::retail_ini_fixture::retail_assets().unwrap();
        let mut scene = crate::headless_scenario::load(&retail, "SOV01UMD.MAP", 339173046)
            .expect("retail SOV01 scenario");
        let sim = &mut scene.runtime.simulation;
        let rules = &scene.runtime.resources.rules;
        let id = sim
            .entities()
            .values()
            .find(|entity| {
                sim.resolve(entity.type_ref()) == "CATIME01"
                    && (entity.position.rx(), entity.position.ry()) == (142, 96)
            })
            .expect("authored Neutral time machine")
            .stable_id();
        assert_eq!(rules.object("CATIME01").unwrap().strength, 2500);
        let viewer = sim.session.current_house.unwrap();
        let viewer_name = sim.resolve(viewer).to_owned();
        for waypoint in [63, 66, 71, 99, 100, 114, 437, 647] {
            assert!(sim.trigger_reveal_at_waypoint(
                waypoint,
                crate::sim::world::UninitContext::with_rules(rules)
            ));
        }
        let target = sim.entities().get(id).unwrap();
        let (sx, sy) = crate::render::locomotor_visual::ground_screen_position(&target.position);
        let heights = sim.resolved_terrain.as_ref().unwrap().build_height_map();
        let hover = crate::app::input::entity_pick::hover_target_at_point(
            sim,
            sx,
            sy,
            &viewer_name,
            false,
            Some(rules),
            &heights,
            None,
        )
        .expect("revealed time-machine hover");
        assert_eq!(hover.stable_id, id);
        assert!(!target.selection.is_selected());
        println!(
            "CATIME01 raw_anchor_mapped={} hover={hover:?}",
            sim.fog
                .is_cell_revealed(viewer, target.position.rx(), target.position.ry())
        );
        assert!(health_bar_drawable(
            sim,
            target,
            Some(hover.stable_id),
            Some(&viewer_name),
            Some(rules)
        ));
        assert!(!health_bar_drawable(
            sim,
            target,
            None,
            Some(&viewer_name),
            Some(rules)
        ));
    }

    /// A selected object draws both halves — bracket and pips.
    #[test]
    fn selected_unit_draws_bracket_and_pips() {
        assert_eq!(status_visibility(true, false), (true, true));
        assert_eq!(status_visibility(true, true), (true, true));
    }

    /// The hover arm draws the pip strip with no bracket behind it: the
    /// bracket blit sits under an is-selected test inside the health-bar
    /// routine, while the pips do not.
    #[test]
    fn hovered_unselected_unit_draws_pips_without_the_bracket() {
        assert_eq!(status_visibility(false, true), (false, true));
    }

    /// The clause the old damage gate broke: an unselected, unhovered object
    /// draws no health bar at all, whatever its health is. The overlay pass
    /// reaches the health-bar slot only through the selected and hover arms.
    #[test]
    fn unselected_unhovered_unit_draws_nothing_regardless_of_damage() {
        assert_eq!(status_visibility(false, false), (false, false));
    }

    /// The arm table `DrawPipScalePips` walks at `0x0070A50C..0x0070A5E4`:
    /// `WhatAmI()==0xF` (Infantry) enters the organic arm directly, a `Unit`
    /// enters it too while `Type+0xD97` (`Organic`) is set and the units arm
    /// when that byte is clear, and every other class fails both tests.
    #[test]
    fn the_self_heal_pip_arm_table_matches_the_original() {
        use EntityCategory::{Aircraft, Infantry, Structure, Unit};
        assert_eq!(
            self_heal_pip(Infantry, true, 1, 1),
            Some(SelfHealPip::Organic)
        );
        // The organic byte decides the arm for a Unit, not the class alone.
        assert_eq!(self_heal_pip(Unit, true, 1, 1), Some(SelfHealPip::Organic));
        assert_eq!(self_heal_pip(Unit, false, 1, 1), Some(SelfHealPip::Units));
        // `WhatAmI()==2` (Aircraft) and `==6` (Building) never reach a draw.
        assert_eq!(self_heal_pip(Aircraft, false, 1, 1), None);
        assert_eq!(self_heal_pip(Structure, false, 1, 1), None);
        // Each arm needs its own house counter above zero
        // (`HasInfSelfHeal @ 0x0050D9C0`, `HasUnitSelfHeal @ 0x0050D9D0`),
        // and the executed predicate is false for a negative count.
        assert_eq!(self_heal_pip(Infantry, true, 0, 1), None);
        assert_eq!(self_heal_pip(Infantry, true, -1, 1), None);
        assert_eq!(self_heal_pip(Unit, true, 0, 1), None);
        assert_eq!(self_heal_pip(Unit, false, 1, 0), None);
        assert_eq!(self_heal_pip(Unit, false, 1, -1), None);
        // The other arm's counter never substitutes for the object's own.
        assert_eq!(self_heal_pip(Unit, false, 1, 0), None);
        assert_eq!(self_heal_pip(Unit, false, 0, 1), Some(SelfHealPip::Units));
    }

    /// The frame and draw-point offset each arm carries: `ebx` `0x0D` at
    /// `0x0070A573` with `edi`/`esi` `(+0x13, -0x23)` at `0x0070A60A`, and
    /// `0x14` at `0x0070A5DF` with `(+0x26, -0x20)` at `0x0070A5F1`. `edi`
    /// reaches the x argument (`0x0070A63C`, `0x0070A67D`) and `esi` the y one
    /// (`0x0070A649`, `0x0070A68A`).
    #[test]
    fn the_self_heal_pip_frames_and_offsets_match_the_original() {
        assert_eq!(SelfHealPip::Organic.frame(), 0x0D);
        assert_eq!(SelfHealPip::Units.frame(), 0x14);
        assert_eq!(SelfHealPip::Organic.offset(), (19.0, -35.0));
        assert_eq!(SelfHealPip::Units.offset(), (38.0, -32.0));
    }
}
