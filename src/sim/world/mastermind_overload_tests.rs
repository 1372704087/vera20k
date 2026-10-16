//! Ordinary stock MIND/PsychicControl Bullet46920B -> CaptureManager471D40
//! -> overload471A50/471C78 -> next real Techno RockingUpdate70B570.

use super::{FrameEffects, Simulation, TickLane};
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::TargetKind;
use crate::sim::combat::combat_weapon::WeaponSlot;
use crate::sim::command::{Command, CommandEnvelope};
use crate::util::fixed_math::SimFixed;
use std::collections::BTreeSet;

struct MindScene {
    rules: std::sync::Arc<RuleSet>,
    sim: Simulation,
    mind: u64,
    targets: [u64; 4],
}

impl MindScene {
    fn retail() -> Option<Self> {
        let mut rules = crate::rules::retail_ini_fixture::retail_battle_rules()?.rules;
        let (_, assets) = crate::rules::retail_ini_fixture::retail_assets().unwrap();
        let roots = crate::rules::effect_asset_catalog::anim_class_roots(&rules);
        rules.bind_anim_class_assets(&roots, &assets, "TEM", "TEMPERATE");
        rules.bind_effect_assets(&assets, "TEM", "TEMPERATE");
        let object = rules.object("MIND").unwrap();
        assert_eq!(object.primary(), Some("MultipleMindControlTank"));
        assert!(rules.object_has_loaded_voxel(rules.type_handle("MIND").unwrap()));
        let weapon = rules.weapon("MultipleMindControlTank").unwrap();
        assert!(weapon.infinite_mind_control);
        assert_eq!(weapon.damage, 3);
        assert_eq!(weapon.projectile.as_deref(), Some("PsychicControl"));
        assert_eq!(weapon.warhead.as_deref(), Some("Controller"));
        assert!(rules.projectile("PsychicControl").unwrap().inviso);
        assert!(rules.warhead("Controller").unwrap().mind_control);
        assert_eq!(weapon.range, SimFixed::from_num(6));
        assert!(rules.weapon("105mm").unwrap().range < weapon.range);
        assert_eq!(rules.mind_control.overload_count, [3, 6, 10, 50]);
        assert_eq!(rules.mind_control.overload_damage, [0, 50, 100, 500]);
        assert_eq!(rules.bridge_warheads.c4_name, "Super");
        let super_wh = rules.warhead(&rules.bridge_warheads.c4_name).unwrap();
        assert_eq!(
            super_wh.verses_f64[crate::sim::combat::armor_index(&object.armor)],
            1.0
        );
        let spark = rules.combat_damage.default_spark_system.as_deref().unwrap();
        assert!(rules.ps_type_id_by_name(spark).is_some());

        let mut sim = crate::sim::arena_fixture::rival_arena_with_size(&rules, 0x471a50, 64);
        // The arena's open LocalSize retains native Size width20 independently
        // of its rectangular terrain. Supply Size height64 for this larger
        // scene: Bullet In_Bounds568300 must admit the cardinal encounter and
        // its ordinary tank pursuit (the arena's default20 ends at x+y=60).
        // Allocated terrain/fog64x64 and native MapSize20x64 are independent.
        sim.playfield_size_height = Some(64);
        let native_width = sim.playfield_bounds.unwrap().base;
        for cell in [(32, 32), (32, 26), (38, 32), (32, 38), (26, 32)] {
            assert!(crate::map::playfield::size_diamond_contains(
                native_width,
                sim.playfield_size_height.unwrap(),
                cell,
            ));
        }
        // Production scenario construction allocates the whole map's fog
        // domain before Reveal/ChangeOwner. The arena supplies terrain only;
        // per-owner mark helpers must not grow mismatched partial domains.
        sim.fog = crate::sim::vision::FogState::new(64, 64);
        sim.intern_rule_type_ids(&rules);
        sim.resolve_type_handles(&rules);
        sim.resolve_rule_animation_lists(&rules);
        let mind = sim
            .spawn_object("MIND", "Americans", 32, 32, 0, &rules)
            .expect("ordinary physical Mastermind Unlimbo");
        let mut targets = Vec::new();
        // Cardinal retail Range6 puts every target in the controller's range,
        // beyond stock105mm Range5, and away from the other three captives.
        for (x, y) in [(32, 26), (38, 32), (32, 38), (26, 32)] {
            targets.push(
                sim.spawn_object("MTNK", "Russians", x, y, 0, &rules)
                    .expect("ordinary hostile tank Unlimbo"),
            );
        }
        let truck = rules.object("TRUCKA").unwrap();
        assert!(truck.primary().is_none() && truck.secondary().is_none());
        for (house, x, y) in [("Americans", 4, 4), ("Russians", 58, 58)] {
            sim.spawn_object("TRUCKA", house, x, y, 0, &rules)
                .expect("ordinary distant house survivor");
        }
        let owner = sim.substrate.entities.get(mind).unwrap().owner();
        let enemy = sim.substrate.entities.get(targets[0]).unwrap().owner();
        assert_ne!(owner, enemy);
        assert!(sim.houses.get(&owner).unwrap().is_human);
        sim.session.current_house = Some(owner);
        // Supplied map knowledge admits the real commands/scanner for both
        // houses; counterfire is not avoided by hiding the controller.
        sim.fog.mark_visible_for_owner(enemy, 32, 32);
        for &id in &targets {
            let actor = sim.substrate.entities.get(id).unwrap();
            sim.fog
                .mark_visible_for_owner(owner, actor.position.rx(), actor.position.ry());
            sim.queue_command(CommandEnvelope::new(
                enemy,
                1,
                Command::Guard {
                    entity_id: id,
                    target: None,
                },
            ));
        }
        sim.queue_command(CommandEnvelope::new(
            owner,
            1,
            Command::Guard {
                entity_id: mind,
                target: None,
            },
        ));
        sim.queue_command(CommandEnvelope::new(
            owner,
            1,
            Command::Attack {
                attacker_id: mind,
                target_id: targets[0],
            },
        ));
        eprintln!(
            "MIND ordinary initial Attack issue_tick={} due_tick=1 target={}",
            sim.session.tick, targets[0]
        );
        Some(Self {
            rules: rules.into(),
            sim,
            mind,
            targets: targets.try_into().unwrap(),
        })
    }

