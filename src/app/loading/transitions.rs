//! App state transitions: map loading into InGame, screen clearing.
//!
//! Extracted from app.rs for file-size limits.

use std::collections::{BTreeMap, HashMap};

use crate::app::loading::init;
use crate::app::match_runtime::sim_tick;
use crate::app::presentation::render;
use crate::map::basic::BasicSection;
use crate::map::houses::HouseRoster;
use crate::map::trigger_graph::TriggerGraph;
use crate::render::minimap::MinimapRenderer;
use crate::render::selection_overlay::SelectionOverlay;
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::ui::game_screen::GameScreen;
use crate::ui::sidebar::SidebarTab;

use crate::app::AppState;

/// Background clear color for menu screens (dark blue).
const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.02,
    g: 0.04,
    b: 0.12,
    a: 1.0,
};

/// Mirror the authoritative speed whenever a complete Simulation replaces the
/// app's live match, including both fresh-map handoff and in-scenario load.
pub(crate) fn sync_in_game_options_speed_from_sim(state: &mut AppState) {
    let Some(game_speed) = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)
        .and_then(crate::sim::world::Simulation::projected_in_game_options_speed)
        .map(u32::from)
    else {
        return;
    };
    state
        .match_state
        .match_presentation
        .in_game_options
        .game_speed = game_speed;
    state.match_state.sim_speed_tps = crate::app::types::tps_for_game_speed(game_speed);
}

pub(crate) fn fallback_map_load_result() -> init::MapLoadResult {
    init::MapLoadResult {
        scenario: init::ScenarioLoadInputs {
            startup: crate::match_bootstrap::LoadingStartup::Generic {
                selected_map_file: "fallback".to_string(),
            },
            map_source: crate::map::source::LoadedMapSource::LegacyFallback {
                label: "fallback".to_string(),
            },
            map_hash: None,
            basic: BasicSection::default(),
            terrain_grid: None,
            resolved_terrain: None,
            simulation: None,
            overlays: Vec::new(),
            terrain_objects: Vec::new(),
            waypoints: HashMap::new(),
            triggers: HashMap::new(),
            events: HashMap::new(),
            actions: HashMap::new(),
            trigger_graph: TriggerGraph::default(),
            overlay_registry: OverlayTypeRegistry::empty(),
            house_roster: HouseRoster::default(),
            height_map: BTreeMap::new(),
            bridge_height_map: BTreeMap::new(),
            rules: None,
            map_lighting_config: crate::map::lighting::LightingConfig::default(),
            theater_name: "TEMPERATE".to_string(),
            theater_ext: "tem".to_string(),
            initial_local_owner: None,
            sandbox_full_visibility: false,
            camera_anchor_x: 0.0,
            camera_anchor_y: 0.0,
        },
        presentation: init::PresentationLoadAssets {
            tile_atlas: None,
            building_zshape: None,
            unit_atlas: None,
            palette_set: None,
            sprite_atlas: None,
            overlay_atlas: None,
            bridge_atlas: None,
            bridge_railing_atlas: None,
            sidebar_cameo_atlas: None,
            sidebar_chrome: None,
            software_cursor: None,
            overlay_names: BTreeMap::new(),
            overlay_radar_colors: HashMap::new(),
            house_color_map: HashMap::new(),
            lighting_grid: crate::map::lighting::CellLightGrid::new(),
            csf: None,
            fnt_file: None,
        },
        asset_manager: None,
    }
}

/// App-only projections transferred with a new runtime. This bundle has no
/// gameplay authority: its source values belong to the loaded map/resources.
pub(crate) struct MapPresentationInputs {
    pub(crate) basic: BasicSection,
    pub(crate) source: crate::map::source::LoadedMapSource,
    pub(crate) map_hash: Option<u64>,
    pub(crate) terrain_grid: Option<crate::map::terrain::TerrainGrid>,
    pub(crate) overlays: Vec<crate::map::overlay::OverlayEntry>,
    pub(crate) terrain_objects: Vec<crate::map::overlay::TerrainObject>,
    pub(crate) house_roster: HouseRoster,
    pub(crate) lighting_config: crate::map::lighting::LightingConfig,
    pub(crate) theater_name: String,
    pub(crate) theater_ext: String,
    pub(crate) height_map: BTreeMap<(u16, u16), u8>,
    pub(crate) bridge_height_map: BTreeMap<(u16, u16), u8>,
}

