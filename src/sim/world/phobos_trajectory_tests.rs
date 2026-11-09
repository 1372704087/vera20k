//! `Trajectory.Straight` flight through the production bullet store and the
//! world's projectile collision owner.
use super::*;
use crate::sim::projectile::{
    ProjectileCollisionPolicy, ProjectileSpawn, ProjectileTrajectory, ProjectileVisualState,
    TargetExpiryPolicy,
};

const HEAD: &str = "[General]\n[Countries]\n0=Americans\n1=Russians\n\
    [InfantryTypes]\n[VehicleTypes]\n0=TGT\n1=BLOCK\n[AircraftTypes]\n\
    [BuildingTypes]\n\
    [Warheads]\n0=WH\n1=PSST\n\
    [WH]\n[PSST]\n\
    [TGT]\nStrength=100\nArmor=light\nSpeed=4\nPrimary=GUN\n\
    Locomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\n\
    [BLOCK]\nStrength=100\nArmor=light\nSpeed=4\nPrimary=GUN\n\
    Locomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\n\
    [GUN]\nDamage=10\nROF=20\nRange=8\nProjectile=PROJ\nSpeed=20\nWarhead=WH\nBurst=1\n";
const TAIL: &str = "";

struct Scene {
    sim: Simulation,
    rules: RuleSet,
}

fn fixture(proj_keys: &str) -> Scene {
    let text = format!("{HEAD}[PROJ]\n{proj_keys}{TAIL}");
    let rules = RuleSet::from_ini(&crate::sim::arena_fixture::clear_ground_ini(&text))
        .expect("trajectory rules");
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 0x0046_66E0);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.session.set_binary_frame_for_test(0);
    Scene { sim, rules }
}

/// Fire the fixture bullet at `origin`, aimed at `target_position`.
fn fire(
    scene: &mut Scene,
    target: ProjectileTarget,
    origin: ProjectileCoord,
    target_position: ProjectileCoord,
) -> u64 {
    let projectile_type = scene.sim.interner.intern("PROJ");
    let warhead = scene.sim.interner.intern("WH");
    let spawn = ProjectileSpawn {
        native_unique_id: 1,
        line_trail: None,
        flat: false,
        source_id: 0,
        origin,
        target,
        initial_target_position: target_position,
        payload: ProjectilePayload::new(10, warhead, Some(projectile_type), false),
        speed_leptons_per_frame: 16,
        velocity: ProjectileVelocity::new(0, 0, 0),
        trajectory: ProjectileTrajectory::Ballistic,
        guidance: None,
        visual: ProjectileVisualState::new(0, 0, 0),
        arm_frames: 0,
        fuse_frames: None,
        ranged_fuse: false,
        tracks_target: false,
        target_expiry: TargetExpiryPolicy::DetonateAtLastKnown,
        collision: ProjectileCollisionPolicy::NONE,
    };
    let id = scene.sim.projectiles.spawn_at(1, 0, spawn, origin);
    let weapon = scene.rules.weapon("GUN").expect("weapon").clone();
    scene.sim.init_phobos_trajectory(id, &scene.rules, &weapon);
    id
}

fn cell_center(cell: (u16, u16)) -> ProjectileCoord {
    ProjectileCoord::new(
        i32::from(cell.0) * 256 + 128,
        i32::from(cell.1) * 256 + 128,
        0,
    )
}

fn velocity_of(scene: &Scene, id: u64) -> ProjectileVelocity {
    scene.sim.projectiles.get(id).expect("bullet").velocity()
}

fn f64_of(bits: NativeF64Bits) -> f64 {
    f64::from_bits(bits.bits())
}

