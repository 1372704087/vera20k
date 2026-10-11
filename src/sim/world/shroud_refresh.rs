//! FootAI4DA692..4DA7AA: admitted high-flying sight refresh before Process.
//! Per-viewer clocks preserve the original local-player admission boundary;
//! House/view-cache reconciliation never advances these clocks or emits events.
use super::{Simulation, UninitContext};
use crate::map::entities::EntityCategory;
use crate::rules::ruleset::RuleSet;
use crate::sim::{pathfinding::PathGrid, vision};

#[cfg(test)]
#[path = "shroud_refresh_tests.rs"]
mod tests;

#[cfg(test)]
#[derive(Default)]
struct RevealCalls {
    walks: Vec<Vec<(u16, u16)>>,
    discovery_queries: std::collections::BTreeSet<(u16, u16)>,
}

#[cfg(test)]
thread_local! {
    static REVEAL_CALLS: std::cell::RefCell<Option<RevealCalls>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn trace_reveal_calls<T>(run: impl FnOnce() -> T) -> (T, RevealCalls) {
    REVEAL_CALLS.with_borrow_mut(|trace| {
        assert!(trace.is_none(), "nested RevealArea traces");
        *trace = Some(RevealCalls::default());
    });
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            REVEAL_CALLS.with_borrow_mut(|trace| *trace = None);
        }
    }
    let reset = Reset;
    let result = run();
    let calls = REVEAL_CALLS.with_borrow_mut(|trace| trace.take().unwrap());
    drop(reset);
    (result, calls)
}

impl Simulation {
    /// Original `TActionClass::RevealAroundWaypoint 0x006E0FE0`, called by
    /// Execute17 at6DE34E. This uses TAction+90 and PlayerPtr, independently
    /// of the trigger's House and the trailing+44 camera waypoint. The
    /// signed level includes Map5657A0's short-circuit bridge100/400 gate;
    /// the truncating half-level subtraction wraps as a signed WORD before
    /// RevealArea5678E0 applies its own Z shift. Executed controls/readers:
    /// tools/spatial_oracle/trigger_action_values.{py,json,meta.json}.
    /// Ledger: no RNG draws, timer writes or detach calls. A negative radius
    /// natively indexes before the count table; VERA maps no cells (retail9).
    pub(crate) fn trigger_reveal_at_waypoint(
        &mut self,
        index: i32,
        context: UninitContext<'_>,
    ) -> bool {
        let Some(rules) = context.rules() else {
            return false;
        };
        let Some(viewer) = self.session.current_house else {
            return false;
        };
        if self.fog.whole_map_revealed_for_owner(viewer) {
            return true;
        }
        let Some(cell) = self.session.waypoint_cell(index) else {
            return false;
        };
        let Some(terrain) = self.resolved_terrain.as_ref() else {
            return false;
        };
        let cells = crate::map::resolved_terrain::NativeCellQuery::canonical(terrain);
        let bridge = cells.flags(cells.lookup(cell)) & 0x100 != 0
            || cells.flags(cells.lookup(cell)) & 0x400 != 0;
        // The third packed-cell query is separate even when a bridge flag
        // short-circuits the second one. Missing cells stamp the shared Dummy.
        let level =
            i32::from(cells.ground_fields(cells.lookup(cell)).0 as i8) + if bridge { 4 } else { 0 };
        let half_level = (level / 2) as i16;
        let center = (
            cell.0.wrapping_sub(half_level) as u16,
            cell.1.wrapping_sub(half_level) as u16,
        );
        let z = level * crate::util::lepton::GROUND_LEVEL_HEIGHT_LEPTONS;
        let radius = rules.reveal_trigger_radius().clamp(0, i32::from(u16::MAX)) as u16;
        let config = self.sight_reveal_config(Some(rules));
        let height_grid = if config.reveal_by_height() {
            self.path_grid().map(PathGrid::ground_height_grid)
        } else {
            None
        };
        // Outline/unreveal/arg6 are zero and by-height is one. Native final0
        // followed by final1 leaves knowledge with no sustained sight source.
        for final_pass in [false, true] {
            self.reveal_area(
                &[viewer],
                center,
                z,
                radius,
                height_grid.as_deref(),
                final_pass,
                &config,
                context,
            );
        }
        true
    }

    /// Original MapClass::RevealArea5678E0, ordinary no-outline/no-unreveal
    /// route. Geometry stays with the shared native spiral/height owner;
    /// MapCell4A9CA0 publishes each write and its synchronous discovery
    /// before the next spiral cell. A final0/final1 pair retains knowledge
    /// without a sustained sight source. Executed controls live in
    /// tools/spatial_oracle/trigger_action_values and reveal_area.
    pub(crate) fn reveal_area(
        &mut self,
        viewers: &[crate::sim::intern::InternedId],
        cell: (u16, u16),
        z: i32,
        radius: u16,
        line_of_sight: Option<&[u8]>,
        final_pass: bool,
        config: &vision::VisionConfig,
        context: UninitContext<'_>,
    ) {
        let cells = config.collect_area_cells(
            cell,
            radius,
            z,
            line_of_sight,
            (self.fog.width, self.fog.height),
        );
        #[cfg(test)]
        REVEAL_CALLS.with_borrow_mut(|trace| {
            if let Some(trace) = trace {
                trace.walks.push(cells.clone());
            }
        });
        for &viewer in viewers {
            for &cell in &cells {
                let notify = {
                    let native_cells = context
                        .terrain()
                        .or(self.resolved_terrain.as_ref())
                        .map(crate::map::resolved_terrain::NativeCellQuery::canonical);
                    self.fog
                        .reveal_map_cell(viewer, cell, final_pass, native_cells.as_ref())
                };
                if let Some(notify) = notify {
                    self.notify_cell_change(notify.cell, viewer, notify.fresh, context);
                }
            }
        }
    }

