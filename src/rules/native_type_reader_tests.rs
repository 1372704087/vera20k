//! Per-pass Type reads the ordered Rules processor alone owns: the TechnoType
//! weapon block, ParticleSystemType HoldsWhat, the 25-byte Image that names a
//! Type's fixed ART section, the references that name another Type, from a
//! Type's rules or fixed ART section or from a RulesClass section, and
//! WeaponType Range.
use super::*;
use crate::rules::art_data::ArtRegistry;
use crate::rules::ruleset::RuleSet;

/// TechnoType ReadINI's weapon block (`0x0071284A..0x00712AAD`) runs on every
/// reached pass with the live TurretCount, WeaponCount and ClearAllWeapons as
/// its defaults, and its ClearAllWeapons clear (`0x00712A8F`) follows either
/// branch.
#[test]
fn weapon_block_reads_each_reached_pass_from_the_live_counts() {
    let mut layers = RulesLayerStack::new(IniFile::from_str(
        "[VehicleTypes]\n0=SWAP\n1=SHRINK\n2=CLEARED\n\
         [SWAP]\nTurretCount=1\nWeaponCount=2\nWeapon1=A\nWeapon2=B\n\
         [SHRINK]\nTurretCount=1\nWeaponCount=3\nWeapon3=C\n\
         [CLEARED]\nTurretCount=1\nWeaponCount=2\nWeapon1=A\nWeapon2=B\nClearAllWeapons=yes\n",
    ));
    layers.push(
        RulesLayerKind::Scenario,
        IniFile::from_str("[SWAP]\nTurretCount=0\nPrimary=P\n[SHRINK]\nWeaponCount=1\n"),
    );
    let rules = RuleSet::from_rules_layers(&layers).unwrap();

    // The second pass takes the Primary branch; slot 1 keeps Weapon2.
    let swap = rules.object("SWAP").unwrap();
    assert_eq!(swap.turret_count, 0);
    assert_eq!((swap.primary(), swap.secondary()), (Some("P"), Some("B")));
    // A smaller WeaponCount stops reading slot 3 without clearing it.
    let shrink = rules.object("SHRINK").unwrap();
    assert_eq!(shrink.weapon_count, 1);
    assert_eq!(shrink.weapon_at(2), Some("C"));
    // ClearAllWeapons nulls slots 0 and 1 after the Weapon%d loop too.
    let cleared = rules.object("CLEARED").unwrap();
    assert_eq!((cleared.primary(), cleared.secondary()), (None, None));
}

/// ParticleSystemType ReadINI (`0x0064430F..0x00644323`) rebinds +294 on every
/// reached pass through ParticleType's factory (`0x00645430`), which answers
/// -1 only for `<none>`: an omitted key binds an empty-named type and `none`
/// a section-less type named `none`, which RuleSet lists with its defaults.
#[test]
fn holds_what_rebinds_on_every_reached_pass() {
    let mut layers = RulesLayerStack::new(IniFile::from_str(
        "[Particles]\n0=Smoke1\n1=Smoke2\n\
         [ParticleSystems]\n0=KEPT\n1=OMITTED\n2=CLEARED\n3=NAMEDNONE\n\
         [Smoke1]\nBehavesLike=Smoke\n[Smoke2]\nBehavesLike=Smoke\n\
         [KEPT]\nHoldsWhat=Smoke1\n[OMITTED]\nHoldsWhat=Smoke1\n\
         [CLEARED]\nHoldsWhat=Smoke1\n[NAMEDNONE]\nHoldsWhat=Smoke1\n",
    ));
    layers.push(
        RulesLayerKind::Scenario,
        IniFile::from_str(
            "[KEPT]\nHoldsWhat=Smoke2\n[OMITTED]\nLifetime=5\n\
             [CLEARED]\nHoldsWhat=<none>\n[NAMEDNONE]\nHoldsWhat=none\n",
        ),
    );
    let processed = layers.process().unwrap();
    let rules = RuleSet::from_processed_rules(&processed).unwrap();
    let holds_what = |system: &str| {
        let id = rules.ps_type_id_by_name(system).unwrap();
        rules.particle_system_type(id).holds_what
    };

    assert_eq!(holds_what("KEPT"), rules.p_type_id_by_name("Smoke2"));
    for system in ["OMITTED", "CLEARED"] {
        assert_eq!(holds_what(system), None, "{system}");
    }
    let named_none = rules.p_type_id_by_name("none");
    assert!(named_none.is_some());
    assert_eq!(holds_what("NAMEDNONE"), named_none);
    assert_eq!(
        processed.ini().section("Particles").unwrap().registry_ids(),
        ["Smoke1", "Smoke2", "", "none"]
    );
}

/// AnimType ReadINI reads `SpawnsParticle=` from the Anim's ART section with
/// no default (`0x00428716`), skips an empty read and hands any other name to
/// the same ParticleType factory (`0x00428730` -> `0x00645430`): `none`
/// allocates a ParticleType named `none`; `<none>` allocates nothing.
#[test]
fn spawns_particle_allocates_through_the_particle_factory() {
    let processed = RulesLayerStack::new(IniFile::from_str(
        "[Animations]\n0=NAMED\n1=NULLED\n2=EMPTY\n",
    ))
    .process_with_fixed_art(&IniFile::from_str(
        "[NAMED]\nSpawnsParticle=none\n[NULLED]\nSpawnsParticle=<none>\n\
         [EMPTY]\nSpawnsParticle=\n",
    ))
    .unwrap();
    let particles = processed
        .ini()
        .section("Particles")
        .expect("SpawnsParticle=none allocates a ParticleType");
    assert_eq!(particles.registry_ids(), ["none"]);
}

