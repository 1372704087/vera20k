# Sell or undeploy a building on impact (`BuildingSell`, `BuildingUndeploy`)

Two `WarheadType` arms that act on a hit building: sell it, or pack it up into
its `UndeploysInto=` unit. `BuildingSell` outranks `BuildingUndeploy`.

## Keys

| Key | Type | Default |
|---|---|---|
| `BuildingSell=` | `Valueable<bool>` | `no` |
| `BuildingSell.IgnoreUnsellable=` | `Valueable<bool>` | `no` |
| `BuildingUndeploy=` | `Valueable<bool>` | `no` |
| `BuildingUndeploy.Leave=` | `Valueable<bool>` | `no` |

Read: `WarheadTypeExt::LoadFromINI` (`Ext/WarheadType/Body.cpp:362-365`),
declarations `Ext/WarheadType/Body.h:207-210`, constructor `Body.h:486-489`.
VERA reads them in `src/rules/phobos_warhead_extras.rs`, owner
`WarheadType::phobos_extras`.

## Behaviour

`WarheadTypeExt::DetonateOnOneUnit` calls `ApplyBuildingUndeploy(pTarget)` after
`RemoveMindControl`, `ChangeOwner` and `ApplyConvert`
(`Ext/WarheadType/Detonate.cpp:213-214`). The body (`:266-352`):

1. A non-building target returns immediately.
2. `BuildingSell` **returns either way** (`:270-279`): with
   `CanBeSold() && !IsStrange()`, or unconditionally under
   `BuildingSell.IgnoreUnsellable`, it clears the archive target and calls
   `Sell(1)`; otherwise it does nothing at all and never falls through to the
   undeploy arm.
3. Otherwise a `Construction` or `Selling` mission returns (`:283-287`).
4. A type with no `UndeploysInto=`, or a `ConstructionYard` while the game mode
   forbids MCV redeploy, returns (`:291-292`).
5. The building's cell (offset by `(1,1)` for a foundation wider or taller than
   2, `:300-301`) becomes the archive target, then `Sell(1)` starts the pack-up
   (`:351-353`). With `BuildingUndeploy.Leave=` the cell is instead chosen by a
   16-direction threat weighting over armed non-allied units within 20 cells,
   then moved about 14 cells opposite the threat and snapped to a deployable
   cell (`:304-349`).

VERA's owner is `sim::combat::building_sell::apply`; the sell arm calls
`production::sell_back(SellOrder::Player)` after `set_archive_target(None)`,
and the undeploy arm calls `Simulation::undeploy_building`, whose
`can_undeploy_building_runtime` already carries the `UndeploysInto` and
`ConstructionYard`/MCVRedeploy gates.

`IsStrange()` is `sim::combat::crit`'s existing
`ObjectType::is_1x1_with_undeploy`, the same predicate the kill-weapon and
trajectory owners use.

## Residuals

- **`BuildingUndeploy.Leave` is parsed but not honoured.** Native builds a
  16-direction weighted threat record over `CellSpread` 20, reverses it, moves
   the target cell ~14 cells away and calls `MapClass::NearByLocation`. VERA's
  `undeploy_building` takes no destination cell, so the pack-up places the
  resulting unit at the ordinary undeploy cell. Trigger: `BuildingUndeploy=yes`
  together with `.Leave=yes`; effect: the new unit appears beside the old
  building instead of driving away from the threat; frequency: only that
  combination; risk: positional, no economic difference. Porting it needs a
  destination-aware undeploy plus VERA's own `NearByLocation`.
- **`CanBeSold()` is VERA's `production::can_sell_building`.** That function
  models `IsSellable` (category, draining, `Unsellable`, the buildup/wall
  arms); native `CanBeSold()` additionally folds in the mission test, which the
  undeploy arm re-checks separately here. A building with a live `Selling`
  mission that also passes `IsSellable` could be sold again natively where VERA
  refuses; the sell path is idempotent, so the visible difference is nil.
- **`Sell(1)` vs `SellOrder::Player`.** Phobos passes control `1` (the
  computer's sale) so the sale is not blocked by the player-facing gates VERA's
  `SellOrder::Player` models. Both end in the same refund and detach; only a
  mod that gates on the control value could tell them apart.

## Tests

- `src/rules/phobos_warhead_extras_tests.rs` — the parse group, including
  `only_the_three_primary_arms_ask_for_a_visit`.