#[test]
fn straight_flies_at_trajectory_speed_and_clamps_it() {
    let mut scene = fixture("Trajectory=Straight\nTrajectory.Speed=16\n");
    let origin = cell_center((22, 10));
    let id = fire(
        &mut scene,
        ProjectileTarget::Cell { rx: 28, ry: 10 },
        origin,
        cell_center((28, 10)),
    );
    for _ in 0..3 {
        let step = scene.sim.step_phobos_trajectory(id, &scene.rules, None);
        assert!(step.detonations.is_empty(), "still far from the target");
    }
    let position = scene.sim.projectiles.get(id).expect("bullet").position();
    assert_eq!(position.x - origin.x, 3 * 16);
    assert_eq!(position.y, origin.y);

    // `Math::max(0.001, x)` keeps a zero or negative speed moving.
    for speed in ["0", "-4"] {
        let mut scene = fixture(&format!("Trajectory=Straight\nTrajectory.Speed={speed}\n"));
        let id = fire(
            &mut scene,
            ProjectileTarget::Cell { rx: 28, ry: 10 },
            origin,
            cell_center((28, 10)),
        );
        let velocity = velocity_of(&scene, id);
        assert!(
            (f64_of(velocity.x) - 0.001).abs() < 1e-12,
            "speed {speed} clamps to 0.001"
        );
    }
}

#[test]
fn detonation_distance_detonates_and_snaps_to_the_target() {
    let mut scene = fixture(
        "Trajectory=Straight\nTrajectory.Speed=64\n\
         Trajectory.Straight.DetonationDistance=0.4\n\
         Trajectory.Straight.TargetSnapDistance=0.5\n",
    );
    let target = scene
        .sim
        .spawn_object("TGT", "Americans", 26, 10, 0, &scene.rules)
        .expect("target");
    let origin = cell_center((24, 10));
    let id = fire(
        &mut scene,
        ProjectileTarget::Entity(target),
        origin,
        cell_center((26, 10)),
    );
    let target_coords = crate::sim::movement::ground_pose::object_get_coords(
        scene.sim.substrate.entities.get(target).expect("target"),
        scene.sim.resolved_terrain.as_ref(),
    );
    let mut detonation = None;
    for _ in 0..12 {
        let step = scene.sim.step_phobos_trajectory(id, &scene.rules, None);
        if let Some(detonation_) = step.detonations.first().copied() {
            detonation = Some(detonation_);
            break;
        }
    }
    let detonation = detonation.expect("the bullet detonates inside 0.4 cells");
    assert_eq!(
        detonation.reason,
        ProjectileDetonationReason::PhobosTrajectory
    );
    assert_eq!(
        (
            detonation.impact.x,
            detonation.impact.y,
            detonation.impact.z
        ),
        (target_coords.x, target_coords.y, target_coords.z),
        "TargetSnapDistance snaps the impact onto the target"
    );
}

#[test]
fn pass_detonate_hits_with_its_warhead_and_damage_on_the_delay() {
    let mut scene = fixture(
        "Trajectory=Straight\nTrajectory.Speed=16\n\
         Trajectory.Straight.PassThrough=yes\n\
         Trajectory.Straight.DetonationDistance=0\n\
         Trajectory.Straight.PassDetonate=yes\n\
         Trajectory.Straight.PassDetonateWarhead=PSST\n\
         Trajectory.Straight.PassDetonateDamage=7\n\
         Trajectory.Straight.PassDetonateDelay=1\n\
         Trajectory.Straight.PassDetonateInitialDelay=0\n",
    );
    let id = fire(
        &mut scene,
        ProjectileTarget::Cell { rx: 28, ry: 10 },
        cell_center((22, 10)),
        cell_center((28, 10)),
    );
    let psst = scene.sim.interner.intern("PSST");
    let step = scene.sim.step_phobos_trajectory(id, &scene.rules, None);
    let detonation = step
        .detonations
        .iter()
        .find(|detonation| detonation.payload.warhead == psst)
        .expect("a pass-through detonation on the first frame");
    assert_eq!(detonation.payload.base_damage, 7);
    assert_eq!(detonation.target, ProjectileTarget::None);

    // The timer restarted: the next frame is quiet, the one after hits again.
    let quiet = scene.sim.step_phobos_trajectory(id, &scene.rules, None);
    assert!(
        !quiet
            .detonations
            .iter()
            .any(|detonation| detonation.payload.warhead == psst),
        "the delay spaces the pass hits"
    );
}