/// BuildingType's ToOverlay read finds its fixed ART section by ObjectType+1F8
/// (`0x004610DE`), the 25-byte Image buffer: a longer Image names the section
/// of its first 24 bytes, and the type keeps that cut Image.
#[test]
fn fixed_art_reads_use_the_25_byte_image() {
    let processed = RulesLayerStack::new(IniFile::from_str(
        "[BuildingTypes]\n0=BLD\n[BLD]\nImage=ABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
    ))
    .process_with_fixed_art(&IniFile::from_str(
        "[ABCDEFGHIJKLMNOPQRSTUVWX]\nToOverlay=CUTOVL\n\
         [ABCDEFGHIJKLMNOPQRSTUVWXYZ]\nToOverlay=FULLOVL\n",
    ))
    .unwrap();
    let overlays: Vec<_> = processed
        .native_type_construction_trace()
        .events()
        .iter()
        .filter(|event| event.family() == NativeTypeConstructorFamily::OverlayType)
        .map(|event| event.native_stored_id())
        .collect();
    assert_eq!(overlays, ["CUTOVL"]);
    let rules = RuleSet::from_processed_rules(&processed).unwrap();
    assert_eq!(
        rules.object("BLD").unwrap().image,
        "ABCDEFGHIJKLMNOPQRSTUVWX"
    );
}

/// Every weapon pointer outside the weapon slots goes through
/// `WeaponTypeClass::FindOrAllocate @ 0x00772FA0`, which answers null for
/// `none` and `<none>` (#1559): TechnoType DeathWeapon (`0x00712308`),
/// InfantryType OccupyWeapon/EliteOccupyWeapon (`0x00524132`, `0x0052416B`),
/// `[CombatDamage] DeathWeapon` (`0x0066C59F`), SuperWeaponType WeaponType
/// (`0x006CEA7D`) and BulletType ShrapnelWeapon (`0x0046C305`). A later pass
/// that says `none` clears the weapon; one that omits the key keeps it.
#[test]
fn weapon_references_read_none_as_null() {
    let mut layers = RulesLayerStack::new(IniFile::from_str(
        "[InfantryTypes]\n0=CLEARED\n1=KEPT\n\
         [CLEARED]\nDeathWeapon=Boom\nOccupyWeapon=Gun\nEliteOccupyWeapon=Gun\n\
         [KEPT]\nDeathWeapon=Boom\nOccupyWeapon=Gun\nEliteOccupyWeapon=Gun\n\
         [Boom]\nDamage=1\n[Gun]\nDamage=2\n\
         [CombatDamage]\nDeathWeapon=Boom\n\
         [SuperWeaponTypes]\n0=Nuke\n[Nuke]\nType=MultiMissile\nWeaponType=Boom\n\
         [VehicleTypes]\n0=TANK\n[TANK]\nPrimary=Cannon\n\
         [Cannon]\nProjectile=Shell\n[Shell]\nShrapnelCount=3\nShrapnelWeapon=Gun\n",
    ));
    layers.push(
        RulesLayerKind::Scenario,
        IniFile::from_str(
            "[CLEARED]\nDeathWeapon=none\nOccupyWeapon=<none>\nEliteOccupyWeapon=none\n\
             [KEPT]\nStrength=1\n\
             [CombatDamage]\nDeathWeapon=<none>\n[Nuke]\nWeaponType=none\n\
             [Shell]\nShrapnelWeapon=none\n",
        ),
    );
    let rules = RuleSet::from_rules_layers(&layers).unwrap();

    let weapons = |id: &str| {
        let object = rules.object(id).unwrap();
        (
            object.death_weapon.as_deref(),
            object.occupy_weapon.as_deref(),
            object.elite_occupy_weapon.as_deref(),
        )
    };
    assert_eq!(weapons("CLEARED"), (None, None, None));
    assert_eq!(weapons("KEPT"), (Some("Boom"), Some("Gun"), Some("Gun")));
    assert_eq!(rules.combat_damage.death_weapon, None);
    assert_eq!(rules.super_weapon("Nuke").unwrap().weapon_type, None);
    assert_eq!(rules.projectile("Shell").unwrap().shrapnel_weapon, None);
}

/// TechnoType ReadINI reads `DeploysInto=` (`0x00713279..0x00713297`) through
/// BuildingType's factory (`0x004653C0`), which answers null for `none`: the
/// vehicle then deploys into nothing, where a present name would make it a
/// deployer for every `deploys_into` consumer.
#[test]
fn deploys_into_reads_none_as_null() {
    let mut layers = RulesLayerStack::new(IniFile::from_str(
        "[BuildingTypes]\n0=YARD\n[YARD]\nStrength=1000\n\
         [VehicleTypes]\n0=CLEARED\n1=KEPT\n\
         [CLEARED]\nDeploysInto=YARD\n[KEPT]\nDeploysInto=yard\n",
    ));
    layers.push(
        RulesLayerKind::Scenario,
        IniFile::from_str("[CLEARED]\nDeploysInto=none\n[KEPT]\nStrength=1\n"),
    );
    let rules = RuleSet::from_rules_layers(&layers).unwrap();

    assert_eq!(rules.object("CLEARED").unwrap().deploys_into, None);
    // The factory finds the existing type case-insensitively.
    assert_eq!(
        rules.object("KEPT").unwrap().deploys_into.as_deref(),
        Some("YARD")
    );
}

/// Only InfantryType ReadINI reads `OccupyWeapon=` and `EliteOccupyWeapon=`
/// (`0x00524102..0x0052417F`): another TechnoType that names one keeps the
/// constructor's null.
#[test]
fn occupy_weapons_are_infantry_keys() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[InfantryTypes]\n0=GARRISON\n[VehicleTypes]\n0=TANK\n[BuildingTypes]\n0=BUNKER\n\
         [GARRISON]\nOccupyWeapon=Gun\nEliteOccupyWeapon=Gun\n\
         [TANK]\nOccupyWeapon=Gun\nEliteOccupyWeapon=Gun\n\
         [BUNKER]\nOccupyWeapon=Gun\nEliteOccupyWeapon=Gun\n[Gun]\nDamage=2\n",
    ))
    .unwrap();

    let weapons = |id: &str| {
        let object = rules.object(id).unwrap();
        (
            object.occupy_weapon.as_deref(),
            object.elite_occupy_weapon.as_deref(),
        )
    };
    assert_eq!(weapons("GARRISON"), (Some("Gun"), Some("Gun")));
    assert_eq!(weapons("TANK"), (None, None));
    assert_eq!(weapons("BUNKER"), (None, None));
}

