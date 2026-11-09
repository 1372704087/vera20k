# Harvester area guard

A human-owned harvester normally leaves Area Guard the moment its visit runs:
the Foot body queues Harvest and commences it unconditionally. Phobos lets a
type keep Area Guard, optionally only while a threat is in range.

```ini
[MINER]                                  ; UnitType
Harvester.CanGuardArea=false             ; Valueable<bool>, default false
Harvester.CanGuardArea.RequireTarget=false ; Valueable<bool>, default false
```

Reads: `Ext/UnitType/Body.cpp:34-35`, declarations `Ext/UnitType/Body.h:19-20`,
constructor `false` at `:89-90`.

## Behavior

Native hook `FootClass_MissionAreaGuard_Miner @ 0x4D6D34`
(`Ext/Unit/Hooks.Harvester.cpp:75-94`), inside `FootClass::Mission_AreaGuard`:

- the object must be a Unit whose type sets `Harvester.CanGuardArea` **and**
  whose owner is human-controlled; a computer harvester always takes the
  stock Harvest switch;
- without `RequireTarget` the type keeps Area Guard;
- with it, `TargetAndEstimateDamage(Location, ThreatType::Area)` must find a
  threat. When it does, the handler jumps past the Harvest queue and RNG draw
  (`0x4D6D69`) and continues with the ordinary Area Guard body.

## Where VERA20K owns it

`sim::world::techno_ai::mission_handlers::evaluate_foot_area_guard` owns the
`FootClass::Mission_AreaGuard` visit; the harvester switch is its
`Foot4D6D34..4D6D68` block. The Phobos arm sits immediately before it and
skips that block when the type and owner qualify. `RequireTarget` reuses the
shared threat scanner (`techno_ai::target_scan::scan`) with
`ScanMission::AreaGuard` (mask 2, the same scan the Area Guard body uses) at
the object's `Location`, standing in for `TargetAndEstimateDamage`'s area
query.

RESIDUAL: native `TargetAndEstimateDamage` returns an estimated-damage result
and may not install the scanned target; VERA's shared scanner assigns the
target it finds (the same side effect the Area Guard body's own scan has). A
`RequireTarget` harvester that finds a threat can therefore acquire it one
step earlier than native would.

RESIDUAL (test coverage): this checkout has no retail INIs, and the synthetic
fixture's scanner cannot form a candidate, so the "a threat keeps Area Guard"
arm is not exercised by a test. The key/owner gate and the no-threat refusal
are covered through the production dispatcher; the positive arm calls the
same shared scanner the Area Guard body uses.
