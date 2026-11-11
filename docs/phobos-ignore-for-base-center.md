# Base-center exclusion (`IgnoreForBaseCenter`)

A TechnoType can opt out of its House's base-center math: the building or
starting techno still exists and plays normally, but it does not pull the
House's base center. Phobos reads the flag as a `Valueable<bool>` (default
`no`) in `Ext/TechnoType/Body.cpp:810`, declared `Body.h:77`, constructed
`false` at `:515`.

```ini
[SOMETECHNO]
IgnoreForBaseCenter=no                      ; boolean, default no
```

## Native evidence and hooks

| Address | Hook | Behavior |
| --- | --- | --- |
| `0x4FD1CD`, `0x4FD463` | `HouseClass_RecalcCenter_LimboDelivery` | The building loop skips a building whose map coordinates are illegal, a limbo-delivered building the House still owns, or one whose type has `IgnoreForBaseCenter`. `Ext/House/Hooks.cpp:94-105`. |
| `0x4AC534` | `DisplayClass_ComputeStartPosition_IllegalCoords` | The starting-techno loop skips a techno with illegal map coordinates or `IgnoreForBaseCenter`. `Ext/House/Hooks.cpp:111-120`. |
| `0x4FD166` | `HouseClass_RecalcCenter_SetContext` | Installs the House extension before the loop so the limbo-delivery ownership test can run (`:85-91`). |

The flag affects only the center math; it does not remove the object, change
its mission, or alter `BaseSpawnCell`.

## Where VERA20K owns it

- `rules::object_type::ObjectType::phobos_ignore_for_base_center` reads
  `IgnoreForBaseCenter` with `read_bool` (constructor default `false`).
- `sim::world::house_base::initialize_house_base_from_build_const` is VERA's
  only building-derived base center (`0x0050C210`: the last non-limbo owned
  BuildConst building anchors the House). Its candidate scan now also skips a
  type with the flag, mirroring the RecalcCenter skip.

VERA's base center is otherwise static: the launch waypoint
(`sim::scenario_bootstrap::assign_launch_base_centers`), the auto-base trigger's
first BuildConst cell, or an MCV deploy's spawn cell. There is no
`HouseClass::RecalcCenter` loop and no starting-techno
`DisplayClass::ComputeStartPosition` scan to extend, so the flag currently
gates the one center derivation VERA performs from a building list.

## Phobos quirks kept

- The key is a `Valueable<bool>`: an absent, empty or invalid value keeps the
  constructor's `false`; there is no unset state or global fallback.
- The skip applies to the building's map coordinate read, not to its
  foundation or ownership: an ignored building still pays upkeep, counts for
  power and can be sold.
- An ignored building cannot become the BuildConst anchor; a non-ignored
  earlier building becomes the center instead.

## RNG draws

None; the center derivation is a list scan and coordinate read.

## Residuals (trigger · impact · frequency · risk)

- **No `HouseClass::RecalcCenter`.** VERA keeps its static base center; this
  chain only filters the BuildConst-anchor candidate list, so a center that
  was already assigned (launch waypoint, trigger, deploy) is not recomputed
  when buildings are added or removed. Trigger: any later base-center event.
  Frequency: always. Risk: ignored buildings only matter where the anchor is
  chosen. A future `recalc_center` owner should apply the same
  `phobos_ignore_for_base_center` gate; the build-const scan is the reference
  site.
- **No `DisplayClass::ComputeStartPosition`.** VERA derives multiplayer
  starts from launch waypoints rather than averaging starting technos, so the
  consumer that skips a starting techno with the flag is not ported. Trigger:
  a scenario whose start position comes from preplaced technos. Frequency:
  campaigns/maps outside the launch path. Risk: none until that scan exists.
- **Non-BuildConst technos.** The flag is read for every TechnoType but only
  consulted for a building that could anchor the base; an ignored ordinary
  unit has no effect today.
- **Retail/visual parity untested** (no retail `ini/` in this checkout).
