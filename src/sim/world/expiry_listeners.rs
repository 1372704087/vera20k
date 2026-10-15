//! Which listeners a pointer expiry reaches.
//!
//! `DispatchPointerExpiredCleanup @ 0x007258D0` calls every object's
//! PointerExpired, and most objects hold nothing an expiry changes, so the
//! broadcast passes over them. This index names, for an expiring id, the
//! entities, anims, particle systems, bullets and waves that hold something,
//! instead of asking every object at every expiry. What each holds is what
//! its callback compares: [`Simulation::entity_expiry_holds`] for an entity,
//! `expiry_names` for the others. A smudge's callback changes nothing, so no
//! smudge is a listener. The index is derived from the stores alone and
//! re-reads an object whenever its store has handed it out mutably (the
//! store's touch log), so it answers as asking every object would
//! ([`ExpiryListeners::asked_of_every`]). Never saved, never hashed; a fresh,
//! cloned or restored store makes it re-read that store's objects.

use std::collections::BTreeSet;

use super::Simulation;
use super::substrate::ObjectKind;
use crate::sim::anim_class::{AnimStore, AnimTouchReader};
use crate::sim::entity_store::{EntityStore, TouchReader};
use crate::sim::game_entity::GameEntity;
use crate::sim::particles::ParticleSystemStore;
use crate::sim::projectile::ProjectileStore;
use crate::sim::touch_log::Touched;
use crate::sim::wave::WaveStore;
use crate::util::lookup_hash::LookupMap;

/// The fewest re-read entities a refresh hands to the worker pool.
const PARALLEL_FROM: usize = 256;

/// The stores whose objects listen to pointer expiry.
pub(crate) struct ListenerStores<'a> {
    pub(crate) entities: &'a mut EntityStore,
    pub(crate) anims: &'a mut AnimStore,
    pub(crate) particle_systems: &'a mut ParticleSystemStore,
    pub(crate) projectiles: &'a mut ProjectileStore,
    pub(crate) waves: &'a mut WaveStore,
}

/// What one listener holds that an expiry can change.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Holds {
    /// The ids its compared fields name, ascending, without repeats.
    names: Vec<u64>,
    /// It holds an object a forward reaches whatever expires.
    every: bool,
}

impl Holds {
    /// What a listener naming `names` holds, or `None` when an expiry passes
    /// it over.
    fn new(mut names: Vec<u64>, every: bool) -> Option<Self> {
        names.sort_unstable();
        names.dedup();
        (every || !names.is_empty()).then_some(Self { names, every })
    }

    /// What `entity` holds.
    fn of_entity(entity: &GameEntity) -> Option<Self> {
        let mut names = Vec::new();
        let every = Simulation::entity_expiry_holds(entity, |id| names.push(id));
        Self::new(names, every)
    }

