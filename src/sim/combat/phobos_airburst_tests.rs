//! Phobos airburst/splits production-reader and emitter regressions.

use super::*;
use crate::rules::art_data::ArtRegistry;
use crate::rules::ini_parser::IniFile;
use crate::sim::projectile::ProjectileCoord;
use crate::util::native_x87::NativeF64Bits;

const VERSES_ZERO: &str = "Verses=0%,0%,0%,0%,0%,0%,0%,0%,0%,0%,0%";
const VERSES_FULL: &str = "Verses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%";

fn rules_ini(
    parent: &str,
    child_weapon: &str,
    child_projectile: &str,
    warhead: &str,
    globals: &str,
    extra: &str,
) -> IniFile {
    rules_ini_full(
        parent,
        child_weapon,
        child_projectile,
        warhead,
        globals,
        extra,
        VERSES_FULL,
        "",
    )
}

#[allow(clippy::too_many_arguments)]
fn rules_ini_full(
    parent: &str,
    child_weapon: &str,
    child_projectile: &str,
    warhead: &str,
    globals: &str,
    extra: &str,
    child_verses: &str,
    victim_keys: &str,
) -> IniFile {
    let text = format!(
        "[CombatDamage]\n{globals}\n\
         [VehicleTypes]\n0=SOURCE\n1=VICTIM\n\
         [AircraftTypes]\n0=PLANE\n\
         [SOURCE]\nStrength=100\nPrimary=PARENT_GUN\nSecondary=CHILD_GUN\nSpeed=4\n\
         [VICTIM]\nStrength=1000\nArmor=none\nSpeed=4\n{victim_keys}\n\
         [PLANE]\nStrength=1000\nArmor=none\nSpeed=10\n\
         [PARENT_GUN]\nProjectile=PARENT_SHOT\nWarhead=QUIET\nDamage=0\nRange=10\nSpeed=40\n\
         [CHILD_GUN]\nProjectile=CHILD_SHOT\nWarhead=CHILD_WH\nDamage=10\nSpeed=50\n\
         Bright=yes\nRange=8\n{child_weapon}\n\
         [PARENT_SHOT]\n{parent}\n\
         [CHILD_SHOT]\n{child_projectile}\n\
         [Warheads]\n0=QUIET\n1=CHILD_WH\n\
         [QUIET]\nCellSpread=0\n{VERSES_ZERO}\n\
         [CHILD_WH]\nCellSpread=0\n{child_verses}\n{warhead}\n\
         {extra}\n",
    );
    crate::sim::arena_fixture::clear_ground_ini(&text)
}

enum TargetChoice {
    Cell(u16, u16),
    Entity(usize),
}

struct Scene {
    world: Simulation,
    rules: RuleSet,
    source: u64,
    parent: u64,
    request: ProjectileDetonation,
    victims: Vec<u64>,
}

