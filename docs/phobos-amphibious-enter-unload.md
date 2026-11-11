# Amphibious enter and unload

Two transport keys let a hull take or leave passengers on water. Both are
`Nullable<bool>` on the transport type and fall back to a `[General]` switch.

```ini
[General]
AmphibiousEnter=yes
AmphibiousUnload=yes

[LCRF]                  ; a Unit/Building transport type
AmphibiousEnter=no      ; this hull still refuses boarding over water
AmphibiousUnload=yes
```

## Keys

- `[TechnoType] AmphibiousEnter=` (`Ext/TechnoType/Body.cpp:1107`,
  `Body.h:342`) → `[General] AmphibiousEnter=`, default `no`
  (`Ext/Rules/Body.cpp:197`, `Body.h:654`).
- `[TechnoType] AmphibiousUnload=` (`:1108`, `Body.h:343`) →
  `[General] AmphibiousUnload=`, default `no`
  (`Ext/Rules/Body.cpp:198`, `Body.h:655`).

## Native evidence

- Enter: the `CanEnterNow` rewrite of `0x73758A` rejects a transport whose
  `GetCell()->LandType` is Water unless the effective key is set
  (`Ext/Techno/Hooks.Transport.cpp:231-253`); the DOCKING arm
  `UnitClass_ReceiveCommand` (`0x73796B`) does the same
  (`:613-628`).
- Unload, water branch: `CanUnloadNow` (rewrite of `0x4835D5`/`0x74004B`)
  replaces `LandType != Water` with the head passenger's own
  `GroundType[land].Cost[SpeedType] != 0` when the key is set
  (`:382-388`), consumed at `0x73D769` (`:660-669`).
- Unload, positions: hooks `0x73D7AB` (`:671-689`) and `0x73DAD8`
  (`:716-728`) replace the hard-coded `SpeedType::Wheel`/`MovementZone::Normal`
  in the nearby-cell and leave-position searches with the head passenger's own;
  `0x740C9C` (`:700-714`) does the same for the exit-octant speed check.
- Deploy/unload admission: `0x70107A` (`:646-657`) lets a transport unload on
  water when the key is set, and `0x7400B5` (`:632-644`) relaxes the
  low-cost land refusal for the same key. Water itself already passes both
  tests.

## Where VERA20K owns it

- `rules::phobos_globals::PhobosGlobals::amphibious_enter` / `_unload`.
- `rules::object_type::ObjectType::phobos_amphibious_enter` / `_unload`.
- `sim::radio::receive`: `transport_amphibious_enter`, `transport_on_water`;
  the DOCKING water branch and the CAN_LOAD boarding gate.
- `sim::transport_unload`: `transport_amphibious_unload`,
  `first_passenger_type`; the PICK_EXIT water branch, the land-search seed and
  the leave-position/exit-octant SpeedType and MovementZone.
- `sim::deploy::Simulation::can_deploy_slash_unload` and
  `sim::world::Simulation::unit_self_deploy_site_refused`.

## Residuals

- The Drive/Ship locomotor unload interruptions (`0x4B08EF`, `0x69FFB6`) and
  the empty nearby-cell check (`0x73D7B7`) are not key-gated; VERA keeps its
  existing process and `(0,0)` fallback.
- `0x7400B5`'s water arm already passes in VERA; only the keyed land arm is
  changed, so the observable result is the same.
