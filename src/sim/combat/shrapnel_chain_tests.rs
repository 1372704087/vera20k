//! Retail Prism bounce integration through FireAt and the live detonation
//! receiver. Original SpawnShrapnel46A310 precedes Apply_area_damage469A83;
//! every child is constructed/fired and its beam copied before either damage
//! callbacks or the next child. These tests do not emulate the native driver.

use super::{
    CombatEmit, TargetKind, build_attacker_snapshot, combat_weapon, world_fire, world_receiver,
};
use crate::rules::ruleset::RuleSet;
use crate::sim::projectile::{ProjectileDetonation, ProjectileDetonationReason, ProjectileTarget};
use crate::sim::world::{FrameEffects, LifecycleOutput, LifecycleTestEvent, Simulation};

struct PrismScene {
    world: Simulation,
    rules: RuleSet,
    source: u64,
    victim: u64,
    neighbor: u64,
}

impl PrismScene {
    fn new(truck: bool, elite: bool) -> Option<Self> {
        Self::at(truck, elite, 7)
    }

    fn at(truck: bool, elite: bool, source_rx: u16) -> Option<Self> {
        let rules = crate::rules::retail_ini_fixture::retail_battle_rules()?.rules;
        // These stock children never read the absent native Bullet+130
        // WeaponType through RadLevel or Inviso FlakScatter. The general
        // null-weapon distinction is a separate unresolved Bullet path.
        for name in ["CometFragment", "SuperCometFragment"] {
            let weapon = rules.weapon(name).unwrap();
            assert_eq!(weapon.rad_level, 0);
            assert!(
                !rules
                    .projectile(weapon.projectile.as_deref().unwrap())
                    .unwrap()
                    .flak_scatter
            );
        }
        let mut world = crate::sim::arena_fixture::rival_arena(&rules, 31);
        world.intern_rule_type_ids(&rules);
        world.resolve_type_handles(&rules);
        let source = world
            .spawn_object("SREF", "Americans", source_rx, 16, 64, &rules)
            .unwrap();
        let victim = world
            .spawn_object(
                if truck { "DTRUCK" } else { "MTNK" },
                "Russians",
                16,
                16,
                0,
                &rules,
            )
            .unwrap();
        let neighbor = world
            .spawn_object("E1", "Russians", 17, 16, 0, &rules)
            .unwrap();
        world
            .spawn_object("E1", "Russians", 16, 17, 0, &rules)
            .unwrap();
        if truck {
            world
                .substrate
                .entities
                .get_mut(victim)
                .unwrap()
                .health
                .set_for_test(1);
        }
        if elite {
            world
                .substrate
                .entities
                .get_mut(source)
                .unwrap()
                .veterancy
                .set_elite();
        }
        world
            .assign_target_represented(
                source,
                Some(TargetKind::Entity(victim)),
                Some(&rules),
                None,
                FrameEffects::default(),
            )
            .unwrap();
        Some(Self {
            world,
            rules,
            source,
            victim,
            neighbor,
        })
    }

    fn launch(&mut self) -> u64 {
        self.world.session.binary_frame = 100;
        let actor = self.world.substrate.entities.get(self.source).unwrap();
        let object = self.rules.object("SREF").unwrap();
        let selected = combat_weapon::resolve_weapon_index(
            &self.rules,
            actor,
            object,
            0,
            &self.world.substrate.entities,
            &self.world.interner,
        )
        .unwrap();
        let target = self.world.substrate.entities.get(self.victim).unwrap();
        let shot = world_fire::AdmittedFire {
            snap: build_attacker_snapshot(actor, TargetKind::Entity(self.victim), None),
            obj: object,
            selected,
            target_coords: (
                target.position.rx(),
                target.position.ry(),
                target.position.sub_x(),
                target.position.sub_y(),
            ),
            target_type_ref: target.type_ref(),
            is_garrison: false,
        };
        world_fire::emit_admitted_fire(
            &mut self.world,
            &self.rules,
            shot,
            100,
            &mut CombatEmit::default(),
            None,
            FrameEffects::default(),
        )
        .expect("retail Comet launch")
    }

    fn detonate(&mut self, parent: u64) {
        let bullet = self.world.projectiles.get(parent).unwrap();
        let impact = ProjectileDetonation {
            projectile_id: parent,
            source_id: bullet.source_id,
            target: bullet.target,
            impact: bullet.position,
            payload: bullet.payload,
            reason: ProjectileDetonationReason::ReachedTarget,
        };
        let mut run = world_receiver::ReceiverRun::default();
        world_receiver::commit_projectiles(&mut self.world, &mut run, &[impact], &self.rules, None);
        run.finish();
    }
}

