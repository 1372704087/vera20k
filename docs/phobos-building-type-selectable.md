# Optional Phobos type select for buildings

Build with `--features phobos` to let type select pick buildings that the
retail ID match refuses.

```ini
[General]                       ; rulesmd.ini
BuildingTypeSelectable=false    ; boolean, default no
```

With the switch on, a second T press (or the sidebar's TypeSelect button) with
a building selected selects every building of that type the local player owns,
on screen first and across the map when the screen has no more of them.

## Native site

Type select asks a per-object predicate before it admits a type. For a building
that is the virtual at `0x00465D40`:

```asm
00465d40  mov  eax, [ecx+0x408]      ; the building's type
00465d46  test eax, eax
00465d48  je   0x465d6b
00465d4a  mov  eax, [ecx+0xef0]      ; the type's table index
00465d55  shl  eax, 2
00465d58  cmp  dword [eax+0x8192b8], 1
00465d5e  jne  0x465d6b
00465d60  cmp  dword [eax+0x819310], 1
00465d66  jne  0x465d6b
00465d68  mov  eax, 1
00465d6a  ret
00465d6b  xor  eax, eax
00465d6d  ret
```

The predicate is the one Phobos names `Is1x1AndUndeployable`: both table
reads have to hold. That is why an unmodified game reaches almost no building —
a `GAPOWR` satisfies neither, while the building an MCV deploys into
(`UndeploysInto=`) satisfies both. Phobos's `BuildingClass_Is1x1AndUndeployable_BuildingMassSelectable` hook
at the same address returns `true` while `UICommands_TypeSelect` is running and
`[General] BuildingTypeSelectable` is set, so every building becomes a type
select seed.

The working method in vanilla - hold the second T press and click the building -
never consults this predicate, which is why it already reached buildings; the
tap and the sidebar button both run `UICommands_TypeSelect`, which is the path
the hook patches.

## Where VERA20K owns it

- `rules/ruleset.rs` - `GeneralRules::building_type_selectable` and
  `RuleSet::structure_type_selectable`, the admission predicate. VERA has no
  footprint test on this path, so the retail arm keeps the `UndeploysInto=`
  half only; the switch admits every building.
- `app/input/entity_pick.rs` - `type_select_candidates` asks the predicate for
  a structure instead of the hardcoded `UndeploysInto` test. Both callers of
  the tap (the T key and the sidebar's TypeSelect button) and the map-wide
  stage run through it.

## Residuals

- The native 1x1 half of the retail predicate is not modelled; any
  `UndeploysInto=` building is a seed even with a large footprint, and the
  switch widens that to every building.
- The held-T click and the T-held band drag reach buildings without asking the
  predicate at all, and this key does not change that. Their admission is the
  ordinary type group, so the switch only shows on the tap and the sidebar
  button.
- Phobos disables the feature without Ares because both patch the same region.
  VERA20K does not load Ares, so the capability never applies.
