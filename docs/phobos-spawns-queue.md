# Customizable spawns queue (`Spawns.Queue`)

A `ValueableVector<AircraftType*>` that overrides which type a spawn manager
builds into each of its slots, and which it rebuilds next when one is lost.

## Keys

| Key | Type | Default |
|---|---|---|
| `Spawns.Queue=` | `ValueableVector<AircraftType*>` | empty |

Read: `TechnoTypeExt::LoadFromINI` (`Ext/TechnoType/Body.cpp:1118`),
declaration `Ext/TechnoType/Body.h:334`, constructor `Body.h:768`. VERA reads it
in `src/rules/phobos_techno_extras.rs`, owner
`ObjectType::phobos_techno_extras.spawns_queue`.

## Behaviour (not yet consumed)

Two hooks, both in `Ext/Techno/Hooks.Misc.cpp`:

```cpp
// 0x6B6D44, SpawnManagerClass::Init — slot i
if (Spawns_Queue.size() <= i || !Spawns_Queue[i]) return Continue;
R->EAX(Spawns_Queue[i]->CreateObject(pThis->Owner->Owner));

// 0x6B78D3, SpawnManagerClass::Update — regeneration
std::vector<AircraftTypeClass*> vec = Spawns_Queue;
for (const auto pNode : SpawnedNodes)
    if (pNode->Unit) { auto it = find(vec, pNode->Unit->Type); if (it != vec.end()) vec.erase(it); }
if (vec.empty() || !vec[0]) return 0;
R->EAX(vec[0]->CreateObject(pOwner->Owner));
```

So the init arm names the type per slot by index, and the regeneration arm
picks the first listed type that no live node currently is.

## Residual

**The key is read and retained, but neither hook is wired.** VERA's
`SpawnManagerState` carries one `spawn_type` for the whole manager and its
`SpawnHost::create_child` builds that one type; `fill_slot`
(`src/sim/spawn_manager.rs`) is the only construction site. Reproducing the two
hooks needs three things this port does not have yet:

1. a per-slot type override (init arm) and a manager-level queue (update arm)
   on `SpawnManagerState`, threaded through `init_spawn_manager`'s pooled
   `SpawnManagerState` construction;
2. a `SpawnHost::create_child_of(type)` used by `fill_slot` in place of the
   single `create_child()`, including the "first type not currently alive"
   subtraction the update arm performs;
3. the `InitialSpawnsNumber=` half of the same `0x6B6D44` hook, which is a
   separate tracker entry and today leaves every slot regenerating.

Until then a type with `Spawns.Queue=` builds its `Spawns=` type in every slot,
and regenerates that same type.

## Tests

- `src/rules/phobos_techno_extras_tests.rs` — the parse group
  (`the_two_type_lists_split_on_commas_only` covers the same `ValueableVector`
  reader through the two `RadarJam*` lists, and `every_key_reads_on_its_own`
  covers `Spawns.Queue`'s own split).
