# Phobos shrapnel admission

`BulletClass_Shrapnel_Forced @ 0x46A3D6` (`Ext/Bullet/Hooks.cpp:252-282`)
replaces the stock admission test that decides whether a detonating bullet
spawns its `ShrapnelWeapon=` children at all. This page covers the admission
gate; the per-target filters are residual.

## Keys

| key | section | default | source |
|---|---|---|---|
| `Shrapnel.AffectsGround` | BulletType | `no` | `Ext/BulletType/Body.cpp:41` |
| `Shrapnel.AffectsBuildings` | BulletType | `no` | `Ext/BulletType/Body.cpp:42` |

Both are `Nullable<bool>` and fall back to the same-named `[CombatDamage]`
global. Phobos passes the global `Nullable` itself to `Get(T)`, which converts
through `Valueable<T>::Value`, so an unset key and an unset global both read
`no`. `no` is also the stock behaviour, so a rules set that never names these
keys is unchanged.

## Behavior

Stock refuses shrapnel when the impact cell holds no ground object and when its
first ground object is a building; it admits a terrain object or any other
entity. Phobos keeps that and adds:

- `Shrapnel.AffectsGround=yes` admits an empty impact cell.
- `Shrapnel.AffectsBuildings=yes` admits a building head.

The firing loop, the child construction and the RNG draws are unchanged; only
the early refusal differs.

## Residuals

- `Shrapnel.AffectsBuildings` is read but not consumed yet: admitting the impact
  building also needs native's `InitialTargetBuilding` skip in the ring
  (`Ext/Bullet/Hooks.cpp:300-301`), which this ring owner does not track.
- `Shrapnel.IgnoreHitBuildings` (`:295,303`) needs the same `TargetsToIgnore`
  bookkeeping.
- `Shrapnel.UseWeaponTargeting` (`:309-330`) replaces the candidate test with the
  child weapon's target eligibility.
- `Shrapnel.ObeyWarheadTriggerConditions`
  (`Ext/Bullet/Hooks.DetonateLogics.cpp:22`) gates the detonation logic list.
- The `[CombatDamage]` globals are read once from the merged layer state
  (`rules::phobos_globals`), which equals Phobos re-reading them on every
  `Process` for scalar keys. A later layer naming only one key keeps the other.
