# Land sequences in water (`OnlyUseLandSequences`)

An infantry type that plays its land animation sequences even while it is in
water.

## Keys

| Key | Type | Default |
|---|---|---|
| `OnlyUseLandSequences=` (art section) | `Nullable<bool>` | unset |
| `[General] OnlyUseLandSequences=` | `Valueable<bool>` | `no` |

Read: `InfantryTypeExt::LoadFromINI` (`Ext/InfantryType/Body.cpp:32`) from the
**art** INI (`exArtINI, pArtSection`), declaration `Ext/InfantryType/Body.h:19`,
constructor `Body.h:44`; the global at `Ext/Rules/Body.cpp:622`, declaration
`Ext/Rules/Body.h:520`, constructor `Body.h:1017`. VERA reads both in
`src/rules/phobos_land_sequences.rs`, owner
`ObjectType::phobos_land_sequences` plus
`RuleSet::phobos_globals.only_use_land_sequences`. Tests:
`src/rules/phobos_land_sequences_tests.rs`.

The type key is an **art**-section key, the same split VERA's art projection
already models for the other art-only keys.

## Behaviour (not yet consumed)

`InfantryClass_DoAction_Water` (`0x51D7E0`,
`Ext/Techno/Hooks.Misc.cpp:883-901`):

```cpp
if (pTypeExt->OnlyUseLandSequences.Get(RulesExt::Global()->OnlyUseLandSequences))
    return SkipWaterSequences;      // 0x51D842
if (sequence == Sequence::Walk || sequence == Sequence::Crawl)
    return UseSwim;                 // 0x51D83D
else if ((sequence == SecondaryFire || sequence == SecondaryProne)
         && pTypeExt->SecondaryFireSequenceLandOnly.Get(Global))
    return UseWetAttack;            // 0x51D82F
return Continue;                    // 0x51D7EC
```

So `OnlyUseLandSequences` short-circuits the whole water-sequence selection:
`Walk`/`Crawl` never become `Swim`, and the wet-attack arm never runs. The
`SecondaryFireSequenceLandOnly` branch is a separate key and is not owned
here.

## Residual

**The keys are read; the water-sequence selector is not wired.** VERA's
`rules::animation_sequence` already carries `Swim` / `WetAttack` / `WetIdle`
as distinct sequence slots, but the owner that chooses between the land and
water family for a walking/crawling infantry in water does not yet consult
this key. Wiring it means passing
`object.phobos_land_sequences.only_land(rules.phobos_globals.only_use_land_sequences)`
into that selector's early-out.

Until then `OnlyUseLandSequences=` is inert and infantry always take their
water sequences in water.

## Tests

- `src/rules/phobos_land_sequences_tests.rs` — the type's value outranks the
  global, an unset key defers to the global (default `no`), and the
  `<default>` / `none` reset tokens.
- `src/rules/phobos_globals_tests.rs` — the `[General]` key reads alongside
  the other `HoverDrownable`-block globals.