    fn step(&mut self) -> Vec<super::SimFireEvent> {
        let before = self.sim.substrate.entities.get(self.mind).map(|actor| {
            (
                actor.health.current(),
                self.words(),
                actor
                    .capture_manager
                    .as_ref()
                    .map(|manager| manager.victims().count()),
            )
        });
        let commands = self.sim.take_due_commands();
        let output = self
            .sim
            .advance_app_frame(
                &commands,
                Some(&self.rules),
                None,
                33,
                TickLane::Ordinary,
                None,
                FrameEffects::default(),
            )
            .expect("ordinary stock Mastermind frame");
        assert!(output.tick.frame_committed);
        if before.is_some_and(|(hp, _, _)| hp > 0)
            && self
                .sim
                .substrate
                .entities
                .get(self.mind)
                .is_none_or(|actor| !actor.lifecycle.object_alive())
        {
            eprintln!(
                "MIND actual terminal visited_frame={} before={before:?} houses={:?} shots={:?} lifecycle={:?}",
                self.sim.session.binary_frame - 1,
                self.sim
                    .houses
                    .iter()
                    .map(|(id, house)| (self.sim.interner.resolve(*id), house.is_defeated))
                    .collect::<Vec<_>>(),
                output
                    .fire_events
                    .iter()
                    .map(|shot| (
                        shot.attacker_id,
                        self.sim.interner.resolve(shot.weapon_id),
                        shot.target
                    ))
                    .collect::<Vec<_>>(),
                output.lifecycle_outputs
            );
        }
        output.fire_events
    }

    fn words(&self) -> [u32; 4] {
        self.sim
            .substrate
            .entities
            .get(self.mind)
            .unwrap()
            .rocking
            .as_ref()
            .map_or([0; 4], |state| state.native_bits())
    }

    fn victims(&self) -> Vec<u64> {
        self.sim
            .substrate
            .entities
            .get(self.mind)
            .unwrap()
            .capture_manager
            .as_ref()
            .unwrap()
            .victims()
            .collect()
    }

    fn capture_four_through_real_attacks(&mut self) {
        self.capture_four_observing(|_| {});
    }

