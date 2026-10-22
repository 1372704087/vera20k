# Superweapon techno gates

Phobos adds three availability keys to every `[SuperWeaponTypes]` entry. They decide whether a house
may have the superweapon at all — the sidebar grant, the charge, the command and the AI's own use —
not what its effect does once fired.

```ini
[SOMESW]                    ; SuperWeaponType
SW.TechLevel=0              ; integer
SW.AuxTechnos=              ; List of TechnoTypes
SW.NegTechnos=              ; List of TechnoTypes
```

- `SW.AuxTechnos` — the house must own at least one listed techno. An empty list adds no gate.
- `SW.NegTechnos` — the house must own none of the listed technos.
- `SW.TechLevel` — the house's TechLevel must reach this value. It is a plain minimum; `-1` is not
  special. Restrict a weapon to AI players with `SW.AllowPlayer`/`SW.AllowAI` instead.

"Owned" is native `HouseExt::CountOwnedPresentAndLimboed`: technos on the map plus those in limbo
(queued out of a factory, inside a transport, inside a garrisoned building). A techno being built
does not count until the house owns it.

## Native identity

`SWTypeExt::IsAvailable` (`src/Ext/SWType/SWHelpers.cpp`) is the whole chain, in this order:

```
TechLevel → Shots → AllowPlayer/AllowAI → RequiredHouses/ForbiddenHouses
→ AuxBuilding → Aux/NegBuildings → Aux/NegTechnos
```

Phobos installs it over Ares' `SuperWeaponTypeClass::IsAvailable` (`src/Misc/Hooks.Ares.cpp`), so
the Ares gates stay in place and the Phobos ones wrap them. The techno vectors are
`ValueableVector<TechnoTypeClass*>`: each token is resolved across every techno family
(`TechnoTypeClass::Find`), unknown names are skipped, a reached list replaces the retained one and an
absent key retains it. The listed building gates — retail `AuxBuilding=` and Ares
`SW.AuxBuildings`/`SW.NegBuildings` — stay with their existing owners.

## Where VERA20K owns it

- `rules/native_processing.rs` reads the three keys in the same ordered per-pass body as the Ares
  keys, so a later INI layer that names the superweapon again replaces its lists and keeps the level
  when the key is absent.
- `rules/ruleset.rs` projects one `PhobosSuperWeaponAvailability` per registered definition, hashed
  under `phobos-super-availability-v1` — the projection covers types whose effect body VERA20K does
  not implement yet.
- `sim/superweapon/mod.rs::super_weapon_available` is the single consumer. It applies the TechLevel
  test first and the techno lists last, around the stock (or Ares) gates, and both the grant pass and
  the AI share it.

A build without the capability reads none of the keys and keeps the stock `AuxBuilding` gate.
