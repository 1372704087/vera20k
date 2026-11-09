# Warhead trigger conditions

Phobos adds per-warhead health, veterancy and invoker gates to the target list
of a detonation (`WarheadTypeExt::CanAffectTarget`,
`Ext/WarheadType/Body.cpp:37-57`).

```ini
[WH]                                  ; WarheadType
AffectsAbovePercent=0.0               ; double, default 0.0
AffectsBelowPercent=1.0               ; double, default 1.0
AffectsVeterancy=all                  ; AffectedVeterancy, default all
AffectsInvokerOnly=no                 ; boolean, default no
AffectsInvokerOnly.Reverse=no         ; boolean, default no
AffectsInvokerOnly.IgnoreInvokerState=; unset inherits [CombatDamage]
```

```ini
[CombatDamage]                        ; rulesmd.ini
AffectsInvokerOnly.IgnoreInvokerState=yes  ; boolean, default yes
```

## Behavior

The gates are read at two native sites: `DetonateOnOneUnit`'s target pass
(`Ext/WarheadType/Detonate.cpp:203`, the crit/laser/attach-effect list) and the
entry of `TechnoClass::ReceiveDamage` (`Ext/Techno/Hooks.ReceiveDamage.cpp:26-34`),
where a refused packet is zeroed. The receive site runs before the shield and
does not set `SkipLowDamageCheck`, so native's `0x7019D8` floor still turns
that zero packet into one point of damage. A target is skipped unless it
passes, in native order:

- **Health**: only checked when `AffectsAbovePercent > 0` or
  `AffectsBelowPercent < 1` (Phobos's derived `HealthCheck`); the target's
  current-health ratio must lie in `(above, below]`, with the zero-health,
  zero-strength object matching unconditionally.
- **Veterancy**: only checked when the mask is not `all` (derived
  `VeterancyCheck`); the target's rank bit must be in `AffectsVeterancy`.
- **Invoker**: with `AffectsInvokerOnly`, a present invoker (the techno that
  caused the damage) must be — or with `.Reverse`, must not be — the target.
  Without an invoker, `.IgnoreInvokerState` or the `[CombatDamage]` global
  (constructing `yes`) decides whether the target is affected.

Phobos reads the two percentages through its `Valueable<double>` reader, which
narrows the parsed value through f32 as Phobos's INI wrapper does.

## Where VERA20K owns it

`rules::warhead_type` reads the keys; `sim::combat` owns the three predicates
and `warhead_triggers_allow`, which `WarheadTypeExt::CanAffectTarget`'
`detonate_on_one_unit` consumer uses (`sim::combat::crit`) before the crit
pass. The same compiler flags are shared with the splits/shrapnel
weapon-targeting gates.

## Residuals

- `Shrapnel.ObeyWarheadTriggerConditions` still needs the bullet's direct
  target path (`BulletClass::Logics @ 0x4690D4`); the gates themselves are
  ready for it.
- The receive hook's `ApplyPerTargetEffectsOnDetonate` arm (which calls
  `DetonateOnOneUnit` from ReceiveDamage) is not modelled; only the trigger
  gate of that hook is.
- `CanAffectTarget`'s later `EffectsRequireVerses`, `AffectsAir`/`AffectsGround`
  gates are already modelled separately and keep their own owner.
