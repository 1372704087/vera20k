# Optional Phobos timer blink colour

Build with `--features phobos` to choose the colour a superweapon timer blinks
in once its countdown has finished.

```ini
[AudioVisual]                  ; rulesmd.ini
TimerBlinkColorScheme=LightGrey ; a name from [Colors]
```

The value is a `[Colors]` entry name, matched case-insensitively as
`ColorScheme::FindIndex` does. The default is the third entry listed in
`[Colors]` — the entry the unmodified game reaches as
`ColorScheme::Array[5]`, since runtime scheme `R` addresses `[Colors]` entry
`R / 2`.

| Value | Effect |
| --- | --- |
| a name in `[Colors]` | the blinking time prints in that entry's colour |
| absent, empty, or a name no entry carries | the native third `[Colors]` entry |

The name is resolved against the `[Colors]` list as loaded, so an entry added
by a later rules layer is reachable. A name that no entry carries keeps the
default rather than falling back to a different entry.

## Where VERA20K owns it

- `rules/native_processing.rs` — the ordered `[AudioVisual]` pass retains
  `TimerBlinkColorScheme` with the other Rules scalars, read through the
  128-byte buffer Phobos uses, so a later pass whose value is empty keeps the
  earlier one.
- `rules/ruleset.rs` — `GeneralRules::timer_blink_color_scheme`, published from
  that retained scalar. `RuleSet::timer_blink_scheme_entry` resolves the name
  against `RuleSet::color_schemes`, returning `None` when no entry carries it.
- `app/presentation/super_timers.rs` — `blink_rgb` takes that entry, falling
  back to the native `BLINK_SCHEME`, instead of always using it.

## Residuals

- Only the superweapon timer lines step a blink in VERA20K, so the scenario
  timer and the blackout lines never take this colour. Phobos describes the
  key as covering "superweapon, mission etc" timers, but the unhardcoded hook
  replaces the colour where the game already blinks.
- A build without the capability keeps the native third entry unconditionally.
- The colour is the entry's base H,S,V through `HSV_To_RGB`, exactly as
  `Fancy_Text_Print_Wide` prints a house's text. The doubled runtime scheme
  pairs share one H,S,V, so either half of an entry prints the same colour.
