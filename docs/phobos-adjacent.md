# Building placement adjacency filters

Phobos can filter which buildings count as adjacency providers for a placing
building type and forbid placement near disallowed types
(`Ext/BuildingType/Body.cpp:235-238`, hooks in
`Ext/BuildingType/Hooks.cpp:203-273`).

```ini
[PLACER]                            ; BuildingType
Adjacent.Allowed=                   ; building type list, default empty (all)
Adjacent.Disallowed=                ; building type list, default empty (none)
Adjacent.Disallowed.Prohibit=false  ; bool, default false
Adjacent.Disallowed.ProhibitDistance=0 ; integer, default 0
```

## Behavior

The proximity check walks the ring of cells around the placement footprint at
the placing type's `Adjacent=` distance (each adjacent cell's first ground
building counts). `Adjacent.Allowed`, when non-empty, keeps only the named
building types as providers. A same-house building of a type in
`Adjacent.Disallowed` is skipped as a provider; with
`Adjacent.Disallowed.Prohibit=yes` it instead makes the placement invalid.
`Adjacent.Disallowed.ProhibitDistance` above zero first runs the whole
proximity check over a larger ring (that distance) with the disallowed list
fatal; the normal ring then ignores the disallowed list. The check is
ring-local: a disallowed building outside the scanned ring never matters.

## Where VERA20K owns it

`rules::phobos_adjacent` reads the four type keys. The single placement
proximity owner `production::production_placement::is_within_build_area`
(the port of `DisplayClass::PassesProximityCheck`/Display4A8EB0) applies the
allow filter, the same-house disallow skip/fatal and the prohibit pre-pass
through `build_area_scan`; stock builds run the unchanged scan.

## Residuals

- Phobos' `NoBuildAreaOnBuildup` from the same hook is a separate key and is
  not ported here.
