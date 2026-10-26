# Ares country presentation

Build with `--features ares`. This covers the Ares 3.0 country keys that the
shell, loading screen and taunt stream consume. The stock ten-country
presentation remains unchanged; countries at index 10 and later take the
installed DLL's unknown-country defaults.

## Keys

| Key | Effect |
| --- | --- |
| `File.Flag=` | Country flag PCX for the Skirmish/LAN/WOL player and opponent rows, the Co-Op score screen and the diplomacy rows. Stock indices keep `usai.pcx` … `yrii.pcx`; unknown indices keep `rani.pcx`; a present-empty value clears the flag instead of falling back. |
| `MenuText.Status=` | Status-help CSF key in the country dropdown. Stock indices keep `STT:PlayerSide*`; unknown indices keep `GUI:Unknown`. |
| `File.Taunt=` | Taunt path with `~~` (Ares docs) or `%02i` (stock defaults) replaced by the two-digit taunt number. |
| `ListIndex=` | Dropdown position; ties keep `[Countries]` order. A negative value hides the row. Default 100. |
| `RandomSelectionWeight=` | Human “Random” country weight. Default 1. |
| `AIRandomSelectionWeight=` | AI “Random” country weight; default is the current `RandomSelectionWeight`. |
| `File.ObserverFlag=`, `File.ObserverBackground=`, `File.ObserverFlagAltPalette=` | Retained for observer presentation; no consumer yet. |

`ParentCountry=` copies the parent's resolved presentation first, then the
child's own keys override it. `ListIndex` becomes parent + 1 unless the child
sets it. The same one-shot copy already applies to the loading-screen fields.

The stock and unknown defaults are the 3.0 DLL's initializer table at
`HouseTypeExt::Initialize` (`0x1001C0F0`). Its unknown branch at `0x1001C413`
is `rani.pcx`, `GUI:Unknown`, `taunts\tauam%02i.wav`, `ls%sobs.shp` and
`mplsobs.pal`. The country dropdown hook is `hWnd_PopulateWithCountryNames @
0x4E3A6A` (Ares function behind the `.inj` entry): it filters
`Multiplay=yes` + nonempty `UIName`, drops negative `ListIndex`, sorts by
`ListIndex` then `ArrayIndex2`, and inserts `GUI:Random` first.

Random country selection uses the Ares pool of `Multiplay=yes` countries with
a positive weight. One bounded Scenario draw over the cumulative weights
selects a country; AI rows use `AIRandomSelectionWeight`. Hidden rows are
still eligible while their weight stays positive, as the Ares documentation
describes.

## Consumers

- `RuleSet::country_flag`, `country_status_key`, `country_taunt_pattern`,
  `country_list_index`, `country_random_weight`, `country_ai_random_weight`
  project the retained CountryExt values.
- `ui::skirmish_shell::country_choices` sorts/hides; `CountryMenuCatalog`
  carries the flag and status beside each visible country.
- `SkirmishShellChromeAtlas` preloads the stock flags plus every custom
  `File.Flag` from the startup rules; the Skirmish/Co-Op rows use the catalog
  names.
- `app::input::taunt` and `SfxPlayer::play_taunt` substitute the taunt
  number into the country pattern; a missing custom pattern falls back to the
  stock byte mapping only for indices 0..9.

## Residuals

- **Observer art** (`File.ObserverFlag`, `File.ObserverBackground`,
  `File.ObserverFlagAltPalette`) is parsed and exposed, but VERA has no
  observer-background/flag renderer to consume it.
- The loading-screen **side insignia** remains the stock ten-country resolver.
  A custom country's `File.LoadScreen`, palette and labels are already handled
  by the Ares country-loading chain, but its loading insignia has no stock
  slot.
- `ParentCountry` is copied in `[Countries]` registration order. A child
  listed before its parent inherits the parent's constructor/default values,
  not values read later in the same pass; Ares likewise reads countries in
  array order.
- A `File.Flag` that is missing from the mounted archives logs the shell
  atlas' optional-asset warning and shows no flag. Ares logs a parse warning
  and clears the flag instead.
- The weighted pick uses one bounded draw over cumulative weights. The
  installed `DiscreteDistributionClass` body was not separately executed in
  this chain; the documented weight/sum probability and the Ares pool gates
  are what the production path implements.
- A negative `RandomSelectionWeight`/`AIRandomSelectionWeight` is treated as
  ineligible. Ares casts the signed field to unsigned in its distribution,
  which is an authoring error rather than a usable exclusion (use 0).
- Country presentation state is process/GUI state, not part of the simulation
  CRC. It is retained by the live Rules owner and rebuilt from the same
  Country reader on a new start.