#[test]
fn retail_comet_creates_each_bounce_beam_at_the_live_child_birth() {
    let Some(mut scene) = PrismScene::new(false, false) else {
        return;
    };
    let parent = scene.launch();
    let origin = scene.world.projectiles.get(parent).unwrap().position;
    scene.world.lifecycle_outputs.clear();
    scene.detonate(parent);
    let beams: Vec<_> = scene
        .world
        .lifecycle_outputs
        .iter()
        .filter_map(|output| match output {
            LifecycleOutput::LaserCreated(birth) => Some(birth),
            _ => None,
        })
        .collect();
    assert_eq!(
        beams.len(),
        5,
        "normal Comet creates five visible bounce beams"
    );
    assert_eq!(
        scene.world.projectiles.len(),
        6,
        "children are live before the detonation receiver returns"
    );
    for beam in beams {
        assert_eq!(beam.from, origin);
        assert_eq!(beam.width, 1);
        assert!(!beam.supported);
        assert_eq!(beam.duration, 15);
    }
}

#[test]
fn stock_truck_death_notifies_a_child_selected_before_parent_damage() {
    let Some(mut scene) = PrismScene::new(true, false) else {
        return;
    };
    let parent = scene.launch();
    scene.world.clear_lifecycle_test_events_for_test();
    scene.detonate(parent);
    assert!(
        !scene
            .world
            .substrate
            .entities
            .get(scene.neighbor)
            .unwrap()
            .lifecycle
            .object_alive(),
        "stock Demobomb callback kills the selected ring neighbor"
    );
    assert!(scene.world.lifecycle_test_events_for_test().iter().any(|event| matches!(event,
        LifecycleTestEvent::ProjectilePointerExpiredVisited { expired_id, projectile_id, projectile_resolvable: true, target: ProjectileTarget::Cell { rx: 17, ry: 16 }, .. }
            if *expired_id == scene.neighbor && *projectile_id != parent
    )), "an already-registered child receives the target's native expiry callback");
}

fn native_coord(value: &serde_json::Value) -> crate::sim::projectile::ProjectileCoord {
    crate::sim::projectile::ProjectileCoord::new(
        value[0].as_i64().unwrap() as i32,
        value[1].as_i64().unwrap() as i32,
        value[2].as_i64().unwrap() as i32,
    )
}