/// Install the complete map presentation after its runtime and process assets
/// are available. Fresh startup and saved restoration share this replacement;
/// route-specific input, sidebar order, waterlines and timers stay outside it.
/// Missing map-owned surfaces replace the old ones with absence, so a different
/// map can never inherit the outgoing geometry or shroud dimensions.
pub(crate) fn install_world_presentation(
    state: &mut AppState,
    map: MapPresentationInputs,
    assets: init::PresentationLoadAssets,
) {
    state.match_state.map_basic = map.basic;
    state.match_state.loaded_map_source = Some(map.source);
    state.match_state.loaded_map_hash = map.map_hash;
    state.match_state.match_presentation.terrain_grid = map.terrain_grid;
    state.match_state.match_presentation.height_map = map.height_map;
    state.match_state.match_presentation.bridge_height_map = map.bridge_height_map;
    state
        .match_state
        .match_presentation
        .installed_playfield_authority = None;
    state.frontend.shell_preview_overlay_registry = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|runtime| runtime.resources.overlay_registry.clone());

    state.match_state.match_presentation.combat_lights.clear();
    state.match_state.match_presentation.lasers.clear_on_load();
    state
        .match_state
        .match_presentation
        .electric_bolts
        .clear_on_load();
    if let Some(runtime) = state.match_state.sim_runtime.as_ref() {
        state
            .match_state
            .match_presentation
            .detail
            .borrow_mut()
            .configure_normal(&runtime.resources.rules.general.detail);
    }
    state
        .match_state
        .match_presentation
        .barrel_image_pitches
        .borrow_mut()
        .clear();
    state
        .match_state
        .match_presentation
        .line_trails
        .clear_on_load();
    state
        .match_state
        .match_presentation
        .hidden_super_anims
        .clear();
    state
        .match_state
        .match_presentation
        .super_timer_blinks
        .clear();
    sync_in_game_options_speed_from_sim(state);

    state.match_state.match_presentation.tile_atlas = assets.tile_atlas;
    state.match_state.match_presentation.building_zshape = assets.building_zshape;
    state.match_state.match_presentation.unit_atlas = assets.unit_atlas;
    // Both voxel caches resolve model images through the replaced RuleSet.
    *state.renderer.vxl_slope_transition_cache.borrow_mut() = Default::default();
    *state.renderer.vxl_pose_frame_cache.borrow_mut() = Default::default();
    state.match_state.match_presentation.palette_set = assets.palette_set;
    state.match_state.match_presentation.sprite_atlas = assets.sprite_atlas;
    state.match_state.match_presentation.overlay_atlas = assets.overlay_atlas;
    state.match_state.match_presentation.bridge_atlas = assets.bridge_atlas;
    state.match_state.match_presentation.bridge_railing_atlas = assets.bridge_railing_atlas;
    state.match_state.match_presentation.sidebar_cameo_atlas = assets.sidebar_cameo_atlas;
    state.match_state.match_presentation.sidebar_chrome = assets.sidebar_chrome;
    // GAME.FNT is process artwork, so retain the established process fallback
    // when optional font preparation did not produce a replacement.
    if let Some(ref fnt) = assets.fnt_file {
        state.renderer.bit_font = crate::render::bit_font::BitFont::from_fnt(
            &state.renderer.gpu,
            &state.renderer.batch_renderer,
            fnt,
        );
    }

    // The final sidebar projection resolves incoming side/radar authority.
    state.match_state.match_presentation.radar_anim = None;
    state.match_state.match_presentation.radar_animation_source = None;
    state.match_state.match_presentation.radar_content_insets = None;
    state.match_state.match_presentation.has_radar = false;
    state.match_state.match_presentation.software_cursor = assets.software_cursor;
    state
        .match_state
        .match_presentation
        .overlays
        .replace_from_source(map.overlays);
    state.match_state.match_presentation.terrain_objects = map.terrain_objects;
    state.match_state.match_presentation.overlay_names = assets.overlay_names;
    state.match_state.match_presentation.overlay_radar_colors = assets.overlay_radar_colors;
    state.match_state.match_presentation.house_color_map = assets.house_color_map;
    state.match_state.match_presentation.house_roster = map.house_roster;
    state.process_assets.csf = assets.csf;
    state.match_state.match_presentation.theater_name = map.theater_name;
    state.match_state.match_presentation.theater_ext = map.theater_ext;

    let live = state.match_state.sim_runtime.as_ref().and_then(|runtime| {
        runtime
            .view()
            .resolved_terrain()
            .map(|terrain| (terrain, &runtime.simulation, &runtime.resources.rules))
    });
    let presentation = &mut state.match_state.match_presentation;
    presentation.lighting.install(
        assets.lighting_grid,
        map.lighting_config,
        presentation.in_game_options.detail_level,
        live,
    );
    state.platform.window.set_cursor_visible(
        state
            .match_state
            .match_presentation
            .software_cursor
            .is_none(),
    );

    // Current MapClass authority, including restored/triggered LocalSize,
    // reaches camera and minimap before either route positions its view.
    let (playfield_bounds, playfield_revision) =
        crate::app::input::camera::sync_playfield_presentation_bounds(state);
    let minimap = state
        .match_state
        .match_presentation
        .terrain_grid
        .as_ref()
        .map(|grid| {
            let runtime = state.match_state.sim_runtime.as_ref();
            let resolved_terrain = runtime.and_then(|runtime| runtime.view().resolved_terrain());
            let overlay_registry = runtime.map(|runtime| &runtime.resources.overlay_registry);
            let rules = runtime.map(|runtime| &runtime.resources.rules);
            let overlay_data = build_minimap_overlay_data(
                state.match_state.match_presentation.overlays.as_slice(),
                &state.match_state.match_presentation.terrain_objects,
                overlay_registry,
                rules,
            );
            MinimapRenderer::new(
                &state.renderer.gpu,
                &state.renderer.batch_renderer,
                grid,
                resolved_terrain,
                &overlay_data,
                &state.match_state.match_presentation.overlay_radar_colors,
                &state.match_state.match_presentation.theater_name,
                playfield_bounds,
                playfield_revision,
            )
        });
    state.match_state.match_presentation.minimap = minimap;
    state.match_state.match_presentation.selection_overlay = Some(SelectionOverlay::new(
        &state.renderer.gpu,
        &state.renderer.batch_renderer,
        state.process_assets.manager(),
    ));
    let shroud_buffer = state.process_assets.manager().and_then(|assets| {
        let grid = state
            .match_state
            .sim_runtime
            .as_ref()?
            .simulation
            .path_grid()?;
        let shp =
            crate::assets::shp_file::ShpFile::from_bytes(assets.get_ref("shroud.shp")?).ok()?;
        let (frame_pixels, cw, ch) = crate::render::shroud_buffer::extract_shp_brightness(&shp);
        Some(crate::render::shroud_buffer::ShroudBuffer::new(
            &state.renderer.gpu,
            state.render_width(),
            state.render_height(),
            grid.width(),
            grid.height(),
            frame_pixels,
            cw,
            ch,
            crate::render::shroud_buffer::SHROUD_EDGE_LUT,
        ))
    });
    state.match_state.match_presentation.shroud_buffer = shroud_buffer;
}