#[test]
fn proximity_hits_an_enemy_along_the_flight_path() {
    let mut scene = fixture(
        "Trajectory=Straight\nTrajectory.Speed=64\n\
         Trajectory.Straight.PassThrough=yes\n\
         Trajectory.Straight.DetonationDistance=0\n\
         Trajectory.Straight.ProximityImpact=1\n\
         Trajectory.Straight.ProximityWarhead=PSST\n\
         Trajectory.Straight.ProximityDamage=5\n\
         Trajectory.Straight.ProximityRadius=1\n",
    );
    let target = scene
        .sim
        .spawn_object("TGT", "Americans", 26, 10, 0, &scene.rules)
        .expect("target");
    let id = fire(
        &mut scene,
        ProjectileTarget::Cell { rx: 28, ry: 10 },
        cell_center((22, 10)),
        cell_center((28, 10)),
    );
    let psst = scene.sim.interner.intern("PSST");
    let mut hit = None;
    for _ in 0..60 {
        let step = scene.sim.step_phobos_trajectory(id, &scene.rules, None);
        if let Some(detonation) = step
            .detonations
            .iter()
            .find(|detonation| detonation.payload.warhead == psst)
        {
            hit = Some(*detonation);
            break;
        }
    }
    let hit = hit.expect("the proximity radius catches the target");
    assert_eq!(hit.target, ProjectileTarget::Entity(target));
    assert_eq!(hit.payload.base_damage, 5);
}

#[test]
fn through_vehicles_off_detonates_on_the_obstacle() {
    let proj_keys = |through: &str| {
        format!(
            "Trajectory=Straight\nTrajectory.Speed=64\n\
             Trajectory.Straight.PassThrough=yes\n\
             Trajectory.Straight.DetonationDistance=0\n\
             Trajectory.Straight.ThroughVehicles={through}\n"
        )
    };
    for (through, expect_hit) in [("no", true), ("yes", false)] {
        let mut scene = fixture(&proj_keys(through));
        let blocker = scene
            .sim
            .spawn_object("BLOCK", "Americans", 25, 10, 0, &scene.rules)
            .expect("blocker");
        let id = fire(
            &mut scene,
            ProjectileTarget::Cell { rx: 28, ry: 10 },
            cell_center((22, 10)),
            cell_center((28, 10)),
        );
        let mut hit = false;
        for _ in 0..40 {
            let step = scene.sim.step_phobos_trajectory(id, &scene.rules, None);
            if let Some(detonation) = step.detonations.first() {
                hit = detonation.target == ProjectileTarget::Entity(blocker)
                    || detonation.target == ProjectileTarget::Cell { rx: 28, ry: 10 };
                break;
            }
        }
        assert_eq!(
            hit, expect_hit,
            "ThroughVehicles={through}: the vehicle blocks or not"
        );
    }
}

#[test]
fn subject_to_ground_detonates_below_the_floor_and_confine_lifts() {
    let mut scene = fixture(
        "Trajectory=Straight\nTrajectory.Speed=16\n\
         Trajectory.Straight.SubjectToGround=yes\n",
    );
    let below = ProjectileCoord::new(22 * 256 + 128, 10 * 256 + 128, -100);
    let id = fire(
        &mut scene,
        ProjectileTarget::Cell { rx: 28, ry: 10 },
        below,
        cell_center((28, 10)),
    );
    let step = scene.sim.step_phobos_trajectory(id, &scene.rules, None);
    assert_eq!(
        step.detonations.len(),
        1,
        "a bullet below the cell floor detonates"
    );

    let mut scene = fixture(
        "Trajectory=Straight\nTrajectory.Speed=16\n\
         Trajectory.Straight.ConfineAtHeight=100\n",
    );
    let id = fire(
        &mut scene,
        ProjectileTarget::Cell { rx: 28, ry: 10 },
        cell_center((22, 10)),
        cell_center((28, 10)),
    );
    let _ = scene.sim.step_phobos_trajectory(id, &scene.rules, None);
    assert!(
        f64_of(velocity_of(&scene, id).z) > 0.0,
        "ConfineAtHeight raises the flight"
    );
}

#[test]
fn a_bullet_without_the_trajectory_keeps_the_stock_arms() {
    let mut scene = fixture("");
    let id = fire(
        &mut scene,
        ProjectileTarget::Cell { rx: 28, ry: 10 },
        cell_center((22, 10)),
        cell_center((28, 10)),
    );
    assert!(
        !scene
            .sim
            .projectiles
            .get(id)
            .expect("bullet")
            .has_phobos_trajectory()
    );
    assert!(
        scene
            .rules
            .projectile("PROJ")
            .expect("type")
            .phobos_trajectory
            .is_stock()
    );
}
