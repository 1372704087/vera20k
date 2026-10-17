//! Telemetry snapshot capture for the browser dashboard.
//!
//! Assembles the whole wire payload from live app state: the simulation-side
//! capture (`Simulation::telemetry_snapshot`) plus the app-only facts —
//! frame timing from the timing ring, and the debug-panel port of the former
//! egui Terrain Debug / Event History panels (cursor cell, walkability,
//! entities at the cursor, selected movers' paths, miner state and per-entity
//! event logs).
//!
//! Dependency rules: part of the app layer; reads state, mutates nothing.

use crate::app::AppState;
use crate::app::types::SIM_TICK_MS;
use crate::map::entities::EntityCategory;
use crate::rules::locomotor_type::SpeedType;
use crate::sim::debug_event_log::{DebugEvent, DebugEventKind};
use crate::sim::movement::motion_query;
use crate::sim::telemetry::{
    DebugEventEntry, DebugPanel, EntityDesc, LandRow, MinerDebug, PathStep, ProcessStats,
    SelectedDebug, TelemetrySnapshot,
};
use crate::sim::world::Simulation;

/// Events retained per selected entity — the egui panel's cap.
const MAX_EVENTS: usize = 30;
/// Path preview steps — the egui panel's cap.
const MAX_PATH_PREVIEW: usize = 8;

/// Capture one full dashboard snapshot, or `None` when no match is running.
pub fn capture_snapshot(state: &AppState) -> Option<TelemetrySnapshot> {
    let runtime = state.match_state.sim_runtime.as_ref()?;
    let mut snapshot = runtime.simulation.telemetry_snapshot();
    // House colours: the same `[Colors]` scheme map the renderer tints units
    // with, so the dashboard shows each player's real team colour instead of a
    // dashboard-invented palette.
    let schemes = &runtime.resources.rules.color_schemes;
    let house_colors = &state.match_state.match_presentation.house_color_map;
    for house in &mut snapshot.houses {
        house.color = house_colors
            .get(&house.name)
            .map(|index| crate::rules::house_colors::house_text_rgb(schemes, *index));
        // The side's own `[Sides]` key — the index is just its position there.
        house.side_name = runtime
            .resources
            .rules
            .side_name(crate::rules::ruleset::SideIdx(house.side_index))
            .map(str::to_string);
    }
    let mut timing = state
        .match_state
        .telemetry_timing
        .summarize(SIM_TICK_MS as f32);
    // Each rate keeps its own owner: FPS is the render loop's, tick_hz is the
    // committed-tick meter's.
    timing.fps = state.diag.frame_timer.fps();
    timing.tick_hz = state
        .match_state
        .telemetry_tick_rate
        .rate(std::time::Instant::now());
    snapshot.timing = Some(timing);
    snapshot.debug = Some(capture_debug(state, &runtime.simulation));
    snapshot.process = Some(capture_process(state));
    // The running split, read from the same accumulator the end-of-match
    // report freezes: mean per frame since this match started.
    snapshot.profile = runtime.simulation.frame_profile().report();
    // Render phases are process wall time, so they come from the app's meter
    // rather than the simulation's profile.
    snapshot.render = state.diag.render_timing.report();
    // Log records since the previous snapshot. A map load can emit thousands a
    // second; the newest win, and the rest are counted as dropped so the view
    // never silently claims to be complete.
    let subscribed = state
        .diag
        .telemetry_server
        .as_ref()
        .is_none_or(|server| server.has_clients());
    if subscribed && let Some(ring) = crate::util::logging::ring() {
        const MAX_PER_SNAPSHOT: usize = 300;
        let (lines, evicted) = ring.take_since_last(MAX_PER_SNAPSHOT);
        snapshot.logs = Some(crate::sim::telemetry::LogBatch {
            dropped: evicted,
            lines: lines
                .into_iter()
                .map(|line| crate::sim::telemetry::LogRecord {
                    seq: line.seq,
                    level: line.level,
                    target: line.target,
                    message: line.message,
                    t_ms: line.t_ms,
                })
                .collect(),
        });
    }
    Some(snapshot)
}

