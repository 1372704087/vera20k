# Gas.MaxDriftSpeed

A Gas particle's horizontal drift is clamped every Gas AI pass. Phobos makes
the bound per particle type instead of the stock ±2.

```ini
[GASCLOUD]                         ; ParticleType section
Gas.MaxDriftSpeed=2                ; integer, default 2
```

Reader: `Ext/ParticleType/Body.cpp:22`, `Valueable<int>` constructed 2 at
`Ext/ParticleType/Body.h:26,29`.

## Native body

- `ParticleClass_Gas_AI_DriftSpeed` (hook `0x62BE30`,
  `Ext/ParticleType/Hooks.cpp:4-30`) replaces the stock clamp block at
  `0x62BE30..0x62BE5F` (`jge/cmp/jle` over `[ebp+0xC0]`/`[ebp+0xC4]` with
  `ecx = -2`, `edx = 2`) with the same two comparisons bounded by the type's
  `Gas.MaxDriftSpeed`: Velocity.X and Velocity.Y clamp to `[-max, max]`, in
  X-then-Y order, each using `> max` before `< -max`.

## VERA

`ParticleType::phobos_gas_max_drift_speed` reads the key. `gas::tick_particle`
applies the clamp after its drift mutation on every even frame (the Gas AI
body), with the native if/else order and `wrapping_neg` for the lower bound;
a stock build keeps the ±2 clamp.

## Tests

- `src/sim/particles/gas_native_tests.rs` —
  `the_gas_drift_clamp_uses_the_particle_types_max_speed`: drift (7, -4)
  clamps to 2 with the default and to 5 with `Gas.MaxDriftSpeed=5`.

The keyed clamp was mutated red and restored.

## Residuals

- The clamp is applied on even frames with the rest of the VERA Gas AI body;
  the native hook sits at the same frame-gated `0x62BE30` block.