pub(crate) fn apply_map_load_result(state: &mut AppState, result: init::MapLoadResult) {
    crate::app::input::dispatch::selection_navigation::reset_for_world_replacement(
        &mut state.match_state.input,
    );
    // Start_Scenario 00683D21 completes Read_Scenario before switching video
    // mode at 00683DF3. Tactical installation uses the applied game dimensions.
    crate::app::App::enter_game_window_mode(state);
    crate::app::loading::pump::present_game_mode_blank(state);
    // Only fresh startup cancels its outgoing scenario-owned request. Saved
    // loading preserves the process Theme state until ThemeLoad721040.
    state.audio.cancel_scenario_theme_request();
    crate::app::reset_scenario_exit_runtime(state);
    let startup = result.scenario.startup;
    let returns_scenario_rng_to_offline_shell = startup.launch_session().is_some();
    state.match_state.match_presentation.local_player_handle = startup
        .launch_session()
        .map(|launch| launch.player_name.clone());
    state.match_state.scenario_elapsed_clock.reset();
    crate::app::loading::pump::clear_loading_state(state);

    // Close the outgoing diagnostic timeline before replacing its runtime.
    crate::app::match_runtime::sim_tick::close_replay_segment_for_new_timeline(state);
    state.match_state.sim_runtime =
        result
            .scenario
            .simulation
            .zip(result.scenario.rules)
            .map(|(simulation, rules)| crate::sim::runtime::SimRuntime {
                simulation,
                resources: crate::sim::runtime::SimResources {
                    overlay_registry: result.scenario.overlay_registry,
                    terrain_template: result.scenario.resolved_terrain,
                    rules,
                    trigger_graph: result.scenario.trigger_graph,
                    triggers: result.scenario.triggers,
                    events: result.scenario.events,
                    actions: result.scenario.actions,
                    waypoints: result.scenario.waypoints,
                },
            });
    // Only an actually-carried lease returns. A fallback result must not wipe
    // the process-sticky manager already restored by its failure path.
    if let Some(manager) = result.asset_manager {
        state.process_assets.return_from_loading(manager);
    }
    install_world_presentation(
        state,
        MapPresentationInputs {
            basic: result.scenario.basic,
            source: result.scenario.map_source,
            map_hash: result.scenario.map_hash,
            terrain_grid: result.scenario.terrain_grid,
            overlays: result.scenario.overlays,
            terrain_objects: result.scenario.terrain_objects,
            house_roster: result.scenario.house_roster,
            lighting_config: result.scenario.map_lighting_config,
            theater_name: result.scenario.theater_name,
            theater_ext: result.scenario.theater_ext,
            height_map: result.scenario.height_map,
            bridge_height_map: result.scenario.bridge_height_map,
        },
        result.presentation,
    );
    state
        .match_state
        .match_presentation
        .sinking_waterlines
        .borrow_mut()
        .clear();
    // Fresh SidebarClass admission silently seeds its first projection.
    state
        .match_state
        .match_presentation
        .sidebar_projection
        .reset_cameo_seed();
    if let Some(runtime) = state.match_state.sim_runtime.as_mut() {
        runtime
            .simulation
            .set_input_delay_ticks(state.match_state.configured_input_delay_ticks);
    }

    let (tactical_width, tactical_height) = crate::app::input::camera::tactical_viewport_size_px(
        state.render_width(),
        state.render_height(),
    );
    let (camera_x, camera_y) = crate::app::input::camera::tactical_camera_top_left(
        (
            result.scenario.camera_anchor_x,
            result.scenario.camera_anchor_y,
        ),
        tactical_width as f32,
        tactical_height as f32,
        state.match_state.input.zoom_level,
    );
    crate::app::input::camera::set_camera_position(
        &mut state.match_state.input,
        (camera_x, camera_y),
    );
    // Native fresh scenario reading initializes every bookmark to the opening view.
    crate::app::input::camera::seed_view_bookmarks_from_current_view(state);
    state.match_state.input.targeting_mode = None;
    state.match_state.input.building_placement_preview = None;
    state.match_state.match_presentation.active_sidebar_tab = SidebarTab::default_active_tab();
    state.match_state.match_presentation.message_list = crate::ui::messages::MessageList::new(
        3,
        0,
        crate::ui::messages::MESSAGE_MAX_VISIBLE_RETAIL,
        0,
    );
    state.match_state.match_presentation.message_clock =
        crate::ui::messages::PauseAwareClock::default();
    state.match_state.input.minimap_dragging = false;
    state.match_state.input.tactical_mouse = Default::default();
    state.match_state.input.keys_held.clear();
    state.match_state.input.cursor_x = tactical_width as f32 * 0.5;
    state.match_state.input.cursor_y = tactical_height as f32 * 0.5;
    state.platform.frame_pacer.reset_for_immediate_frame();
    state.match_state.input.queued_order_mode = render::OrderMode::Move;
    for group in &mut state.match_state.input.control_groups {
        group.clear();
    }
    state.match_state.local_owner_override = result.scenario.initial_local_owner;
    state.match_state.match_audio.reset_for_new_match();
    state.match_state.sandbox_full_visibility = result.scenario.sandbox_full_visibility;

    // `Start_Scenario @ 0x00683AB0` after `Read_Scenario`: `[Basic] Theme`
    // resolves through `From_Name` (`0x004758F0`); -1 -> `Stop(fade=1)` of the
    // LOADING stream, else `Queue_Song(index)`. `Main_Tick` then issues
    // `Queue_Song(-2)` and the audio pump's AI picks the first allowed track
    // once the fade lands. Theme AI borrows the installed process Main cursor;
    // `Is_Allowed`'s `Side=` gate compares the local player's side.
    let music_now_ms = sim_tick::monotonic_frame_pacer_ms(state, std::time::Instant::now());
    crate::app::audio_runtime::bind_match_theme_context(state);
    if let Some(assets) = state.process_assets.manager() {
        state.audio.request_scenario_theme(
            state.match_state.map_basic.theme.as_deref(),
            assets,
            music_now_ms,
        );
    }

    match startup {
        crate::match_bootstrap::LoadingStartup::Accepted(prepared) => {
            let receipt = state.match_state.startup.acknowledge(
                prepared,
                state
                    .match_state
                    .sim_runtime
                    .as_ref()
                    .map(|rt| &rt.simulation),
                matches!(state.frontend.screen, GameScreen::Loading),
            );

            match receipt {
                Ok(()) => {
                    let now_ms =
                        sim_tick::monotonic_frame_pacer_ms(state, std::time::Instant::now());
                    state.match_state.scenario_elapsed_clock.start(now_ms);
                    state.frontend.screen = GameScreen::InGame;
                    state
                        .frontend
                        .offline_skirmish_runtime
                        .mark_gameplay_rng_return_pending();
                    log::info!("Transitioned to InGame after Rust L0 acknowledgement");
                }
                Err(err) => {
                    state.match_state.startup.clear();
                    state.frontend.screen = GameScreen::MissionResult {
                        title: "Startup Rejected".to_string(),
                        detail: err.clone(),
                    };
                    log::error!("Accepted startup failed closed at Rust L0: {err}");
                }
            }
        }
        crate::match_bootstrap::LoadingStartup::UnverifiedLegacy { .. }
        | crate::match_bootstrap::LoadingStartup::Generic { .. } => {
            state.match_state.startup.clear();
            let now_ms = sim_tick::monotonic_frame_pacer_ms(state, std::time::Instant::now());
            state.match_state.scenario_elapsed_clock.start(now_ms);
            state.frontend.screen = GameScreen::InGame;
            if returns_scenario_rng_to_offline_shell {
                state
                    .frontend
                    .offline_skirmish_runtime
                    .mark_gameplay_rng_return_pending();
            }
            log::info!("Transitioned to InGame on noncertifying startup path");
        }
    }
    crate::app::presentation::sidebar_render::refresh_sidebar_projection(state);
}