    /// Original CellChangeNotify5865F0..586709: seven height-admitted
    /// diagonal cells, one nearest ground Techno per cell in native linked
    /// order, then its virtual+198. It does not scan every resident.
    /// Fresh knowledge additionally marks radar terrain dirty (6565A0);
    /// rendering owns the pixel/surface clipping. No RNG, timer or detach.
    pub(crate) fn notify_cell_change(
        &mut self,
        cell: (u16, u16),
        viewer: crate::sim::intern::InternedId,
        fresh: bool,
        context: UninitContext<'_>,
    ) {
        for rung in 0..7i16 {
            let diagonal = (
                (cell.0 as i16).wrapping_add(rung),
                (cell.1 as i16).wrapping_add(rung),
            );
            let candidate = {
                let Some(terrain) = context.terrain().or(self.resolved_terrain.as_ref()) else {
                    continue;
                };
                let query = crate::map::resolved_terrain::NativeCellQuery::canonical(terrain);
                let identity = query.lookup(diagonal);
                let level = i16::from(query.ground_fields(identity).0 as i8);
                if level < 2 * rung - 1 || level > 2 * rung + 1 {
                    continue;
                }
                let coord = query.coord(identity);
                (coord.0 as u16, coord.1 as u16)
            };
            if fresh {
                self.mark_radar_terrain_dirty_cells([candidate]);
            }
            #[cfg(test)]
            REVEAL_CALLS.with_borrow_mut(|trace| {
                if let Some(trace) = trace {
                    trace.discovery_queries.insert(candidate);
                }
            });
            if let Some(id) = self.nearest_cell_object(
                candidate,
                crate::sim::movement::locomotor::MovementLayer::Ground,
                None,
            ) {
                self.discover_techno(id, viewer, context);
            }
        }
    }

    /// `MapClass::Reveal @ 0x00577D90` for one house: every allocated cell
    /// becomes explored, once (`House+0x240`). Natively only `PlayerPtr`'s
    /// shroud changes; VERA keeps one plane per house, so the lobby's no-shroud
    /// option and the Reveal crate both ask for their house here.
    pub(crate) fn reveal_whole_map_for_owner(&mut self, owner: crate::sim::intern::InternedId) {
        if let Some(terrain) = self.resolved_terrain.as_ref() {
            let cells: Vec<(u16, u16)> = terrain.iter().map(|cell| (cell.rx, cell.ry)).collect();
            self.fog.reveal_cells_for_owner(owner, cells);
        } else {
            self.fog.reveal_all_for_owner(owner);
        }
    }

    /// The settings every reveal reads (`TechnoClass::UpdateReveal @
    /// 0x0070AF50` and its callers, the Psychic Reveal): with a live map the
    /// stored playfield byte gates a Techno's, the map's `Size=` diamond
    /// bounds them, and the rules' sight keys and `AllyReveal=` shape them.
    pub(crate) fn sight_reveal_config(&self, rules: Option<&RuleSet>) -> vision::VisionConfig {
        vision::VisionConfig::new(
            rules,
            self.map_size_diamond(),
            self.playfield_bounds.is_some(),
        )
    }

    pub(super) fn refresh_high_flying_sight_before_process(
        &mut self,
        stable_id: u64,
        rules: Option<&RuleSet>,
    ) {
        let frame = self.session.binary_frame;
        let Some(entity) = self.substrate.entities.get(stable_id) else {
            return;
        };
        if entity.dying
            || entity.lifecycle.in_limbo
            || !matches!(
                entity.category,
                EntityCategory::Unit | EntityCategory::Infantry | EntityCategory::Aircraft
            )
            || !crate::sim::movement::motion_query::is_moving_now(
                entity,
                rules.map(|rules| {
                    crate::sim::movement::SpeedRules::new(
                        rules,
                        &self.interner,
                        &self.type_handles,
                        &self.houses,
                    )
                }),
                frame,
            )
        {
            return;
        }
        let high = crate::sim::movement::air_movement::is_high_flying(
            entity,
            self.resolved_terrain.as_ref(),
            rules.map(|rules| (rules, &self.interner)),
        );
        if !high {
            return;
        }
        self.fog.alliances = self.house_alliances.clone();
        //4DA6B4 loads SOURCE House before4F9A50; reverse-only alliance
        //does not admit this event or mutate this viewer's countdown.
        let viewers = vision::allied_viewers(&self.fog, entity.owner(), &self.interner);
        let due: Vec<_> = viewers
            .into_iter()
            .filter(|&viewer| {
                entity
                    .sight_refresh_timers
                    .timer(viewer)
                    .expired(frame as i32)
            })
            .collect();
        if due.is_empty() {
            return;
        }
        let config = self.sight_reveal_config(rules);
        let height_grid = if config.reveal_by_height() {
            self.path_grid().map(PathGrid::ground_height_grid)
        } else {
            None
        };
        let ability = vision::entity_has_sight_ability(entity, &self.interner, rules);
        for &viewer in &due {
            // The two reveal leaves may reject membership/zero-radius. The
            // due FootAI branch nevertheless reloads15 after its calls.
            if entity.in_playfield && !entity.passenger_role.is_inside_transport() {
                vision::refresh_entity_vision_for_viewer(
                    &mut self.fog,
                    entity,
                    &config,
                    height_grid.as_deref(),
                    ability,
                    &self.interner,
                    viewer,
                );
            }
        }
        if let Some(entity) = self.substrate.entities.get_mut(stable_id) {
            for viewer in due {
                entity.sight_refresh_timers.reload(viewer, frame);
            }
        }
    }
}
