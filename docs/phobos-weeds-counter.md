# Phobos weeds counter

The third branch of `CreditsClass_GraphicLogic_HarvesterCounter @ 0x4A25E0`
(`src/Misc/Hooks.UI.cpp:175-190`), after the harvester counter and the power
delta. It prints the local player's stored weed total on the sidebar.

## Keys

| key | file | default | source |
|---|---|---|---|
| `[Sidebar] WeedsCounter.Show` | `uimd.ini` | `no` | `Phobos.INI.cpp:186-187` |
| `[Phobos] ShowWeedsCounter` | `RA2MD.INI` / profile | `yes` | `Phobos.INI.cpp:110` |
| `[<Side>] Sidebar.WeedsCounter.Offset` | `rulesmd.ini` | `0,0` | `Ext/Side/Body.cpp:39`, `Body.h:35,66` |
| `[<Side>] Sidebar.WeedsCounter.Color` | `rulesmd.ini` | unset | `Ext/Side/Body.cpp:40`, `Body.h:36,67` |

`ShowWeedsCounter`'s static initializer is `false` (`Phobos.INI.cpp:71`) but the
reader's default argument is `true` (`:110`), and the read always runs, so the
effective default is `yes`. `Sidebar.WeedsCounter.Color` is a
`Nullable<ColorStruct>`; unset falls back to `Drawing::TooltipColor` at the draw
(`Hooks.UI.cpp:179`).

## Behavior

Both switches must hold. The defeated player returns at the hook entry (`:104`)
before any branch. Unlike the power delta, this branch asks nothing about owned
buildings.

The text is `L"%d"` of `static_cast<int>(pPlayer->OwnedWeed.GetTotalAmount())` —
the bare total, with no label prefix and no `/max` half. `GetTotalAmount @
0x006C9650` already leaves an integer in ST0, so the narrowing is exact.

The anchor is the sidebar midpoint `+50` on X and the literal `2` on Y, plus the
side's offset: the same column as the harvester counter, which Phobos documents
as overlapping by design. The alignment bits are hardcoded
`UseGradPal | Center | Metal12`; there is no alignment key.

## Data owner

The store is House `+0x314`, ported with `HouseClass::GiveWeed @ 0x004F9700` and
`StorageClass::GetTotalAmount` / `Add_Amount` in `sim::storage_class`. The
indicator reads it through `Economy::owned_weed_total`; it owns no state.

## Residuals

- The weed source terrain, the `Weeder=` harvest branches, the gather animation
  and the Teleport locomotor arm are unported, so nothing fills the store in a
  retail match yet; the counter reads zero until a weeder chain lands.
- `SWType.UseWeeds.*` (weed-consuming superweapons) is unported.
- VERA resolves the indicator on the per-frame pass that ticks the power bar,
  where native runs it inside the credits tick. Same numbers, one pass per
  frame.