/// Build overlay classification data for the minimap from map overlay entries.
///
/// Carries parsed native overlay flags/IDs and the current OverlayData frame;
/// only the Ore-v-Gem display label uses the stock three-character name family.
/// `CellClass::GetRadarColor` source selection does not consume that label.
pub(crate) fn build_minimap_overlay_data(
    overlays: &[crate::map::overlay::OverlayEntry],
    terrain_objects: &[crate::map::overlay::TerrainObject],
    overlay_registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
    rules: Option<&crate::rules::ruleset::RuleSet>,
) -> Vec<crate::render::minimap::MinimapOverlayDatum> {
    use crate::render::minimap::{
        MinimapCellRadarSource, MinimapOverlayDatum, OverlayClassification, minimap_overlay_datum,
    };

    let mut data = Vec::with_capacity(overlays.len() + terrain_objects.len());

    for entry in overlays {
        data.push(minimap_overlay_datum(
            entry.rx,
            entry.ry,
            entry.overlay_id,
            entry.frame,
            overlay_registry,
            rules,
        ));
    }

    for obj in terrain_objects {
        data.push(MinimapOverlayDatum {
            rx: obj.rx,
            ry: obj.ry,
            classification: OverlayClassification::TerrainObject,
            source: MinimapCellRadarSource::TerrainObject,
        });
    }

    data
}

/// Clear the screen to the background color (no depth buffer).
pub(crate) fn clear_screen(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Clear Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
}

#[cfg(test)]
mod game_speed_tests {
    use crate::sim::command::{Command, CommandEnvelope};
    use crate::sim::house_state::HouseState;
    use crate::sim::world::Simulation;

    #[test]
    fn loaded_lobby_speed_projection_includes_due_transition() {
        let mut sim = Simulation::new();
        let owner = sim.interner.intern("Local");
        sim.houses
            .insert(owner, HouseState::new(owner, 0, None, true, 0, 10));
        sim.session.house_order.push(owner);
        sim.queue_command(CommandEnvelope::new(
            owner,
            1,
            Command::SetGameSpeed { speed: 4 },
        ));

        assert_eq!(sim.projected_in_game_options_speed(), Some(4));
    }
}
