# Extended aircraft missions

Phobos's `ExtendedAircraftMissions` rework of the aircraft mission machinery.
Two `[General]` keys switch it on and set the unlandable damage; six
`[AircraftType]` keys tune it per type. This page records what VERA reads,
which hook behaviours are ported, and the residuals.

## Keys

| Layer | Key | Type | Default |
| --- | --- | --- | --- |
| `[General]` | `ExtendedAircraftMissions` | bool master switch | `no` |
| `[General]` | `ExtendedAircraftMissions.UnlandDamage` | int | `0` |
| `[AircraftType]` | `ExtendedAircraftMissions` | nullable bool, overrides the global master | unset |
| `[AircraftType]` | `ExtendedAircraftMissions.SmoothMoving` | nullable bool | unset |
| `[AircraftType]` | `ExtendedAircraftMissions.EarlyDescend` | nullable bool | unset |
| `[AircraftType]` | `ExtendedAircraftMissions.RearApproach` | nullable bool | unset |
| `[AircraftType]` | `ExtendedAircraftMissions.FastScramble` | nullable bool | unset |
| `[AircraftType]` | `ExtendedAircraftMissions.UnlandDamage` | nullable int | unset |

Readers: `Ext/Rules/Body.cpp:191-192` with the constructor defaults in
`Ext/Rules/Body.h:97-98`, and `Ext/AircraftType/Body.cpp:30-35` with the
`Nullable` fields in `Ext/AircraftType/Body.h:21-26`. The keys live in
`AircraftTypeExt`, so a non-aircraft section that repeats them carries no value
(`src/rules/object_type.rs`, gated on `ObjectCategory::Aircraft`).

Resolution follows the hooks (`Ext/Aircraft/Hooks.cpp:513`, `:596`, `:701`,
`:671`, `:1090`, `:773`, `:1049`): the type's master is
`type.Master.Get([General] ExtendedAircraftMissions)`; each of `SmoothMoving`,
`EarlyDescend`, `RearApproach` and `FastScramble` is `key.Get(master)`; and
`UnlandDamage` is `key.Get([General] ExtendedAircraftMissions.UnlandDamage)`,
which bypasses the master switch. Absent, blank, `none` and `<default>` all
leave the type key unset, so the fallback applies
(`src/rules/phobos_extended_aircraft_missions.rs`,
`RuleSet::extended_aircraft_missions_for`).

## Ported behaviours

### Unlandable damage (`0x414DA8`)

