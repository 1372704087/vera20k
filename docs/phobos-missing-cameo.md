# Optional Phobos missing cameo

Build with `--features phobos` to replace the placeholder the sidebar shows for
a type whose cameo art is missing.

```ini
[AudioVisual]                 ; rulesmd.ini
MissingCameo=XXICON.SHP       ; filename, including the .shp or .pcx extension
```

The default is the stock `XXICON.SHP`. The extension selects the loader, exactly
as Phobos's two hooks split it:

| Name | Path |
| --- | --- |
| anything but `XXICON.SHP` containing `.shp` | the type's loaded `XXICON.SHP` handle is replaced at load time (`0x715A4D`, `0x716D13`, `0x6CE8AA`, `0x6CEE31`) |
| anything containing `.pcx` | `StripClass_Draw_DrawMissing` (`0x6A99F3`) blits the PCX into the 60x48 cell |
| `XXICON.SHP` itself, or no matching extension | the stock `XXICON.SHP` pointer |

A configured `.pcx` therefore also works for a mod that keeps the stock name out
of its MIX. The order matters for a name carrying both extensions: the
load-time `.shp` arm runs first, so `.shp` wins.

The key applies to TechnoType cameos and to SuperWeaponType icons; Phobos hooks
both `ReadINI`/`Load` pairs. A type that does have its own cameo art is not
affected.

## Where VERA20K owns it

- `rules/native_processing.rs` — the ordered `[AudioVisual]` pass retains
  `MissingCameo` with the other Rules scalars, read by the 32-byte reader
  Phobos uses, so a later pass whose value is empty keeps the earlier one.
- `rules/ruleset.rs` — `GeneralRules::missing_cameo`, published from that
  retained scalar and defaulting to `DEFAULT_MISSING_CAMEO`. Exposed as
  `RuleSet::missing_cameo`.
- `render/sidebar_cameo_atlas.rs` — `render_cameo` tries the declared,
  image-derived and legacy SHP candidates first and only then
  `render_missing_cameo`; `render_superweapon_cameo` does the same for an icon
  with no `SidebarImage=`. `missing_cameo_source` is the extension decision
  above, and `render_pcx_cameo` is now shared by Ares `CameoPCX` and this key.

## Residuals

- A build without the capability keeps the previous behaviour: a type with no
  cameo art has no atlas entry, and the cell stays background. The stock
  `XXICON.SHP` fallback itself is not modelled outside the Phobos build.
- `AltCameo=` keeps its own lookup with no fallback, matching the native
  `+0x1F8` field, which the hook does not touch.
- The PCX arm draws on Ares's 60x48 canvas convention rather than re-deriving a
  destination from the surface, because VERA composites the sidebar atlas
  instead of blitting into the native `SidebarSurface`.
