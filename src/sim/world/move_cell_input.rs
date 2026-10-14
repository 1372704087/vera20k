//! Input-time Foot Cell queries: What_Action on a Cell for a Move click, and
//! the Cell a Move click resolves. Synchronized commands already carry the
//! chosen Cell; their executor never repeats these input queries.

use super::Simulation;
use crate::map::cell_index::NativeCellIdentity;
use crate::map::entities::EntityCategory;
use crate::map::resolved_terrain::NativeCellQuery;
use crate::rules::locomotor_type::{MovementZone, SpeedType};
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::cell_rect::{PlayfieldBounds, cell_is_in_playfield_height_aware_in_query};
use crate::sim::components::DriveCoord;
use crate::sim::find_nearby_cell::{
    NearbyAnchorGate, NearbyFootprint, NearbyQuery, PassabilityArgs, find_nearby_passable_cell,
    map_owned_radius_cap,
};
use crate::sim::intern::InternedId;
use crate::sim::movement::ground_pose;
use crate::sim::movement::ground_pose::query_ground_height as query_ground;
use crate::sim::movement::infantry_entry::{EntryQueryMode, InfantryEntryArgs};
use crate::sim::occupancy::RawCellOccupationGrid;
use crate::sim::pathfinding::zone_map::{ZoneGrid, ZoneQueryCell};
use crate::util::lepton::{BRIDGE_DECK_HEIGHT_LEPTONS, GROUND_LEVEL_HEIGHT_LEPTONS};

/// What_Action's answer on a Cell (vt+0x70) in the rungs a Move click
/// reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellMoveAction {
    /// Action 0: the arrow cursor, and a click orders nothing.
    None,
    /// Action 1.
    Move,
    /// Action 2: the barred cursor. A click still resolves a Cell
    /// (`0x004D8065`).
    NoMove,
}

impl CellMoveAction {
    pub(crate) const fn native(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Move => 1,
            Self::NoMove => 2,
        }
    }
}

/// How What_Action on a Cell is asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CellActionQuery {
    /// DetermineAction's (`0x00692610`) third argument, which Techno
    /// `0x00700600` reads as its second: 1 from the cursor update
    /// (`0x0069300B`); 0 from the button release (`0x00693267`), the
    /// selection's click loop (`0x004AE974`), ClickedAction (`0x004D7DA1`) and
    /// the Cell resolver (`0x004DE1E4`). With it set, the Move rungs answer
    /// Move before asking Can_Enter_Cell (`0x00700B51..0x00700B57`).
    pub(crate) mouse_over: bool,
    /// The Shift key (`0x00A8EC08`/`0x00A8EC0C`, `0x00700696..0x007006C5`),
    /// which Ctrl clears (`0x00700706..0x0070071B`).
    pub(crate) shift: bool,
}

impl Simulation {
    /// Whether the receiver's class What_Action over an object refuses a Move
    /// onto it for its deployment state; a Cell click asks
    /// [`Self::cell_move_action`] instead. Over an object only the Deployer
    /// arms run: Infantry `0x0051E3B0` for a player's Deployer in Doing27..30
    /// (`0x0051EB81..0x0051EBD7`), Unit `0x0073FD50` for a player's deployed
    /// (6E0) IsSimpleDeployer (`0x0074037A..0x007403AD`). Each class's
    /// Berserk refusal (`0x0051F1D8`, `0x007388EB`) is
    /// [`Self::click_order_receivers`].
    pub(crate) fn object_move_click_refused(&self, id: u64, rules: &RuleSet) -> bool {
        let Some(entity) = self.substrate.entities.get(id) else {
            return false;
        };
        let human = self.house_is_human_player(entity.owner());
        let object = rules.object(self.interner.resolve(entity.type_ref()));
        match entity.category {
            EntityCategory::Infantry => {
                human
                    && object.is_some_and(|object| object.deployer)
                    && entity.infantry_deploy_doing()
            }
            EntityCategory::Unit => {
                human
                    && entity.is_fully_deployed()
                    && object.is_some_and(|object| object.is_simple_deployer)
            }
            EntityCategory::Aircraft | EntityCategory::Structure => false,
        }
    }

