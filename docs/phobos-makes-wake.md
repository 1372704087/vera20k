# MakesWake

A moving hull on water leaves the `[General] Wake=` animation. Phobos makes
that wake switchable per locomotor and per type.

```ini
[AudioVisual]
WalkLocomotorMakesWake=no    ; Valueable<bool>, default no
DriveLocomotorMakesWake=yes  ; default yes
HoverLocomotionClassMakesWake=yes ; default yes
ShipLocomotionClassMakesWake=yes  ; default yes

[BOAT]                       ; TechnoType section
MakesWake=no                 ; Nullable<bool>, unset -> the locomotor global
```

Readers: the four globals at `Ext/Rules/Body.cpp:543-546` (constructors
`Ext/Rules/Body.h:960-963`); the type key at `Ext/TechnoType/Body.cpp:1070`
(`Nullable<bool>`, `Body.h:301,733`).

## Native body

`TechnoExt::TypeExtData->MakesWake.Get(<locomotor>MakesWake)` gates each
locomotor's movement-wake producer:

- `0x4B079D` Drive (`Ext/Techno/Hooks.Misc.cpp:357-364`) — `NoWake` at
  `0x4B0828`;
- `0x514A32` Hover (`:366-374`) — `NoWake` at `0x514AC8`;
- `0x69FE4A` Ship (`:376-384`) — `NoWake` at `0x69FEF0`;
- `0x75AC93` Walk (`:425-441`), which otherwise requires the walker to be
  really moving, not on a bridge, on a Water cell, and on a 10-frame cadence.

## VERA

`PhobosGlobals` reads the four globals; `ObjectType::phobos_makes_wake` reads
the type key. `Simulation::spawn_wakes_for_frame` resolves the effective value
for each movant (type key, else the global for its active locomotor kind) and
`wake_anchor_for` returns no anchor when it is false. The batched wake pass
still spawns the `[General] Wake=` animation through the shared constructor.

## Tests

- `src/sim/world/world_tests.rs` — `mikes_wake_gates_each_locomotor_wake`:
  water-going Drive and Ship hulls wake by default, `MakesWake=no` and a
  global `no` both skip it, and an authored `yes` re-enables a
  global-disabled Drive hull. The gate was mutated red (`if false &&`) and
  restored.

## Residuals

- VERA spawns no infantry (`Walk`) movement wakes at all, so
  `WalkLocomotorMakesWake=` is read but dormant. Phobos' own default (`no`)
  makes the Phobos build match; the stock walk wake is a pre-existing VERA
  residual.
- The `Wake_Grapple`/`Wake_Sinking` per-type overrides are separate keys with
  their own consumer sites; neither is part of this key.
