# Parabombs (`Parachuted`, `BombParachute`)

A `BulletType` whose projectile is born under a canopy and descends to the
ground instead of flying its trajectory.

## Keys

| Key | Type | Default |
|---|---|---|
| `Parachuted=` | `Valueable<bool>` | `no` |
| `Parachuted.FallRate=` | `Valueable<int>` | `1` |
| `Parachuted.MaxFallRate=` | `Nullable<int>` | unset |
| `BombParachute=` | `Nullable<AnimTypeClass*>` | unset |

Read: `BulletTypeExt::LoadFromINI` (`Ext/BulletType/Body.cpp:79-82`),
declarations `Ext/BulletType/Body.h:83-86`, constructor `Body.h:145-148`. VERA
reads them in `src/rules/phobos_parabomb.rs`, owner
`ProjectileType::phobos_parabomb`. Tests:
`src/rules/phobos_parabomb_tests.rs`.

An unset `Parachuted.MaxFallRate=` falls back to `[General] ParachuteMaxFallRate`
and an unset `BombParachute=` to `[General] BombParachute` (`Rules+0xBB8`), both
of which the `RulesClass` owner VERA already has.

### Nullable semantics

`Nullable<T>::Read` (`Utilities/TemplateDef.h:1833-1849`) resets on
`<default>`; a blank value never reaches the reader (`ReadString` answers 0)
and therefore keeps whatever the caller had. `Nullable<AnimTypeClass*>` also
accepts the type-pointer sentinel `none`/`<none>`, which VERA's
`read_phobos_nullable_with` folds into the same reset.

## Descent rate

`Ext/Bullet/Hooks.cpp:128-137` (the `BulletClass::AI` tail):

```cpp
if (pThis->HasParachute)
{
    int fallRate = pBulletExt->ParabombFallRate - pBulletTypeExt->Parachuted_FallRate;
    const int maxFallRate = pBulletTypeExt->Parachuted_MaxFallRate
        .Get(RulesClass::Instance->ParachuteMaxFallRate);
    if (fallRate < maxFallRate)
        fallRate = maxFallRate;
    pBulletExt->ParabombFallRate = fallRate;
    pThis->FallRate = fallRate;
}
```

So `Parachuted.FallRate=` is the per-frame **acceleration** (a positive step
subtracted from a rate that is negative while descending) and
`Parachuted.MaxFallRate=` / `[General] ParachuteMaxFallRate` the **terminal**
rate — a signed lower bound, i.e. the fastest the canopy may fall.
`ParabombFallRate` starts at `0` (`BulletExt::ParabombFallRate`,
`Ext/Bullet/Body.h:61`), so the first frame computes `0 - step` and the cap
holds the rate at the terminal value whenever the step is positive.

VERA owns this formula as
`sim::movement::parachute_descent::parachuted_bullet_rate`
(`src/sim/movement/parachute_descent.rs`), the same step/cap shape
`ObjectFallState::advance_rate` uses for a parachuting **object** — only the
owners of `step` and `cap` differ. Tests:
`src/sim/movement/parachute_descent.rs`'s `parabomb_rate_tests`.

## Behaviour (partly not yet consumed)

Three separate native arms make a parabomb:

1. **`Unlimbo` becomes `SpawnParachuted`** (`Ext/Bullet/Hooks.cpp:573-583`,
   hooking `0x468684`): a `Parachuted=yes` bullet is born under its canopy
   rather than flying.
2. **The canopy anim** (`:585-608`, hooking `0x5F5A62` inside
   `ObjectClass::SpawnParachuted`): `BombParachute.Get([General] BombParachute)`
   constructs an `AnimClass` at the spawn coordinate, owned by the firing house
   and remapped to its colour scheme, stored as the bullet's `Parachute`.
3. **The vanilla motion is skipped** (`:610-618`, hooking `0x467AB2`): when
   `HasParachute`, the `0x467AB2..0x467B1A` ordinary gravity/reflection commit
   is bypassed outright and the descent is driven purely by `FallRate`, which
   is exactly the block VERA's projectile AI calls its "ordinary scratch
   gravity/reflection commit".

## Residual

**The keys and the rate model are in place; the bullet is not yet parachuted.**
Two owners are missing and one hook is not rewired:

- **Per-bullet state.** `BulletExt::ParabombFallRate` (accumulated rate) and
  `ObjectClass::Parachute` (the canopy `AnimId`) have no slot on VERA's
  `Projectile`. Adding them means a `#[serde(default)]` field on
  `Projectile`, which is safe for savegames but also touches the single
  struct-literal constructor (`ProjectileStore::construct`) and the
  `world_hash`/checksum projections.
- **A motion arm.** The parachuted descent replaces the whole trajectory
  dispatch (`ProjectileTrajectory::{Straight,Ballistic,Vertical}`), and must
  also skip the `collides_at(..., ProjectileCollisionPhase::Ordinary)` tail
  the same way `0x467AB2` skips `0x467B1A`. That match is exhaustive over the
  trajectory enum and feeds the shared collision ladder; inserting a fourth
  arm is a checksum-sensitive change to the projectile physics that is not
  safe to land without a compile-and-run cycle.
- **The canopy anim.** A bullet has no attached-anim owner in VERA (only
  `awaited_anim`, the `NUKE` impact wait). The `BombParachute=` anim would
  need one, plus the per-frame repositioning `ObjectClass::Update` does to
  keep the canopy over its object.

Until those land, `Parachuted=yes` bullets keep flying their ordinary
trajectory and `BombParachute=` is inert. The rate formula is ready for the
day they do.

## Tests

- `src/rules/phobos_parabomb_tests.rs` — the four keys, the
  `[General] ParachuteMaxFallRate` fallback, and the `<default>` / `none`
  reset tokens.
- `src/sim/movement/parachute_descent.rs` `parabomb_rate_tests` — the
  step/cap descent, including the saturating and overflow shapes.