#[allow(clippy::too_many_arguments)]
fn build_scene(
    ini: &IniFile,
    source_cell: (u16, u16),
    parent_cell: (u16, u16),
    parent_z: i32,
    target: TargetChoice,
    victims: &[(&str, u16, u16, i32)],
    parent_firepower: f64,
) -> Scene {
    let mut rules = RuleSet::from_ini(ini).unwrap();
    rules.set_weapon_speed_for_test("CHILD_GUN", 50);
    rules.set_weapon_speed_for_test("PARENT_GUN", 40);
    let mut world = crate::sim::arena_fixture::rival_arena(&rules, 31);
    world.intern_rule_type_ids(&rules);
    world.resolve_type_handles(&rules);
    world.session.set_binary_frame_for_test(100);
    let source = world
        .spawn_object(
            "SOURCE",
            "Americans",
            source_cell.0,
            source_cell.1,
            0,
            &rules,
        )
        .unwrap();
    world.add_entity_occupancy(source);
    world
        .substrate
        .entities
        .get_mut(source)
        .unwrap()
        .set_in_playfield(true);
    let mut victim_ids = Vec::new();
    for &(kind, rx, ry, height) in victims {
        let id = world
            .spawn_object(kind, "Russians", rx, ry, 0, &rules)
            .unwrap();
        world.add_entity_occupancy(id);
        world
            .substrate
            .entities
            .get_mut(id)
            .unwrap()
            .set_in_playfield(true);
        if world.substrate.entities.get(id).unwrap().category()
            == crate::map::entities::EntityCategory::Aircraft
        {
            crate::sim::world::Simulation::aircraft_tracker_add(
                &world.session,
                &mut world.substrate,
                id,
            );
        }
        if height != 0 {
            crate::sim::world::Simulation::set_object_height_unmarked(
                &mut world.substrate,
                &world.resolved_terrain,
                id,
                height,
            );
        }
        victim_ids.push(id);
    }
    let target = match target {
        TargetChoice::Cell(rx, ry) => ProjectileTarget::Cell { rx, ry },
        TargetChoice::Entity(index) => ProjectileTarget::Entity(victim_ids[index]),
    };
    let payload = ProjectilePayload::new(
        0,
        world.interner.intern("QUIET"),
        Some(world.interner.intern("PARENT_SHOT")),
        true,
    )
    .with_firepower_multiplier(NativeF64Bits::from_bits(parent_firepower.to_bits()));
    let parent = world.construct_projectile(source, target, payload, &rules);
    let position = ProjectileCoord::new(
        i32::from(parent_cell.0) * 256 + 128,
        i32::from(parent_cell.1) * 256 + 128,
        parent_z,
    );
    let target_position = match target {
        ProjectileTarget::Entity(id) => {
            let coords = crate::sim::movement::ground_pose::object_get_coords(
                world.substrate.entities.get(id).unwrap(),
                world.resolved_terrain.as_ref(),
            );
            ProjectileCoord::new(coords.x, coords.y, coords.z)
        }
        ProjectileTarget::Cell { rx, ry } => {
            crate::sim::projectile::cell_target_coord(world.resolved_terrain.as_ref(), rx, ry)
        }
        _ => position,
    };
    let parent_projectile = world.projectiles.get_mut(parent).unwrap();
    parent_projectile.set_position_for_test(position);
    parent_projectile.set_last_target_position_for_test(target_position);
    let request = ProjectileDetonation {
        projectile_id: parent,
        source_id: source,
        target,
        impact: ProjectileCoord::new(position.x + 11, position.y + 11, position.z + 10),
        payload,
        reason: crate::sim::projectile::ProjectileDetonationReason::ReachedTarget,
    };
    Scene {
        world,
        rules,
        source,
        parent,
        request,
        victims: victim_ids,
    }
}

fn detonate(scene: &mut Scene) {
    let mut run = crate::sim::combat::world_receiver::ReceiverRun::default();
    crate::sim::combat::world_receiver::commit_projectiles(
        &mut scene.world,
        &mut run,
        &[scene.request],
        &scene.rules,
        None,
    );
    run.finish();
}

fn child_ids(scene: &Scene) -> Vec<u64> {
    scene
        .world
        .projectiles
        .iter()
        .filter(|(id, _)| **id != scene.parent)
        .map(|(id, _)| *id)
        .collect()
}

fn child_targets(scene: &Scene) -> Vec<ProjectileTarget> {
    child_ids(scene)
        .into_iter()
        .map(|id| scene.world.projectiles.get(id).unwrap().target())
        .collect()
}

fn child_damage(scene: &Scene) -> Vec<i32> {
    child_ids(scene)
        .into_iter()
        .map(|id| {
            scene
                .world
                .projectiles
                .get(id)
                .unwrap()
                .payload()
                .base_damage
        })
        .collect()
}

fn component(velocity: ProjectileVelocity, index: usize) -> f64 {
    f64::from_bits(velocity.native()[index].bits())
}