    fn capture_four_observing(&mut self, mut observe: impl FnMut(&Self)) {
        let mut fired_at = BTreeSet::new();
        let mut requested = self.targets[0];
        for _ in 0..600 {
            let shots = self.step();
            for shot in shots.iter().filter(|shot| shot.attacker_id == self.mind) {
                assert_eq!(
                    self.sim.interner.resolve(shot.weapon_id),
                    "MultipleMindControlTank"
                );
                assert_eq!(shot.weapon_slot, WeaponSlot::Primary);
                let TargetKind::Entity(target) = shot.target else {
                    panic!("stock mind control must have a real tank target");
                };
                assert!(self.targets.contains(&target));
                fired_at.insert(target);
                eprintln!(
                    "MIND ordinary Fire visited_frame={} tick={} target={} fire={:?}",
                    self.sim.session.binary_frame - 1,
                    self.sim.session.tick,
                    target,
                    shot.fire_coord
                );
            }
            let controller = self.sim.substrate.entities.get(self.mind).unwrap();
            assert!(controller.lifecycle.object_alive());
            assert_eq!(
                controller.health.current(),
                self.rules.object("MIND").unwrap().strength,
                "ordinary capture prerequisites must avoid counterfire before overload"
            );
            let owner = controller.owner();
            let victims = self.victims();
            for &id in &victims {
                let captive = self.sim.substrate.entities.get(id).unwrap();
                assert!(captive.lifecycle.object_alive() && captive.health.current() > 0);
                assert_eq!(captive.owner(), owner);
                assert_eq!(captive.mind_control.controller(), Some(self.mind));
                assert!(
                    fired_at.contains(&id),
                    "real selected FireAt must precede CaptureUnit"
                );
            }
            observe(self);
            if victims.contains(&requested) {
                eprintln!(
                    "MIND ordinary capture visited_frame={} tick={} requested={} nodes={victims:?}",
                    self.sim.session.binary_frame - 1,
                    self.sim.session.tick,
                    requested
                );
            }
            if victims.len() == 4 {
                assert_eq!(fired_at.len(), 4);
                assert!(self.targets.iter().all(|id| victims.contains(id)));
                self.sim.queue_command(CommandEnvelope::new(
                    owner,
                    self.sim.session.tick + 1,
                    Command::Stop {
                        entity_id: self.mind,
                    },
                ));
                return;
            }
            if victims.contains(&requested) {
                requested = *self
                    .targets
                    .iter()
                    .find(|id| !victims.contains(id))
                    .unwrap();
                eprintln!(
                    "MIND ordinary next Attack issue_tick={} due_tick={} target={requested}",
                    self.sim.session.tick,
                    self.sim.session.tick + 1
                );
                self.sim.queue_command(CommandEnvelope::new(
                    owner,
                    self.sim.session.tick + 1,
                    Command::Attack {
                        attacker_id: self.mind,
                        target_id: requested,
                    },
                ));
            }
        }
        panic!(
            "ordinary stock Mastermind did not reach four real captures: fired={fired_at:?}, victims={:?}",
            self.victims()
        );
    }

    fn select_controller(&mut self) {
        let owner = self.sim.substrate.entities.get(self.mind).unwrap().owner();
        self.sim.queue_command(CommandEnvelope::new(
            owner,
            self.sim.session.tick + 1,
            Command::Select {
                entity_ids: vec![self.mind],
                additive: false,
            },
        ));
    }

    fn pip_frames(&self, hovered: Option<u64>) -> Vec<usize> {
        let Some(controller) = self.sim.substrate.entities.get(self.mind) else {
            return Vec::new();
        };
        crate::app::presentation::ui_overlays::scale_pip_frames(
            &self.sim,
            controller,
            hovered,
            Some(self.sim.interner.resolve(controller.owner())),
            &self.rules,
        )
    }

    fn first_pulse(&mut self) -> u32 {
        let initial_health = self
            .sim
            .substrate
            .entities
            .get(self.mind)
            .unwrap()
            .health
            .current();
        let spark = self
            .rules
            .ps_type_id_by_name(
                self.rules
                    .combat_damage
                    .default_spark_system
                    .as_deref()
                    .unwrap(),
            )
            .unwrap();
        for _ in 0..180 {
            let before_words = self.words();
            let before: BTreeSet<_> = self
                .sim
                .particle_systems()
                .iter()
                .map(|(&id, _)| id)
                .collect();
            self.step();
            let controller = self.sim.substrate.entities.get(self.mind).unwrap();
            assert!(controller.lifecycle.object_alive() && controller.health.current() > 0);
            assert_eq!(self.victims().len(), 4);
            if controller.health.current() < initial_health {
                assert_eq!(
                    initial_health - controller.health.current(),
                    self.rules.mind_control.overload_damage[1],
                    "actual nonlethal C4 receiver loss must precede the missing rocking assertion"
                );
                let born: Vec<_> = self
                    .sim
                    .particle_systems()
                    .iter()
                    .filter(|(id, system)| !before.contains(id) && system.type_id == spark)
                    .collect();
                assert_eq!(
                    born.len(),
                    5,
                    "the actual overload pulse must construct five live stock SparkSys objects"
                );
                assert!(born.iter().all(|(_, system)| system.alive()
                    && !system.in_limbo()
                    && system.in_logic_vector));
                let pulse_frame = self.sim.session.binary_frame - 1;
                eprintln!(
                    "MIND ordinary overload visited_frame={pulse_frame} tick={} health={} new_sparks={}",
                    self.sim.session.tick,
                    controller.health.current(),
                    born.len()
                );
                let words = self.words();
                assert_eq!(
                    words[2] & 0x7fff_ffff,
                    0x3c75_c28f,
                    "native471C78 must assign row1's signed sideways RATE after the real pulse; visited_frame={}",
                    self.sim.session.binary_frame - 1
                );
                assert_eq!(
                    [words[0], words[1], words[3]],
                    [before_words[0], before_words[1], before_words[3]],
                    "direct overload store preserves the other three native words"
                );
                self.step();
                let controller = self.sim.substrate.entities.get(self.mind).unwrap();
                assert!(controller.lifecycle.object_alive() && controller.health.current() > 0);
                assert!(
                    controller.rocking.as_ref().unwrap().angle_sideways().abs()
                        > SimFixed::lit("0.005"),
                    "the next ordinary Techno visit must expose the overload's visible roll"
                );
                return pulse_frame;
            }
        }
        panic!("four real captive nodes never produced the stock overload pulse");
    }
}

