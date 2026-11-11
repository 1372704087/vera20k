# Break mind control on impact (`RemoveMindControl`)

A `WarheadType` key that frees every mind-controlled target it reaches before
any other per-target effect, so a later `ChangeOwner=` or `Convert*` sees the
unit's real House.

## Keys

| Key | Type | Default |
|---|---|---|
| `RemoveMindControl=` | `Valueable<bool>` | `no` |
| `RemoveMindControl.Silent=` | `Nullable<bool>` | unset |

Read: `WarheadTypeExt::LoadFromINI` (`Ext/WarheadType/Body.cpp:178-179`),
declarations `Ext/WarheadType/Body.h:49-50`, constructor `Body.h:328-329`.
VERA reads them in `src/rules/phobos_warhead_extras.rs`, owner
`WarheadType::phobos_extras`.

`RemoveMindControl.Silent=` falls back to `[AudioVisual]
RemoveMindControl.Silent` (`Ext/Rules/Body.cpp:597`, constructor
`Ext/Rules/Body.h:998`, default `no`), held on
`RuleSet::phobos_globals.remove_mind_control_silent`.

## Behaviour

`WarheadTypeExt::DetonateOnOneUnit` (`Ext/WarheadType/Detonate.cpp:197-231`)
runs the arms in a fixed order and says why:

```
// Put these at first since they can change the target's house
if (this->RemoveMindControl) pHouse = this->ApplyRemoveMindControl(...);
if (this->ChangeOwner)      this->ApplyOwnerChange(pHouse, pTarget);
```

`ApplyRemoveMindControl` (`:477-486`) tests `pTarget->MindControlledBy`. With a
controller it calls `CaptureManagerExt::FreeUnit(manager, target, silent)`
(`Ext/CaptureManager/Body.cpp:113-152`) and returns `pTarget->Owner` — the
House `FreeUnit` just restored. Without one it returns the incoming House
untouched, so the arm is inert on an uncontrolled target.

`FreeUnit` uninitialises the ring animation, plays `MindClearedSound` (the
type's, else `[AudioVisual] MindClearedSound`) **unless silent**, restores the
original owner or `HouseClass::FindNeutral()` for a defeated one, runs
`DecideUnitFate` and clears `MindControlledBy`.

VERA's owner is `sim::combat::crit`'s per-target loop: the arm sits before
`apply_change_owner`, and a free replaces the loop's `house` with the target's
restored owner so every later arm inherits it. The sound is the only thing
`silent` changes, expressed as `Simulation::free_unit_with_sound`; the
pre-existing [`Simulation::free_unit`] keeps playing it.

## Residuals

- **`SetOwningHouse(pOriginOwner, !silent)`.** Native passes `!silent` as the
  "announce" flag, which also suppresses the House-level owner-change
  announcement. VERA's `change_owner_with_rules` has no announce parameter, so
  a silent free still announces through whatever that path announces. The
  audible difference is the `MindClearedSound`, which is suppressed.
- **`BeControlledThreatFrame = 0`.** `FreeUnit` clears the victim's threat
  bookkeeping (`TechnoExt::Fetch(pTarget)->BeControlledThreatFrame`). VERA has
  no such field on a freed unit; the threat scan recomputes from live state, so
  nothing reads a stale frame.
- **`pTarget->MindControlledBy` vs the capture node.** VERA reads
  `mind_control.controller()`, which is the same link. A target whose node has
  already been dropped still answers `None`, matching native.

## Tests

- `src/rules/phobos_warhead_extras_tests.rs` — the parse group.
- `src/rules/phobos_globals_tests.rs` —
  `the_mind_control_silent_and_merge_building_damage_globals`.
