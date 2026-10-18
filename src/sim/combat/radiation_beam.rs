//! Copied straight radiation-beam construction inputs. SpawnRadBeam6FD620
//! supplies these to the non-Abstract registry659110; presentation owns its
//! Tactical-composite progress and drawing. Native evidence is retained in
//! tools/procedural_drawing_oracle/rad_beam.{py,json,md}.

use crate::sim::projectile::ProjectileCoord;
use crate::util::native_x87::NativeF64Bits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadiationBeamBirth {
    pub frame: i32,
    pub from: ProjectileCoord,
    pub to: ProjectileCoord,
    pub z_adjust: i32,
    pub color: [u8; 3],
    pub duration: i32,
    pub amplitude: NativeF64Bits,
}

/// FireAt6FF594..6FF5C6, after the laser/bolt alternatives. The selected
/// chain is ordinary non-Temporal mode0. Chrono mode1, Magna mode2 and
/// eruption kind2 remain separate caller domains of the native family.
pub(crate) fn fired(
    world: &mut crate::sim::world::Simulation,
    rules: &crate::rules::ruleset::RuleSet,
    source_id: u64,
    target: super::TargetKind,
    weapon: &crate::rules::weapon_type::WeaponType,
) {
    if weapon.is_laser || weapon.is_electric_bolt || !weapon.is_rad_beam {
        return;
    }
    if weapon
        .warhead
        .as_deref()
        .and_then(|name| rules.warhead(name))
        .is_some_and(|warhead| warhead.temporal)
    {
        return;
    }
    create(world, rules, source_id, target.projectile_target(world));
}

/// SpawnRadBeam6FD620's mode0: recompute GetFLH(index0, NullCoord) after
/// burst advancement, copy both endpoints and Rules.RadColor, duration15
/// and amplitude40. Neither endpoint is attached; no RNG, ID or timer.
pub(crate) fn create(
    world: &mut crate::sim::world::Simulation,
    rules: &crate::rules::ruleset::RuleSet,
    source_id: u64,
    target: crate::sim::projectile::ProjectileTarget,
) {
    let Some(source) = world.substrate.entities.get(source_id) else {
        return;
    };
    let Some(object) = world.interner.object_type(source.type_ref(), rules) else {
        return;
    };
    let from = super::fire_coord::fire_coordinate(
        world,
        rules,
        &super::fire_coord::FireSource::of_entity(source),
        object,
        0,
        (source.weapon_burst.index() & 1) as u8,
        Default::default(),
    )
    .coord;
    let z_adjust = super::fire_coord::effect_z_adjust(
        world,
        source,
        from,
        super::fire_coord::EffectDepthProjection::ChangedY,
    );
    let Some(to) = super::fire_coord::effect_target_coordinate(world, rules, target) else {
        return;
    };
    let (r, g, b) = rules.radiation.color;
    world
        .lifecycle_outputs
        .push(crate::sim::world::LifecycleOutput::RadiationBeamCreated(
            RadiationBeamBirth {
                frame: world.session.binary_frame as i32,
                from,
                to,
                z_adjust,
                color: [r, g, b],
                duration: 15,
                amplitude: NativeF64Bits::from_bits(40.0_f64.to_bits()),
            },
        ));
}