/// Original46A310 executes its complete selected/random branches, child
/// Construct/Fire and laser calls. Its Unit vtables, lists and coordinates
/// are supplied boundaries; it does not run a whole native skirmish.
#[test]
fn shrapnel_children_and_lasers_match_original_spawn_shrapnel() {
    use crate::map::entities::EntityCategory;
    use crate::sim::game_entity::GameEntity;
    use crate::sim::movement::locomotor::MovementLayer;
    use crate::sim::native_identity::NativeUniqueIdCursor;
    use crate::sim::occupancy::{CellListInsertion, OccupancyGrid};
    use crate::sim::rng::{MainRng, SimRng};
    use crate::util::fixed_math::SimFixed;
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = fixture.rules;
    let corpus: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
        "tools/projectile_oracle/shrapnel_chain.json",
    ))
    .unwrap();
    for row in corpus["cases"].as_array().unwrap() {
        let name = row["input"]["name"].as_str().unwrap();
        let seed = row["input"]["seed"].as_u64().unwrap();
        let mut world = Simulation::with_seed(seed);
        crate::sim::arena_fixture::flat_arena(&mut world, &rules);
        for y in 0..32 {
            for x in 0..32 {
                world
                    .resolved_terrain
                    .as_mut()
                    .unwrap()
                    .cell_mut(x, y)
                    .unwrap()
                    .level = 6;
            }
        }
        // guided_step.create supplies only these native CellClass slots.
        // Other array coordinates retain the shared Dummy instead of a
        // convenient extra clear cell, including the elite outer ring.
        let allocated: Vec<_> = (16..25)
            .flat_map(|y| (6..25).map(move |x| (x, y)))
            .collect();
        world
            .resolved_terrain
            .as_mut()
            .unwrap()
            .test_set_native_allocated_cells(&allocated);
        let owners = [
            world.interner.intern("Americans"),
            world.interner.intern("Russians"),
        ];
        let mut ids = std::collections::BTreeMap::new();
        for (index, actor) in row["actors"].as_array().unwrap().iter().enumerate() {
            let id = index as u64 + 1;
            let actor_name = actor["name"].as_str().unwrap();
            ids.insert(actor_name, id);
            let coord = native_coord(&actor["location"]);
            let mut entity = GameEntity::test_default(
                id,
                "MTNK",
                "Americans",
                (coord.x / 256) as u16,
                (coord.y / 256) as u16,
            );
            entity.set_category_for_test(EntityCategory::Unit);
            entity.set_type_ref_for_test(world.interner.intern("MTNK"));
            entity.set_owner_for_test(owners[actor["house"].as_u64().unwrap() as usize]);
            entity
                .position
                .set_sub_x_for_test(SimFixed::from_num(coord.x % 256));
            entity
                .position
                .set_sub_y_for_test(SimFixed::from_num(coord.y % 256));
            entity.position.set_exact_z_leptons_for_test(Some(coord.z));
            entity
                .lifecycle
                .set_cell_marked(!(row["input"]["refused"] == true && actor_name == "victim"));
            world.substrate.entities.insert(entity);
        }
        world.substrate.occupancy = OccupancyGrid::new();
        // The native fixture supplies explicit head->next lists. Feed that
        // list through the existing prepend owner, without relying on IDs.
        for actor in row["actors"].as_array().unwrap().iter().rev() {
            if row["input"]["refused"] == true && actor["name"] == "victim" {
                continue;
            }
            let id = ids[actor["name"].as_str().unwrap()];
            world.substrate.occupancy.add(
                actor["cell"][0].as_u64().unwrap() as u16,
                actor["cell"][1].as_u64().unwrap() as u16,
                id,
                MovementLayer::Ground,
                None,
                CellListInsertion::PrependNonBuilding,
            );
        }
        world.substrate.next_stable_object_id = ids.len() as u64 + 1;
        world.native_unique_ids = Some(NativeUniqueIdCursor::test_at_current_value(
            row["native_id_before"].as_u64().unwrap() as u32,
        ));
        world.main_rng = MainRng::new(seed);
        world.scenario_rng = SimRng::new(seed + 1);
        world.mapgen_rng = SimRng::new(seed + 2);
        assert_eq!(
            world.scenario_rng.native_state_hex(),
            row["rng_before"]["scenario"],
            "{name}: initial Scenario"
        );
        assert_eq!(
            world.main_rng.native_state_hex(),
            row["rng_before"]["main"],
            "{name}: initial Main"
        );
        let parent_weapon = rules
            .weapon(row["input"]["parent_weapon"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            rules.general.gravity,
            row["effective"]["gravity"].as_i64().unwrap() as i32,
            "{name}: original AudioVisual Gravity reader"
        );
        let source_id = if row["input"]["detached"] == true {
            super::RAD_NO_ATTACKER
        } else {
            ids["source"]
        };
        let detonation = ProjectileDetonation {
            projectile_id: 999,
            source_id,
            target: ProjectileTarget::Entity(ids["victim"]),
            impact: native_coord(&row["origin"]),
            payload: crate::sim::projectile::ProjectilePayload::new(
                parent_weapon.damage,
                world
                    .interner
                    .intern(parent_weapon.warhead.as_deref().unwrap()),
                world.interner.intern(&parent_weapon.id),
            ),
            reason: ProjectileDetonationReason::ReachedTarget,
        };
        super::emit_projectile_shrapnel(
            &mut world,
            &rules,
            &detonation,
            native_coord(&row["origin"]),
        );
        let children: Vec<_> = world.projectiles.iter().map(|(_, p)| p).collect();
        assert_eq!(
            children.len(),
            row["children"].as_array().unwrap().len(),
            "{name}"
        );
        for (actual, expected) in children.iter().zip(row["children"].as_array().unwrap()) {
            let target = match expected["target"]["kind"].as_str().unwrap() {
                "entity" => {
                    ProjectileTarget::Entity(ids[expected["target"]["name"].as_str().unwrap()])
                }
                "cell" => ProjectileTarget::Cell {
                    rx: expected["target"]["cell"][0].as_u64().unwrap() as u16,
                    ry: expected["target"]["cell"][1].as_u64().unwrap() as u16,
                },
                kind => panic!("unhandled native target {kind}"),
            };
            assert_eq!(actual.target, target, "{name}");
            assert_eq!(actual.source_id, source_id, "{name}");
            assert_eq!(
                actual.native_unique_id,
                expected["native_id"].as_i64().unwrap() as i32,
                "{name}"
            );
            assert_eq!(
                actual.launch_origin,
                native_coord(&expected["fire_location"]),
                "{name}"
            );
            assert_eq!(
                actual.launch_target,
                native_coord(&expected["target_location"]),
                "{name}"
            );
            assert_eq!(
                actual.position,
                native_coord(&expected["placed_location"]),
                "{name}"
            );
            assert_eq!(
                actual.payload.base_damage,
                expected["damage"].as_i64().unwrap() as i32,
                "{name}"
            );
            assert_eq!(
                world.interner.resolve(actual.payload.warhead),
                expected["warhead"].as_str().unwrap(),
                "{name}"
            );
            let child_weapon = rules
                .weapon(row["effective"]["child_weapon"].as_str().unwrap())
                .unwrap();
            assert_eq!(
                child_weapon.speed,
                expected["speed"].as_i64().unwrap() as i32,
                "{name}: processed weapon speed"
            );
            assert_eq!(
                world.interner.resolve(actual.payload.weapon),
                child_weapon.id,
                "{name}"
            );
            assert_eq!(
                child_weapon.projectile.as_deref(),
                expected["projectile"].as_str(),
                "{name}"
            );
            let launch = crate::sim::projectile::launch::shrapnel_launch_velocity(
                native_coord(&expected["launch_location"]),
                native_coord(&expected["target_location"]),
                child_weapon.speed,
                expected["target"]["kind"] == "cell",
            );
            for (component, bits) in launch
                .native()
                .iter()
                .zip(expected["launch_velocity"]["bits"].as_array().unwrap())
            {
                assert_eq!(
                    component.bits(),
                    u64::from_str_radix(bits.as_str().unwrap(), 16).unwrap(),
                    "{name}: original pre-Fire vector"
                );
            }
            assert_eq!(
                actual.arm_timer.start_frame(),
                expected["arm"][0].as_i64().unwrap() as i32,
                "{name}"
            );
            assert_eq!(
                actual.arm_timer.duration(),
                expected["arm"][1].as_i64().unwrap() as i32,
                "{name}"
            );
            assert!(
                actual.in_logic_vector,
                "{name}: original child is registered"
            );
            assert!(!actual.on_bridge, "{name}");
            // Existing Fire's zero-vector representation canonicalizes the
            // sign of zero; compare numeric values, not +0/-0 storage bits.
            for (component, expected) in actual
                .velocity
                .native()
                .iter()
                .zip(expected["fired_velocity"]["value"].as_array().unwrap())
            {
                assert_eq!(
                    f64::from_bits(component.bits()),
                    expected.as_f64().unwrap(),
                    "{name}"
                );
            }
        }
        let beams: Vec<_> = world
            .lifecycle_outputs
            .iter()
            .filter_map(|e| {
                if let LifecycleOutput::LaserCreated(b) = e {
                    Some(b)
                } else {
                    None
                }
            })
            .collect();
        let child_ids: Vec<_> = children.iter().map(|child| child.id).collect();
        let logic_order: Vec<_> = world
            .substrate
            .logic
            .as_slice()
            .iter()
            .filter_map(|id| child_ids.iter().position(|child_id| child_id == id))
            .collect();
        let native_logic_order: Vec<_> = row["logic_child_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|index| index.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(
            logic_order, native_logic_order,
            "{name}: original Logic append order"
        );
        assert_eq!(
            beams.len(),
            row["lasers"].as_array().unwrap().len(),
            "{name}"
        );
        for (actual, expected) in beams.iter().zip(row["lasers"].as_array().unwrap()) {
            assert_eq!(actual.from, native_coord(&expected["source"]), "{name}");
            assert_eq!(actual.to, native_coord(&expected["target"]), "{name}");
            assert_eq!(
                actual.width,
                expected["width"].as_i64().unwrap() as i32,
                "{name}"
            );
            assert_eq!(
                actual.duration,
                expected["duration"].as_i64().unwrap() as i32,
                "{name}"
            );
            assert_eq!(actual.supported, expected["supported"] == 1, "{name}");
            assert_eq!(
                actual.z_adjust,
                expected["z_adjust"].as_i64().unwrap() as i32,
                "{name}"
            );
            assert_eq!(
                actual.color,
                super::laser::LaserColor::House(owners[0]),
                "{name}: native fixture supplies owner color"
            );
        }
        assert_eq!(
            world.native_unique_ids.as_ref().unwrap().current_raw(),
            row["native_id_after"].as_u64().unwrap() as u32,
            "{name}"
        );
        assert_eq!(
            world.scenario_rng.native_state_hex(),
            row["rng_after"]["scenario"],
            "{name}: Scenario"
        );
        assert_eq!(
            world.main_rng.native_state_hex(),
            row["rng_after"]["main"],
            "{name}: Main"
        );
        assert_eq!(
            world.mapgen_rng.native_state_hex(),
            row["rng_after"]["mapgen"],
            "{name}: MapGen"
        );
    }
}

