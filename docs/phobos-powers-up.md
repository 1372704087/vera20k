# Powers-up host list and owner filter

Phobos lets one upgrade building type serve several host types and filters who
may install it (`Ext/BuildingType/Body.cpp:185-186`; the legacy
`PowersUpBuilding` string is kept in sync at `:270-277`; admission in
`BuildingTypeExt::CanUpgrade`, `Ext/BuildingType/Hooks.Upgrade.cpp:6-34`, hooks
`0x452678`/`0x4408EB`).

```ini
[UPGRADE]                     ; BuildingType (the upgrade)
PowersUp.Buildings=           ; host building types, default empty
PowersUp.Owner=Owner          ; AffectedHouse, default Owner
PowersUpBuilding=             ; legacy single host, kept in sync
```

## Behavior

- `PowersUp.Buildings` lists the host types the upgrade may be installed on.
  When the legacy `PowersUpBuilding` string is empty, the first list entry is
  copied into it; when the legacy string is set, its type joins the list if
  missing. Native code that still reads the single string therefore sees a
  consistent host.
- `PowersUp.Owner` is the House relation between the installing owner and the
  host's owner (`AffectedHouse`); the default `Owner` requires the same house.
- The upgrade is admitted when the type exists, the owner filter admits the
  relation and the host's type appears in the list.

## Where VERA20K owns it

`rules::building_upgrade::BuildingUpgradeRules` reads both keys and performs
the legacy sync in its per-pass reader. The single admission owner
`sim::building_upgrades::can_install_building_upgrade` uses the list and the
`can_target_house` filter under `phobos`; stock builds keep the legacy
`PowersUpBuilding` equality check.

## Residuals

- AI and House consumers that look for an upgrade's single host name
  (`sim/ai_build_need.rs`, `sim/production/can_build.rs`, and the Phobos House
  upgrade scans) see only the first list entry, which the sync writes into the
  legacy field; additional `PowersUp.Buildings` entries are considered only by
  the player's installation admission.