#[test]
fn stock_mastermind_four_real_captures_overload_then_rock_on_the_next_visit() {
    let Some(mut scene) = MindScene::retail() else {
        return;
    };
    scene.capture_four_through_real_attacks();
    scene.first_pulse();
}

#[test]
fn stock_selected_mastermind_submits_real_capture_count_and_overload_pips() {
    let Some(mut scene) = MindScene::retail() else {
        return;
    };
    scene.select_controller();
    let mut observed_counts = BTreeSet::new();
    scene.capture_four_observing(|scene| {
        let controller = scene.sim.substrate.entities.get(scene.mind).unwrap();
        if controller.selection.is_selected() {
            let count = scene.victims().len();
            let mut expected = vec![0; 3];
            expected[..count.min(3)].fill(1);
            if count > 3 {
                expected.push(4);
            }
            assert_eq!(scene.pip_frames(None), expected);
            observed_counts.insert(count);
        }
    });
    assert_eq!(observed_counts, BTreeSet::from([0, 1, 2, 3, 4]));
    let owner = scene
        .sim
        .substrate
        .entities
        .get(scene.mind)
        .unwrap()
        .owner();
    let controller = scene.sim.substrate.entities.get(scene.mind).unwrap();
    assert!(controller.selection.is_selected());
    assert!(controller.lifecycle.object_alive() && !controller.lifecycle.in_limbo());
    assert_eq!(
        controller.health.current(),
        scene.rules.object("MIND").unwrap().strength,
        "real four-capture feedback is observed before the first overload pulse"
    );
    assert_eq!(scene.victims().len(), 4);
    assert!(scene.sim.display_layers().layer_of(scene.mind).is_some());
    // Native DrawPipScale709A90 case5: the current primary weapon supplies
    // three ordinary slots, then IsOverloading4726C0 adds exactly one idle
    // overload slot. Exercise the same status submission used by rendering,
    // after actual FireAt/CaptureUnit and ordinary selection/body admission.
    let frames = crate::app::presentation::ui_overlays::scale_pip_frames(
        &scene.sim,
        controller,
        None,
        Some(scene.sim.interner.resolve(owner)),
        &scene.rules,
    );
    assert_eq!(frames, [1, 1, 1, 4]);

    scene.sim.queue_command(CommandEnvelope::new(
        owner,
        scene.sim.session.tick + 1,
        Command::Select {
            entity_ids: Vec::new(),
            additive: false,
        },
    ));
    scene.step();
    assert!(
        !scene
            .sim
            .substrate
            .entities
            .get(scene.mind)
            .unwrap()
            .selection
            .is_selected()
    );
    assert!(scene.pip_frames(None).is_empty());
    assert_eq!(scene.pip_frames(Some(scene.mind)), [1, 1, 1, 4]);
}

