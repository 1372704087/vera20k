# Crate goodie reroll chance

`CrateGoodie.RerollChance=` turns a crate unit candidate into a probabilistic
one: a type with `CrateGoodie=yes` only passes the Unit crate pick while a
random draw stays below the configured chance.

```ini
[SOMEVEHICLE]                       ; UnitType
CrateGoodie=yes
CrateGoodie.RerollChance=0.0        ; Valueable<double>, default 0.0
```

Reader: `Ext/UnitType/Body.cpp:76` (field `Ext/UnitType/Body.h:61`, constructor
`:123`). Phobos parses doubles through `Parser<double>`, which stores
`static_cast<float>(value)` widened back to double
(`Utilities/Parser.h:261-283`), so VERA keeps the value as `f64`.

## Native body

`CellClass_GoodieCheck_CrateGoodie` (`0x4821BD`,
`Misc/Hooks.Crates.cpp:80-92`): when the drawn type's `CrateGoodie=` is set and
its reroll chance is above zero, the boolean becomes
`chance < Scenario.Random.RandomDouble()`; a false result fails the type and
the selection loop draws another candidate. The draw precedes the base-unit
gates, so a rejected base unit still spends it.

## VERA

`ObjectType::phobos_crate_goodie_reroll_chance` carries the key (read for
vehicle types, the UnitType family). In
`sim/crates/effects.rs::choose_unit_crate_type` the pick loop applies the same
draw after the type's `CrateGoodie=` and before the base-unit gates, using
`SimRng::random_double`. The draw-free no-candidate guard keeps its previous
behaviour.

## Residuals

- The native retry loop never terminates when every candidate rerolls; VERA's
  guard returns `None` without a draw, the same rules-data-only divergence it
  already records for an empty eligible pool. Trigger: authored data where
  every goodie can fail the roll. Frequency: invalid mod data. Risk: a custom
  crate hangs native but selects nothing here.
- Retail rules author no `CrateGoodie.RerollChance`; the default 0 disables
  the draw and keeps the stock pick.