/// Every retail RULESMD ParticleSystem the processor reads names a registered
/// particle, so none reaches the empty-named type. `SmallGreySmokeSys` is
/// allocated by `DamageParticleSystems=` but has no section, so it is never
/// read and keeps the constructor's -1.
#[test]
fn retail_particle_systems_bind_registered_particles() {
    let Some(rules) = crate::rules::retail_ini_fixture::retail_unlayered_rules() else {
        return;
    };
    let systems = rules.particle_system_type_count();
    assert!(systems > 0);
    let unbound: Vec<_> = (0..systems)
        .map(|index| {
            rules.particle_system_type(crate::rules::particle_system_type::ParticleSystemTypeId(
                index as u32,
            ))
        })
        .filter(|system| system.holds_what.is_none())
        .map(|system| system.name.as_str())
        .collect();
    assert_eq!(unbound, ["SmallGreySmokeSys"]);
}

/// `[CombatDamage] DeathWeapon=` is RulesClass state (+FDC): ReadCombatDamage
/// keeps it on an empty read (`0x0066C57E..0x0066C5A8`), so a continued
/// process whose current stack omits it uses the earlier scenario's weapon,
/// and identical current INIs must not hide that difference.
#[test]
fn continued_process_keeps_the_combat_damage_death_weapon() {
    let tail = RulesLayerStack::new(IniFile::from_str("[CombatDamage]\nMaxDamage=500\n"));
    let continued = |death_weapon: &str| {
        let prior = RulesLayerStack::new(IniFile::from_str(&format!(
            "[VehicleTypes]\n0=TANK\n[TANK]\nPrimary=BoomA\nSecondary=BoomB\n\
             [BoomA]\nDamage=1\n[BoomB]\nDamage=2\n\
             [CombatDamage]\nDeathWeapon={death_weapon}\n"
        )))
        .process()
        .unwrap();
        let (_, trace) = prior.into_ini_and_native_type_construction_trace();
        let processed = tail
            .process_with_fixed_art_and_registry_state(
                &IniFile::empty(),
                trace.into_registry_state_discarding_events(),
            )
            .unwrap();
        RuleSet::from_processed_rules(&processed).unwrap()
    };
    let first = continued("BoomA");
    let second = continued("BoomB");

    assert_eq!(first.combat_damage.death_weapon.as_deref(), Some("BoomA"));
    assert_eq!(second.combat_damage.death_weapon.as_deref(), Some("BoomB"));
    assert_eq!(first.source_ini_hash(), second.source_ini_hash());
    assert_ne!(
        first.simulation_config_hash(),
        second.simulation_config_hash()
    );
}

/// Retail RULESMD sets none of these references to `none` or `<none>`, spells
/// each referenced Type as it is registered and authors `OccupyWeapon=` only
/// on infantry, so the processor publishes exactly what each section authors
/// (gamemd's ReadString of the raw file), as the typed reads did.
#[test]
fn retail_references_publish_their_authored_names() {
    use crate::rules::retail_ini_fixture::{retail_ini, retail_unlayered_rules};
    let (Some(rules), Some(raw)) = (retail_unlayered_rules(), retail_ini("rulesmd.ini")) else {
        return;
    };
    let section = |id: &str| raw.section_or_empty(id);
    let authored = |id: &str, key: &str| {
        Some(section(id).read_string(key, "", 0x80)).filter(|value| !value.is_empty())
    };
    let mut counts = [0; 8];
    for object in rules.all_objects() {
        let id = object.id.as_str();
        let image = section(id).read_string("Image", id, 0x19);
        let references = [
            (&object.death_weapon, "DeathWeapon"),
            (&object.deploys_into, "DeploysInto"),
            (&object.occupy_weapon, "OccupyWeapon"),
            (&object.elite_occupy_weapon, "EliteOccupyWeapon"),
        ];
        assert_eq!(object.image, image, "{id}");
        counts[0] += usize::from(image != id);
        for (slot, (published, key)) in references.into_iter().enumerate() {
            assert_eq!(*published, authored(id, key), "{id} {key}");
            counts[slot + 1] += usize::from(published.is_some());
        }
    }
    for weapon in rules.weapons_iter() {
        let range = section(&weapon.id).read_range("Range", 0);
        assert_eq!(weapon.range_leptons, range, "{}", weapon.id);
        counts[5] += usize::from(range % 256 != 0);
    }
    for projectile in rules.projectiles_iter() {
        let authored = authored(&projectile.id, "ShrapnelWeapon");
        assert_eq!(projectile.shrapnel_weapon, authored, "{}", projectile.id);
        counts[6] += usize::from(authored.is_some());
    }
    for super_weapon in rules.super_weapons.values() {
        let authored = authored(&super_weapon.id, "WeaponType");
        assert_eq!(super_weapon.weapon_type, authored, "{}", super_weapon.id);
        counts[7] += usize::from(authored.is_some());
    }
    assert_eq!(
        rules.combat_damage.death_weapon,
        authored("CombatDamage", "DeathWeapon")
    );
    // Image, DeathWeapon, DeploysInto, OccupyWeapon, EliteOccupyWeapon,
    // fractional Range, ShrapnelWeapon, SuperWeapon WeaponType.
    assert_eq!(counts, [61, 12, 4, 3, 3, 43, 4, 1]);
}