/// Process-level resources for the dashboard: the OS counters, plus the
/// adapter identity and the palette-index texture pages this process keeps
/// resident. Pure observation — nothing here touches simulation state.
fn capture_process(state: &AppState) -> ProcessStats {
    let mut stats = state.diag.process_stats.sample();
    stats.gpu_adapter = Some(
        state
            .renderer
            .gpu
            .capture_adapter_observation()
            .name
            .to_string(),
    );
    stats.gpu_texture_mb = resident_texture_mib(state);
    stats
}

/// Resident palette-index texture bytes, in MiB.
///
/// Both atlases page palette-index (`R8Uint`, one byte per pixel) textures, so
/// a page's footprint is exactly its pixel count. This is deliberately the
/// renderer's own accounting and not a driver query: it is exact for what we
/// allocate and needs no vendor API.
/// Resident palette-index texture bytes, in MiB.
///
/// Both atlases page palette-index (`R8Uint`, one byte per pixel) textures, so
/// a page's footprint is exactly its pixel count. This is deliberately the
/// renderer's own accounting and not a driver query: it is exact for what we
/// allocate and needs no vendor API.
fn resident_texture_mib(state: &AppState) -> f64 {
    const BYTES_PER_PIXEL: f64 = 1.0;
    let pixels = (sprite_atlas_bytes(state) + unit_atlas_bytes(state)) as f64;
    pixels * BYTES_PER_PIXEL / (1024.0 * 1024.0)
}

fn sprite_atlas_bytes(state: &AppState) -> u64 {
    state
        .match_state
        .match_presentation
        .sprite_atlas
        .as_ref()
        .map(|atlas| {
            atlas
                .pages
                .iter()
                .map(|page| u64::from(page.texture.width) * u64::from(page.texture.height))
                .sum::<u64>()
        })
        .unwrap_or(0)
}

fn unit_atlas_bytes(state: &AppState) -> u64 {
    state
        .match_state
        .match_presentation
        .unit_atlas
        .as_ref()
        .map(|atlas| {
            atlas
                .pages
                .iter()
                .map(|page| u64::from(page.texture.width) * u64::from(page.texture.height))
                .sum::<u64>()
        })
        .unwrap_or(0)
}

/// Capture and publish one snapshot outside the tick-driven cadence. Used for
/// updates that must be visible immediately: browser command echoes and frame
/// steps (which commit a tick the `tick % 4` publish gate would otherwise
/// skip, and while paused no later tick would publish it).
pub fn publish_now(state: &AppState) {
    let Some(server) = state.diag.telemetry_server.as_ref() else {
        return;
    };
    let Some(snapshot) = capture_snapshot(state) else {
        return;
    };
    match serde_json::to_string(&snapshot) {
        Ok(json) => server.publish(json),
        Err(error) => log::warn!("telemetry publish failed: {error}"),
    }
}