    /// What_Action on a Cell (vt+0x70) for a Foot, in the rungs a Move click
    /// reaches: the class wrapper (Unit `0x007404B0`, Infantry `0x0051F800`,
    /// Aircraft `0x00417F80`) around Foot `0x004DDDE0` around Techno
    /// `0x00700600`. The cursor and the click both ask it.
    ///
    /// In order:
    /// - each class wrapper answers 0 for a House that is not the player's
    ///   (`0x007404BF`, `0x0051F80E`, `0x00417F89`);
    /// - Techno answers 0 for a berserk object (`0x00700609`), a warped one
    ///   (`+0x270`/`+0x271`, `0x00700731`, `0x00700743`) and one the player
    ///   cannot order (vt+0xA0, `0x00700ACE`). vt+0x284 is the constant false
    ///   `0x0041BEE0` for all three classes, so Alt adds nothing;
    /// - outside the playfield it answers NoMove (`0x00700AF6`), then Move for
    ///   Shift (`0x00700B03`) and for the cursor's query (`0x00700B51`);
    /// - otherwise the mover's own Can_Enter_Cell with no facing, level or
    ///   source Cell (`0x00700B5F..0x00700B7E`) answers Move up to 1, else
    ///   NoMove. The Subterranean (Type+D2C) second probe without the
    ///   locomotor (`0x00700BD1..0x00700C06`) repeats that answer, since no
    ///   class reads the argument ([`EntryQueryMode`]); the building arm
    ///   before it (`0x00700B84`) is not a Foot's;
    /// - Foot turns any answer but 0 over a shrouded Cell centre into Move for
    ///   a MoveToShroud type inside the playfield, else NoMove
    ///   (`0x004DDE64..0x004DDEBE`);
    /// - a Unit's Move becomes 0 while it is deployed, deploying or
    ///   undeploying (6E0/6E1/6E2, `0x00740709..0x00740727`), so its deployed
    ///   IsSimpleDeployer arm (`0x00740752..0x00740767`) finds no Move;
    /// - an Infantry Deployer in Doing27..30 turns Move into NoMove
    ///   (`0x0051F8BB..0x0051F94E`), and an Infantry 0 becomes NoMove unless
    ///   it is a JumpJet or cannot be ordered (`0x0051FAAB..0x0051FADD`): a
    ///   berserk soldier.
    ///
    /// Not modelled, each answering another action where native reaches it:
    /// the planning rung (Ctrl+Alt or +0x43A, `0x00700815..0x007008A3`); the
    /// attack rungs (`0x007008A6..0x00700AB4`: Ctrl, an enemy wall, a
    /// destroyable cliff, a veinhole, a fogged building, a TerrainFire weapon
    /// over a tree); a Unit harvester over ore or weeds
    /// (`0x007405F5..0x007406C3`, app-side in the cursor and the click); a
    /// tube entrance (Unit `0x007406CF`, Infantry `0x0051F9AC`); a JumpJet
    /// soldier over a tube's neighbourhood, which answers 0
    /// (`0x0051F860..0x0051F8A6`); the fogged-object scans; and DirectRocker
    /// (`+0x2A8`, `0x00700617`), which nothing in retail sets.
    pub(crate) fn cell_move_action(
        &self,
        viewer: InternedId,
        id: u64,
        cell: (u16, u16),
        query: CellActionQuery,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<CellMoveAction, String> {
        let entity = self
            .substrate
            .entities
            .get(id)
            .ok_or("What_Action receiver disappeared")?;
        if entity.category == EntityCategory::Structure {
            return Err("a building answers What_Action through its own class".into());
        }
        if !self.house_is_human_player(entity.owner()) {
            return Ok(CellMoveAction::None);
        }
        let object = rules
            .object(self.interner.resolve(entity.type_ref()))
            .ok_or("What_Action receiver lacks its type")?;
        let terrain = self
            .resolved_terrain
            .as_ref()
            .ok_or("What_Action needs map cells")?;
        let cells = NativeCellQuery::isolated(terrain);
        let clicked = (cell.0 as i16, cell.1 as i16);
        let in_playfield = cell_is_in_playfield_height_aware_in_query(
            (i32::from(clicked.0), i32::from(clicked.1)),
            self.playfield_bounds,
            Some(terrain),
            Some(&cells),
        );
        let controllable = self.techno_player_controllable(id, rules);
        let mut action = if entity.berserk.active
            || entity.is_warped_out()
            || entity.is_warping_in()
            || !controllable
        {
            CellMoveAction::None
        } else if !in_playfield {
            CellMoveAction::NoMove
        } else if query.shift
            || query.mouse_over
            || self.mover_can_enter(
                id,
                clicked,
                InfantryEntryArgs::REPAIR,
                EntryQueryMode::CheckLocomotor,
                rules,
                registry,
            )? <= 1
        {
            CellMoveAction::Move
        } else {
            CellMoveAction::NoMove
        };
        if action != CellMoveAction::None
            && crate::sim::vision::point_is_shrouded(
                &self.fog,
                &cells,
                viewer,
                clicked_coordinate(&cells, clicked)?,
            )
        {
            action = if object.move_to_shroud && in_playfield {
                CellMoveAction::Move
            } else {
                CellMoveAction::NoMove
            };
        }
        match entity.category {
            EntityCategory::Unit if action == CellMoveAction::Move && entity.is_deployed() => {
                action = CellMoveAction::None;
            }
            EntityCategory::Infantry => {
                if action == CellMoveAction::Move
                    && object.deployer
                    && entity.infantry_deploy_doing()
                {
                    action = CellMoveAction::NoMove;
                }
                if action == CellMoveAction::None && !object.jumpjet && controllable {
                    action = CellMoveAction::NoMove;
                }
            }
            _ => {}
        }
        Ok(action)
    }

    /// The Cell a Foot's Move click resolves, before event encoding. The
    /// selection's click loop (`0x004AE94E..0x004AE990`) asks each receiver's
    /// own What_Action ([`Self::cell_move_action`], `0x004AE976`); its
    /// ClickedAction (vt+0x140: Unit `0x00738910`, Infantry `0x0051F250`,
    /// Aircraft `0x00417C80`, then Foot `0x004D7D50`) sends Move and NoMove
    /// alike to the Cell resolver `0x004DE1D0` (`0x004D7EB5`, `0x004D806F`),
    /// whatever the locomotor. Refused before it:
    /// - action 0, which ClickedAction has no arm for (`0x004D7DCC..0x004D7DD2`);
    /// - NoMove for a deployed (6E0) Unit (`0x0073891D..0x00738933`) or for an
    ///   Infantry in Doing27..30 (`0x0051F25D..0x0051F285`).
    ///
    /// ClickedAction's outside-playfield refusal (`0x004D7DB3..0x004D7DBF`)
    /// never fires: outside the playfield What_Action answers NoMove or 0.
    /// The Berserk refusals (`0x004D7D58`, `0x0073893D`, `0x0051F28F`) are
    /// [`Self::click_order_receivers`], which the producers apply first.
    /// `None` sends no order. Native comparisons: walk_first_path.json
    /// `gi_reissue` (GI and MTNK What_Action and click rows) and
    /// walk_move_admission.json (the resolver).
    pub(crate) fn cell_click_destination(
        &self,
        viewer: InternedId,
        id: u64,
        clicked: (u16, u16),
        shift: bool,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<Option<(u16, u16)>, String> {
        let action = self.cell_move_action(
            viewer,
            id,
            clicked,
            CellActionQuery {
                mouse_over: false,
                shift,
            },
            rules,
            registry,
        )?;
        let entity = self
            .substrate
            .entities
            .get(id)
            .ok_or("Cell click receiver disappeared")?;
        let refused = match action {
            CellMoveAction::None => true,
            CellMoveAction::NoMove => match entity.category {
                EntityCategory::Unit => entity.is_fully_deployed(),
                EntityCategory::Infantry => entity.infantry_deploy_doing(),
                EntityCategory::Aircraft | EntityCategory::Structure => false,
            },
            CellMoveAction::Move => false,
        };
        if refused {
            return Ok(None);
        }
        let object = rules
            .object(self.interner.resolve(entity.type_ref()))
            .ok_or("Cell click receiver lacks its type")?;
        let terrain = self
            .resolved_terrain
            .as_ref()
            .ok_or("Cell click needs map cells")?;
        let zones = self.zone_grid.as_ref().ok_or("Cell click needs zones")?;
        let bounds = self
            .playfield_bounds
            .ok_or("Cell click needs the playfield")?;
        let size = self
            .playfield_size_height
            .map(|height| (bounds.base, height))
            .or_else(|| self.bridge_state.as_ref()?.native_zone_source_size())
            .ok_or("Cell click needs the map Size")?;
        let coordinate = self.foot_navigation_coordinate(id)?;
        //One fresh query identity starts from the canonical retained bytes.
        //All nested calls share it.
        let cells = NativeCellQuery::isolated(terrain);
        if cells.dummy().raw_flags() & 3 != 0 {
            return Err("input Dummy visibility outside the constructor-derived domain".into());
        }
        resolve_foot_cell_click(
            &cells,
            zones,
            &self.substrate.raw_cell_occupation,
            bounds,
            size,
            self.session.binary_frame,
            &FootCellClick {
                clicked: (clicked.0 as i16, clicked.1 as i16),
                action: action.native(),
                current: ground_pose::position_world_coord(&entity.position),
                coordinate,
                marked: entity.lifecycle.cell_marked,
                on_bridge: entity.on_bridge,
                in_tube: entity.low_bridge_tube_state.is_some(),
                move_to_shroud: object.move_to_shroud,
                teleporter: object.teleporter,
                special_type: object.movement_zone == MovementZone::Subterranean
                    || entity.category == EntityCategory::Aircraft
                    || (entity.category == EntityCategory::Infantry && object.jumpjet),
                movement_zone: object.movement_zone,
                speed_type: object.speed_type,
            },
            |cell| {
                Ok(crate::sim::vision::cell_ground_mapped(
                    &self.fog, terrain, viewer, cell,
                ))
            },
        )
    }
}

/// Values the Foot Cell resolver 4DE1D0 reads. Its own What_Action
/// (`0x004DE1E9`) asks what the click already asked, so the action arrives
/// selected; it is not a mission.
struct FootCellClick {
    clicked: (i16, i16),
    action: u32,
    current: DriveCoord,
    coordinate: DriveCoord,
    marked: bool,
    on_bridge: bool,
    in_tube: bool,
    move_to_shroud: bool,
    teleporter: bool,
    /// The type terms of 4DE1D0's special test after IsInAir
    /// (`0x004DE2EA..0x004DE335`): Type+D2C (Subterranean), an Infantry's
    /// Type+D94 (JumpJet; House53A130 is constant false), or an Aircraft.
    special_type: bool,
    movement_zone: MovementZone,
    speed_type: SpeedType,
}

fn clicked_coordinate(
    cells: &NativeCellQuery<'_>,
    clicked: (i16, i16),
) -> Result<DriveCoord, String> {
    let mut point = DriveCoord {
        x: i32::from(clicked.0) * 256 + 128,
        y: i32::from(clicked.1) * 256 + 128,
        z: 0,
    };
    point.z = query_ground(cells, point)?;
    //4DE242/4DE38C are separate Map calls after the height query.
    if cells.flags(cells.lookup_world(point.x, point.y)) & 0x100 != 0 {
        point.z = point.z.wrapping_add(BRIDGE_DECK_HEIGHT_LEPTONS);
    }
    Ok(point)
}

/// Original 4DE1D0 for every Foot. No A* search and no
/// entity/mission/navigation mutation occurs here. Native comparisons live in
/// tools/spatial_oracle/walk_move_admission (Infantry and Unit/Drive
/// receivers, original input action supplied).
///
/// FNPC's tenth argument (`0x004DE3DB..0x004DE3F9`: 1 for a Type+D2C mover
/// at or above the ground, else 0) is not passed: no retail type is
/// Subterranean.
fn resolve_foot_cell_click(
    cells: &NativeCellQuery<'_>,
    zones: &ZoneGrid,
    raw: &RawCellOccupationGrid,
    bounds: PlayfieldBounds,
    size: (i32, i32),
    frame: u32,
    click: &FootCellClick,
    open: impl Fn(NativeCellIdentity) -> Result<bool, String>,
) -> Result<Option<(u16, u16)>, String> {
    let terrain = cells.terrain();
    let in_playfield = cell_is_in_playfield_height_aware_in_query(
        (i32::from(click.clicked.0), i32::from(click.clicked.1)),
        Some(bounds),
        Some(terrain),
        Some(cells),
    );
    let point = clicked_coordinate(cells, click.clicked)?;
    if !click.move_to_shroud && crate::sim::vision::coordinate_is_shrouded(cells, point, &open)? {
        return Ok(None);
    }
    let source = cells.lookup_world(click.coordinate.x, click.coordinate.y);
    //5F6B90 samples height only when Object+74 is true; +1C8 subtracts
    //OnBridge's height independently of the ShouldBeOnBridge query below.
    let high_flying = if click.marked {
        let height = click
            .current
            .z
            .wrapping_sub(query_ground(cells, click.current)?)
            .wrapping_sub(if click.on_bridge {
                BRIDGE_DECK_HEIGHT_LEPTONS
            } else {
                0
            });
        height >= 2 * GROUND_LEVEL_HEIGHT_LEPTONS
    } else {
        false
    };
    //4DE2B7..4DE2DA: Subterranean searches as Normal. Fly becomes Infantry
    //only when House53A130 answers true, and it is constant false.
    let movement_zone = match click.movement_zone {
        MovementZone::Subterranean => MovementZone::Normal,
        zone => zone,
    };
    let special = high_flying || click.special_type;
    let source_bridge = ground_pose::navigation_should_be_on_bridge(
        cells,
        click.coordinate,
        click.current,
        click.on_bridge,
        click.in_tube,
    )?;
    let point = clicked_coordinate(cells, click.clicked)?;
    let shrouded = crate::sim::vision::coordinate_is_shrouded(cells, point, &open)?;
    let unrestricted = special && click.action == 2;
    if !unrestricted && in_playfield && !shrouded {
        if special || (click.teleporter && click.action != 2) {
            return Ok((click.clicked != (0, 0))
                .then_some((click.clicked.0 as u16, click.clicked.1 as u16)));
        }
        let bridge = cells.flags(cells.lookup(click.clicked)) & 0x100 != 0;
        if zones
            .can_reach_native(
                cells,
                ZoneQueryCell::Retained(source),
                ZoneQueryCell::Copied(click.clicked),
                movement_zone,
                source_bridge,
                bridge,
                false,
                bounds,
                size,
            )
            .ok_or("input lacks native zone topology")?
        {
            return Ok((click.clicked != (0, 0))
                .then_some((click.clicked.0 as u16, click.clicked.1 as u16)));
        }
    }
    let target_bridge = cells.flags(cells.lookup(click.clicked)) & 0x100 != 0;
    let required_zone = if unrestricted {
        None
    } else {
        let p = cells.coord(source);
        let zone = zones
            .get_zone_id_native_in_query(
                terrain,
                (p.0 as u16, p.1 as u16),
                movement_zone,
                source_bridge,
                Some(cells),
            )
            .ok_or("input FNPC source lacks native zone topology")?;
        // FNPC disables the comparison for DWORD -1 and raw 0xFFFF alike.
        Some(zone)
    };
    Ok(find_nearby_passable_cell(
        (i32::from(click.clicked.0), i32::from(click.clicked.1)),
        &NearbyQuery {
            native_cells: Some(cells),
            raw_occupation: Some(raw),
            passability: PassabilityArgs {
                speed_type: click.speed_type,
                required_zone_id: required_zone,
                movement_zone,
                bridge_aware_zone: target_bridge,
            },
            footprint: NearbyFootprint::SINGLE,
            anchor_gate: NearbyAnchorGate::NativeHeightAware,
            allow_bridge_cells: true,
            check_height: true,
            check_occupancy: false,
            radius_cap: map_owned_radius_cap(size.0, size.1),
            target_cell: None,
            path_grid: None,
            resolved_terrain: Some(terrain),
            overlay_grid: None,
            occupancy: None,
            entities: None,
            zone_grid: Some(zones),
            playfield_bounds: Some(bounds),
        },
        frame,
    )
    .filter(|cell| *cell != (0, 0)))
}

#[cfg(test)]
#[path = "move_cell_input_tests.rs"]
mod tests;