/// Every TechnoType pointer and list [`TechnoTypeReferences`] keeps goes
/// through a FindOrAllocate that answers null for `none` and `<none>`
/// (BuildingType `FreeUnit=` `0x00460540`, `UnloadingClass=` `0x007146E8`,
/// `Enslaves=` through `0x0067BAC0`, the particle systems through
/// `0x00644890`; each list token likewise): a `none` pointer is null and a
/// list drops the token. VERA's typed reads kept a Type of that name.
#[test]
fn techno_references_read_none_as_null() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[BuildingTypes]\n0=YARD\n[YARD]\nFreeUnit=none\n\
         [VehicleTypes]\n0=TANK\n\
         [TANK]\nUnloadingClass=none\nEnslaves=<none>\nNaturalParticleSystem=none\n\
         RefinerySmokeParticleSystem=<none>\nExplosion=none,BOOM\nDestroyAnim=<none>,BOOM\n\
         DamageParticleSystems=none,SMOKE\nDestroyParticleSystems=none\n\
         DebrisTypes=none,TIRE\nDebrisAnims=<none>,BOOM\nDock=none,YARD\n\
         [InfantryTypes]\n0=GI\n[GI]\nDeadBodies=none,CORPSE\n",
    ))
    .unwrap();

    assert_eq!(rules.object("YARD").unwrap().free_unit, None);
    let tank = rules.object("TANK").unwrap();
    assert_eq!((&tank.unloading_class, &tank.enslaves), (&None, &None));
    assert_eq!(tank.natural_particle_system, None);
    assert_eq!(tank.refinery_smoke_particle_system, None);
    assert_eq!(tank.explosion_anims, ["BOOM"]);
    assert_eq!(tank.destroy_anims, ["BOOM"]);
    assert_eq!(tank.damage_particle_systems, ["SMOKE"]);
    assert!(tank.destroy_particle_systems.is_empty());
    assert_eq!(tank.debris_types, ["TIRE"]);
    assert_eq!(tank.debris_anims, ["BOOM"]);
    assert_eq!(tank.dock, ["YARD"]);
    assert_eq!(rules.object("GI").unwrap().dead_bodies, ["CORPSE"]);
}

/// WarheadType ReadINI's `DebrisTypes=` (`0x0075DAF5`) keeps only the tokens
/// VoxelAnimType's FindOrAllocate (`0x0074B960`) answers, which drops `none`
/// and `<none>` (`0x0075DB43`). The typed read kept them as names.
#[test]
fn warhead_debris_types_read_none_as_null() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[Warheads]\n0=WH\n[WH]\nDebrisTypes=none,TIRE,<none>\n",
    ))
    .unwrap();

    assert_eq!(rules.warhead("WH").unwrap().debris_types, ["TIRE"]);
}

/// WeaponType ReadINI keeps only the `Anim=` tokens the AnimType factory
/// (`0x00428B80`) answers, which drops `none` and `<none>`, and reads
/// `OccupantAnim=` and `OpenToppedAnim=` the same way. `AttachedParticleSystem=`
/// goes through `ParticleSystemTypeClass::FindOrCreate @ 0x00644630`
/// (`0x00772929`): its -1 for `<none>` keeps the field (`0x00772931`), while
/// `none` names a system like any other spelling. VERA's typed reads kept every
/// name, which shifted the damage-indexed `Anim=` pick.
#[test]
fn weapon_anims_read_none_as_null() {
    let mut layers = RulesLayerStack::new(IniFile::from_str(
        "[VehicleTypes]\n0=TANK\n[TANK]\nPrimary=GUN\nSecondary=ZAP\n\
         [GUN]\nAnim=none,GUNFIRE\nOccupantAnim=none\nOpenToppedAnim=<none>\n\
         AttachedParticleSystem=SPARKS\n\
         [ZAP]\nAttachedParticleSystem=none\n",
    ));
    layers.push(
        RulesLayerKind::Scenario,
        IniFile::from_str("[GUN]\nAttachedParticleSystem=<none>\n"),
    );
    let rules = RuleSet::from_rules_layers(&layers).unwrap();

    let gun = rules.weapon("GUN").unwrap();
    assert_eq!(gun.anim, ["GUNFIRE"]);
    assert_eq!((&gun.occupant_anim, &gun.open_topped_anim), (&None, &None));
    assert_eq!(gun.attached_particle_system.as_deref(), Some("SPARKS"));
    assert_eq!(
        rules
            .weapon("ZAP")
            .unwrap()
            .attached_particle_system
            .as_deref(),
        Some("none")
    );
}

/// ParticleType `Warhead=` (`0x00645043..0x00645067`) and the RulesClass
/// pointers of [`RulesTypeReferences`] go through factories that answer null
/// for `none` and `<none>`. VERA's typed reads kept those names, so a `none`
/// named a warhead or particle system nothing defines.
#[test]
fn rules_type_references_read_none_as_null() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[Particles]\n0=GAS\n[GAS]\nWarhead=none\n\
         [CombatDamage]\nIvanWarhead=none\nDefaultSparkSystem=<none>\nC4Warhead=none\n\
         CrushWarhead=<none>\nIonCannonWarhead=none\n\
         [Radiation]\nRadSiteWarhead=none\n\
         [SpecialWeapons]\nMutateWarhead=none\nMutateExplosionWarhead=<none>\n",
    ))
    .unwrap();

    let gas = rules.p_type_id_by_name("GAS").unwrap();
    assert_eq!(rules.particle_type(gas).warhead, None);
    assert_eq!(rules.combat_damage.ivan_warhead, None);
    assert_eq!(rules.combat_damage.default_spark_system, None);
    let bridge = &rules.bridge_warheads;
    assert_eq!(
        [&bridge.c4_name, &bridge.crush_name, &bridge.ion_cannon_name],
        ["", "", ""]
    );
    assert_eq!(rules.radiation.site_warhead, "");
    assert_eq!(
        [
            &rules.general.mutate_warhead,
            &rules.general.mutate_explosion_warhead
        ],
        ["", ""]
    );
}

/// The RulesClass constructor leaves `C4Warhead=`, `CrushWarhead=`,
/// `IonCannonWarhead=` and `RadSiteWarhead=` null (`0x00666B52`, `0x00666B58`,
/// `0x00666BD5`, `0x00667801`), and a reader keeps the field on an empty read
/// (RadSite: `0x0066D0EA` reloads it, `0x0066D127` stores it back). VERA's
/// typed reads invented `Super`, `Crush`, `IonCannonWH` and `RadSite`.
#[test]
fn rules_class_type_pointers_start_null() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[CombatDamage]\nMaxDamage=500\nCrushWarhead=\n[Radiation]\nRadDurationMultiple=1\n",
    ))
    .unwrap();

    let bridge = &rules.bridge_warheads;
    assert_eq!(
        [&bridge.c4_name, &bridge.crush_name, &bridge.ion_cannon_name],
        ["", "", ""]
    );
    assert_eq!(rules.radiation.site_warhead, "");
}