#[test]
fn ordinary_overload_pip_uses_ten_manager_visits_and_queries_preserve_rng() {
    let Some(mut scene) = MindScene::retail() else {
        return;
    };
    scene.select_controller();
    scene.capture_four_through_real_attacks();
    let initial_health = scene
        .sim
        .substrate
        .entities
        .get(scene.mind)
        .unwrap()
        .health
        .current();
    assert_eq!(scene.pip_frames(None), [1, 1, 1, 4]);
    let mut saw_pulse = false;
    for _ in 0..180 {
        scene.step();
        let controller = scene.sim.substrate.entities.get(scene.mind).unwrap();
        if controller.health.current() < initial_health {
            assert_eq!(
                initial_health - controller.health.current(),
                scene.rules.mind_control.overload_damage[1]
            );
            saw_pulse = true;
            break;
        }
        assert_eq!(scene.pip_frames(None), [1, 1, 1, 4]);
    }
    assert!(
        saw_pulse,
        "the feedback window follows an actual ordinary damage pulse"
    );
    // Native471ADF writes10 on the damaging manager visit.471A64 decrements
    // before the next countdown-return visits; the pip consumer70A17F reads
    // only positivity, without a separate presentation clock.
    for visits_since_pulse in 0..=10 {
        let expected = if visits_since_pulse < 10 { 3 } else { 4 };
        let before = (
            scene.sim.main_rng.native_state_hex(),
            scene.sim.scenario_rng.native_state_hex(),
            scene.sim.mapgen_rng.native_state_hex(),
        );
        let manager_before = scene
            .sim
            .substrate
            .entities
            .get(scene.mind)
            .unwrap()
            .capture_manager
            .clone();
        for _ in 0..4 {
            assert_eq!(scene.pip_frames(None), [1, 1, 1, expected]);
            assert_eq!(scene.pip_frames(Some(scene.mind)), [1, 1, 1, expected]);
        }
        assert_eq!(
            before,
            (
                scene.sim.main_rng.native_state_hex(),
                scene.sim.scenario_rng.native_state_hex(),
                scene.sim.mapgen_rng.native_state_hex(),
            )
        );
        assert_eq!(
            manager_before,
            scene
                .sim
                .substrate
                .entities
                .get(scene.mind)
                .unwrap()
                .capture_manager
        );
        if visits_since_pulse < 10 {
            scene.step();
        }
    }
}

fn spark_ids(scene: &MindScene) -> BTreeSet<u64> {
    let ty = scene
        .rules
        .ps_type_id_by_name(
            scene
                .rules
                .combat_damage
                .default_spark_system
                .as_deref()
                .unwrap(),
        )
        .unwrap();
    scene
        .sim
        .particle_systems()
        .iter()
        .filter(|(_, system)| system.type_id == ty)
        .map(|(&id, _)| id)
        .collect()
}

fn captive_ring_ids(scene: &MindScene) -> BTreeSet<crate::sim::anim_class::AnimId> {
    let name = scene.rules.mind_control.controlled_anim.as_deref().unwrap();
    scene
        .sim
        .anims()
        .filter(|(_, anim)| {
            scene.sim.interner.resolve(anim.type_id) == name
                && anim
                    .owner_entity
                    .is_some_and(|id| scene.targets.contains(&id))
        })
        .map(|(&id, _)| id)
        .collect()
}