#[test]
fn production_reader_stores_every_key_and_global_fallback() {
    let ini = rules_ini(
        "Splits=yes\nRetargetAccuracy=50%\nRetargetSelf=no\n\
         RetargetSelf.Probability=25%\nSplits.TargetingDistance=2.5\n\
         Splits.TargetCellRange=-1\nAirburstSpread=3\n\
         Airburst.RandomClusters=yes\nAirburst.TargetAsSource=yes\n\
         AirburstWeapon.SourceScatterMin=1.5\nAirburstWeapon.SourceScatterMax=2\n\
         AirburstWeapon.RadialFireSegments=4\nAroundTarget=no\n",
        "CanTargetHouses=enemies\n",
        "AA=yes\nAAOnly=yes\n",
        "",
        "Splits.AllowRepeatTargets=yes\nSplits.UseWeaponTargeting=yes\n\
         Airburst.UseCluster=yes\nAirburst.TargetAsSource.SkipHeight=yes\n\
         AirburstWeapon.ApplyFirepowerMult=yes\n\
         AirburstWeapon.UseFiringEffects=yes\nAirburstWeapon.HeadToTarget=yes\n",
        "",
    );
    let rules = RuleSet::from_ini(&ini).unwrap();
    let projectile = rules.projectile("PARENT_SHOT").unwrap();
    let value = &projectile.phobos_airburst;
    assert!(value.splits);
    assert_eq!(value.retarget_accuracy, 0.5);
    assert!(!value.retarget_self);
    assert_eq!(value.retarget_self_probability, 0.25);
    assert_eq!(value.splits_targeting_distance, 640);
    assert_eq!(value.splits_target_cell_range, -1);
    assert_eq!(value.airburst_spread, 3.0);
    assert!(value.airburst_random_clusters);
    assert!(value.airburst_target_as_source);
    assert_eq!(value.airburst_weapon_source_scatter_min, 384);
    assert_eq!(value.airburst_weapon_source_scatter_max, 512);
    assert_eq!(value.airburst_weapon_radial_fire_segments, 4);
    assert_eq!(value.around_target, Some(false));
    assert!(
        rules
            .projectile("CHILD_SHOT")
            .unwrap()
            .phobos_airburst
            .aa_only
    );
    let globals = &rules.phobos_airburst;
    assert!(value.allow_repeat_targets(globals));
    assert!(value.use_weapon_targeting(globals));
    assert!(value.airburst_use_cluster(globals));
    assert!(value.skip_height(globals));
    assert!(value.apply_firepower_mult(globals));
    assert!(value.use_firing_effects(globals));
    assert!(value.head_to_target(globals));
    assert_eq!(
        rules.weapon("CHILD_GUN").unwrap().can_target_houses(),
        crate::rules::phobos_shields::AffectedHouse::ENEMIES
    );
}