/// AnimType ReadINI resolves its fixed-ART references through factories that
/// answer null for `none` and `<none>` (`0x00428045..0x0042867A`), and
/// `SpawnsParticle=` through ParticleType's (`0x00645430`), null only for
/// `<none>`. The ART registry's own reads kept `NONE` and `<NONE>` as names.
#[test]
fn anim_art_references_read_none_as_null() {
    let art = IniFile::from_str(
        "[EXPL]\nNext=none\nTrailerAnim=<none>\nExpireAnim=smoke\nWarhead=none\n\
         SpawnsParticle=<none>\nTiberiumSpawnType=none\n[SMOKE]\nRate=100\n",
    );
    let mut rules = RuleSet::from_ini_with_fixed_art_for_test(
        &IniFile::from_str("[Animations]\n0=EXPL\n"),
        &art,
    )
    .unwrap();
    rules.install_art_data(ArtRegistry::from_ini(&art));

    let config = rules.art().anim_runtime_config("EXPL").unwrap();
    assert_eq!((&config.next, &config.trailer_anim), (&None, &None));
    assert_eq!(config.expire_anim.as_deref(), Some("SMOKE"));
    assert_eq!((&config.warhead, &config.spawns_particle), (&None, &None));
    assert_eq!(config.tiberium_spawn_type, None);
}

/// VoxelAnimType ReadINI resolves `Spawns=` through `VoxelAnimTypeClass::
/// FindOrCreate @ 0x0074BB20` (`0x0074B219`), which appends a VoxelAnimType for
/// a name [VoxelAnims] does not list. VERA's typed read kept the name with no
/// Type behind it.
#[test]
fn voxel_anim_spawns_allocate_a_voxel_anim_type() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[VoxelAnims]\n0=METEOR\n[METEOR]\nSpawns=pebble\nSpawnCount=3\n",
    ))
    .unwrap();

    assert_eq!(rules.voxel_anim_type_count(), 2);
    let pebble = rules.voxel_anim_type_id_by_name("PEBBLE").unwrap();
    let meteor = rules.voxel_anim_type_id_by_name("METEOR").unwrap();
    assert_eq!(
        rules.voxel_anim_type(meteor).spawns.as_deref(),
        Some(rules.voxel_anim_type(pebble).name.as_str())
    );
}

/// The rocket types (`[General] <prefix>Type=`, AircraftType's factory
/// through `0x0067BD30`) and the six `[CombatDamage]` rocket warheads
/// (`0x0075E3B0`) answer null for `none` and `<none>`; an empty read keeps
/// the earlier pass's. VERA's second read kept `none` as a name.
#[test]
fn rocket_references_read_none_as_null() {
    let mut layers = RulesLayerStack::new(IniFile::from_str(
        "[General]\nV3RocketType=V3ROCKET\nDMislType=DMISL\n\
         [CombatDamage]\nV3Warhead=V3WH\nDMislWarhead=DMISLWH\n",
    ));
    layers.push(
        RulesLayerKind::Scenario,
        IniFile::from_str(
            "[General]\nV3RocketType=none\nDMislType=\n\
             [CombatDamage]\nV3Warhead=<none>\nDMislEliteWarhead=none\n",
        ),
    );
    let missiles = layers.process().unwrap().missile_spawn().clone();

    assert_eq!(missiles.v3.type_name, "");
    assert_eq!(missiles.dmisl.type_name, "DMISL");
    assert_eq!(missiles.family_of("none"), None);
    assert_eq!([&missiles.v3.warhead, &missiles.v3.elite_warhead], ["", ""]);
    assert_eq!(
        [&missiles.dmisl.warhead, &missiles.dmisl.elite_warhead],
        ["DMISLWH", ""]
    );
}

/// BuildingType ReadINI reads `ToOverlay=` from the fixed ART section its
/// Image names (`0x004611F5..0x0046121F`) and nowhere else; the ART
/// projection's read fell back to the section of the type's ID.
#[test]
fn to_overlay_reads_only_the_image_section() {
    let art = IniFile::from_str("[WALL]\nToOverlay=WALLOVL\n");
    let mut rules = RuleSet::from_ini_with_fixed_art_for_test(
        &IniFile::from_str("[BuildingTypes]\n0=WALL\n[WALL]\nImage=MISSING\n"),
        &art,
    )
    .unwrap();
    rules.install_art_data(ArtRegistry::from_ini(&art));

    assert_eq!(rules.object("WALL").unwrap().to_overlay, None);
}

/// The RulesClass pointers are process-resident state each reader keeps on an
/// empty read, so a continued process whose current stack omits a key keeps
/// the earlier scenario's Type, where the typed reads saw only the current
/// merged INI.
#[test]
fn continued_process_keeps_the_rules_class_type_pointers() {
    let prior = RulesLayerStack::new(IniFile::from_str(
        "[CombatDamage]\nIvanWarhead=IvanWH\nDefaultSparkSystem=SparkSys\nC4Warhead=C4WH\n\
         ControlledAnimationType=MINDANIM\nDrainAnimationType=DISKRAY\n\
         [Radiation]\nRadSiteWarhead=RadWH\n\
         [SpecialWeapons]\nMutateWarhead=Mutate\n",
    ))
    .process()
    .unwrap();
    let (_, trace) = prior.into_ini_and_native_type_construction_trace();
    let processed = RulesLayerStack::new(IniFile::from_str(
        "[CombatDamage]\nMaxDamage=500\n[Radiation]\nRadDurationMultiple=1\n",
    ))
    .process_with_fixed_art_and_registry_state(
        &IniFile::empty(),
        trace.into_registry_state_discarding_events(),
    )
    .unwrap();
    let rules = RuleSet::from_processed_rules(&processed).unwrap();

    let damage = &rules.combat_damage;
    assert_eq!(damage.ivan_warhead.as_deref(), Some("IvanWH"));
    assert_eq!(damage.default_spark_system.as_deref(), Some("SparkSys"));
    assert_eq!(damage.drain_animation_type.as_deref(), Some("DISKRAY"));
    assert_eq!(rules.bridge_warheads.c4_name, "C4WH");
    assert_eq!(
        rules.mind_control.controlled_anim.as_deref(),
        Some("MINDANIM")
    );
    assert_eq!(rules.radiation.site_warhead, "RadWH");
    assert_eq!(rules.general.mutate_warhead, "Mutate");
}

