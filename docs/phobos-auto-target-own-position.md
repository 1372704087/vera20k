# Auto-firing at the object's own position (`AutoTargetOwnPosition`)

A `TechnoType` pair that makes an object skip target acquisition entirely and
aim at itself or at the cell it stands on.

## Keys

| Key | Type | Default |
|---|---|---|
| `AutoTargetOwnPosition=` (alias `AutoFire=`) | `Valueable<bool>` | `no` |
| `AutoTargetOwnPosition.Self=` (alias `AutoFire.TargetSelf=`) | `Valueable<bool>` | `no` |

Read: `TechnoTypeExt::LoadFromINI` (`Ext/TechnoType/Body.cpp:893-906`),
declarations `Ext/TechnoType/Body.h:164-165`, constructor `Body.h:561-562`.
VERA reads them in `src/rules/phobos_techno_extras.rs`, owner
`ObjectType::phobos_techno_extras`.

The deprecated spelling is read **first** at both sites, so a section carrying
both keeps the new name (`Body.cpp:899`, `:905`).

## Behaviour

`TechnoClass::TargetSomethingNearby @ 0x7098B9` (`Ext/Techno/Hooks.TargetEvaluation.cpp:8-23`):

```cpp
if (pExt->AutoTargetOwnPosition)
{
    if (pExt->AutoTargetOwnPosition_Self) pThis->SetTarget(pThis);
    else                                  pThis->SetTarget(pThis->GetCell());
    return 0x7099B8;
}
```

`0x7099B8` is the `Retaliate_And_Scan` tail that answers "is a target held", so
the hook replaces **both** later steps: the drop of a passively acquired target
on `ILLEGAL`/`CANT`/`RANGE`, and the `Greatest_Threat` pick with its
`DistributedFire` handoff and damage estimate.

VERA's owner is `sim::world::techno_ai::target_scan::retaliate_and_scan`, right
after the targeting timer is armed: `ScanHost::auto_target_own_position` answers
`None` when the type does not ask for it, otherwise `Some(self_target)`, and
`ScanHost::own_position_target` builds `TargetKind::Entity(id)` for `.Self=yes`
or `TargetKind::Cell(cell)` from `object_get_coords` otherwise.

## Residuals

- **`GetCell()` on a Building.** Native reduces the building's base cell;
  `own_position_target` reduces `ground_pose::object_get_coords`, the same
  getter `GetCoords` mirrors. For a multi-cell foundation both name the origin
  cell.
- **The timer still arms.** Native's replaced instructions start at `0x7098B9`,
  after `0x70982E..0x7098B6` stamped `+0x4FC` and the `RandomRanged(0, 2)`
  draw, so VERA keeps the timer write and the draw in the same place.

## Tests

- `src/rules/phobos_techno_extras_tests.rs` —
  `the_deprecated_autofire_spellings_still_work_and_lose_to_the_new_ones`.
