# Feedback weapon (`FeedbackWeapon`)

Phobos lets a weapon also detonate a second weapon on its own firer, once per
shot, right after `FireAt` has built the bullet and its muzzle animation.

## Keys

| Key | Owner | Type | Default |
|---|---|---|---|
| `FeedbackWeapon=` | WeaponType | `Valueable<WeaponTypeClass*>` | empty — no second detonation |
| `FireInTransport=` | WeaponType | `Valueable<bool>` | `yes` (stock key, already read by VERA) |

Reads: `WeaponTypeExt::LoadFromINIFile` (`Ext/WeaponType/Body.cpp:124`, field
`Ext/WeaponType/Body.h:56`, constructed null `:151`). The `<true>` read is
`WeaponTypeClass::FindOrAllocate`, so the named weapon joins the rules family
even when no stock type references it; VERA's ordered weapon pass follows the
reference for that reason.

## Native body

`TechnoClass_FireAt_FeedbackWeapon` (`0x6FF43F`,
`Ext/Techno/Hooks.Firing.cpp:802-812`):

```cpp
if (pThis->InOpenToppedTransport && !pWeaponFeedback->FireInTransport)
    return 0;
WeaponTypeExt::DetonateAt(pWeaponFeedback, pThis, pThis);
```

`DetonateAt(weapon, AbstractClass* target, TechnoClass* owner)`
(`Ext/WeaponType/Body.cpp:363-366` → `:374-377`) builds a bullet at
`pThis->GetCoords()` whose owner *and* target are the firer, detonates it at
once and passes no firing house. The hook sits at the end of `FireAt`'s muzzle
block (`0x006FF394..0x006FF43F`), before Sonic6FF470 and the laser ladder.

## VERA

The key is retained in `src/rules/phobos_weapon_extras.rs`
(`WeaponType::phobos_feedback_weapon`) and its reference is allocated by the
ordered rules processor.

## Residuals

- The sim consumer is not wired: VERA's synthetic projectile detonation for
  this self target produces no receivers, so a `FeedbackWeapon=` shot has no
  second detonation. Trigger: any weapon with the key. Frequency: mod-authored
  weapons. Risk: the feedback damage is missing. Next owner: a direct
  self-detonation commit in `world_fire`, or a represented temporary bullet.
- `FireInTransport` is the stock key; its open-topped gate belongs to the same
  unwired consumer.
- Retail rules author no `FeedbackWeapon=`.
