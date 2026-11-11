# Feedback weapon (`FeedbackWeapon`)

Phobos lets a weapon also detonate a second weapon on its own firer, once per
shot, right after `FireAt` has built the bullet and its muzzle animation.

## Keys

| Key | Owner | Type | Default |
|---|---|---|---|
| `FeedbackWeapon=` | WeaponType | `Valueable<WeaponTypeClass*>` | empty — no second detonation |
| `FireInTransport=` | WeaponType | `Valueable<bool>` | `yes` (stock key, already read by VERA) |

Reads: `WeaponTypeExt::LoadFromINIFile` (`Ext/WeaponType/Body.cpp:124`, field
`Ext/WeaponType/Body.h:56`, constructed null `:151`). VERA reads the key in
`src/rules/phobos_weapon_extras.rs` and exposes it as
`WeaponType::phobos_feedback_weapon`.

## Behaviour

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
once (`BulletExt::Detonate`, `Ext/Bullet/Body.cpp:482-495`) and passes no
firing house, so the bullet takes the owner's.

The hook address is the end of `FireAt`'s muzzle block
(`0x006FF394..0x006FF43F`) and precedes Sonic6FF470 and the laser/bolt ladder:
the shot's own bullet and muzzle anim already exist and are never replaced,
and the feedback damage lands before the later effects of the same shot.

VERA applies it in `src/sim/combat/world_fire.rs` at that exact point:
`phobos_feedback_weapon` builds the same `WeaponTypeExt::DetonateAt` bullet
through `Simulation::weapon_detonation_with_target` (reason
`ProjectileDetonationReason::FeedbackWeapon`) and commits it through the
receiver pipeline. The detonation runs synchronously inside the firing visit,
so a feedback warhead that kills the firer does so mid-`FireAt`, as native
does.

## Residuals

- `FireInTransport` is the stock weapon key; an open-topped passenger is
  detected through the same `passenger_role.in_open_transport()` predicate the
  firing gate already uses.
- Native re-reads `GetCurrentWeapon` nowhere here: `FeedbackWeapon` belongs to
  the weapon that was fired, which is the slot VERA resolves for the shot.

## Tests

- `src/rules/phobos_weapon_extras_tests.rs` — the key and its unset default.
- `src/sim/combat/phobos_visual_scatter_tests.rs` —
  `a_feedback_weapon_detonates_on_its_firer` (30 damage on a 100% warhead) and
  `a_feedback_weapon_keeps_the_real_shot` (still one projectile).