/// Retail RULESMD over ARTMD, alone and under each of the nine mode INIs
/// `mpmodesmd.ini` names, sets none of these references to `none` or `<none>`
/// (bar `UnitCrateType=none`, which both readers null) and spells each
/// referenced Type as it is first allocated. The processor therefore publishes
/// what VERA's deleted typed reads of the merged INI and of ARTMD did: the
/// authored names (Dock, Spawns and the ART AnimType references upper-cased),
/// the authored RulesClass warheads, and the `ToOverlay=` of each building's
/// Image section. Only the ten ART sections no AnimType reads lose their
/// references.
#[test]
fn retail_references_publish_the_typed_reads() {
    use crate::rules::retail_ini_fixture::{art_registry, retail_ini, retail_rules_and_art};
    let Some((rulesmd, artmd)) = retail_rules_and_art() else {
        return;
    };
    let modes = [
        None,
        Some("MPBattleMD.ini"),
        Some("MPCoopMD.ini"),
        Some("MPDuelMD.ini"),
        Some("MPFreeForAllMD.ini"),
        Some("MPMWMD.ini"),
        Some("MPMeatMD.ini"),
        Some("MPNavalMD.ini"),
        Some("MPTeamMD.ini"),
        Some("MPUnholyMD.ini"),
    ];
    for mode in modes {
        let mut layers = RulesLayerStack::new(rulesmd.clone());
        if let Some(mode) = mode {
            let Some(mode_ini) = retail_ini(mode) else {
                return;
            };
            layers.push(RulesLayerKind::GameMode, mode_ini);
        }
        let processed = layers.process_with_fixed_art(&artmd).unwrap();
        let mut rules = RuleSet::from_processed_rules(&processed).unwrap();
        rules.install_art_data(art_registry(&artmd));
        let (counts, orphans) =
            assert_typed_reads(&rules, processed.ini(), &artmd, mode.unwrap_or("RULESMD"));
        if mode.is_none() {
            // Authored TechnoType pointers and list entries, weapon anims and
            // particle systems, warhead DebrisTypes and ParticleType warheads,
            // VoxelAnim references and AuxBuildings, AnimTypes with an ART
            // reference, buildings with a ToOverlay, ART sections with
            // references that name no AnimType and RulesClass pointers.
            assert_eq!(counts, [24, 3839, 209, 13, 20, 31, 8, 10, 15]);
            // Seven commented-out `;GEF` [Animations] entries and three debris
            // sections nothing names: no AnimClass can be built from them.
            assert_eq!(
                orphans,
                [
                    "BURN-L", "BURN-M", "DBRIS1HV", "DBRIS2HV", "DBRIS3HV", "METDEBRI", "METLARGE",
                    "METLTRAL", "METSMALL", "METSTRAL"
                ]
            );
        }
    }
}

