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
(`Ext/Unit/Hooks.Harvester.cpp:71-90`), inside `FootClass::Mission_AreaGuard`:

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

RESIDUAL: Phobos also hooks the scan entry `0x70982C` to apply a
per-TechnoType targeting delay; that hook is not part of this chain, so a
`RequireTarget` scan still uses the global `[General]` Guard Area delay.

RESIDUAL (test coverage): this checkout has no retail INIs, so
`retail_ini_fixture` returns `None` and the retail-gated positive-arm test
(the key keeps Area Guard with a threat) is skipped here. The synthetic
fixture cannot form a scanner candidate, so the positive arm runs only on a
machine with the retail data. The key/owner/reachability gate, the no-threat
refusal and the scan attempt itself (the targeting-timer stamp) are covered
through the production dispatcher.