#[test]
fn stock_captive_force_attack_death_reduces_overload_then_the_controller_settles() {
    let Some(mut scene) = MindScene::retail() else {
        return;
    };
    scene.select_controller();
    scene.capture_four_through_real_attacks();
    scene.first_pulse();
    let owner = scene
        .sim
        .substrate
        .entities
        .get(scene.mind)
        .unwrap()
        .owner();
    let attacker = scene.targets[0];
    let victim = scene.targets[1];
    let rings_before = captive_ring_ids(&scene);
    assert_eq!(rings_before.len(), 4);
    eprintln!(
        "MIND reduction ForceAttack issue_tick={} due_tick={} attacker={attacker} target={victim}",
        scene.sim.session.tick,
        scene.sim.session.tick + 1
    );
    scene.sim.queue_command(CommandEnvelope::new(
        owner,
        scene.sim.session.tick + 1,
        Command::ForceAttack {
            attacker_id: attacker,
            target_id: victim,
        },
    ));
    let weapon = scene.rules.weapon("105mm").unwrap();
    assert_eq!(weapon.projectile.as_deref(), Some("Cannon"));
    assert_eq!(weapon.warhead.as_deref(), Some("AP"));
    assert!(!scene.rules.projectile("Cannon").unwrap().inviso);
    let ap = scene.rules.warhead("AP").unwrap();
    assert!(ap.affects_allies, "actual retail AP permits allied damage");
    assert!(
        !scene.rules.object("MTNK").unwrap().type_immune,
        "retail MTNK is not same-type immune"
    );
    let mut flight = std::collections::VecDeque::new();
    let mut genuine_fire = false;
    let mut reduced = false;
    for _ in 0..500 {
        let shots = scene.step();
        let controller = scene
            .sim
            .substrate
            .entities
            .get(scene.mind)
            .expect("ordinary captive destruction must precede fatal overload");
        assert!(controller.lifecycle.object_alive() && controller.health.current() > 0);
        for shot in shots.iter().filter(|shot| {
            shot.attacker_id == attacker && shot.target == TargetKind::Entity(victim)
        }) {
            assert_eq!(scene.sim.interner.resolve(shot.weapon_id), "105mm");
            assert_eq!(shot.weapon_slot, WeaponSlot::Primary);
            genuine_fire = true;
            assert!(crate::map::playfield::size_diamond_contains(
                scene.sim.playfield_bounds.unwrap().base,
                scene.sim.playfield_size_height.unwrap(),
                (
                    (shot.fire_coord.x / 256) as i16,
                    (shot.fire_coord.y / 256) as i16,
                ),
            ));
            eprintln!(
                "MIND reduction105mm visited_frame={} fire={:?} target_hp={:?} controller_hp={}",
                scene.sim.session.binary_frame - 1,
                shot.fire_coord,
                scene
                    .sim
                    .substrate
                    .entities
                    .get(victim)
                    .map(|actor| actor.health.current()),
                controller.health.current()
            );
        }
        if genuine_fire {
            flight.push_back(format!(
                "frame={} target={:?} shooter={:?} bullets={:?}",
                scene.sim.session.binary_frame - 1,
                scene.sim.substrate.entities.get(victim).map(|actor| (
                    actor.health.current(),
                    crate::sim::movement::ground_pose::position_world_coord(&actor.position)
                )),
                scene.sim.substrate.entities.get(attacker).map(|actor| {
                    crate::sim::movement::ground_pose::position_world_coord(&actor.position)
                }),
                scene
                    .sim
                    .projectiles
                    .iter()
                    .filter(|(_, bullet)| bullet.source_id == attacker)
                    .map(|(id, bullet)| (
                        id,
                        bullet.position,
                        bullet.launch_origin,
                        bullet.launch_target,
                        bullet.target,
                        bullet.velocity,
                        bullet.payload
                    ))
                    .collect::<Vec<_>>()
            ));
            if flight.len() > 24 {
                flight.pop_front();
            }
        }
        if scene.victims().len() == 3 {
            assert_eq!(scene.pip_frames(None), [1, 1, 1]);
            assert!(
                genuine_fire,
                "ordinary friendly-target105mm shots must cause node removal"
            );
            assert!(
                scene
                    .sim
                    .substrate
                    .entities
                    .get(victim)
                    .is_none_or(
                        |actor| actor.health.current() == 0 && !actor.lifecycle.object_alive()
                    )
            );
            assert!(!scene.victims().contains(&victim));
            eprintln!(
                "MIND reduction completed visited_frame={} Stop issue_tick={} due_tick={} controller_hp={}",
                scene.sim.session.binary_frame - 1,
                scene.sim.session.tick,
                scene.sim.session.tick + 1,
                controller.health.current()
            );
            scene.sim.queue_command(CommandEnvelope::new(
                owner,
                scene.sim.session.tick + 1,
                Command::Stop {
                    entity_id: attacker,
                },
            ));
            reduced = true;
            break;
        }
    }
    assert!(
        reduced,
        "real ForceAttack must destroy one captive before the controller overloads fatally:\n{}",
        flight.iter().cloned().collect::<Vec<_>>().join("\n")
    );
    let mut retained_health = scene
        .sim
        .substrate
        .entities
        .get(scene.mind)
        .unwrap()
        .health
        .current();
    for _ in 0..180 {
        scene.step();
        let controller = scene.sim.substrate.entities.get(scene.mind).unwrap();
        assert!(controller.lifecycle.object_alive());
        assert!(
            controller.health.current() >= retained_health,
            "three retained nodes select the nondamaging row while native self-healing continues"
        );
        retained_health = controller.health.current();
        assert_eq!(scene.victims().len(), 3);
        assert_eq!(scene.pip_frames(None), [1, 1, 1]);
    }
    assert!(
        scene.words().iter().all(|word| word & 0x7fff_ffff == 0),
        "removing a node must allow the existing native rocking owner to settle naturally"
    );
    assert!(!scene.sim.substrate.entities.contains(victim));
    let remaining_rings = captive_ring_ids(&scene);
    assert_eq!(remaining_rings.len(), 3);
    assert_eq!(
        rings_before.difference(&remaining_rings).count(),
        1,
        "the dead captive's actual owned ring must retire without erasing surviving links"
    );
}