/// Assert that `rules` publishes, for every reference this chain moved to the
/// ordered processor, the value the deleted typed read of `ini` (the merged
/// projection) or of `art` produced, and count the authored ones. Also name
/// the ART sections with references that no AnimType reads.
fn assert_typed_reads(
    rules: &RuleSet,
    ini: &IniFile,
    art: &IniFile,
    label: &str,
) -> ([usize; 9], Vec<String>) {
    let owned = |name: Option<&str>| name.map(str::to_owned);
    let upper = |name: Option<&str>| name.map(str::to_ascii_uppercase);
    let list = |section: &IniSection, key: &str| -> Vec<String> {
        section
            .read_list(key, 0x80)
            .unwrap_or_default()
            .into_iter()
            .map(str::to_owned)
            .collect()
    };
    let mut counts = [0; 9];
    // ArtRegistry::get's case-insensitive section lookup; the last wins.
    let art_sections: HashMap<String, &IniSection> = art
        .section_names()
        .into_iter()
        .filter_map(|name| Some((name.to_ascii_uppercase(), art.section(name)?)))
        .collect();

    for object in rules.all_objects() {
        let id = object.id.as_str();
        let section = ini.section_or_empty(id);
        let infantry = object.category == crate::rules::object_type::ObjectCategory::Infantry;
        let pointers = [
            (
                &object.free_unit,
                owned(section.read_name("FreeUnit", 0x80)),
                "FreeUnit",
            ),
            (
                &object.undeploys_into,
                owned(section.read_type_name("UndeploysInto", 0x80)),
                "UndeploysInto",
            ),
            (
                &object.powers_unit,
                owned(section.read_type_name("PowersUnit", 0x80)),
                "PowersUnit",
            ),
            (
                &object.unloading_class,
                owned(section.read_name("UnloadingClass", 0x80)),
                "UnloadingClass",
            ),
            (
                &object.deploying_anim,
                owned(section.read_type_name("DeployingAnim", 0x80)),
                "DeployingAnim",
            ),
            (
                &object.enslaves,
                owned(section.read_name("Enslaves", 0x80)),
                "Enslaves",
            ),
            (
                &object.spawns,
                upper(section.read_type_name("Spawns", 0x80)),
                "Spawns",
            ),
            (
                &object.natural_particle_system,
                owned(section.read_name("NaturalParticleSystem", 0x80)),
                "NaturalParticleSystem",
            ),
            (
                &object.refinery_smoke_particle_system,
                owned(section.read_name("RefinerySmokeParticleSystem", 0x80)),
                "RefinerySmokeParticleSystem",
            ),
        ];
        for (published, typed, key) in pointers {
            assert_eq!(*published, typed, "{label} {id} {key}");
            counts[0] += usize::from(typed.is_some());
        }
        let dock: Vec<String> = list(section, "Dock")
            .iter()
            .map(|name| name.to_ascii_uppercase())
            .collect();
        let lists = [
            (&object.dock, dock, "Dock"),
            (
                &object.explosion_anims,
                list(section, "Explosion"),
                "Explosion",
            ),
            (
                &object.destroy_anims,
                list(section, "DestroyAnim"),
                "DestroyAnim",
            ),
            (
                &object.dead_bodies,
                if infantry {
                    list(section, "DeadBodies")
                } else {
                    Vec::new()
                },
                "DeadBodies",
            ),
            (
                &object.damage_particle_systems,
                list(section, "DamageParticleSystems"),
                "DamageParticleSystems",
            ),
            (
                &object.destroy_particle_systems,
                list(section, "DestroyParticleSystems"),
                "DestroyParticleSystems",
            ),
            (
                &object.debris_types,
                list(section, "DebrisTypes"),
                "DebrisTypes",
            ),
            (
                &object.debris_anims,
                list(section, "DebrisAnims"),
                "DebrisAnims",
            ),
        ];
        for (published, typed, key) in lists {
            assert_eq!(*published, typed, "{label} {id} {key}");
            counts[1] += typed.len();
        }
        let to_overlay = (object.category == crate::rules::object_type::ObjectCategory::Building)
            .then(|| {
                art_sections
                    .get(&object.image.to_ascii_uppercase())
                    .or_else(|| art_sections.get(&id.to_ascii_uppercase()))
                    .and_then(|section| owned(section.read_name("ToOverlay", 0x80)))
            })
            .flatten();
        assert_eq!(object.to_overlay, to_overlay, "{label} {id} ToOverlay");
        counts[6] += usize::from(to_overlay.is_some());
    }

    for weapon in rules.weapons_iter() {
        let section = ini.section_or_empty(&weapon.id);
        assert_eq!(weapon.anim, list(section, "Anim"), "{label} {}", weapon.id);
        let pointers = [
            (
                &weapon.occupant_anim,
                section.read_name("OccupantAnim", 0x80),
            ),
            (
                &weapon.open_topped_anim,
                section.read_name("OpenToppedAnim", 0x80),
            ),
            (
                &weapon.attached_particle_system,
                section.read_name("AttachedParticleSystem", 0x14),
            ),
        ];
        for (published, typed) in pointers {
            assert_eq!(published.as_deref(), typed, "{label} {}", weapon.id);
            counts[2] += usize::from(typed.is_some());
        }
        counts[2] += weapon.anim.len();
    }
    for warhead in rules.warheads_iter() {
        let typed = list(ini.section_or_empty(&warhead.id), "DebrisTypes");
        assert_eq!(warhead.debris_types, typed, "{label} {}", warhead.id);
        counts[3] += typed.len();
    }
    for particle in rules.particle_types_iter() {
        let typed = ini
            .section_or_empty(&particle.name)
            .read_name("Warhead", 0x80);
        assert_eq!(
            particle.warhead.as_deref(),
            typed,
            "{label} {}",
            particle.name
        );
        counts[3] += usize::from(typed.is_some());
    }
    for index in 0..rules.voxel_anim_type_count() {
        let voxel_anim =
            rules.voxel_anim_type(crate::rules::voxel_anim_type::VoxelAnimTypeId(index as u32));
        let section = ini.section_or_empty(&voxel_anim.name);
        for (published, key) in [
            (&voxel_anim.spawns, "Spawns"),
            (&voxel_anim.bounce_anim, "BounceAnim"),
            (&voxel_anim.expire_anim, "ExpireAnim"),
            (&voxel_anim.trailer_anim, "TrailerAnim"),
            (&voxel_anim.warhead, "Warhead"),
            (&voxel_anim.attached_system, "AttachedSystem"),
        ] {
            let typed = section.read_type_name(key, 0x80);
            assert_eq!(
                published.as_deref(),
                typed,
                "{label} {} {key}",
                voxel_anim.name
            );
            counts[4] += usize::from(typed.is_some());
        }
    }
    for super_weapon in rules.super_weapons.values() {
        let typed = ini
            .section_or_empty(&super_weapon.id)
            .read_type_name("AuxBuilding", 0x80);
        assert_eq!(
            super_weapon.aux_building.as_deref(),
            typed,
            "{label} {}",
            super_weapon.id
        );
        counts[4] += usize::from(typed.is_some());
    }

    // The AnimTypes' fixed-ART references; the first slot of a name wins.
    let mut seen = HashSet::new();
    let anim_keys = [
        ("Next", 0x80),
        ("Spawns", 0x80),
        ("TiberiumSpawnType", 0x80),
        ("BounceAnim", 0x80),
        ("ExpireAnim", 0x80),
        ("TrailerAnim", 0x80),
        ("Warhead", 0x80),
        ("SpawnsParticle", 0x20),
    ];
    let typed_anim = |section: Option<&&IniSection>| {
        anim_keys.map(|(key, capacity)| {
            section.and_then(|section| upper(section.read_name(key, capacity)))
        })
    };
    for (name, read) in rules.anim_type_art_read_states() {
        let key = name.to_ascii_uppercase();
        if !seen.insert(key.clone()) {
            continue;
        }
        let Some(config) = rules.art().anim_runtime_config(&key) else {
            continue;
        };
        let typed = typed_anim(art_sections.get(&key).filter(|_| *read));
        let published = [
            &config.next,
            &config.spawns,
            &config.tiberium_spawn_type,
            &config.bounce_anim,
            &config.expire_anim,
            &config.trailer_anim,
            &config.warhead,
            &config.spawns_particle,
        ]
        .map(Clone::clone);
        assert_eq!(published, typed, "{label} {name}");
        counts[5] += usize::from(typed.iter().any(Option::is_some));
    }
    // ART sections with references that name no AnimType: no AnimClass can
    // be built from them, so their references go unread.
    let mut orphans: Vec<String> = art_sections
        .iter()
        .filter(|(key, section)| {
            !seen.contains(*key) && typed_anim(Some(*section)).iter().any(Option::is_some)
        })
        .map(|(key, _)| key.clone())
        .collect();
    orphans.sort();
    counts[7] = orphans.len();

    // The RulesClass pointers.
    let combat = ini.section("CombatDamage");
    let combat_or_empty = ini.section_or_empty("CombatDamage");
    let damage = &rules.combat_damage;
    assert_eq!(
        damage.drain_animation_type.as_deref(),
        combat_or_empty.read_type_name("DrainAnimationType", 0x80),
        "{label}"
    );
    assert_eq!(
        damage.ivan_warhead.as_deref(),
        combat_or_empty.read_name("IvanWarhead", 0x80),
        "{label}"
    );
    for (published, key) in [
        (
            &damage.default_large_grey_smoke_system,
            "DefaultLargeGreySmokeSystem",
        ),
        (
            &damage.default_small_grey_smoke_system,
            "DefaultSmallGreySmokeSystem",
        ),
        (&damage.default_spark_system, "DefaultSparkSystem"),
        (
            &damage.default_large_red_smoke_system,
            "DefaultLargeRedSmokeSystem",
        ),
        (
            &damage.default_small_red_smoke_system,
            "DefaultSmallRedSmokeSystem",
        ),
        (
            &damage.default_debris_smoke_system,
            "DefaultDebrisSmokeSystem",
        ),
        (
            &damage.default_fire_stream_system,
            "DefaultFireStreamSystem",
        ),
        (
            &damage.default_test_particle_system,
            "DefaultTestParticleSystem",
        ),
        (
            &damage.default_repair_particle_system,
            "DefaultRepairParticleSystem",
        ),
    ] {
        assert_eq!(
            published.as_deref(),
            combat_or_empty.read_name(key, 0x80),
            "{label} {key}"
        );
        counts[8] += usize::from(published.is_some());
    }
    let defaulted = |key: &str, default: &str| {
        combat
            .and_then(|section| section.read_name(key, 0x80))
            .unwrap_or(default)
            .to_owned()
    };
    let bridge = &rules.bridge_warheads;
    assert_eq!(
        bridge.ion_cannon_name,
        defaulted("IonCannonWarhead", "IonCannonWH"),
        "{label}"
    );
    assert_eq!(bridge.c4_name, defaulted("C4Warhead", "Super"), "{label}");
    assert_eq!(
        bridge.crush_name,
        defaulted("CrushWarhead", "Crush"),
        "{label}"
    );
    assert_eq!(
        rules.mind_control.controlled_anim.as_deref(),
        combat_or_empty.read_type_name("ControlledAnimationType", 0x80),
        "{label}"
    );
    assert_eq!(
        rules.mind_control.perma_controlled_anim.as_deref(),
        combat_or_empty.read_type_name("PermaControlledAnimationType", 0x80),
        "{label}"
    );
    let radiation = ini
        .section("Radiation")
        .and_then(|section| section.read_name("RadSiteWarhead", 0x80))
        .unwrap_or("RadSite");
    assert_eq!(rules.radiation.site_warhead, radiation, "{label}");
    let special = ini.section_or_empty("SpecialWeapons");
    assert_eq!(
        rules.general.mutate_warhead,
        special.read_string("MutateWarhead", "", 0x80),
        "{label}"
    );
    assert_eq!(
        rules.general.mutate_explosion_warhead,
        special.read_string("MutateExplosionWarhead", "", 0x80),
        "{label}"
    );
    counts[8] += [
        &bridge.ion_cannon_name,
        &bridge.c4_name,
        &bridge.crush_name,
        &rules.radiation.site_warhead,
        &rules.general.mutate_warhead,
        &rules.general.mutate_explosion_warhead,
    ]
    .into_iter()
    .filter(|name| !name.is_empty())
    .count();

    // The crate Types and rocket references the processor read twice.
    let crates = ini.section_or_empty("CrateRules");
    let crate_type = |key: &str| {
        crates
            .is_present(key)
            .then(|| crates.read_string(key, "", 0x80))
            .filter(|name| !crate::rules::ini_parser::is_native_none_type_name(name))
            .map(|name| name.to_ascii_uppercase())
    };
    let crate_rules = &rules.crate_rules;
    assert_eq!(
        crate_rules.wood_crate_img,
        crate_type("WoodCrateImg"),
        "{label}"
    );
    assert_eq!(crate_rules.crate_img, crate_type("CrateImg"), "{label}");
    assert_eq!(
        crate_rules.water_crate_img,
        crate_type("WaterCrateImg"),
        "{label}"
    );
    assert_eq!(
        crate_rules.unit_crate_type,
        crate_type("UnitCrateType"),
        "{label}"
    );
    let general = ini.section_or_empty("General");
    let missiles = &rules.missile_spawn;
    for (block, prefix) in [
        (&missiles.v3, "V3Rocket"),
        (&missiles.dmisl, "DMisl"),
        (&missiles.cmisl, "CMisl"),
    ] {
        let typed = general
            .read_name(&format!("{prefix}Type"), 0x80)
            .unwrap_or("");
        assert_eq!(block.type_name, typed, "{label} {prefix}");
    }
    for (block, prefix) in [
        (&missiles.v3, "V3"),
        (&missiles.dmisl, "DMisl"),
        (&missiles.cmisl, "CMisl"),
    ] {
        let warhead = combat_or_empty
            .read_name(&format!("{prefix}Warhead"), 0x80)
            .unwrap_or("");
        let elite = combat_or_empty
            .read_name(&format!("{prefix}EliteWarhead"), 0x80)
            .unwrap_or(warhead);
        assert_eq!(
            (block.warhead.as_str(), block.elite_warhead.as_str()),
            (warhead, elite),
            "{label} {prefix}"
        );
    }
    (counts, orphans)
}
