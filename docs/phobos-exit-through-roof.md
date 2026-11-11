# Roof production exit (`ExitThroughRoof`)

A produced unit whose factory draws it through the roof animation instead of
the gate.

## Keys

| Key | Type | Default |
|---|---|---|
| `ExitThroughRoof=` | `Nullable<bool>` | unset |

Read: `TechnoTypeExt::LoadFromINI` (`Ext/TechnoType/Body.cpp:1200`),
declaration `Ext/TechnoType/Body.h:437`, constructor `Body.h:859`. VERA reads
it in `src/rules/phobos_exit_through_roof.rs`, owner
`ObjectType::phobos_exit_through_roof`. Tests:
`src/rules/phobos_exit_through_roof_tests.rs`.

## Behaviour (not yet consumed)

Two consumers answer the same key with **different** fallbacks
(`Ext/Building/Hooks.cpp`):

```cpp
// :1053 — "is this a roof-exit techno?"
static bool IsRoofExitTechno(TechnoTypeClass* pType)
{
    return TechnoTypeExt::Fetch(pType)->ExitThroughRoof
        .Get(pType->JumpJet || pType->BalloonHover);
}

// :1068 — BuildingClass_DrawDoorAnim (0x43D0CF)
return ExitThroughRoof.Get(pType->JumpJet) ? UseRoof : UseGate;
```

So the predicate defaults to `JumpJet || BalloonHover` while the door-anim
**draw** defaults to `JumpJet` alone, and an explicit `yes`/`no` on the type
outranks both. `IsRoofExitBuildingUnit` (`:1058`) then applies it to the
building's `GetNthLink` unit. A third hook, `BuildingClass_DrawUnderDoorAnim`
(`:1071`, `0x43D363`), marks the roof or skips the under-door anim on the
same answer.

VERA owns both answers as
`PhobosExitThroughRoof::exits_through_roof(jump_jet, balloon_hover)` and
`::draws_through_roof(jump_jet)`.

## Residual

**The key is read; the factory door-anim draw is not wired.** VERA has no
`BuildingClass::DrawDoorAnim` equivalent — the produced-unit exit is a
presentation step of the factory's production animation, and the owner that
would choose roof versus gate does not exist yet. Wiring it means finding the
factory's door-anim selection and passing it
`draws_through_roof(object.jump_jet)`, plus the
`IsRoofExitBuildingUnit` predicate on the linked unit.

Until then `ExitThroughRoof=` is inert.

## Tests

- `src/rules/phobos_exit_through_roof_tests.rs` — the two fallbacks are not
  the same predicate, the type's value outranks both, and the `<default>` /
  `none` reset tokens.
