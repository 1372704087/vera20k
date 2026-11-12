# Terrain passability

A terrain object (tree, rock, tiberium tree) normally blocks its cell.
Phobos lets an individual terrain type — or all ordinary terrain, or all
tiberium trees — be treated as passable.

```ini
[General]
Terrain.IsPassable=no      ; Valueable<bool>, default no (ordinary types)
Tibtree.IsPassable=no      ; Valueable<bool>, default no (SpawnsTiberium=yes types)

[TIBTRE01]                 ; [TerrainTypes] section
IsPassable=yes             ; Nullable<bool>, unset keeps the globals
```

Readers: `Ext/Rules/Body.cpp:479-480` (constructors `Ext/Rules/Body.h:894-895`,
both false) and `Ext/TerrainType/Body.h:33` (the per-type Nullable, read in
`Ext/TerrainType/Body.cpp`).

## Native body

`TerrainClass_SetOccupyBit_PassableTerrain` (`0x71C110`,
`Ext/TerrainType/Hooks.Passable.cpp:7-25`) returns before the occupation-bit
store when the resolved value is true, so the terrain contributes no cell
occupation. Companion arms treat the object as completely passable for
`CellClass::CheckPassability` (`0x483DDF`) and `UnitClass::CanEnterCell`
(`0x73FB71`), and show a move cursor instead of the attack cursor unless
force-firing (`0x7002E9`).

## VERA

`PhobosGlobals::terrain_is_passable` / `tibtree_is_passable` read the
`[General]` pair, and `TerrainObjectType::is_passable` holds the per-type
Nullable. The RuleSet constructor projects the resolved value
(`project_passability`, type key else the global selected by
`SpawnsTiberium=`). `TerrainObjectType::raw_occupation_bits` returns 0 for a
passable type, which is the single source for the live Terrain unlimbo mask,
the map-authored terrain-object occupation and the pathfinding block bits.

## Tests

- `src/sim/terrain_object.rs` — the type key, both globals, `SpawnsTiberium`
  selection, an explicit `no` beating the globals, the constructed state's
  zero bits and the projected cell no longer blocking.

## Residuals

- The `WhatAction` cursor arm (`0x7002E9`) is client presentation and is not
  ported; passable terrain keeps the stock attack cursor unless force-firing.
- The buildable-upon half of the same Phobos family (`CanBeBuiltOn`) is a
  separate chain.
