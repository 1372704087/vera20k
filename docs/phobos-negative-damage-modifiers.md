# Negative-damage modifiers

Negative damage packets (heals) normally bypass the warhead falloff and
Verses kernel. `ApplyModifiersOnNegativeDamage=` makes them run it instead.

```ini
[General]
ApplyModifiersOnNegativeDamage=no      ; Valueable<bool>, default no

[HIT]                                  ; WarheadType
ApplyModifiersOnNegativeDamage=yes     ; Nullable<bool>, unset uses [General]
```

Readers: `Ext/Rules/Body.cpp:492` (constructor `Ext/Rules/Body.h:907`, false)
and `Ext/WarheadType/Body.cpp:185`.

## Native body

`GetTotalDamage` (`0x00489180`) returns a negative packet raw when the impact
is closer than 8 leptons and zero otherwise. Phobos hook
`0x4891AF` (`Ext/WarheadType/Hooks.cpp:334`) jumps to the modifier path
`0x4891C6` for a negative packet whose resolved key is true, and hook
`0x48922D` then skips the signed floor to 0 before the Verses multiply
(`:352-360`). The result is a scaled, possibly still negative, damage value;
the final upper cap is unchanged.

## VERA

`PhobosGlobals::apply_modifiers_on_negative_damage` and
`WarheadType::phobos_apply_modifiers_on_negative_damage` resolve the key
(`WarheadType::apply_modifiers_on_negative_damage`). The shared kernel
`sim/combat/damage/kernel.rs::apply_warhead_damage` takes the resolved flag:
it skips the distance<8 early-out and the zero floor only for an armed
negative packet. The production receiver resolves it in
`receiver_call::resolve_receive_damage_with_input`, and the terrain,
attach-effect and crit callers do the same.

## Tests

- `src/sim/combat/damage/kernel_tests.rs` — stock near/far rows and the armed
  Verses-scaled negative result.
- `src/sim/combat/ares_warhead_combat_tests.rs` — a keyed negative packet
  heals through the production receiver, including the `[General]` fallback.
- `src/rules/phobos_globals_tests.rs` — the global's section/default and the
  warhead override.

## Residuals

- The UI damage estimate (`damage/estimate.rs`) early-outs negative packets
  before the kernel and keeps the raw value; the hook's native caller there is
  not modeled.
- The Ares shield negative branch negates its input before the kernel and
  passes `false`; a shield-absorbed heal keeps the stock result.