#[test]
fn building_bounce_places_child_at_get_coords_and_beam_at_target_offset() {
    let Some(mut scene) = PrismScene::new(false, false) else {
        return;
    };
    let terrain = scene.world.resolved_terrain.take();
    let yard = scene
        .world
        .spawn_object("GAYARD", "Russians", 18, 15, 0, &scene.rules)
        .unwrap();
    scene.world.resolved_terrain = terrain;
    let target = scene.world.substrate.entities.get(yard).unwrap();
    let center = crate::sim::movement::ground_pose::object_get_coords(
        target,
        scene.world.resolved_terrain.as_ref(),
    );
    let center = crate::sim::projectile::ProjectileCoord::new(center.x, center.y, center.z);
    let endpoint = super::fire_coord::effect_target_coordinate(
        &scene.world,
        &scene.rules,
        ProjectileTarget::Entity(yard),
    )
    .unwrap();
    assert_ne!(
        center, endpoint,
        "retail yard TargetCoordOffset exercises the different target virtuals"
    );
    let parent = scene.launch();
    scene.world.lifecycle_outputs.clear();
    scene.detonate(parent);
    let child = scene
        .world
        .projectiles
        .iter()
        .map(|(_, p)| p)
        .find(|p| p.target == ProjectileTarget::Entity(yard))
        .expect("yard participates in the rings");
    assert_eq!(child.position, center);
    assert_eq!(child.launch_target, center);
    assert!(
        scene
            .world
            .lifecycle_outputs
            .iter()
            .any(|e| matches!(e,LifecycleOutput::LaserCreated(b) if b.to == endpoint))
    );
}

