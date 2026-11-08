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
| `Shrapnel.IgnoreHitBuildings` | BulletType | `no` | `Ext/BulletType/Body.cpp:44` |
| `Shrapnel.UseWeaponTargeting` | BulletType | `no` | `Ext/BulletType/Body.cpp:43` |

Both type-level keys are `Nullable<bool>` and fall back to the same-named
`[CombatDamage]` global. That global is a `Valueable<bool>` with constructor
default `false` (`Ext/Rules/Body.h:464-465`, `:968-969`), and Phobos passes it to
`Get(T)` through `Valueable<T>::operator const T&`, so an unset key and an unset
global both read `no`. `no` is also the stock behaviour, so a rules set that
never names these keys is unchanged. The global has no unset state and no
`none`/`<default>` clearing token — do not give these reads `Nullable`
semantics.

## Behavior

Stock refuses shrapnel when the impact cell holds no ground object and when its
first ground object is a building; it admits a terrain object or any other
entity. Phobos keeps that and adds:

- `Shrapnel.AffectsGround=yes` admits an empty impact cell.
- `Shrapnel.AffectsBuildings=yes` admits a building head. That building becomes
  the emission's `InitialTargetBuilding`, which the ring skips **by identity**,
  so it is never a child's target.
- `Shrapnel.IgnoreHitBuildings=yes` keeps every other building to one child per
  emission: a building that passed the ring's tests is recorded, and later cells
  headed by it are skipped. The record is per emission (`Hooks.cpp:260`).

The firing loop, the child construction and the RNG draws are unchanged; only
the early refusal differs.

## Residuals

- `Shrapnel.UseWeaponTargeting` is parsed but not consumed
  (`Ext/Bullet/Hooks.cpp:309-342`): it replaces the ally test with the child
  weapon's target eligibility — legal target, cell eligibility, house/techno
  eligibility, health and veterancy thresholds, required attached effects,
  shield armor, and the warhead's zero-versus-armor skip. Its own chain.
- `Shrapnel.ObeyWarheadTriggerConditions`
  (`Ext/Bullet/Hooks.DetonateLogics.cpp:22`) gates the detonation logic list.
- The `[CombatDamage]` globals are read once from the merged layer state
  (`rules::phobos_globals`), which equals Phobos re-reading them on every
  `Process` for scalar keys. A later layer naming only one key keeps the other.
- Retention across scenarios in one process is unverified: Phobos reads these
  into the `RulesClass`-lifetime `RulesExt::Data` and never resets them, while
  VERA builds a fresh `PhobosGlobals` from the current layer stack. Within one
  scenario the fold is equivalent; a second scenario that does not author a key
  the first map authored would read `no` in VERA and keep the earlier `yes` in
  Phobos. Settle it with a two-map load.
