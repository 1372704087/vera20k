# Translucency.Cloaked

An animation drawn over a cloaked Techno normally keeps its `Translucency=`
level. Phobos can give it a different level while its owner is cloaking or
cloaked.

```ini
[SOMEANIM]                         ; AnimationType / art section
Translucency=50
Translucency.Cloaked=25            ; integer, unset = keep Translucency
```

Reader: `Ext/AnimType/Body.cpp:114` (an `Animatable<TranslucencyLevel>` read
with `requireParsedFallback`, so an absent or unparsed key leaves it unset;
plain value at `Ext/AnimType/Body.h:69`).

## Native body

- `AnimClass_DrawIt_Translucency` (hook `0x423183`,
  `Ext/Anim/Hooks.cpp:573-615`) replaces DrawIt's translucency block. For an
  anim attached to a `TechnoClass` whose `CloakState` is `Cloaking` or
  `Cloaked`, `Translucency.Cloaked` supplies the level; otherwise the plain
  `Translucency` value is used. A `None` level falls back to the stock
  instance-ramp ladder.
- The level maps to the native blitter flags: 25 → 2, 50 → 4, 75 → 6, any
  other integer → 0 (`TranslucencyLevel::operator=`,
  `Utilities/Constructs.h:481-497`).

## VERA

`AnimTypeRuntimeConfig::phobos_translucency_cloaked` reads the key.
`Simulation::anim_owner_is_cloaked` reports the Cloaking/Cloaked owner, and
`AnimTranslucencyInput` carries it plus the alternate level;
`anim_translucency_selection` substitutes the cloaked level for
`explicit_translucency` only while that owner flag is set, leaving the stock
ladder for a zero/absent level. The draw callers pass the owner flag from the
live sim; building slot anims (never cloaked) pass false.

## Tests

- `src/sim/anim_class.rs` —
  `translucency_cloaked_overrides_on_a_cloaked_owner_only`: a 50% anim with
  `Translucency.Cloaked=25` draws 50% uncloaked, 25% while Cloaking and
  Cloaked, and 50% again after uncloaking; an anim without the key keeps 50%
  under the same cloak states.

The cloaked-level substitution was mutated red and restored.

## Residuals

- Phobos makes both `Translucency` and `Translucency.Cloaked` keyframeable
  (`Animatable`); VERA reads and applies only the plain integer values. The
  keyframe system (`Translucency.Keyframe*`, interpolation and the 0/25/50/75
  clamp) remains unported for both keys.
- The `.Cloaked` parser accepts the shared Phobos integer grammar
  (`read_phobos_nullable_int`); a non-numeric token leaves it unset.
