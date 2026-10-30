//! Phobos `BulletExt::FirerHouse` retention regressions.

use super::*;
use crate::rules::ini_parser::IniFile;
use crate::sim::projectile::{
    ProjectileCoord, ProjectileDetonation, ProjectileDetonationReason, ProjectilePayload,
    ProjectileTarget,
};

fn scene() -> (Simulation, RuleSet, u64, u64, InternedId) {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[VehicleTypes]\n0=SOURCE\n[SOURCE]\nPrimary=GUN\n\
         [GUN]\nProjectile=SHOT\nWarhead=WH\nDamage=10\n\
         [Warheads]\n0=WH\n[WH]\n",
    ))
    .unwrap();
    let mut world = Simulation::with_seed(7);
    world.intern_rule_type_ids(&rules);
    world.resolve_type_handles(&rules);
    let source = world
        .spawn_object("SOURCE", "Americans", 5, 5, 0, &rules)
        .unwrap();
    let warhead = world.interner.intern("WH");
    let payload = ProjectilePayload::new(10, warhead, None, false);
    let bullet = world.construct_projectile(source, ProjectileTarget::None, payload, &rules);
    (world, rules, source, bullet, warhead)
}

#[test]
fn bullet_retains_its_firers_house_after_the_owner_expires() {
    let (mut world, _rules, source, bullet, warhead) = scene();
    let americans = world.interner.intern("Americans");
    assert_eq!(
        world.projectiles.get(bullet).unwrap().phobos_firer_house(),
        Some(americans)
    );
    let detonation = ProjectileDetonation {
        projectile_id: bullet,
        source_id: source,
        target: ProjectileTarget::None,
        impact: ProjectileCoord::new(0, 0, 0),
        payload: ProjectilePayload::new(10, warhead, None, false),
        reason: ProjectileDetonationReason::ReachedTarget,
    };
    assert_eq!(
        detonation_source_house(&world, &detonation),
        Some(americans)
    );

    world.substrate.entities.remove(source);
    assert_eq!(
        detonation_source_house(&world, &detonation),
        Some(americans),
        "the retained House stands in for the expired owner"
    );
}

#[test]
fn reconstruct_without_a_live_owner_keeps_the_saved_house() {
    let (mut world, rules, source, bullet, _) = scene();
    let americans = world.interner.intern("Americans");
    world.substrate.entities.remove(source);
    Simulation::construct_bullet_scheme(
        &world.interner,
        &world.substrate,
        &mut world.projectiles,
        bullet,
        &rules,
    );
    assert_eq!(
        world.projectiles.get(bullet).unwrap().phobos_firer_house(),
        Some(americans),
        "InitializeState writes only when the Owner exists"
    );
}
