# Projectile `AU`

`[Projectile] AU=` lets a weapon whose projectile carries the flag attack a
techno that is underground. It is a plain `Valueable<bool>` read by
`Ext/BulletType/Body.cpp:83` and defaults to `no`.

```ini
[PROJECTILE]
AU=yes
```

## Native evidence

| Address | Site | Behavior |
| --- | --- | --- |
| `0x772AB3` | `WeaponTypeClass_AllowedThreats_AU` (`Ext/Techno/Hooks.cpp:1537-1545`) | When the weapon's projectile has `AU`, ORs `0x20000` into the allowed-threat flags. |
| `0x6F8DF0` | `TechnoClass_SelectAutoTarget_Start_AU` (`:1553-1558`) | Keeps that flag in the auto-target scan context. |
| `0x6F8FA8`, `0x6F8F??` | `SelectAutoTarget_SetCanTargetWhatAmI_AU` (`:1560-1570`) | With the flag, adds Infantry/Unit/Aircraft to `CanTargetWhatAmI`. |
| `0x70023B` | `TechnoClass_MouseOverObject_AttackUnderGround` (`:1497-1510`) | The manual fire gate accepts the object when it is surfaced or the selected weapon's projectile has `AU`. |

The auto-target side reads the object's `GetHeight` (vt+0x1C8,
`0x005F5F40`): a candidate below -20 leptons is refused at
`0x006F7E11..0x006F7E1E` unless `AU` admits underground targets.

## Where VERA20K owns it

- `rules::projectile_type::ProjectileType::au` reads the key.
- `sim::combat::greatest_threat::evaluate_candidate` resolves the selected
  weapon's projectile (`ctx.rules.projectile`) and skips the `< -20` height
  rejection when `au` is set (`sim/combat/greatest_threat.rs:2410`).
- The flag is `#[cfg(feature = "phobos")]`, so a build without the feature
  keeps the vanilla height gate.

## Residuals

- The manual mouse-over fire gate (`0x70023B`) has no separate VERA owner: the
  command pipeline does not run that cursor admission, so only the auto-target
  scan consumes `AU` today. Trigger: a player ordering fire at an underground
  techno. Frequency: none in retail (no Tunnel locomotor). Risk: manual order
  rejection still uses the vanilla path.
- Retail/visual parity is untested here (no retail `ini/` in this checkout).
