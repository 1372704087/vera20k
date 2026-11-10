# Trajectory base and StraightTrajectory (`Trajectory.*`)

Phobos lets a BulletType fly with its own trajectory implementation instead of
the stock `BulletClass::AI` arms. This page covers the base selection
(`Trajectory`, `Trajectory.Speed`) and the `Straight` type: **32 keys**.
Bombard (21 keys) and Parabola (18 keys) have their own pages
([bombard](phobos-trajectory-bombard.md), [parabola](phobos-trajectory-parabola.md)).

```ini
[PROJECTILE]                              ; BulletType
Trajectory=Straight                       ; Nullable<TrajectoryFlag>
Trajectory.Speed=100                      ; double, clamped to >= 0.001

Trajectory.Straight.TargetSnapDistance=128      ; Leptons
Trajectory.Straight.DetonationDistance=102      ; Leptons
Trajectory.Straight.PassThrough=false
Trajectory.Straight.PassDetonate=false
Trajectory.Straight.PassDetonateWarhead=
Trajectory.Straight.PassDetonateDamage=0
Trajectory.Straight.PassDetonateDelay=1
Trajectory.Straight.PassDetonateInitialDelay=0
Trajectory.Straight.PassDetonateLocal=false
Trajectory.Straight.LeadTimeCalculate=false
Trajectory.Straight.OffsetCoord=0,0,0
Trajectory.Straight.RotateCoord=0
Trajectory.Straight.MirrorCoord=true
Trajectory.Straight.UseDisperseBurst=false
Trajectory.Straight.AxisOfRotation=0,0,1
Trajectory.Straight.ProximityImpact=0
Trajectory.Straight.ProximityWarhead=
Trajectory.Straight.ProximityDamage=0
Trajectory.Straight.ProximityRadius=179
Trajectory.Straight.ProximityDirect=false
Trajectory.Straight.ProximityMedial=false
Trajectory.Straight.ProximityAllies=false
Trajectory.Straight.ProximityFlight=false
Trajectory.Straight.ThroughVehicles=true
Trajectory.Straight.ThroughBuilding=true
Trajectory.Straight.SubjectToGround=false
Trajectory.Straight.ConfineAtHeight=0
Trajectory.Straight.EdgeAttenuation=1.0
Trajectory.Straight.CountAttenuation=1.0
Trajectory.Straight.ApplyRangeModifiers=false
```

## Native evidence and hooks

| Address | Hook | Behavior |
| --- | --- | --- |
| — | `TrajectoryTypePointer::LoadFromINI` (`PhobosTrajectory.cpp:52-68`) | `Trajectory` is a `Nullable<TrajectoryFlag>` (`_strcmpi` Straight/Bombard/Parabola); an absent or unknown name keeps the stock arms. The constructed type reads `Trajectory.Speed` first (clamped `max(0.001, x)`) and then its own keys. |
| `0x4666F7` | `BulletClass_AI_Trajectories` | `OnAI` runs first; a true answer jumps to the standard detonation at `0x467E53`. |
| `0x46745C` | `BulletClass_AI_Position_Trajectories` | `OnAIVelocity` adds the type-adjusted gravity back to the speed scratch, so a trajectory flight has no net gravity. |
| `0x4677D3`, `0x467927` | target-coordinate / techno checks | Straight returns `SkipGameCheck`: its own checks replace them. |
| `0x467C1C` | ranged check | `ShouldSkipRangedCheck()`; Straight skips. |
| `0x4674D4` | bridge check | `ShouldSkipBridgeCheck()` = `!SubjectToGround`. |
| `0x467E53` | pre-detonation | `OnAIPreDetonate`: damage through `GetTheTrueDamage`, optional ground snap, target snap within `TargetSnapDistance`. |
| `0x468B72` | `BulletClass_Unlimbo_Trajectories` | Creates the per-bullet instance and runs `OnUnlimbo` (speed, offset, burst data, lead wait). |

`StraightTrajectory.cpp` owns the per-frame arms: `OnAI` (remaining distance,
`DetonationDistance`, ground, map, then the velocity check, pass and proximity
detonations, confine and last check), the stock motion with gravity cancelled,
`GetVelocityZ`, burst rotation, `EdgeAttenuation`/`CountAttenuation` damage,
and the `GetCellsInRectangle` proximity scan.

## Where VERA20K owns it

- `rules::phobos_trajectory::PhobosTrajectoryRules` holds the flag, speed and
  every Straight key; `ProjectileType::phobos_trajectory` carries them. The
  two `Read<true>` warhead names are pre-allocated on the same rules pass by
  `NativeRulesProcessor::allocate_trajectory_warhead_references` (the
  WarpInWeapon pattern).