#[test]
fn stock_fatal_overload_releases_real_captives_and_retires_rings_and_sparks() {
    let Some(mut scene) = MindScene::retail() else {
        return;
    };
    scene.select_controller();
    scene.capture_four_through_real_attacks();
    let original = scene.sim.interner.get("Russians").unwrap();
    let rings = captive_ring_ids(&scene);
    assert_eq!(rings.len(), 4);
    let damage = scene.rules.mind_control.overload_damage[1];
    let mut previous_health = scene.rules.object("MIND").unwrap().strength;
    let mut pulses = 0;
    let mut fatal = false;
    for _ in 0..720 {
        let before = spark_ids(&scene);
        let before_words = scene.words();
        let shots = scene.step();
        let health = scene
            .sim
            .substrate
            .entities
            .get(scene.mind)
            .filter(|actor| actor.lifecycle.object_alive())
            .map_or(0, |actor| actor.health.current());
        if health < previous_health {
            let after = spark_ids(&scene);
            eprintln!(
                "MIND actual HPdrop visited_frame={} previous={} health={} words_before={before_words:?} words_after={:?} new_sparks={} shots={:?}",
                scene.sim.session.binary_frame - 1,
                previous_health,
                health,
                scene
                    .sim
                    .substrate
                    .entities
                    .get(scene.mind)
                    .map(|_| scene.words()),
                after.difference(&before).count(),
                shots
                    .iter()
                    .map(|shot| (
                        shot.attacker_id,
                        scene.sim.interner.resolve(shot.weapon_id),
                        shot.target
                    ))
                    .collect::<Vec<_>>()
            );
            if health > 0 {
                assert_eq!(
                    previous_health - health,
                    damage,
                    "actual nonlethal ordinary C4 pulse damage"
                );
            } else {
                assert!(
                    previous_health <= damage,
                    "a fatal50 packet can clip the remaining live HP"
                );
            }
            assert_eq!(
                after.difference(&before).count(),
                5,
                "even the fatal concrete receiver returns to five real SparkSys constructors"
            );
            pulses += 1;
            eprintln!(
                "MIND natural overload visited_frame={} pulse={pulses} health={health}",
                scene.sim.session.binary_frame - 1
            );
        }
        if health == 0 {
            assert!(
                scene.pip_frames(None).is_empty(),
                "fatal controller cleanup cannot retain a pip strip"
            );
            assert!(
                previous_health > 0 && previous_health <= damage,
                "real preceding pulses and self-healing reached the fatal boundary"
            );
            assert!(
                scene
                    .sim
                    .substrate
                    .entities
                    .get(scene.mind)
                    .is_none_or(
                        |actor| actor.health.current() == 0 && !actor.lifecycle.object_alive()
                    )
            );
            for &id in &scene.targets {
                let actor = scene.sim.substrate.entities.get(id).unwrap();
                assert_eq!(actor.owner(), original);
                assert_eq!(actor.mind_control.controller(), None);
                assert!(!actor.mind_control.is_mind_controlled());
            }
            assert!(
                captive_ring_ids(&scene).is_empty(),
                "real FreeAll releases rings inside the fatal receiver"
            );
            fatal = true;
            break;
        }
        previous_health = health;
    }
    assert!(
        fatal,
        "ordinary overload pulses must eventually kill the stock controller"
    );
    assert!(
        pulses > 1,
        "actual repeated pulses, including healing intervals, must precede fatal overload"
    );
    // Released AI units receive real Stop inputs so their native fate/Hunt
    // does not turn the cleanup observation into a second distant battle.
    for &id in &scene.targets {
        scene.sim.queue_command(CommandEnvelope::new(
            original,
            scene.sim.session.tick + 1,
            Command::Stop { entity_id: id },
        ));
    }
    for _ in 0..240 {
        scene.step();
    }
    assert!(!scene.sim.substrate.entities.contains(scene.mind));
    assert!(scene.pip_frames(Some(scene.mind)).is_empty());
    assert!(rings.iter().all(|&id| scene.sim.anim(id).is_none()));
    assert!(
        spark_ids(&scene).is_empty(),
        "actual overload systems/particles must finish through their normal owners"
    );
    assert!(
        scene
            .sim
            .projectiles
            .iter()
            .all(|(_, bullet)| bullet.source_id != scene.mind)
    );
}

