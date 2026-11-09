# Penetrates transport

Phobos lets a warhead reach the passengers of a transport it hits
(`Ext/WarheadType/Body.cpp:437-442`, `Ext/TechnoType/Body.cpp:1174-1177`;
consumer `WarheadTypeExt::ApplyPenetratesTransport`,
`Ext/WarheadType/Detonate.cpp:780-930`, called from `DetonateOnOneUnit` at
`:233`).

```ini
[WARHEAD]                               ; WarheadType
PenetratesTransport.Level=0             ; int, default 0 (disabled)
PenetratesTransport.PassThrough=1.0     ; double, default 1.0
PenetratesTransport.FatalRate=0.0       ; double, default 0.0
PenetratesTransport.DamageMultiplier=1.0
PenetratesTransport.DamageAll=false
PenetratesTransport.CleanSound=         ; ValueableIdx<VocClass>, default none

[TRANSPORT]                             ; TechnoType
PenetratesTransport.Level=              ; Nullable<int>, falls back to global
PenetratesTransport.PassThroughMultiplier=1.0
PenetratesTransport.FatalRateMultiplier=1.0
PenetratesTransport.DamageMultiplier=1.0

[CombatDamage]
PenetratesTransport.Level=10            ; global fallback, default 10
```

## Behavior

The arm runs in the bullet detonation's per-target pass, before the
shield/AE/crit blocks; Phobos passes the bullet's Health as the damage (a pure
`MapClass::DamageArea` passes 0). A warhead level at or below the transport's
effective level returns. `PassThrough` is the warhead value times the
transport's multiplier; when below 1, a `RandomDouble` above it skips the
effect. `FatalRate` likewise; when above 0, a second `RandomDouble` at or
below it makes the hit fatal (the passenger's max Strength, defenses
ignored). Otherwise the damage is
`ceil(bullet Health * warhead DamageMultiplier * transport DamageMultiplier)`.
`DamageAll=yes` damages every passenger whose own level is below the
warhead's; otherwise one `Random(0, n-1)` passenger is picked. An attack that
leaves no passenger plays `CleanSound` at the transport.

## Where VERA20K owns it

`rules::phobos_penetrates_transport` reads the warhead/techno keys,
`ObjectType` and `WarheadType` carry them and `phobos_globals` reads the
global level. `sim::combat::penetrates_transport` runs inside
`crit::detonate`'s per-target loop (the `DetonateOnOneUnit` port);
`crit::detonate` now carries the bullet damage. Passenger damage goes through
the shared receiver (`commit_entities`), so armor, verses and shields apply.

## Residuals

- `SetLocation(transporterCoords)` before each passenger's damage is not
  applied; a killed passenger's death effects use its own stored position.
- Gunner removal/re-add (`Gunner=` transports) is not ported.
- The `distance` handed to ReceiveDamage is 0; native forwards the area
  distance, which the VERA damage kernel does not consume.
- Under-attack events produced by the passenger damage are dropped.
- `ApplyPerTargetEffectsOnDetonate=no` keeps the existing VERA residual: the
  pass stays on the bullet entry rather than moving into ReceiveDamage.
