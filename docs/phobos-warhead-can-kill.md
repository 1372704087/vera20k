# Warhead `CanKill`

`[WarheadType] CanKill=no` makes a lethal hit leave the victim at 1 HP instead
of killing it. A plain `Valueable<bool>` read by
`Ext/WarheadType/Body.cpp:369`, default **`yes`** (the arm is opt-in).

```ini
[WARHEAD]
CanKill=no
```

## Native evidence

`Ext/Techno/Hooks.ReceiveDamage.cpp:178-188` in the Phobos ReceiveDamage
wrapper, after the shield block and before the original body:

```
if ((!pWHExt->CanKill || pExt->AE.Unkillable)
    && pThis->Health > 0 && nDamageLeft != 0
    && pWHExt->CanTargetHouse(pSourceHouse, pThis)
    && MapClass::GetTotalDamage(nDamageLeft, args->WH,
           pThis->GetTechnoType()->Armor, args->DistanceToEpicenter) >= pThis->Health)
{
    damage = 0;
    pThis->Health = 1;
    pThis->EstimatedHealth = 1;
    ReceiveDamageTemp::SkipLowDamageCheck = true;
}
```

The same clamp the AttachEffect `Unkillable` flag opens, with the warhead flag
ORed in.

## Where VERA20K owns it

- `rules::warhead_type::WarheadType::can_kill` (default `true`).
- `sim::attach_effect::Simulation::apply_unkillable_clamp` opens on
  `target.phobos_unkillable() || !warhead.can_kill`, then keeps the existing
  house gate, `GetTotalDamage` kernel and the 1-HP write
  (`sim/attach_effect.rs:2672`). Its caller is the ReceiveDamage wrapper
  (`sim/combat/world_receiver.rs:1571`).

## Native anchors

The arm sits in the ReceiveDamage wrapper Phobos hooks at `TechnoClass_ReceiveDamage_Shield`
(`0x701900`), after the shield block and before the original body
(`Ext/Techno/Hooks.ReceiveDamage.cpp:178..188`).

## Residuals

- `SkipLowDamageCheck` is implicit: the caller treats an applied clamp as the
  zeroed packet, as it already does for the AttachEffect `Unkillable` arm.
- Retail/visual parity is untested here (no retail `ini/` in this checkout).