#[test]
fn default_airburst_spread_is_the_native_three_by_three_area() {
    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=2\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        600,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    detonate(&mut scene);
    let targets = child_targets(&scene);
    assert_eq!(targets.len(), 9, "AirburstSpread defaults to 1.5 cells");
    let mut cells: Vec<(u16, u16)> = targets
        .iter()
        .map(|target| match target {
            ProjectileTarget::Cell { rx, ry } => (*rx, *ry),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    cells.sort_unstable();
    let mut expected = Vec::new();
    for ry in 19..=21 {
        for rx in 9..=11 {
            expected.push((rx, ry));
        }
    }
    expected.sort_unstable();
    assert_eq!(cells, expected, "Cluster is ignored without UseCluster");
}

#[test]
fn airburst_use_cluster_selects_even_and_random_cells() {
    for random in [false, true] {
        let random_key = if random {
            "Airburst.RandomClusters=yes\n"
        } else {
            ""
        };
        let ini = rules_ini(
            &format!(
                "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=3\n\
                 AirburstSpread=2\nAirburst.UseCluster=yes\n{random_key}"
            ),
            "",
            "",
            "",
            "",
            "",
        );
        let mut scene = build_scene(
            &ini,
            (6, 20),
            (10, 20),
            0,
            TargetChoice::Cell(10, 20),
            &[],
            1.0,
        );
        detonate(&mut scene);
        let targets = child_targets(&scene);
        assert_eq!(targets.len(), 3, "random={random}");
        let mut unique: Vec<String> = targets.iter().map(|target| format!("{target:?}")).collect();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 3, "random={random}");
    }
}

#[test]
fn target_as_source_moves_the_origin_and_skip_height_keeps_the_bullet_z() {
    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         AirburstSpread=1.5\nAirburst.UseCluster=yes\n\
         Airburst.TargetAsSource=yes\n",
        "",
        "",
        "",
        "Airburst.TargetAsSource.SkipHeight=yes\n",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        600,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    detonate(&mut scene);
    let ids = child_ids(&scene);
    assert_eq!(ids.len(), 1);
    let origin = scene.world.projectiles.get(ids[0]).unwrap().launch_origin();
    assert_eq!(origin.z, 600, "SkipHeight keeps the projectile's height");
    assert_ne!(
        (origin.x, origin.y),
        (10 * 256 + 128, 20 * 256 + 128),
        "TargetAsSource uses the selected cell, not the detonation point"
    );

    // Without SkipHeight the target cell's ground Z is used.
    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         AirburstSpread=1.5\nAirburst.UseCluster=yes\n\
         Airburst.TargetAsSource=yes\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        600,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    detonate(&mut scene);
    let ids = child_ids(&scene);
    assert_eq!(ids.len(), 1);
    assert_eq!(
        scene
            .world
            .projectiles
            .get(ids[0])
            .unwrap()
            .launch_origin()
            .z,
        0
    );
}

#[test]
fn splits_targets_nearby_technos_and_excludes_the_source() {
    let ini = rules_ini(
        "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=2\n\
         Splits.TargetingDistance=5\nSplits.TargetCellRange=-1\nRetargetSelf=no\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Entity(0),
        &[("VICTIM", 10, 20, 0), ("VICTIM", 12, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    let targets = child_targets(&scene);
    assert_eq!(targets.len(), 2);
    assert!(targets.contains(&ProjectileTarget::Entity(scene.victims[0])));
    assert!(targets.contains(&ProjectileTarget::Entity(scene.victims[1])));
}

#[test]
fn retarget_accuracy_one_keeps_the_original_target() {
    let ini = rules_ini(
        "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=2\nRetargetAccuracy=1.0\n\
         Splits.TargetingDistance=5\nSplits.TargetCellRange=-1\nRetargetSelf=no\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Entity(0),
        &[("VICTIM", 10, 20, 0), ("VICTIM", 12, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    let targets = child_targets(&scene);
    assert_eq!(targets.len(), 2);
    assert!(
        targets
            .iter()
            .all(|target| *target == ProjectileTarget::Entity(scene.victims[0])),
        "RetargetAccuracy=1 must keep the existing target"
    );
}

#[test]
fn retarget_self_controls_whether_the_firer_is_a_candidate() {
    let ini = rules_ini(
        "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         Splits.TargetingDistance=5\nSplits.TargetCellRange=-1\nRetargetSelf=no\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (9, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    detonate(&mut scene);
    assert!(
        child_ids(&scene).is_empty(),
        "RetargetSelf=no leaves no Techno target"
    );

    let ini = rules_ini(
        "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         Splits.TargetingDistance=5\nSplits.TargetCellRange=-1\n\
         RetargetSelf=yes\nRetargetSelf.Probability=1.0\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (9, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    detonate(&mut scene);
    let targets = child_targets(&scene);
    assert_eq!(targets, vec![ProjectileTarget::Entity(scene.source)]);
}

#[test]
fn allow_repeat_targets_cycles_instead_of_adding_random_cells() {
    let base = "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=3\n\
                Splits.TargetingDistance=5\nRetargetSelf=no\n";
    let mut scene = build_scene(
        &rules_ini(
            &format!("{base}Splits.TargetCellRange=2\n"),
            "",
            "",
            "",
            "",
            "",
        ),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Entity(0),
        &[("VICTIM", 10, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    let targets = child_targets(&scene);
    assert_eq!(targets.len(), 3);
    assert_eq!(
        targets
            .iter()
            .filter(|target| **target == ProjectileTarget::Entity(scene.victims[0]))
            .count(),
        1,
        "without repeats the CellRange cells fill the remaining cluster slots"
    );

    let mut scene = build_scene(
        &rules_ini(
            &format!("{base}Splits.TargetCellRange=-1\nSplits.AllowRepeatTargets=yes\n"),
            "",
            "",
            "",
            "",
            "",
        ),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Entity(0),
        &[("VICTIM", 10, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    let targets = child_targets(&scene);
    assert_eq!(targets.len(), 3);
    assert!(
        targets
            .iter()
            .all(|target| *target == ProjectileTarget::Entity(scene.victims[0])),
        "AllowRepeatTargets cycles the single target"
    );
}

#[test]
fn weapon_targeting_overrides_warhead_affects_and_reads_can_target_houses() {
    let base = "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
                Splits.TargetingDistance=5\nSplits.TargetCellRange=-1\nRetargetSelf=no\n";
    let warhead = "AffectsEnemies=no\n";
    let mut scene = build_scene(
        &rules_ini(base, "", "", warhead, "", ""),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[("VICTIM", 10, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    assert!(
        child_ids(&scene).is_empty(),
        "without UseWeaponTargeting the warhead AffectsEnemies gate applies"
    );

    let mut scene = build_scene(
        &rules_ini(
            &format!("{base}Splits.UseWeaponTargeting=yes\n"),
            "",
            "",
            warhead,
            "",
            "",
        ),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[("VICTIM", 10, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    let targets = child_targets(&scene);
    assert_eq!(targets, vec![ProjectileTarget::Entity(scene.victims[0])]);

    let mut scene = build_scene(
        &rules_ini(
            &format!("{base}Splits.UseWeaponTargeting=yes\n"),
            "CanTargetHouses=allies\n",
            "",
            warhead,
            "",
            "",
        ),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[("VICTIM", 10, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    assert!(child_ids(&scene).is_empty(), "CanTargetHouses=allies");
}

#[test]
fn split_projectile_aa_and_aa_only_filter_air_and_ground_targets() {
    let base = "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
                Splits.TargetingDistance=5\nSplits.TargetCellRange=-1\n\
                RetargetSelf=no\nSplits.UseWeaponTargeting=yes\n";
    // AA=no refuses an airborne target.
    let mut scene = build_scene(
        &rules_ini(base, "", "", "", "", ""),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[("PLANE", 10, 20, 300)],
        1.0,
    );
    detonate(&mut scene);
    assert!(child_ids(&scene).is_empty());

    // AA=yes accepts it.
    let mut scene = build_scene(
        &rules_ini(base, "", "AA=yes\n", "", "", ""),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[("PLANE", 10, 20, 300)],
        1.0,
    );
    detonate(&mut scene);
    assert_eq!(child_targets(&scene).len(), 1);

    // AA=yes + AAOnly=yes refuses a grounded target.
    let mut scene = build_scene(
        &rules_ini(base, "", "AA=yes\nAAOnly=yes\n", "", "", ""),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[("VICTIM", 10, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    assert!(child_ids(&scene).is_empty());
}

#[test]
fn apply_firepower_mult_scales_the_child_damage() {
    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         AirburstSpread=1.5\nAirburst.UseCluster=yes\n\
         AirburstWeapon.ApplyFirepowerMult=yes\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[],
        2.0,
    );
    detonate(&mut scene);
    assert_eq!(child_damage(&scene), vec![20]);

    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         AirburstSpread=1.5\nAirburst.UseCluster=yes\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[],
        2.0,
    );
    detonate(&mut scene);
    assert_eq!(child_damage(&scene), vec![10]);
}

#[test]
fn radial_fire_segments_fan_around_the_original_target() {
    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=2\n\
         AirburstSpread=1.5\nAirburst.UseCluster=yes\n\
         AirburstWeapon.HeadToTarget=yes\nAirburstWeapon.RadialFireSegments=2\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    detonate(&mut scene);
    let ids = child_ids(&scene);
    assert_eq!(ids.len(), 2);
    let first = scene.world.projectiles.get(ids[0]).unwrap().velocity();
    let second = scene.world.projectiles.get(ids[1]).unwrap().velocity();
    // Segment 0 points at -90 degrees from the target direction, segment 1
    // at 0 degrees; the retail trig table keeps a 0.0015 rad quarter-turn
    // residue, well below a visible direction change.
    assert!(component(first, 0).abs() < 0.1);
    assert!((component(first, 1) - 50.0).abs() < 0.1);
    assert!((component(second, 0) - 50.0).abs() < 0.1);
    assert!(component(second, 1).abs() < 0.1);
}

#[test]
fn source_scatter_moves_the_child_origin_around_the_parent() {
    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         AirburstSpread=1.5\nAirburst.UseCluster=yes\n\
         AirburstWeapon.SourceScatterMin=2\nAirburstWeapon.SourceScatterMax=2\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    detonate(&mut scene);
    let ids = child_ids(&scene);
    assert_eq!(ids.len(), 1);
    let origin = scene.world.projectiles.get(ids[0]).unwrap().launch_origin();
    let dx = f64::from(origin.x - (10 * 256 + 128));
    let dy = f64::from(origin.y - (20 * 256 + 128));
    let distance = (dx * dx + dy * dy).sqrt();
    assert!(
        (500.0..=524.0).contains(&distance),
        "the scatter distance is 512 leptons, got {distance}"
    );
    assert_eq!(origin.z, 0);
}

#[test]
fn firing_effects_play_the_child_weapon_anim_and_report() {
    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         AirburstSpread=1.5\nAirburst.UseCluster=yes\n",
        "Anim=GUNFIRE\nReport=GUNRPT\n",
        "",
        "",
        "AirburstWeapon.UseFiringEffects=yes\n",
        "[Animations]\n0=GUNFIRE\n",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    let mut art = ArtRegistry::from_ini(&IniFile::from_str("[GUNFIRE]\nRate=900\n"));
    art.bind_anim_frame_count_for_test("GUNFIRE", 6);
    scene.rules.install_art_fixture(art);
    detonate(&mut scene);
    assert!(
        scene.world.anims().count() >= 1,
        "weapon Anim is constructed"
    );
    assert!(
        scene.world.sound_events().iter().any(|event| match event {
            crate::sim::world::SimSoundEvent::VocAt { sound_id, .. } => sound_id == "GUNRPT",
            _ => false,
        }),
        "weapon Report is played"
    );

    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         AirburstSpread=1.5\nAirburst.UseCluster=yes\n",
        "Anim=GUNFIRE\nReport=GUNRPT\n",
        "",
        "",
        "",
        "[Animations]\n0=GUNFIRE\n",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    let mut art = ArtRegistry::from_ini(&IniFile::from_str("[GUNFIRE]\nRate=900\n"));
    art.bind_anim_frame_count_for_test("GUNFIRE", 6);
    scene.rules.install_art_fixture(art);
    detonate(&mut scene);
    assert_eq!(scene.world.anims().count(), 0);
    assert!(!scene.world.sound_events().iter().any(|event| match event {
        crate::sim::world::SimSoundEvent::VocAt { sound_id, .. } => sound_id == "GUNRPT",
        _ => false,
    }));
}

#[test]
fn around_target_chooses_between_the_original_target_and_the_impact() {
    let base = "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
                Splits.TargetingDistance=2\nSplits.TargetCellRange=-1\nRetargetSelf=no\n";
    let victims = [("VICTIM", 10, 22, 0)];
    let mut scene = build_scene(
        &rules_ini(base, "", "", "", "", ""),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(20, 20),
        &victims,
        1.0,
    );
    detonate(&mut scene);
    assert!(
        child_ids(&scene).is_empty(),
        "AroundTarget defaults to Splits"
    );

    let mut scene = build_scene(
        &rules_ini(&format!("{base}AroundTarget=no\n"), "", "", "", "", ""),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(20, 20),
        &victims,
        1.0,
    );
    detonate(&mut scene);
    assert_eq!(
        child_targets(&scene),
        vec![ProjectileTarget::Entity(scene.victims[0])]
    );
}

#[test]
fn weapon_targeting_checks_legal_target_and_warhead_verses() {
    let base = "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
                Splits.TargetingDistance=5\nSplits.TargetCellRange=-1\n\
                RetargetSelf=no\nSplits.UseWeaponTargeting=yes\n";
    let mut scene = build_scene(
        &rules_ini_full(base, "", "", "", "", "", VERSES_ZERO, ""),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[("VICTIM", 10, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    assert!(
        child_ids(&scene).is_empty(),
        "zero Verses rejects the target"
    );

    let mut scene = build_scene(
        &rules_ini_full(base, "", "", "", "", "", VERSES_FULL, "LegalTarget=no\n"),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[("VICTIM", 10, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    assert!(
        child_ids(&scene).is_empty(),
        "LegalTarget=no rejects the target"
    );
}

#[test]
fn targeting_distance_cylindrical_ignores_aircraft_height() {
    let base = "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
                Splits.TargetingDistance=5\nSplits.TargetCellRange=-1\n\
                RetargetSelf=no\nSplits.UseWeaponTargeting=yes\n";
    // 4 cells horizontal plus 2600 leptons of height exceeds 5 cells in 3-D
    // even after Phobos' quarter-squared-distance aircraft discount.
    let targets = [("PLANE", 14, 20, 2600)];
    let mut scene = build_scene(
        &rules_ini(base, "", "AA=yes\n", "", "", ""),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &targets,
        1.0,
    );
    detonate(&mut scene);
    assert!(
        child_ids(&scene).is_empty(),
        "3-D distance rejects the plane"
    );

    let mut scene = build_scene(
        &rules_ini(
            &format!("{base}Splits.TargetingDistance.Cylindrical=yes\n"),
            "",
            "AA=yes\n",
            "",
            "",
            "",
        ),
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &targets,
        1.0,
    );
    detonate(&mut scene);
    assert_eq!(child_targets(&scene).len(), 1);
}

#[test]
fn splits_takes_the_airburst_impact_arms() {
    for (parent, expected) in [
        (
            "Splits=yes
AirburstWeapon=CHILD_GUN
",
            true,
        ),
        (
            "Airburst=yes
AirburstWeapon=CHILD_GUN
",
            true,
        ),
        ("", false),
    ] {
        let ini = rules_ini(parent, "", "", "", "", "");
        let rules = RuleSet::from_ini(&ini).unwrap();
        let delivery = crate::sim::combat::projectile_delivery::classify_projectile_delivery(
            rules.weapon("PARENT_GUN").unwrap(),
            &rules,
        );
        assert_eq!(
            delivery.collision.airburst, expected,
            "Phobos Explodes makes Splits use the Airburst arm"
        );
    }
}

#[test]
fn cell_spread_enumerator_starts_at_the_center_and_covers_the_first_ring() {
    let offsets = cell_spread_offsets(1);
    assert_eq!(offsets[0], (0, 0));
    assert_eq!(offsets.len(), 9);
    let mut expected = Vec::new();
    for y in -1..=1 {
        for x in -1..=1 {
            expected.push((x, y));
        }
    }
    expected.sort_unstable();
    let mut actual = offsets;
    actual.sort_unstable();
    assert_eq!(actual, expected);
}

#[test]
fn simulated_inviso_child_stays_at_its_launch_origin() {
    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         AirburstSpread=1.5\nAirburst.UseCluster=yes\n",
        "",
        "Inviso=yes\n",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    detonate(&mut scene);
    let ids = child_ids(&scene);
    assert_eq!(ids.len(), 1);
    let child = scene.world.projectiles.get(ids[0]).unwrap();
    assert_eq!(
        (child.position().x, child.position().y),
        (child.launch_origin().x, child.launch_origin().y),
        "MoveTo does not take Fire's Inviso snap-to-target arm"
    );
    let ProjectileTarget::Cell { rx, ry } = child.target() else {
        panic!("expected a cell target");
    };
    assert_ne!(
        (child.position().x, child.position().y),
        (i32::from(rx) * 256 + 128, i32::from(ry) * 256 + 128),
        "Fire's Inviso arm would stand the child on its target"
    );
}

#[test]
fn splits_detonates_once_and_emits_only_its_cluster() {
    let ini = rules_ini(
        "Splits=yes\nAirburstWeapon=CHILD_GUN\nCluster=3\n\
         Splits.TargetingDistance=5\nSplits.TargetCellRange=2\nRetargetSelf=no\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Entity(0),
        &[("VICTIM", 10, 20, 0)],
        1.0,
    );
    detonate(&mut scene);
    assert_eq!(
        child_ids(&scene).len(),
        3,
        "a Splits parent takes one detonation, not Cluster detonations"
    );
}

#[test]
#[cfg(all(feature = "phobos", feature = "ares"))]
fn phobos_replaces_ares_airburst() {
    let ini = rules_ini(
        "Airburst=yes\nAirburstWeapon=CHILD_GUN\nCluster=1\n\
         AirburstSpread=1.5\nAirburst.UseCluster=yes\n",
        "",
        "",
        "",
        "",
        "",
    );
    let mut scene = build_scene(
        &ini,
        (6, 20),
        (10, 20),
        0,
        TargetChoice::Cell(10, 20),
        &[],
        1.0,
    );
    detonate(&mut scene);
    assert_eq!(
        child_ids(&scene).len(),
        1,
        "Phobos' airburst hook takes precedence over Ares' replaced tail"
    );
}