- `sim::world::phobos_trajectory` holds the per-bullet
  `PhobosTrajectoryState` (the native `StraightTrajectory` instance fields),
  `OnUnlimbo`/`PrepareForOpenFire` in `Simulation::init_phobos_trajectory`
  (called from the Fire admission) and the per-visit AI in
  `Simulation::step_phobos_trajectory`. The bullet AI visit
  (`techno_ai::visit_logic_projectile`) routes a trajectory bullet through the
  step instead of `ProjectileStore::advance_after_head`; detonations go
  through the normal `ProjectileDetonation` / `commit_logic_projectile_detonations`
  boundary and direct proximity damage through `commit_entities`.
- Motion reuses the world's projectile collision owner: a new
  `ProjectileCollisionWorld::phobos_trajectory_motion` keeps the stock
  ordinary geometry (floor, bridge, obstacle, map bounds, stopped-bullet
  ground impact) but omits the target-coordinate/techno hit tests. The
  impact ladder builds the detonation.
- `Trajectory.Speed` is `max(0.001, x)`; `EdgeAttenuation`/`CountAttenuation` clamp to `>= 0.0`.
  `OffsetCoord`/`AxisOfRotation` use the native `Read3Integers` prefix rule.
- `ApplyRangeModifiers` uses the AttachEffect range owner
  (`range_with_attach_effects`) for the detonation distance and the
  attenuation range.

## RNG draws this chain passes

- `PrepareForOpenFire`'s `Inaccurate=` scatter: one `RandomRanged(offsetMin,
  offsetMax)` magnitude draw, then one raw direction draw through the retail
  angle table.
- The pass-through, proximity and last-check warhead detonations draw only
  what the warhead impact itself draws (the shared combat owner).
- `OnAI`'s checks, the velocity check and the rectangle scan draw nothing.

## Phobos quirks kept

- `MirrorCoord` constructs **yes**; an odd `CurrentBurstIndex` mirrors
  `OffsetCoord.Y` at Unlimbo and the burst sign at rotation time.
- `CountAttenuation != 1` scales the bullet's own damage and the stored
  pass/proximity damages after every hit; a zero ratio zeroes them.
- `ProximityImpact` counts down the hits; a value of 1 makes the first valid
  target the `ExtraCheck` obstacle for the next frame instead of damaging it.
- `ProximityFlight` adds airborne targets on top of the ground scan.
- The map test is the native diamond (`MapClass::CoordinatesLegal`), not a
  rectangle.
- `PassDetonate` and proximity hits are warhead detonations that leave the
  bullet flying; they ride `TrajectoryStep::extra_detonations`, not the
  terminal impact that retires it.
- A snapping pre-detonation sets `PhobosTrajectoryState::snapped_to_target`,
  the retained `BulletExt::SnappedToTarget` byte.

## Residuals (trigger · impact · frequency · risk)

- **`GetRandomCoordsNear` direction draw.** The native helper is not ported;
  the `Inaccurate=` scatter uses the existing retail-table polar draw
  (`random_coords_near`). Trigger: an `Inaccurate=` bullet with a trajectory.
  Impact: the scatter direction may differ by one table step. Frequency:
  inaccurate shots only. Risk: determinism/aim, bounded by `BallisticScatter`.
- **`BallisticScatter.Min/Max` source.** Native reads Ares' BulletTypeExt; a
  Phobos-only build uses `0` and `[CombatDamage] BallisticScatter=`. Trigger:
  Ares keys on a Phobos-only build. Frequency: Ares-authored data. Risk: low.
- **`AircraftTracker`.** `ProximityFlight` scans every aircraft in stable-id
  order instead of the tracker's cell vector. Impact: same set, different
  ordering for simultaneous ties. Frequency: proximity flight with several
  aircraft. Risk: low.
- **Trajectory hook ordering.** The step runs after the visit's visual frame
  and Trailer construction instead of before them (`0x4666F7` precedes
  `0x466826`). Impact: a detonation frame's anim spawn order. Frequency: rare.
  Risk: presentation only.
- **LaserTrails on bullets.** Bullet `LaserTrail.Types` parsing, the
  per-bullet `LaserTrailClass` list and the `0x46745C` future-coordinate
  update (`PhobosTrajectory.cpp:515-531`) are one separate bullet-trail
  chain; VERA models only the stock `UseLineTrail` receipt.
  Frequency: none in retail. Risk: visual.
- **Merged-section reads.** A later INI layer re-declaring `Trajectory=` with
  a different flag reconstructs the type natively; the merged reader sees only
  the final value. Frequency: layered re-declarations. Risk: low.
- **`Trajectory.Sample.*`.** `SampleTrajectoryType::Flag()` returns
  `TrajectoryFlag::Invalid` (`SampleTrajectory.h:15`) and the `Trajectory`
  reader accepts only `Straight`/`Bombard`/`Parabola`
  (`PhobosTrajectory.cpp:27-49`), so no INI flag constructs a Sample instance;
  `Trajectory.Sample.TargetSnapDistance` is dormant and unported.
- **Retail/visual parity untested** (no retail `ini/` in this checkout).
