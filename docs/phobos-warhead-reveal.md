# Warhead `Reveal`

`[WarheadType] Reveal=N` reveals the map around the impact to the firing
house. `N` is in cells; a negative value reveals the whole map. Plain
`Valueable<int>` read by `Ext/WarheadType/Body.cpp:157`, default `0`.

```ini
[WARHEAD]
Reveal=5      ; reveal a 5-cell radius for the owner
Reveal=-1     ; reveal the whole map for the owner
```

## Native evidence

`Ext/WarheadType/Detonate.cpp:82-100` runs before `TransactMoney`:

```
if (reveal > 0) {
    if ((pHouse->IsControlledByCurrentPlayer() || pHouse->IsAlliedWith(pCurrent))
        && !pCurrent->Defeated && !pCurrent->Visionary)
        MapClass::RevealArea2(&coords, reveal, pHouse, 0, 0, 0, 1, 0);
} else if (reveal < 0) {
    MapClass::Reveal(pHouse);
}
```

The positive arm writes only the client maps whose RevealArea gate admits the
firing house; the negative arm sets that house's map-clear byte and explores
every allocated cell. Neither arm runs without a firing house.

## Where VERA20K owns it

- `rules::warhead_type::WarheadType::reveal`.
- `sim::combat::crit::detonate`'s prefix, before TransactMoney
  (`sim/combat/crit.rs:130`):
  - negative: `Simulation::reveal_map_for_owner` (`sim/world/shroud_refresh.rs:1259`,
    `MapClass::Reveal @ 0x00577D90`).
  - positive: `vision::direct_reveal_viewers` picks the admitted client maps
    and `Simulation::reveal_area` runs the native final0/final1 pair at the
    impact cell (`shroud_refresh.rs:874`).
  - no firing house: no-op.

## Residuals

- The positive arm's native `Visionary`/defeat gate is carried by the shared
  admitted-viewer domain; the per-cell projection uses the transient reveal
  route the psychic reveal owner also uses.
- Retail/visual parity is untested here (no retail `ini/` in this checkout).
