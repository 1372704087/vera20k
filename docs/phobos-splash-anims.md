# AnimType splash animations

`SplashAnims=` replaces the stock `[CombatDamage] SplashList=` used when a
bouncing animation lands on water, and `SplashAnims.PickRandom=` makes the
landing draw its splash instead of taking a fixed entry.

```ini
[METSMALL]                      ; art.ini AnimType section
SplashAnims=CHILD,SPLASH1       ; NullableVector<AnimTypeClass*>
SplashAnims.PickRandom=no       ; Valueable<bool>, default no
```

Readers: `Ext/AnimType/Body.cpp:102-103` (fields
`Ext/AnimType/Body.h:57-58`, constructors empty/false `:109-110`). VERA reads
them into `AnimTypeRuntimeConfig::splash_anims` (`Option`, where `None` is an
absent or `<default>` read) and `splash_anims_pick_random`.

## Native body

`AnimClass_AI_HasExtras_Expired` (`0x423CC7`, `Ext/Anim/Hooks.cpp:222-242`)
replaces the anim's extras-expired block with `AnimExt::HandleDebrisImpact`
(`Ext/Anim/Body.cpp:245-307`). In its water arm (`LandType::Water`, no height
flag, not `ExplodeOnWater`) the splash list is
`SplashAnims.GetElements(RulesClass::SplashList)`: the type's list when the key
was read, the stock list otherwise. The chosen index is the last entry for a
meteor, the first otherwise, unless `SplashAnims.PickRandom` makes it
`Scenario.Random.RandomRanged(0, lastIndex)`. The draw happens before the Wake
and splash animations are created.

## VERA

`Simulation::anim_bounce_landing` (`src/sim/anim_class.rs`) resolves the list
at the landing: the type's names are filtered against `rules.anim_type_names`
(the native `AnimTypeClass::Find`), else the stock `SplashList=` entries are
used. With the key set, one `RandomRanged(0, last)` draw picks the entry; the
draw precedes the Wake spawn, matching the hook's order. An empty custom list
spawns no splash.

The shared `NullableVector<T*>` reader moved to
`IniSection::read_phobos_nullable_type_list` (`src/rules/ini_value.rs`) and the
shield type lists now use it.

## Tests

- `src/rules/art_data_tests.rs` — the list, `<default>`/absent, and the bool.
- `src/sim/anim_meteor_tests.rs` — a custom list replaces the stock water
  splash without a draw, and `PickRandom` consumes exactly one draw whose index
  selects the spawned entry.

## Residuals

- Only the water landing reads the keys; the Wake animation is unchanged and
  the VoxelAnim debris path keeps its own stock list.
- The custom list still applies when the key names no registered animation:
  the filtered list is empty and no splash spawns, as the native Find drop
  leaves an empty vector.
