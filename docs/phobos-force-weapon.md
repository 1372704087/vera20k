# Forced weapon selection

Phobos can force the weapon a Techno selects against a target, overriding the
native `WhatWeaponShouldIUse` ladder (`Ext/TechnoType/Body.cpp:60-155`,
reader `:905-936`, hook `0x6F3428` in `Ext/Techno/Hooks.Firing.cpp:94-118`).

```ini
[TECHNO]                              ; TechnoType
ForceWeapon.Naval.Decloaked=-1        ; slot, default -1 (unset)
ForceWeapon.Cloaked=-1
ForceWeapon.Disguised=-1
ForceWeapon.UnderEMP=-1
ForceWeapon.InRange=                  ; ordered slots, default empty
ForceWeapon.InRange.Overrides=        ; ranges in cells, default empty
ForceWeapon.InRange.TechnoOnly=       ; Nullable<bool>, default [General] yes
ForceWeapon.InRange.ApplyRangeModifiers= ; Nullable<bool>, default [General] no
ForceWeapon.Buildings=-1
ForceWeapon.Defenses=-1
ForceWeapon.Infantry=-1
ForceWeapon.Naval.Units=-1
ForceWeapon.Units=-1
ForceWeapon.Aircraft=-1
ForceAAWeapon.InRange=                ; air-target list
ForceAAWeapon.InRange.Overrides=
ForceAAWeapon.InRange.ApplyRangeModifiers= ; default [General] no
ForceAAWeapon.Infantry=-1             ; airborne Infantry targets
ForceAAWeapon.Units=-1                ; airborne Unit targets
ForceAAWeapon.Aircraft=-1             ; airborne Aircraft targets

[General]                             ; global fallbacks
ForceWeapon.InRange.TechnoOnly=yes
ForceWeapon.InRange.ApplyRangeModifiers=no
ForceAAWeapon.InRange.ApplyRangeModifiers=no
```

## Behavior

The forced slot is answered before native selection. A live Techno target is
tested in order: `Naval.Decloaked` (Cloakable, Naval, Uncloaked), `Cloaked`,
`Disguised`, `UnderEMP`. Otherwise `InRange`/`ForceAAWeapon.InRange` walk their
slots in order; a slot inside its range (an override in cells, else the
weapon's own `Range=`, optionally with attach-effect range modifiers) is
forced, and a `-1` entry inside its range stops the walk without forcing.
Finally the class arms answer: a `BuildCat=Combat` building takes
`Defenses` before `Buildings`; an Infantry target takes `ForceAAWeapon.Infantry`
when airborne, else `Infantry`; a Unit takes `ForceAAWeapon.Units` when
airborne, else `Naval.Units` for a naval target, else `Units`; an Aircraft
takes `ForceAAWeapon.Aircraft` when airborne, else `Aircraft`. With no armed
field the type skips the check entirely.

In-range uses `ObjectClass::Distance_To` and the native `GetWeapon` of each
slot; the in-range gate needs a Techno target when `TechnoOnly` is yes.

## Where VERA20K owns it

`rules::phobos_force_weapon` reads the type fields (int list reader for slots,
Ares double-vector reader for the overrides, Phobos `Nullable<bool>` for the
modifiers) and `phobos_globals` reads the three `[General]` fallbacks. The one
selection owner `sim::combat::combat_weapon::select_weapon_index_against_in_query`
calls `phobos_force_weapon_index` before `what_weapon_should_i_use`, so every
SelectWeapon consumer sees the forced slot. Range modifiers reuse
`attach_effect::range_with_attach_effects`, distance reuses
`object_distance_to`.

## Residuals

- `MultiWeapon.*` (the next hook arm at the same site) is a separate chain.
- The in-range overrides are `ValueableVector<double>` read with the Ares
  `Parser<double>` scan, matching the project's Phobos `Valueable` reader
  convention; prefixed tokens may differ from a pure native `ReadDouble`.

Tests: the reader defaults/overrides, the `[General]` fallbacks, and a
production selection test covering a class arm, the air-only arm replacing the
class arm, the in-range hit and the fall-through outside its range.
