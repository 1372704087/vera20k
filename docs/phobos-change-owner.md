# ChangeOwner warhead keys

A warhead transfers the target to the firing House.

```ini
[Warhead]
ChangeOwner=no                     ; Valueable<bool>
ChangeOwner.SetAsMindControl=no    ; Valueable<bool>
ChangeOwner.MindControlAnim=       ; Nullable<AnimTypeClass*>
```

Reader `Ext/WarheadType/Body.cpp:431-433`, declarations
`Ext/WarheadType/Body.h:252-254`; consumer
`WarheadTypeExt::ExtData::ApplyOwnerChange`
(`Ext/WarheadType/Detonate.cpp:207-211,488-522`), reached from
`DetonateOnOneUnit` after `RemoveMindControl` and before `ApplyConvert`.

## Behavior

- `ChangeOwner`: `SetOwningHouse(firing House, true)` on every target the
  detonation visits.
- `ChangeOwner.SetAsMindControl=yes` makes it a mind-control capture: it is
  refused when the target's type is `ImmuneToPsionics=` or the target is
  already mind-controlled; an accepted capture also raises the native
  `MindControlledByAUnit` byte (no VERA consumer yet) and, when
  `ChangeOwner.MindControlAnim` resolves, rings the target:
  a unit at `MindControlRingOffset=`, a building at its `Height=` in levels,
  with `ZAdjust = -1024` and the target as the ring's owner-object. VERA
  reuses the capture manager's `CreateMindControlRingAnim` owner for this.
- A plain `ChangeOwner` ignores psionic immunity and the existing
  mind-control link.

## Where VERA20K owns it

`WarheadType::phobos_change_owner*` feed
`sim::combat::crit::apply_change_owner`; the ring is
`sim::capture_manager::attach_control_ring` and the house transfer is
`Simulation::change_owner_with_rules`.

RESIDUAL — `MindControlledByAUnit`: trigger = a `SetAsMindControl` capture;
impact = the native byte stays unmodeled (no VERA consumer reads it);
frequency = only Phobos `ChangeOwner` warheads; risk = none while no consumer
exists.
