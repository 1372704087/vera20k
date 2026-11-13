# AreaFire.Target

An `AreaFire=yes` weapon normally fires at the cell under its firer. Phobos
lets the weapon aim elsewhere.

```ini
[NukeEruption]                  ; [WeaponTypes] section
AreaFire.Target=base            ; base | random | self (default base)
```

Reader: `Ext/WeaponType/Body.cpp:123` (`Valueable<AreaFireTarget>` declared
`Ext/WeaponType/Body.h:55`, constructed `Base` `:150`; enum
`Utilities/Enum.h:171-177`).

## Native body

`TechnoClass_FireAt_AreaFire` (`0x6FE19A`,
`Ext/Techno/Hooks.Firing.cpp:743-798`) replaces the stock target before the
shot:

- `random`: the range in cells is `GetRangeWithModifiers(weapon, firer)` / 256
  rounded down through `+ 0.99`; `AdjacentCellsInRange` enumerates the cells in
  `CellSpreadEnumerator` order. Each try spends one
  `RandomRanged(0, last)` Scenario draw, takes `(i + rand) % size`, adds that
  offset to the aimed cell and admits it when the cell exists, the weapon's
  `SkipWeaponPicking` derivation is true, or
  `AreCellAndObjectsEligible(cell, CanTarget, CanTargetHouses, owner, …)`
  passes. Running out of candidates answers `0x6FE4E7` — `xor eax,eax; ret 8`,
  no shot and no rearm.
- `self`: when the gate refuses the firer's own cell the shot does not fire;
  otherwise the firer object replaces the cell argument (`R->EAX(pThis)`) and
  the block jumps past its `SetTarget` (`0x6FE1D5`), so the shot aims at the
  firer with its own coordinates.
- `base`: the stock cell under the firer, after the same gate.

The gate's `SkipWeaponPicking` is derived at load: true exactly when no
`CanTarget`, `CanTargetHouses`, health-range, veterancy or AE list is authored
(`Ext/WeaponType/Body.cpp:210-223`).

## VERA

`PhobosWeaponExtras::area_fire_target` reads the key;
`combat::world_fire` resolves it at the stock area-fire cell block. `random`
reuses `cell_rect::cell_spread_offsets` (the `CellSpreadEnumerator` port) and
the Scenario stream; `self` swaps the snapshot to `TargetKind::Entity(firer)`
and the firer's own coordinates; `base` keeps the previous behaviour. The
candidate gate reuses `crit::cell_eligible` and the force-fire path's
techno-content/bridge inputs. A `random` shot with no admitted candidate
returns the no-shot path before any bullet work.

## Tests

- `src/rules/phobos_weapon_extras_tests.rs` — the three names, case folding
  and the unknown-name fallback.
- `src/sim/combat/combat_tests.rs` —
  `phobos_area_fire_target_self_and_random_replace_the_stock_cell`: a deployed
  Desolator's area-fire weapon keeps `Cell(10,10)` for `base`, targets the
  firer for `self`, and for `random` lands on
  `cell + offsets[RandomRanged(0, last)]` predicted from a Scenario-stream
  clone. Mutating the mode gate to `base` turns the self row red; restored.

## Residuals

- `CanTargetHouses=` and the AE required/disallowed lists do not exist in
  VERA's weapon-targeting data, so the `SkipWeaponPicking` derivation covers
  the three gate groups it models (`CanTarget`, health range, veterancy) and
  the candidate gate omits the house filter.
- `AdjacentCellsInRange`'s `CellSpreadEnumerator` port caps the spread at 256
  cells (the native `Max`).
