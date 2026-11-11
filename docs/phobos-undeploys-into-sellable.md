# Sellable repack buildings

`UndeploysInto.Sellable=` lets a non-Construction-Yard building with
`UndeploysInto=` sell normally instead of packing up into its vehicle.

```ini
[SOMEBUILDING]                      ; BuildingType
UndeploysInto=UNIT
UndeploysInto.Sellable=false        ; Valueable<bool>, constructor false
```

Reader: `Ext/BuildingType/Body.cpp:256` (`Body.h:110`, constructor `:229`).

## Native body

`BuildingClass::Mission_Selling` decides with
`BuildingExt::CanUndeployOnSell` (`Ext/Building/Hooks.Selling.cpp:33-58`).
A building with no `UndeploysInto=` never packs up. A Construction Yard packs
up only with `MCVRedeploy`, the human-owner/deploy-controlled gates and an
ArchiveTarget. Any other repack building packs up unless
`UndeploysInto.Sellable=yes` is set, in which case it packs up only while it
holds an ArchiveTarget (`pThis->ArchiveTarget != nullptr`); otherwise the
sale runs normally. The `0x449CC1` EVA line and `0x44A7CF` sell-sound hook
consult the same answer.

## VERA

`ObjectType::phobos_undeploys_into_sellable` carries the key and
`sim/production/production_sell.rs::qualifying_undeploy` applies it in the
non-ConYard branch: the default returns true (pack up), while a sellable
building returns the shared `sale_archive` answer. The same owner feeds
`sell_stage_zero` and the sell-sound/EVA paths.

## Residuals

- The key only changes the non-ConYard branch; Construction Yard behaviour
  keeps its existing `MCVRedeploy` and controller gates.
- Retail rules author no `UndeploysInto.Sellable`; the default packs up.