#[test]
fn elite_bounces_recurse_only_when_children_get_their_logic_visits() {
    let Some(mut scene) = PrismScene::new(false, true) else {
        return;
    };
    let parent = scene.launch();
    scene.world.lifecycle_outputs.clear();
    scene.detonate(parent);
    let first_child = scene
        .world
        .projectiles
        .iter()
        .filter(|(id, _)| **id != parent)
        .map(|(&id, _)| id)
        .min()
        .unwrap();
    assert_eq!(scene.world.projectiles.len(), 6);
    assert!(
        scene
            .world
            .projectiles
            .iter()
            .filter(|(id, _)| **id != parent)
            .all(|(_, p)| scene.world.interner.resolve(p.payload.weapon) == "SuperCometFragment")
    );
    assert_eq!(
        scene
            .world
            .lifecycle_outputs
            .iter()
            .filter(|e| matches!(e, LifecycleOutput::LaserCreated(_)))
            .count(),
        5
    );
    for _ in 0..20 {
        scene.world.session.binary_frame += 1;
        scene
            .world
            .visit_combat_tail(first_child, &scene.rules, None, FrameEffects::default());
        scene.world.flush_pending_delete();
    }
    assert_eq!(
        scene.world.projectiles.len(),
        1,
        "all child generations retire; parent is deliberately outside this tail boundary"
    );
    assert!(scene.world.projectiles.get(parent).is_some());
    assert!(
        scene
            .world
            .lifecycle_outputs
            .iter()
            .filter(|e| matches!(e, LifecycleOutput::LaserCreated(_)))
            .count()
            > 5,
        "visited elite children emit ordinary Comet fragments"
    );
}

#[test]
fn stock_truck_death_clears_already_born_child_owners_but_preserves_beams() {
    let Some(mut scene) = PrismScene::at(true, false, 14) else {
        return;
    };
    scene
        .world
        .substrate
        .entities
        .get_mut(scene.source)
        .unwrap()
        .health
        .set_for_test(1);
    let owner = scene
        .world
        .substrate
        .entities
        .get(scene.source)
        .unwrap()
        .owner();
    let parent = scene.launch();
    scene.world.lifecycle_outputs.clear();
    scene.detonate(parent);
    assert!(
        !scene
            .world
            .substrate
            .entities
            .get(scene.source)
            .unwrap()
            .lifecycle
            .object_alive()
    );
    let children: Vec<_> = scene
        .world
        .projectiles
        .iter()
        .filter(|(id, _)| **id != parent)
        .map(|(_, p)| p)
        .collect();
    assert_eq!(children.len(), 5);
    assert!(
        children
            .iter()
            .all(|p| p.source_id == super::RAD_NO_ATTACKER)
    );
    let beams: Vec<_> = scene
        .world
        .lifecycle_outputs
        .iter()
        .filter_map(|e| {
            if let LifecycleOutput::LaserCreated(b) = e {
                Some(b)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(beams.len(), 5);
    assert!(
        beams
            .iter()
            .all(|b| b.color == super::laser::LaserColor::House(owner))
    );
}
