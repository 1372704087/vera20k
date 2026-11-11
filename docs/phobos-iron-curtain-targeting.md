# Iron-curtained targeting

Phobos lets a weapon, or the rules, decide whether an iron-curtained target
may be shot, auto-acquired and retaliated against.

```ini
[WEAPON]                            ; WeaponType
CanTarget.IronCurtained=            ; Nullable<bool>, unset inherits the rule
AutoTarget.IronCurtained=           ; Nullable<bool>, unset inherits the rule

[CombatDamage]
CanTarget.IronCurtained=yes         ; Valueable<bool>, human fallback
CanTargetAI.IronCurtained=no        ; Valueable<bool>, AI fallback
AutoTarget.IronCurtained=yes        ; Valueable<bool>, human auto-target fallback
```

Readers: `Ext/WeaponType/Body.cpp:118-119`, declarations `Body.h:50-51`
(constructed unset); `Ext/Rules/Body.cpp:437-439`, defaults `Body.h:831-834`.
The defaults reproduce the stock split: a human may fire at the curtain, a
computer house may not.

## Firing

Hook `0x6FC339` (`Ext/Techno/Hooks.Firing.cpp:313-369`) runs at the head of
`TechnoClass::CanFire` and the installed jump `0x6FC22A -> 0x6FC24D` removes
the stock T16 AI-only check. For a techno target the shot is `CannotFire` when
the target is iron-curtained and the weapon's `CanTarget.IronCurtained`, else
the owner's `CanTarget.IronCurtained` (human) or `CanTargetAI.IronCurtained`
(AI), answers no.

VERA keeps this in `sim/combat/fire_error.rs::evaluate_fire_error`'s T16 slot:
the weapon is fetched there under the feature, so the stock build keeps its
instruction-shaped `!owner_is_human && iron_curtained` term while the Phobos
build replaces it with the override/global chain. `FireFacts` carries the two
globals through `fire_error_world`; `WeaponFacts` carries the two weapon
overrides.

## Retaliation and auto-target

Hook `0x708AD0` (`Hooks.Firing.cpp:1226-1264`) sits inside
`TechnoClass::ShouldRetaliate` before the Verses test. Only a human owner is
affected: a curtained source is refused (`0x708B17`) when the selected
weapon's `AutoTarget.IronCurtained`, else the global, says no. VERA evaluates
it at the same point in `combat_targeting::should_retaliate`.

Hook `0x6F7CE2` (`Hooks.Targeting.cpp:57-76`) sits in
`TechnoClass::CanAutoTargetObject` right after SelectWeapon. A human scanner
answers false for a curtained candidate when the weapon override, else the
global, says no; VERA performs it in `greatest_threat::evaluate_candidate`
before the GetFireError probe (G2b).

## Residuals

- The AI firing path is enforced by the CanFire gate itself, so an AI victim
  with `CanTargetAI.IronCurtained=no` also refuses retaliation before the
  human-only ShouldRetaliate gate; that matches native.
- `CanTarget.IronCurtained` governs explicit orders through the same CanFire
  gate; no separate order-time check exists natively.
- Retail rules author none of these keys; the regressions use synthetic rules
  and the defaults above.
- The C4 planter tests now expect Phobos `GetBurstDelay(0) = 0` (the earlier
  `Burst.Delays` chain), so their stock mid-burst draw expectation is
  feature-aware; that is a test correction, not part of this mechanism.
