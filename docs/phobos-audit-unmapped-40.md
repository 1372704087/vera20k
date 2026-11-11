# Phobos audit: the 40 unmapped `###` sections

Forty sections of `docs/New-or-Enhanced-Logics.md` carry no backtick token that
the v3 coverage scanner resolved to a known Phobos INI key, so
`phobos_feature_coverage.md` could not assign them a status. This page records,
per section, the keys Phobos actually reads, the VERA owner (or `absent`) and one
classification. It is an audit, not a port plan; nothing here is implemented by
it.

Phobos source: `Ext/...` paths are relative to `G:/Phobos-develop/src`. VERA
paths are relative to this checkout's `src/`.

Classification rule used here:

- `implemented` — every key and behaviour arm of the section exists in VERA.
- `partial` — at least one key/arm is ported, at least one is missing.
- `key-missed` — VERA has no implementation, but the section reduces to plain
  INI reads on a native site VERA already ports, so the scanner should extract
  the key and coverage can be re-run instead of tracking a design item.
- `behaviour-only` — the Phobos side is keyless or enum-only logic needing a
  bespoke design against the named native hook.
- `not-started` — no VERA implementation and no reusable mechanism; a new owner
  or subsystem is needed even when a key exists.

## Rows

| # | Category | Feature | Phobos real key(s) / mechanism | VERA status | Classification | Evidence (Phobos → VERA / absent) |
|---|----------|---------|-------------------------------|-------------|----------------|------------------------------------|
| 1 | Buildings | Destroyable pathfinding obstacles | `[BuildingType] IsDestroyableObstacle` (bool, default no; the section body's `IsDestroyableBlockage` spelling is wrong). Read `Ext/BuildingType/Body.cpp:221`, declared `Body.h:66`. An obstacle building answers `IsBlockage` to `CellClass::CheckPassability` (`Ext/Building/Hooks.cpp:568`, `0x483D8E`) and recalculates zones/sub-zones on unlimbo/limbo (`:533` `0x440D01`, `:543` `0x445D87`); only `MovementZone` rows that pass the blockage class (Destroyer / AmphibiousDestroyer / InfantryDestroyer) may path into it. | Key absent. VERA owns building-footprint blocking (`sim/pathfinding/core.rs:1910`) and the `MovementZone` class rows incl. the three Destroyer rows (`rules/locomotor_type.rs:346-400`), but no building ever reports a destroyable blockage and no path terminates at an obstacle to be shot. | key-missed | `Ext/BuildingType/Body.cpp:221` + `Ext/Building/Hooks.cpp:568` → key absent; owner `sim/pathfinding/core.rs:1910`, `rules/locomotor_type.rs:377-390` |
| 2 | Infantry | Customizable FLH when infantry is prone or deployed | `[InfantryType] PronePrimaryFireFLH`, `ProneSecondaryFireFLH`, `DeployedPrimaryFireFLH`, `DeployedSecondaryFireFLH` (`[art]` layer, `Nullable<CoordStruct>`). Read `Ext/InfantryType/Body.cpp:34-37`, declared `Body.h:21-24`; consumed by `InfantryExt::GetSimpleFLH` (`Ext/Infantry/Body.cpp:19-30`, deployed first then prone, per weapon slot), composed with the burst-row offsets. | Read as `Deployed{Primary,Secondary}FireFLH` / `Prone{Primary,Secondary}FireFLH` in `rules/weapon_offsets.rs:280-300`; `state_flh` picks the deployed row, then the prone row. Wired through `sim/combat/fire_coord.rs` `infantry_flh_states` (`sim/combat/laser.rs:394`) with tests in `sim/combat/infantry_flh_tests.rs`. | implemented | `Ext/InfantryType/Body.cpp:34-37`, `Ext/Infantry/Body.cpp:19-30` → `rules/weapon_offsets.rs:287-296`, `sim/combat/infantry_flh_tests.rs:127-144` |
| 3 | Infantry | Random death animation for NotHuman infantry | `[General] NotHuman.RandomDeathSequence` (bool, default no) read `Ext/Rules/Body.cpp:621`; `[InfantryType] NotHuman.RandomDeathSequence` (`Nullable<bool>`, falls back to `[General]`) read `Ext/InfantryType/Body.cpp:20`. Consumer `Ext/Techno/Hooks.ReceiveDamage.cpp:330`: a `NotHuman=` infantry draws one `Die1..Die5` sequence instead of the hardcoded `Die1`. | `[General]` global in `rules/phobos_globals.rs:116-119`, read `:734-737`; type override `rules/object_type.rs:1007-1012`, read `:5185-5186`; consumed at death in `sim/world/infantry_death.rs:621-622` (global fallback), one RNG draw. | implemented | `Ext/Techno/Hooks.ReceiveDamage.cpp:330` → `sim/world/infantry_death.rs:621-622`, `rules/phobos_globals.rs:734` |
| 4 | Projectiles | Attack technos underground | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 5 | Super Weapons | AI Superweapon delay timer | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 6 | Technos | Attack move — without weapon | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 7 | Technos | Attack non-threatening structures (Techno) | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 8 | Technos | Cloak Enhancement | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 9 | Technos | Convert TechnoType on owner house change | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 10 | Technos | Customize the country displayed in `Sight` | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 11 | Technos | Exclusion from base center calculations | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 12 | Technos | Firing offsets for specific Burst shots | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 13 | Technos | Initial spawns number | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 14 | Technos | Low priority for deploy | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 15 | Technos | Mind Control enhancement | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 16 | Technos | Multi VoiceAttack | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 17 | Technos | No Manual Move | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 18 | Technos | No rearm and reload in EMP or temporal | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 19 | Technos | Overload characteristic dehardcoded | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 20 | Technos | Promoted Spawns | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 21 | Technos | Recount burst index | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 22 | Technos | Reset MindControl after transformation | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 23 | Technos | Shared Ammo | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 24 | Technos | Targeting limitation for berzerk technos | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 25 | Vehicles | Amphibious access vehicle | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 26 | Vehicles | Independent SHP Vehicle Turret Files | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 27 | Vehicles | Turret Response | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 28 | Warheads | Damage technos underground | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 29 | Warheads | Modify ammo on impact | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 30 | Warheads | Remove disguise on impact | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 31 | Warheads | Reveal map for owner on impact | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 32 | Warheads | Reverse engineer warhead | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 33 | Warheads | Shroud map for enemies on impact | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 34 | Warheads | Spawn powerup crate | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 35 | Warheads | Taunt warhead | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 36 | Warheads | Trigger specific NotHuman infantry Death anim sequence | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 37 | Warheads | Warhead that can not kill | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 38 | Warheads | Customize whether warhead can prevent crew escape from techno | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 39 | Weapons | Radiation enhancements | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
| 40 | Weapons | Range finding in cylinder | TODO — not audited | TODO — not audited | TODO — not audited | TODO — not audited |
