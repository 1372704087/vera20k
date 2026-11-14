# ProjectileRange.ApplyModifiers

A `Ranged=` projectile carries the weapon's `ProjectileRange` as its travel
budget. Phobos can fold the firer's range modifiers into that budget:
`GetRangeWithModifiers` (the veteran `Range` ability and AttachEffect range
effects).

```ini
[CombatDamage]
ProjectileRange.ApplyModifiers=yes  ; boolean, default yes

[COIL]                              ; Weapon section
ProjectileRange.ApplyModifiers=no   ; nullable, unset -> global
```

Readers: the global at `Ext/Rules/Body.cpp:650` (`Valueable<bool>`,
constructor yes at `Ext/Rules/Body.h:544,1044`); the weapon at
`Ext/WeaponType/Body.cpp:88` (`Nullable<bool>` unset, `Body.h:30,125`).

## Native body

- `BulletClass_Update_Ranged` (hook in `Ext/Bullet/Hooks.cpp:640-660`) starts
  every step from `maxRange = pThis->Range`; with a weapon and owner it
  replaces that by `WeaponTypeExt::GetRangeWithModifiers(weapon, owner,
  maxRange)` when the weapon's nullable key else `[CombatDamage]`'s global
  is set. `DistanceTraveled >= maxRange` then detonates the bullet.
- `GetRangeWithModifiers` (`Ext/WeaponType/Body.cpp:398-443`): `-512` returns
  first; a firer with `AdditionalAbility(Range)` multiplies by
  `max(VeteranRange.Get([General] VeteranRange), 0.0)`; then every active
  AttachEffect whose weapon lists admit the weapon multiplies its range and
  sums `WeaponRange.ExtraRange` cells, with a final floor at zero.

## VERA

`CombatDamageDefaults::phobos_projectile_range_apply_modifiers` and
`WeaponType::phobos_projectile_range_apply_modifiers` read the two keys.
`attach_effect::get_range_with_modifiers` is the one `GetRangeWithModifiers`
owner: sentinel, the veteran arm (`crit::has_phobos_ability` +
`PhobosTechnoExtras::veteran_range_multiplier`), the open-topped transport
source and the existing AttachEffect fold. InRange, weapon-range queries, the
force-weapon sweep and the target-distance helper call it instead of the
AttachEffect-only fold. `attach_effect::initial_projectile_range` applies the
two keys and the fold to a `Ranged=` projectile at fire time (both the
ordinary fire and the airburst child), whose remaining range the Ares bullet
update already spends.

## Tests

- `src/sim/attach_effect.rs` —
  `the_veteran_range_ability_scales_get_range_with_modifiers` (rookie,
  veteran, elite, type-over-global, elite-only tier, sentinel) and
  `initial_projectile_range_reads_the_global_and_weapon_keys` (default fold,
  global off, weapon over global).

The veteran arm and the key gate were each mutated red and restored.

## Residuals

- VERA applies the fold when the bullet is created rather than on every
  update; the firer's rank, effects and transport at creation are the values
  the native update reads on the first step, so the spent budget matches
  unless one of them changes mid-flight.
- `GetRangeWithModifiers`'s `CanOccupyFire` arm (an occupied building's
  `OccupyWeaponRange`) is not part of this owner; callers pass their own base
  range as native's two-argument call site does.
