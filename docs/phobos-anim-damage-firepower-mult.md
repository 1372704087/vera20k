# Anim Damage.ApplyFirepowerMult

An animation's loop damage can inherit the firepower multiplier of the
Techno that spawned it, so a veteran or crate-boosted unit's scorch or
explosion hits as hard as its weapon would.

```ini
[PULSE]                        ; Animation ART section
Damage.DealtByInvoker=yes      ; must be set for the fold to run
Damage.ApplyFirepowerMult=yes  ; Nullable<bool>, unset -> the global

[CombatDamage]
AnimDamage.ApplyFirepowerMult=no  ; Valueable<bool>, default no
```

Readers: the type key at `Ext/AnimType/Body.cpp:98`; the global at
`Ext/Rules/Body.cpp:571` (constructor `Ext/Rules/Body.h:983` stores false).

## Native body

`AnimClass_AI_Damage` (`0x42453E`, `Ext/Anim/Hooks.cpp:41-140`):

- Inside the `Damage.DealtByInvoker` block, when the anim has an invoker,
  the first damage round (`Animation.Value == max(Damage.Delay - 1, 1)`)
  caches `pExt->FirepowerMult = TechnoExt::GetCurrentFirepowerMultiplier(pInvoker)`.
- Every applied damage is then `(int)(appliedDamage * pExt->FirepowerMult)`,
  before the `Weapon=`/warhead dispatch. `FirepowerMult` is constructed 1.0
  (`Ext/Anim/Body.h:62`), so visits before the first damage round and a
  no-invoker anim are unscaled.
- Without `Damage.DealtByInvoker` the whole block, including the fold, does
  not run.

`GetCurrentFirepowerMultiplier` composes the object's crate firepower, the
house difficulty/country bias, the `FIREPOWER` veteran ability and an
occupiable building's `OccupyDamageMultiplier=`; VERA's owner is
`combat::crit::current_firepower_multiplier`.

## VERA

`CombatDamageRules::anim_damage_apply_firepower_mult` and
`AnimTypeRuntimeConfig::damage_apply_firepower_mult` read the key pair.
`AnimObject::phobos_firepower_mult` (serialized, hashed when not 1.0) caches
the multiplier on the first damage round; the anim damage owner folds each
application (including the `Weapon=` detonation damage) with
`attach_effect::scale_i32`, the shared model of the native cast.

## Tests

`src/sim/anim_damage_tests.rs::anim_damage_applies_the_invokers_firepower_multiplier`
drives a real anim against a target with an invoker carrying a 2x crate
multiplier: the keyed run's damage equals `scale_i32(base, multiplier)` while
the unset key and the no-`DealtByInvoker` runs stay at the base damage, and
the `[CombatDamage]` global supplies an unset type key. The fold was mutated
red and restored.

## Residuals

- The cache is refreshed only when the anim's `Animation.Value` equals
  `max(Damage.Delay-1, 1)`; if that frame is skipped (a delay larger than the
  animation length), the multiplier stays 1.0, as natively.
- `GetCurrentFirepowerMultiplier`'s occupiable-building and bunker terms are
  owned by `current_firepower_multiplier`; a bunker term it does not fold is
  that function's recorded residual, not this key's.
