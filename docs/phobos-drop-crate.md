# Drop a crate on death (`DropCrate`)

A `Nullable<Powerup>` that makes a dying object leave a crate of a fixed
`[Powerups]` kind on the cell it dies on.

## Keys

| Key | Type | Default |
|---|---|---|
| `DropCrate=` | `Nullable<Powerup>` | unset |

Read: `TechnoTypeExt::LoadFromINI` (`Ext/TechnoType/Body.cpp:1446`),
declaration `Ext/TechnoType/Body.h:431`, constructor `Body.h:851`. VERA reads it
in `src/rules/phobos_techno_extras.rs`, owner
`ObjectType::phobos_techno_extras.drop_crate`.

`detail::read<Powerup>` (`Utilities/TemplateDef.h:648-676`) is an exact,
case-insensitive scan of the fixed `[Powerups]` name table (`Powerups::Effects`),
so an unknown name is a parse failure and leaves the nullable unset. VERA
resolves the name against `crate::rules::powerups::POWERUP_NAMES`, the same
nineteen names in the same slot order, and stores the slot index.

## Behaviour (not yet consumed)

`TechnoExt::GetDropCrateIndex` (`Ext/Techno/Body.Internal.cpp:275-299`) answers
the type's slot when the instance has no per-instance override, else `-1`. Two
call sites, both after a techno's destruction is registered:

```cpp
// 0x702E4E, TechnoClass::RegisterDestruction
const int nSelectedPowerup = TechnoExt::GetDropCrateIndex(pVictim);
if (nSelectedPowerup >= 0)
    TechnoExt::TryToCreateCrate(pVictim->Location, static_cast<Powerup>(nSelectedPowerup));

// 0x70337D, HouseClass::RegisterDestruction (the Chronoshift teleport path)
```

`TryToCreateCrate` is `MapClass::Place_Crate @ 0x0056BEC0` with a fixed powerup
slot, which VERA already owns as `sim::crates::place_crate`
(`src/sim/crates.rs`): it snaps the cell, takes the first empty crate slot,
places the overlay, arms `CrateRegen`, and writes the powerup byte the pickup
reads as its stored selection.

## Residual

**The key is read; the destruction hook is not wired.** VERA has no
`TechnoClass::RegisterDestruction` equivalent — a techno's death is a
`KillRecord` plus the death-weapon/explosion arms, and the site that would
place the crate has no `OverlayTypeRegistry` in scope. Wiring it means finding
the single death transaction that owns both the victim's location and the
overlay registry, then calling `place_crate(..., Some(slot as u8), effects)`,
and modelling the per-instance `DropCrate`/`DropCrateType` override
(`TechnoExt::DropCrate`, `Body.Internal.cpp:285-295`) that outranks the type
key.

Until then `DropCrate=` is inert.

## Tests

- `src/rules/phobos_techno_extras_tests.rs` —
  `drop_crate_resolves_a_powerup_slot_and_rejects_unknown_names`.