/// The egui Terrain Debug panel's data, read from live cursor and selection.
fn capture_debug(state: &AppState, sim: &Simulation) -> DebugPanel {
    let (cursor_rx, cursor_ry) = crate::app::match_runtime::sim_tick::screen_point_to_world_cell(
        state,
        state.match_state.input.cursor_x,
        state.match_state.input.cursor_y,
    );
    let path_grid = sim.path_grid();
    let active_speed_type =
        crate::app::diagnostics::debug_overlays::resolve_debug_speed_type(state);

    let cursor_elevation = state
        .height_map()
        .get(&(cursor_rx, cursor_ry))
        .copied()
        .unwrap_or(0);
    let cursor_walkable = path_grid.map(|grid| grid.is_walkable(cursor_rx, cursor_ry));

    let land_rows = sim
        .resolved_terrain
        .as_ref()
        .and_then(|terrain| terrain.cell(cursor_rx, cursor_ry))
        .map(|cell| {
            [
                (SpeedType::Foot, "Foot"),
                (SpeedType::Track, "Track"),
                (SpeedType::Wheel, "Wheel"),
                (SpeedType::Float, "Float"),
                (SpeedType::Amphibious, "Amphibious"),
            ]
            .into_iter()
            .map(|(speed_type, name)| LandRow {
                speed_type: name.to_string(),
                cost: crate::app::diagnostics::debug_overlays::land_row(cell, speed_type),
            })
            .collect()
        })
        .unwrap_or_default();

    let mut entities_at_cursor: Vec<EntityDesc> = Vec::new();
    let mut footprint: Option<String> = None;
    for entity in sim.entities().values() {
        if entity.position.rx() != cursor_rx || entity.position.ry() != cursor_ry {
            continue;
        }
        entities_at_cursor.push(EntityDesc {
            type_name: sim.interner.resolve(entity.type_ref()).to_string(),
            category: category_label(entity.category()).to_string(),
            owner: sim.interner.resolve(entity.owner()).to_string(),
            hp: entity.health.current(),
            max_hp: entity_max_hp(state, sim, entity),
        });
    }
    if let Some(rules) = state.rules() {
        for entity in sim.entities().values() {
            if entity.category() != EntityCategory::Structure {
                continue;
            }
            let foundation = sim
                .interner
                .object_type(entity.type_ref(), rules)
                .map(|obj| obj.foundation.clone())
                .unwrap_or_else(|| "1x1".to_string());
            let (fw, fh) = crate::rules::foundation::foundation_dimensions(&foundation);
            let (ex, ey) = (entity.position.rx(), entity.position.ry());
            if cursor_rx >= ex && cursor_rx < ex + fw && cursor_ry >= ey && cursor_ry < ey + fh {
                footprint = Some(format!(
                    "{} ({}) @ ({},{}) {}",
                    sim.interner.resolve(entity.type_ref()),
                    foundation,
                    ex,
                    ey,
                    sim.interner.resolve(entity.owner())
                ));
                break;
            }
        }
    }

    let selected: Vec<SelectedDebug> = sim
        .entities()
        .values()
        .filter(|entity| entity.selection.is_selected())
        .map(|entity| capture_selected(sim, entity, path_grid))
        .collect();

    DebugPanel {
        cursor_cell: Some((cursor_rx, cursor_ry)),
        cursor_elevation,
        grid: path_grid.map(|grid| (grid.width(), grid.height())),
        cursor_walkable,
        land_rows,
        active_speed_type: active_speed_type.name().to_string(),
        vxl_sprites: state
            .match_state
            .match_presentation
            .unit_atlas
            .as_ref()
            .map(|atlas| atlas.rendered as u32),
        entities_at_cursor,
        footprint,
        selected,
        overlays: crate::sim::telemetry::OverlayFlags {
            terrain_costs: state.diag.debug_show_pathgrid,
            heightmap: state.diag.debug_show_heightmap,
            cell_grid: state.diag.debug_show_cell_grid,
            fog: !state.match_state.sandbox_full_visibility,
            unit_inspector: state.diag.debug_unit_inspector,
            foot_path: state.diag.debug_show_foot_path,
            paused: state.match_state.paused(),
        },
    }
}

/// The egui panel's short category labels.
fn category_label(category: EntityCategory) -> &'static str {
    match category {
        EntityCategory::Unit => "Unit",
        EntityCategory::Infantry => "Inf",
        EntityCategory::Structure => "Bld",
        EntityCategory::Aircraft => "Air",
    }
}

/// Strength from the entity's object type, or 0 when the rules are not bound.
fn entity_max_hp(
    state: &AppState,
    sim: &Simulation,
    entity: &crate::sim::game_entity::GameEntity,
) -> i32 {
    state
        .rules()
        .and_then(|rules| sim.interner.object_type(entity.type_ref(), rules))
        .map_or(0, |obj| obj.strength)
}