#[test]
fn a_due_overload_component_replaces_its_rate_without_erasing_natural_roll() {
    let Some(mut scene) = MindScene::retail() else {
        return;
    };
    scene.capture_four_through_real_attacks();
    scene.first_pulse();
    let before = scene.words();
    assert!(
        before[0] & 0x7fff_ffff != 0,
        "prior angle comes from a real ordinary rocking visit"
    );
    assert_ne!(
        before[2] & 0x7fff_ffff,
        0x3c75_c28f,
        "the real normal updater has already damped the previous pulse's rate"
    );
    let health = scene
        .sim
        .substrate
        .entities
        .get(scene.mind)
        .unwrap()
        .health
        .current();
    let births = spark_ids(&scene);
    // Declared CaptureManager component continuation. Visit every real
    // countdown step monotonically, without Actor/Particle AI or another
    // RockingUpdate. No timer, node, angle, rate, RNG or health is supplied.
    for _ in 0..=scene.rules.mind_control.overload_frames[1] {
        scene
            .sim
            .capture_manager_update(scene.mind, &scene.rules, None, FrameEffects::default());
        let current = scene
            .sim
            .substrate
            .entities
            .get(scene.mind)
            .unwrap()
            .health
            .current();
        if current < health {
            assert_eq!(
                health - current,
                scene.rules.mind_control.overload_damage[1]
            );
            assert_eq!(spark_ids(&scene).difference(&births).count(), 5);
            let after = scene.words();
            assert_eq!(after[2] & 0x7fff_ffff, 0x3c75_c28f);
            assert_eq!(
                [after[0], after[1], after[3]],
                [before[0], before[1], before[3]],
                "471C78 assigns one rate, preserving naturally nonzero roll"
            );
            return;
        }
        assert_eq!(scene.words(), before);
        scene.sim.session.binary_frame += 1;
    }
    panic!("the actual retained countdown never reached its next component pulse");
}

#[test]
fn internal_snapshot_continues_real_overload_links_rocking_and_next_pulse() {
    let Some(mut scene) = MindScene::retail() else {
        return;
    };
    scene.select_controller();
    scene.capture_four_through_real_attacks();
    scene.first_pulse();
    for _ in 0..6 {
        scene.step();
    }
    let before = scene.words();
    assert!(before[0] & 0x7fff_ffff != 0);
    assert_eq!(scene.pip_frames(None), [1, 1, 1, 3]);
    let saved =
        crate::sim::snapshot::GameSnapshot::save(&scene.sim, 0, 0, "mastermind-overload", 0);
    let mut loaded = crate::sim::snapshot::GameSnapshot::load(&saved)
        .unwrap()
        .sim;
    // Explicit process-stream continuation for an internal Rust state test;
    // this does not claim app/native SaveGame RNG or resource persistence.
    loaded.main_rng = scene.sim.main_rng.snapshot_for_test().into();
    loaded.scenario_rng = scene.sim.scenario_rng.clone();
    loaded.mapgen_rng = scene.sim.mapgen_rng.clone();
    loaded.restore_after_snapshot_load().unwrap();
    loaded.rebuild_caches_after_load(
        scene.sim.resolved_terrain.clone().unwrap(),
        scene.sim.terrain_speed_config.clone(),
        &scene.rules,
    );
    loaded.resolve_type_handles(&scene.rules);
    assert!(loaded.rebuild_dynamic_navigation(&scene.rules));
    let mut restored = MindScene {
        rules: scene.rules.clone(),
        sim: loaded,
        mind: scene.mind,
        targets: scene.targets,
    };
    assert_eq!(restored.words(), before);
    assert_eq!(restored.pip_frames(None), scene.pip_frames(None));
    let mut saw_next_pulse = false;
    let health = scene
        .sim
        .substrate
        .entities
        .get(scene.mind)
        .unwrap()
        .health
        .current();
    for _ in 0..70 {
        scene.step();
        restored.step();
        let live = scene.sim.substrate.entities.get(scene.mind).unwrap();
        let replay = restored.sim.substrate.entities.get(restored.mind).unwrap();
        assert_eq!(live.health.current(), replay.health.current());
        assert_eq!(scene.pip_frames(None), restored.pip_frames(None));
        assert_eq!(scene.words(), restored.words());
        assert_eq!(
            live.capture_manager, replay.capture_manager,
            "owned countdown/voice/node state continues"
        );
        assert_eq!(spark_ids(&scene), spark_ids(&restored));
        assert_eq!(
            scene.sim.main_rng.native_state_hex(),
            restored.sim.main_rng.native_state_hex()
        );
        assert_eq!(
            scene.sim.scenario_rng.native_state_hex(),
            restored.sim.scenario_rng.native_state_hex()
        );
        assert_eq!(
            scene.sim.mapgen_rng.native_state_hex(),
            restored.sim.mapgen_rng.native_state_hex()
        );
        for &id in &scene.targets {
            let a = scene.sim.substrate.entities.get(id).unwrap();
            let b = restored.sim.substrate.entities.get(id).unwrap();
            assert_eq!((a.owner(), a.mind_control), (b.owner(), b.mind_control));
        }
        if live.health.current() < health {
            saw_next_pulse = true;
        }
    }
    assert!(
        saw_next_pulse,
        "continuation must pass a real new overload pulse"
    );
}
