# Missile cruise and takeoff

Phobos gives an aircraft type that flies as a rocket three keys
(`Ext/AircraftType/Body.cpp:42-44`, `AircraftTypeExt::Initialize` at `:12`;
consumers in `Ext/Techno/Hooks.cpp:2240-2325`).

```ini
[MISSILE]                        ; AircraftType
Missile.Cruise=false             ; bool, default false
Missile.TakeOffSeparation=24     ; int, default 24
Missile.TakeOffAnim=V3TAKOFF     ; AnimType, default V3TAKOFF
```

## Behavior

- `Missile.Cruise=yes` forces the cruise-missile raise path on the linked
  rocket (`RocketLocomotionClass_Process_CruiseMissileCheck` hook `0x662354`
  and `_CruiseMissileRaise` `0x662720`), even when the type is not the
  configured CMisl type; the pause state then promotes to the raise state
  instead of the tilt state.
- `Missile.TakeOffSeparation` is the takeoff trailer-timer length in frames
  (`_CustomSmokeInterval` `0x6623FC`); the stock value is 24.
- `Missile.TakeOffAnim` names the takeoff animation
  (`_CustomMissileTakeoff` `0x6624FB`); the type's key replaces the rules
  block's anim, and `none`/`<none>` means no puff.

## Where VERA20K owns it

`rules::phobos_missile` reads the three keys into an `ObjectType` field;
`sim::movement::rocket_movement` consults the linked owner through the new
`RocketHost::phobos_missile` method: `process` ORs the type's cruise flag into
the CMisl selection, and the pause/raise states take the separation and
takeoff anim from `takeoff_settings`. `sim::world::rocket_flight`'s host
resolves the owner's type. Stock builds keep the 24-frame timer and the rules
block's anim.

## Residuals

- An unknown `Missile.TakeOffAnim` name is kept literally; native
  `AnimTypeClass::Find` would keep the previous value instead. No retail key
  names an unregistered animation.
- `Missile.Cruise` is asserted through the rules helper and the stock rocket
  corpus (which proves the flag does not disturb the CMisl paths); a scripted
  production row with `Missile.Cruise=yes` is not part of the corpus.
- The host clones the small settings struct for every rocket frame.