/// One selected entity: identity, position, remaining path, miner state and
/// its event log — the egui panel's per-selection block.
fn capture_selected(
    sim: &Simulation,
    entity: &crate::sim::game_entity::GameEntity,
    path_grid: Option<&crate::sim::pathfinding::PathGrid>,
) -> SelectedDebug {
    let moving = motion_query::is_moving(entity) == Some(true);
    let (path_steps_left, path_blocked, goal, path_preview) = if moving {
        let path: Vec<(u16, u16)> = entity
            .navigation
            .path_replay
            .remaining_cells_from((entity.position.rx() as i16, entity.position.ry() as i16))
            .into_iter()
            .map(|(x, y)| (x as u16, y as u16))
            .collect();
        let preview = path
            .iter()
            .take(MAX_PATH_PREVIEW)
            .enumerate()
            .map(|(index, &(px, py))| PathStep {
                cell: (px, py),
                walkable: path_grid.is_none_or(|grid| grid.is_walkable(px, py)),
                first: index == 0,
            })
            .collect();
        (
            path.len() as u32,
            entity.navigation.path_runtime.path_blocked(),
            crate::sim::movement::movement_goal_cell(entity),
            preview,
        )
    } else {
        (0, false, None, Vec::new())
    };

    let miner = entity.miner.as_ref().map(|miner| MinerDebug {
        kind: format!("{miner:?}"),
        state: format!(
            "{:?}",
            entity
                .miner_state()
                .unwrap_or(crate::sim::miner::MinerState::SearchOre)
        ),
        cargo: miner.cargo().len() as u32,
        capacity: u32::from(miner.capacity_bales()),
        stage: entity.native_stage().value(),
        rate: entity.native_stage().rate(),
        harvesting: miner.harvesting(),
        archived_ore: match entity.archive_target() {
            Some(crate::sim::combat::TargetKind::Cell(x, y)) => Some((x, y)),
            _ => None,
        },
        teleport_active: entity.teleport_state().is_some(),
    });

    let events = entity
        .debug_log()
        .map(|log| {
            log.events()
                .iter()
                .rev()
                .take(MAX_EVENTS)
                .map(format_debug_event)
                .collect()
        })
        .unwrap_or_default();

    SelectedDebug {
        stable_id: entity.stable_id(),
        type_name: sim.interner.resolve(entity.type_ref()).to_string(),
        cell: (entity.position.rx(), entity.position.ry()),
        sub: (
            entity.position.sub_x().to_num::<f32>().clamp(0.0, 255.0) as u16,
            entity.position.sub_y().to_num::<f32>().clamp(0.0, 255.0) as u16,
        ),
        moving,
        path_steps_left,
        path_blocked,
        goal,
        path_preview,
        miner,
        inspector_active: entity.debug_log().is_some(),
        events,
    }
}

/// Format a debug event as plain text — the egui Event History panel's
/// strings without its light-theme colours.
fn format_debug_event(event: &DebugEvent) -> DebugEventEntry {
    let tick = event.tick;
    let text = match &event.kind {
        DebugEventKind::PhaseChange { from, to, reason } => {
            format!("[{tick}] {from} \u{2192} {to} ({reason})")
        }
        DebugEventKind::Repath {
            reason,
            new_path_len,
        } => format!("[{tick}] Repath: {reason} ({new_path_len} steps)"),
        DebugEventKind::Blocked { by_entity, cell } => format!(
            "[{tick}] Blocked at ({},{}){}",
            cell.0,
            cell.1,
            by_entity.map(|id| format!(" by #{id}")).unwrap_or_default()
        ),
        DebugEventKind::StuckAbort { blocked_ticks } => {
            format!("[{tick}] STUCK ABORT after {blocked_ticks} ticks")
        }
        DebugEventKind::PathSegmentComplete { final_goal } => format!(
            "[{tick}] Path segment complete{}",
            final_goal
                .map(|(x, y)| format!(", repathing to ({x},{y})"))
                .unwrap_or_default()
        ),
        DebugEventKind::MinerStateChange { from, to } => {
            format!("[{tick}] Miner: {from} \u{2192} {to}")
        }
        DebugEventKind::SpecialMovementStart { kind } => {
            format!("[{tick}] {kind} started")
        }
        DebugEventKind::SpecialMovementPhase { phase } => {
            format!("[{tick}] \u{2192} {phase}")
        }
        DebugEventKind::SpecialMovementEnd => {
            format!("[{tick}] Special movement ended")
        }
        DebugEventKind::LocomotorOverride { kind, active } => format!(
            "[{tick}] Override {}: {kind}",
            if *active { "ON" } else { "OFF" }
        ),
    };
    DebugEventEntry { tick, text }
}
