# Cloak keys

A cloaking host can eject its parasite.

```ini
[TechnoType]
Cloak.KickOutParasite=               ; Nullable<bool>, unset -> [General]

[General]
Cloak.KickOutParasite=no             ; Valueable<bool>
```

Reader `Ext/TechnoType/Body.cpp:1205`, global `Ext/Rules/Body.cpp:289`
(constructor `Ext/Rules/Body.h:1056`).

## Behavior

- `FootClass_PointerGotInvalid_Parasite` (`0x4D9992`,
  `Ext/Techno/Hooks.cpp:1415-1445`): the parasite owner's `PointerExpired`
  receives the real removal control instead of stock's constant true, and a
  cloak's `Detach_All(false)` control falls back to the host type's key, else
  `[General]`. With neither set, a cloaking host keeps its parasite.
- A true control releases the parasite and clears the victim's own link; a
  false one leaves both in place.

## Where VERA20K owns it

`rules::phobos_globals::cloak_kick_out_parasite` and
`ObjectType::phobos_cloak_kick_out_parasite` feed
`sim::combat::parasite::foot_parasite_pointer_expired`, the single pointer-
expiry owner. The Ares hooks (`10048280`/`100482D0`) use the same real-control
model, so the two features agree; without either feature stock's constant true
still releases the parasite.