`AircraftClass_Update_UnlandableDamage`
(`Ext/Aircraft/Hooks.cpp:735-790`) runs after `FootClass::Update` for an alive,
`AirportBound`, non-airstrike aircraft (`IsALoaner` is not represented and stays
false). While the aircraft `IsInAir` (vt+0x54, VERA's `is_high_flying`):

- `UnlandDamage > 0`: every fourth frame, selected by
  `(CurrentFrame - LastFireBulletFrame + UniqueID) & 3 == 0`, it takes that much
  damage from `[CombatDamage] C4Warhead=` with no attacker
  (`ReceiveDamage(&damage, 0, C4Warhead, nullptr, true, false, nullptr)`).
  VERA applies the same receiver event with `ignore_defenses` set and the second
  call flag clear (`src/sim/world/techno_ai.rs`,
  `aircraft_unlandable_damage_step`).
- `UnlandDamage < 0` and the master switch on: `Crash(nullptr)`. A negative
  value with the master off stays stock.

### Fast scramble (`0x4425B6`)

`BuildingClass_ReceiveDamage_NoDestroyLink`
(`Ext/Aircraft/Hooks.cpp:1084-1093`) rewrites `BuildingClass::ReceiveDamage`'s
destroy-link loop (`0x442511..0x4425EE`). The loop kills every linked object
within 0x100 leptons, and every linked object of a `Helipad=`, with
`Strength * 10` C4 damage; a farther contact of a non-helipad gets RUN_AWAY
(`0x44259D`). For a helipad's linked aircraft whose type resolves
`ExtendedAircraftMissions.FastScramble`, Phobos takes the RUN_AWAY arm instead:
the aircraft is released by the dying building rather than destroyed with it
(`src/sim/world/lifecycle.rs`, `building_now_dead_contacts`).

## Residuals

Each residual names its trigger, effect and frequency. The stock behaviour is
untouched; the hooks fire only for a mod that turns the relevant key on.

- **SmoothMoving** (`Ext/Aircraft/Hooks.cpp:495-526`, sites `0x4168C7` and
  `0x416A0A` in `AircraftClass::Mission_Move @ 0x004166E0`). Trigger: an
  `AirportBound` aircraft outside a team, not loaner/airstrike, on Move, with
  `ExtendedAircraftMissions.SmoothMoving` resolved on. Effect: the hook keeps
  the aircraft flying while its horizontal distance to the waypoint exceeds
  `max(SlowdownDistance / 2, turningRadius)` instead of latching arrival by cell,
  and on arrival tries `TryNextPlanningTokenNode()` before entering idle (keeping
  the mission while `QueuedMission == Area_Guard`). VERA's `Move` port
  (`src/sim/aircraft/move_mission.rs`) has no planning-token queue and latches
  arrival on the destination cell, so the smoother arrival and the waypoint
  continuation are not reproduced. Frequency: visible only for mods that drive
  aircraft along YR planning waypoints; retail missions are unaffected.
- **RearApproach** (`:596`, `:701`). Trigger: `RearApproach` resolved on for an
  aircraft returning to a dock it is heading to (`Destination` set and
  `DockNowHeadingTo == Destination` or `SpawnOwner == Destination`). Effect: the
  hook offsets the flight destination sideways by up to
  `max(SlowdownDistance / 256, turningRadius / 128)` cells (using
  `AdjacentCoord[landingDir]`, and one more face when the bearing differs by
  `>= 12288` raw) and points the primary facing at that offset, so the aircraft
  curves in from the landing direction instead of flying straight at the pad.
  VERA does not retain `DockNowHeadingTo`/`SpawnOwner == Destination`, so the
  primary-facing block in `Simulation::fly_horizontal_step`
  (`src/sim/world/fly_process.rs`) keeps the stock heading. Frequency: the
  approach path of every dock-returning aircraft in a mod that enables the key.
- **EarlyDescend** (`:669-671`, site `0x4CF3D0` in
  `FlyLocomotionClass::FlightUpdate`). Trigger: `EarlyDescend` resolved on for a
  non-`HunterSeeker` aircraft. Effect: the hook replaces the height block: the
  stock `IsDropship && distance < SlowdownDistance` slow-descent gate becomes
  `Destination && (DockNowHeadingTo == Destination || SpawnOwner ==
  Destination)`, additionally requiring, under `RearApproach`, that the current
  facing be within 16384 raw of the landing direction; the aircraft starts
  descending within `SlowdownDistance` instead of only above the pad. VERA's
  height block (`fly_horizontal_step`, `src/sim/world/fly_process.rs`) keys off
  the Fly's destination mode and `IsDropship`, not the dock destination, so the
  early slow descent is not reproduced. Frequency: the descent profile of a
  mod's returning aircraft; a visual difference in approach height.
- **Unlandable damage, redirect arms** (`Ext/Aircraft/Hooks.cpp:745-762`).
  Trigger: the master switch on. Effect: the hook first redirects an aircraft
  whose `ArchiveTarget` is farther than `GetGuardRange(1) * 1.1` (clear Target,
  set destination to the archive) and calls `TryNearestDockBuilding(&Dock, 0,
  0)`; with the switch off the positive-damage arm refuses damage when that dock
  search answers. Neither arm is ported, so VERA can damage an aircraft Phobos
  would first send to a nearby dock and does not pull a distant Area-Guard
  aircraft back to its archive. Frequency: only with a positive
  `UnlandDamage`; the retail default `0` is dormant.
- **Idle no-crash** (`:1044-1073`, sites `0x4179F7` and `0x417B82` in
  `AircraftClass::Enter_Idle_Mode`). Trigger: an `AirportBound` aircraft with no
  dock that would crash on entering idle, not airstrike/loaner, with
  `UnlandDamage >= 0`. Effect: instead of crashing, an unteamed aircraft not
  already Area-Guarding at an archive gets the nearest friendly airfield cell
  as destination and archive target and queues Area Guard; a team member just
  skips the crash. VERA's `enter_idle_mode` (`src/sim/aircraft/idle_entry.rs`)
  still crashes it. Frequency: AirportBound aircraft that run out of docks in a
  Phobos build; default `UnlandDamage=0` means the hook is active whenever a
  Phobos build runs, so this is the widest residual on this page.

## Tests

- `rules::phobos_extended_aircraft_missions` — the two globals and six type
  keys through the production `RuleSet` reader: defaults, the type master
  override, per-key fallbacks, `UnlandDamage` bypassing the master, and the
  aircraft-only category gate.
- `sim::aircraft::unland_damage_tests` — a positive value hurts an airborne
  airport-bound aircraft in five-point hits every fourth frame; a negative value
  with the master on crashes it.
- `sim::aircraft::dock_cycle_tests::a_fast_scramble_aircraft_takes_off_instead_of_dying_with_its_helipad`
  — the same killing hit destroys a normal docked ORCA and releases a
  `FastScramble` one.