    /// What an object whose callback compares `names` holds. No object but
    /// an entity holds something any expiry reaches.
    fn of_names(names: [Option<u64>; 2]) -> Option<Self> {
        Self::new(names.into_iter().flatten().collect(), false)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ExpiryListeners {
    entities: StoreIndex,
    anims: StoreIndex,
    particle_systems: StoreIndex,
    projectiles: StoreIndex,
    waves: StoreIndex,
}

/// The listeners of one store.
#[derive(Debug, Clone, Default)]
struct StoreIndex {
    /// What each listener holding anything held when last read. Only looked
    /// up; the one walk over it sorts what it finds.
    holds: LookupMap<u64, Holds>,
    /// (named id, listener naming it): an id's namers are one ascending range,
    /// and a listener that stops naming it leaves in one step, however many
    /// others name it too.
    named_by: BTreeSet<(u64, u64)>,
    /// The listeners an expiry of anything reaches.
    every: BTreeSet<u64>,
    /// Whether every listener has been read.
    built: bool,
}

impl ExpiryListeners {
    /// The listeners `expired`'s expiry reaches, ascending, each with its
    /// store: the entity itself while stored, those naming it, and those any
    /// expiry reaches. The listeners that [`Self::asked_of_every`] selects.
    pub(crate) fn reached_by(
        &mut self,
        stores: ListenerStores<'_>,
        expired: u64,
    ) -> Vec<(u64, ObjectKind)> {
        let ListenerStores {
            entities,
            anims,
            particle_systems,
            projectiles,
            waves,
        } = stores;
        let ids = self.entities.rereads(
            entities.take_touched(TouchReader::ExpiryListeners),
            entities.len(),
            || entities.keys_sorted(),
        );
        let entities = &*entities;
        let read = |&id: &u64| entities.get(id).and_then(Holds::of_entity);
        // Handing a few ids to the worker pool costs more than reading them.
        let now = if ids.len() < PARALLEL_FROM {
            ids.iter().map(read).collect()
        } else {
            crate::sim::parallel::map(&ids, read)
        };
        self.entities.update(ids, now);
        // The other listeners each hold two ids at most: reading them costs
        // less than handing them to the worker pool.
        let ids = self.anims.rereads(
            anims.take_touched(AnimTouchReader::ExpiryListeners),
            anims.len(),
            || anims.iter().map(|(&id, _)| id).collect(),
        );
        let now = ids
            .iter()
            .map(|&id| {
                anims
                    .get(id)
                    .and_then(|anim| Holds::of_names(anim.expiry_names()))
            })
            .collect();
        self.anims.update(ids, now);
        let ids = self.particle_systems.rereads(
            particle_systems.take_touched(),
            particle_systems.len(),
            || particle_systems.iter().map(|(&id, _)| id).collect(),
        );
        let now = ids
            .iter()
            .map(|&id| {
                particle_systems
                    .get(id)
                    .and_then(|system| Holds::of_names(system.expiry_names()))
            })
            .collect();
        self.particle_systems.update(ids, now);
        let ids = self
            .projectiles
            .rereads(projectiles.take_touched(), projectiles.len(), || {
                projectiles.iter().map(|(&id, _)| id).collect()
            });
        let now = ids
            .iter()
            .map(|&id| {
                projectiles
                    .get(id)
                    .and_then(|bullet| Holds::of_names(bullet.expiry_names()))
            })
            .collect();
        self.projectiles.update(ids, now);
        let ids = self.waves.rereads(waves.take_touched(), waves.len(), || {
            waves.iter().map(|(&id, _)| id).collect()
        });
        let now = ids
            .iter()
            .map(|&id| {
                waves
                    .get(id)
                    .and_then(|wave| Holds::of_names(wave.expiry_names()))
            })
            .collect();
        self.waves.update(ids, now);

        let mut reached: Vec<(u64, ObjectKind)> = self
            .entities
            .reaching(expired)
            .map(|id| (id, ObjectKind::Entity))
            .collect();
        if entities.contains(expired) {
            reached.push((expired, ObjectKind::Entity));
        }
        for (index, kind) in [
            (&self.anims, ObjectKind::Anim),
            (&self.particle_systems, ObjectKind::ParticleSystem),
            (&self.projectiles, ObjectKind::Projectile),
            (&self.waves, ObjectKind::Wave),
        ] {
            reached.extend(index.reaching(expired).map(|id| (id, kind)));
        }
        // Object ids are unique across stores; an entity can be reached more
        // than one way.
        reached.sort_unstable_by_key(|&(id, _)| id);
        reached.dedup_by_key(|&mut (id, _)| id);
        reached
    }

    /// The listeners `expired`'s expiry reaches, found by asking every stored
    /// object: what [`Self::reached_by`] answers, for checks.
    pub(crate) fn asked_of_every(
        stores: &ListenerStores<'_>,
        expired: u64,
    ) -> Vec<(u64, ObjectKind)> {
        let names = |names: [Option<u64>; 2]| names.contains(&Some(expired));
        let mut reached: Vec<(u64, ObjectKind)> = stores
            .entities
            .par_select(|id, entity| Simulation::entity_expiry_listener_acts(entity, id, expired))
            .into_iter()
            .map(|id| (id, ObjectKind::Entity))
            .collect();
        reached.extend(
            stores
                .anims
                .iter()
                .filter(|(_, anim)| names(anim.expiry_names()))
                .map(|(&id, _)| (id, ObjectKind::Anim)),
        );
        reached.extend(
            stores
                .particle_systems
                .iter()
                .filter(|(_, system)| names(system.expiry_names()))
                .map(|(&id, _)| (id, ObjectKind::ParticleSystem)),
        );
        reached.extend(
            stores
                .projectiles
                .iter()
                .filter(|(_, bullet)| names(bullet.expiry_names()))
                .map(|(&id, _)| (id, ObjectKind::Projectile)),
        );
        reached.extend(
            stores
                .waves
                .iter()
                .filter(|(_, wave)| names(wave.expiry_names()))
                .map(|(&id, _)| (id, ObjectKind::Wave)),
        );
        reached.sort_unstable_by_key(|&(id, _)| id);
        reached
    }
}

impl StoreIndex {
    /// The listeners to re-read: those `touched` names, or every stored one
    /// (`stored_ids`, ascending) when the log cannot say which or names as
    /// many ids as the store holds (its ids repeat, so sorting them would
    /// cost more), after dropping what the removed ones held.
    fn rereads(
        &mut self,
        touched: Touched,
        stored: usize,
        stored_ids: impl FnOnce() -> Vec<u64>,
    ) -> Vec<u64> {
        match touched {
            Touched::Ids(mut ids) if self.built && ids.len() < stored => {
                ids.sort_unstable();
                ids.dedup();
                ids
            }
            _ => {
                self.built = true;
                let ids = stored_ids();
                let mut gone: Vec<u64> = self
                    .holds
                    .keys()
                    .copied()
                    .filter(|id| ids.binary_search(id).is_err())
                    .collect();
                gone.sort_unstable();
                for id in gone {
                    self.replace(id, None);
                }
                ids
            }
        }
    }

    /// Record what each re-read listener holds now.
    fn update(&mut self, ids: Vec<u64>, now: Vec<Option<Holds>>) {
        for (id, now) in ids.into_iter().zip(now) {
            self.replace(id, now);
        }
    }

    /// The listeners an expiry of `expired` reaches: those any expiry
    /// reaches, then those naming it.
    fn reaching(&self, expired: u64) -> impl Iterator<Item = u64> + '_ {
        self.every.iter().copied().chain(
            self.named_by
                .range((expired, 0)..=(expired, u64::MAX))
                .map(|&(_, listener)| listener),
        )
    }

    fn replace(&mut self, id: u64, now: Option<Holds>) {
        if self.holds.get(&id) == now.as_ref() {
            return;
        }
        if let Some(old) = self.holds.remove(&id) {
            for &name in &old.names {
                self.named_by.remove(&(name, id));
            }
            if old.every {
                self.every.remove(&id);
            }
        }
        if let Some(now) = now {
            self.named_by
                .extend(now.names.iter().map(|&name| (name, id)));
            if now.every {
                self.every.insert(id);
            }
            self.holds.insert(id, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::particle_system_type::ParticleSystemTypeId;
    use crate::sim::anim_class::{AnimObject, AnimRuntime, AnimWorldCoord};
    use crate::sim::capture_manager::CaptureManagerState;
    use crate::sim::combat::{RAD_NO_ATTACKER, TargetKind};
    use crate::sim::intern::InternedId;
    use crate::sim::particles::ParticleSystem;
    use crate::sim::projectile::{ProjectileCoord, ProjectilePayload, ProjectileTarget};
    use crate::sim::wave::Wave;

    /// Every listener store.
    #[derive(Default)]
    struct Stores {
        entities: EntityStore,
        anims: AnimStore,
        particle_systems: ParticleSystemStore,
        projectiles: ProjectileStore,
        waves: WaveStore,
    }

    impl Stores {
        fn listeners(&mut self) -> ListenerStores<'_> {
            ListenerStores {
                entities: &mut self.entities,
                anims: &mut self.anims,
                particle_systems: &mut self.particle_systems,
                projectiles: &mut self.projectiles,
                waves: &mut self.waves,
            }
        }
    }

    /// Reach `expired` through the index and check it against asking every
    /// stored object.
    fn reach(index: &mut ExpiryListeners, stores: &mut Stores, expired: u64) -> Vec<u64> {
        let reached = index.reached_by(stores.listeners(), expired);
        assert_eq!(
            reached,
            ExpiryListeners::asked_of_every(&stores.listeners(), expired),
            "index and stores disagree on {expired}"
        );
        reached.into_iter().map(|(id, _)| id).collect()
    }

    fn anim(id: u64, owner: Option<u64>) -> AnimObject {
        let mut anim = AnimObject::new_unattached_for_test(
            id,
            id as i32,
            InternedId::from_index(0),
            AnimWorldCoord { x: 0, y: 0, z: 0 },
            AnimRuntime {
                current_frame: 0,
                frame_step: 1,
                delay_remaining: 0,
                rate_reload: 1,
                frame_timer: crate::sim::timer::CdTimer::started(0, 1),
                loop_remaining: 0,
                first_ai_guard: false,
                constructor_reverse: false,
                inactive: false,
                paused: false,
            },
        );
        anim.owner_entity = owner;
        anim
    }

    fn bullet(stores: &mut Stores, id: u64, source: u64, target: ProjectileTarget) {
        let payload = ProjectilePayload::new(40, InternedId::from_index(3), None, false);
        stores
            .projectiles
            .construct(id, 0, 0, source, target, payload);
    }

    fn attacker(id: u64, target: u64) -> GameEntity {
        let mut entity = GameEntity::test_default(id, "MTNK", "Americans", 10, 10);
        entity
            .attack_target
            .set_for_test(Some(TargetKind::Entity(target)));
        entity
    }

    #[test]
    fn the_index_reaches_what_asking_every_entity_reaches() {
        let mut entities = EntityStore::new();
        entities.insert(attacker(1, 3));
        let mut controller = GameEntity::test_default(2, "YURI", "Russians", 12, 12);
        controller.capture_manager =
            Some(CaptureManagerState::with_victims_for_test(3, false, &[]));
        entities.insert(controller);
        entities.insert(GameEntity::test_default(3, "HTNK", "Russians", 14, 14));
        let mut docked = attacker(4, 3);
        docked.mark_live_contact_with(3);
        entities.insert(docked);
        let mut stores = Stores {
            entities,
            ..Stores::default()
        };
        let mut index = ExpiryListeners::default();

        // Itself, the entities naming it once or twice, and the one that any
        // expiry reaches.
        assert_eq!(reach(&mut index, &mut stores, 3), [1, 2, 3, 4]);
        assert_eq!(reach(&mut index, &mut stores, 9), [2]);
        // A retarget is read back on the next expiry.
        stores
            .entities
            .get_mut(1)
            .unwrap()
            .attack_target
            .set_for_test(Some(TargetKind::Entity(4)));
        assert_eq!(reach(&mut index, &mut stores, 3), [2, 3, 4]);
        assert_eq!(reach(&mut index, &mut stores, 4), [1, 2, 4]);
        // An entity inserted after the index was built.
        stores.entities.insert(attacker(5, 3));
        assert_eq!(reach(&mut index, &mut stores, 3), [2, 3, 4, 5]);
        // A removed entity reaches nothing and is no listener itself.
        stores.entities.remove(4);
        assert_eq!(reach(&mut index, &mut stores, 4), [1, 2]);
        assert_eq!(reach(&mut index, &mut stores, 3), [2, 3, 5]);
        // A walk over every entity makes it re-read them all.
        for entity in stores.entities.values_mut() {
            entity.capture_manager = None;
        }
        assert_eq!(reach(&mut index, &mut stores, 3), [3, 5]);
        assert_eq!(reach(&mut index, &mut stores, 4), [1]);
    }

    #[test]
    fn many_entities_naming_one_id_are_reread_on_the_worker_pool() {
        let mut entities = EntityStore::new();
        for id in 1..=600 {
            entities.insert(attacker(id, 1000 + id % 3));
        }
        let mut stores = Stores {
            entities,
            ..Stores::default()
        };
        let mut index = ExpiryListeners::default();
        assert_eq!(reach(&mut index, &mut stores, 1000).len(), 200);
        // More hand-outs than stored entities: every entity is re-read, and
        // the removed ones drop out.
        for id in 1..=600 {
            if id % 2 == 0 {
                stores
                    .entities
                    .get_mut(id)
                    .unwrap()
                    .attack_target
                    .set_for_test(Some(TargetKind::Entity(1000)));
            }
            stores.entities.get_mut(id).unwrap();
        }
        for id in (5..=600).step_by(5) {
            stores.entities.remove(id);
        }
        for expired in [1000, 1001, 1002, 5] {
            reach(&mut index, &mut stores, expired);
        }
        // Fewer than the store holds, but enough for the worker pool.
        for id in 1..=400 {
            if let Some(entity) = stores.entities.get_mut(id) {
                entity
                    .attack_target
                    .set_for_test(Some(TargetKind::Entity(1001)));
            }
        }
        for expired in [1000, 1001, 1002] {
            reach(&mut index, &mut stores, expired);
        }
    }

    #[test]
    fn the_index_reaches_the_anims_systems_bullets_and_waves_naming_an_id() {
        let mut stores = Stores::default();
        stores
            .entities
            .insert(GameEntity::test_default(3, "HTNK", "Russians", 14, 14));
        // An anim owned by 3 and one owning nothing; a system on 3 owned by
        // 4; a bullet from 4 at 3 and one from no attacker at a cell; a wave
        // from 3 at 4.
        stores.anims.insert(anim(10, Some(3)));
        stores.anims.insert(anim(11, None));
        let mut system =
            ParticleSystem::test_fixture(20, ParticleSystemTypeId(0), glam::IVec3::ZERO);
        system.attached_entity = Some(3);
        system.owner_entity = Some(4);
        stores.particle_systems.insert(system);
        bullet(&mut stores, 30, 4, ProjectileTarget::Entity(3));
        bullet(
            &mut stores,
            31,
            RAD_NO_ATTACKER,
            ProjectileTarget::Cell { rx: 1, ry: 1 },
        );
        let origin = ProjectileCoord::new(0, 0, 0);
        let mut wave = Wave::new(0, origin, origin);
        wave.owner_id = Some(3);
        wave.target_ref = Some(TargetKind::Entity(4));
        stores.waves.spawn(40, wave);
        let mut index = ExpiryListeners::default();

        assert_eq!(reach(&mut index, &mut stores, 3), [3, 10, 20, 30, 40]);
        assert_eq!(reach(&mut index, &mut stores, 4), [20, 30, 40]);
        // A bullet without an attacker names nothing.
        assert_eq!(reach(&mut index, &mut stores, RAD_NO_ATTACKER), [0u64; 0]);
        // A new owner, a retarget, a cleared target and a removal are read
        // back on the next expiry.
        stores.anims.get_mut(11).unwrap().owner_entity = Some(4);
        stores.projectiles.get_mut(30).unwrap().target = ProjectileTarget::Entity(5);
        assert!(stores.particle_systems.pointer_expired(20, 3));
        stores.waves.remove(40);
        assert_eq!(reach(&mut index, &mut stores, 3), [3, 10]);
        assert_eq!(reach(&mut index, &mut stores, 4), [11, 20, 30]);
        assert_eq!(reach(&mut index, &mut stores, 5), [30]);
        // A cloned store and a walk over every bullet make it re-read them.
        stores.anims = stores.anims.clone();
        stores.anims.remove(10);
        for (_, bullet) in stores.projectiles.iter_mut() {
            bullet.source_id = 6;
        }
        assert_eq!(reach(&mut index, &mut stores, 3), [3]);
        assert_eq!(reach(&mut index, &mut stores, 6), [30, 31]);
    }
}
