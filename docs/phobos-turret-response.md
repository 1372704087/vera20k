# Turret response

`[VehicleType] TurretResponse=` decides whether a Unit's idle turret returns
toward its hull when it has no target. `Nullable<bool>` read by
`Ext/UnitType/Body.cpp:83` (decl `Ext/UnitType/Body.h:68`); unset falls back to
`UnitExt::CannotMove`.

```ini
[TANK]
TurretResponse=yes   ; keep the idle return even with Speed=0
TurretResponse=no    ; never return the turret while idle
```

## Native evidence

`Ext/Unit/Hooks.DisallowMoving.cpp:69-75` hooks `UnitClass::Facing_Update`'s
idle arm at `0x00736B60`:

```cpp
return (pTypeExt->TurretResponse.isset()
            ? !pTypeExt->TurretResponse.Get()
            : UnitExt::CannotMove(pThis))
    ? 0x736AFB : 0;
```

`0x736AFB` skips the idle return, so `no` and an unset key on a stationary unit
both hold the last aim. `UnitExt::CannotMove` (`Ext/Unit/Body.cpp:193-220`) is:

- a sourced locomotor (`LocomotorSource`) answers false;
- `Speed == 0` answers true;
- an unset `MovementRestrictedTo` answers false;
- a Tunnel cell answers false;
- on a bridge, a Water or Beach cell reads as Road;
- otherwise the required land must equal the cell's.

## Where VERA20K owns it

- `rules::object_type::ObjectType::phobos_turret_response` (`TurretResponse`,
  Vehicle types only).
- `sim::movement::turret::cannot_move`: the `UnitExt::CannotMove` port, reading
  the type's `speed`, `movement_restricted_to`, the unit's cell land type
  (`CellRef::land_type`) and `GameEntity::on_bridge`.
- `sim::movement::turret::facing_update`'s idle arm: `skip_idle_return` is
  `!TurretResponse` when set, else `cannot_move`, and suppresses
  `turret_destination` exactly where `0x736B60` jumps out.

## Residuals

- Native's `LocomotorSource` early exit has no VERA state. Nothing in VERA
  assigns a sourced locomotor, so the branch can only answer false.
- `CannotMove`'s other consumers (`Mission::Move`/`Hunt`/`AreaGuard`,
  `ApproachTarget`, `GetFireError`, auto-target and retaliation hooks) are
  separate chains and stay stock.
