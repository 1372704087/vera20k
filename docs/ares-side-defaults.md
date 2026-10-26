# Ares fourth-side defaults

Build with `--features ares`. The stock build and INIs without Ares Side
sections keep their existing three-side behavior.

`[Sides]` already registers any number of Side identities; a Country's
`Side=` selects one. Ares 3.0 attaches a `SideExt` to every allocated Side.
For a fourth or later side, the getters fall back to the Allied stock values,
while a side's own section can replace them:

| Key | Fourth-side default (Ares 3.0) | Consumer |
| --- | --- | --- |
| `Crew` | `[General] AlliedCrew` | `sim::crew_survival` |
| `Engineer` | `[General] Engineer` | building survivor engineer |
| `Technician` | `[General] Technician` | armed survivor technician roll |
| `DefaultDisguise` | `[General] AlliedDisguise` | spy/perma-disguise construction, GetDisguise |
| `SurvivorDivisor` | `[General] AlliedSurvivorDivisor` | building survivor count |
| `AI.BaseDefenses` | `[AI] AlliedBaseDefenses` | AI base-defence candidates |
| `AI.BaseDefenseCounts` | `[General] AlliedBaseDefenseCounts` | BasePlan Recalc/preflight |
| `ParaDrop.Types` + `ParaDrop.Num` | `[General] AllyParaDropInf/Num` (both keys required) | generic paradrop |
| `ParaDrop.Aircraft` | `PDPLANE` resolved at SideExt initialization | paradrop plane |
| `Parachute.Anim` | unset: the existing `[General] Parachute=` / NPSE `VehicleParachute=` owner | attached canopy |
| `ToolTipColor` | `164,210,255` | in-game tooltip text/border |
| `GraphicalText.Image` + `GraphicalText.Palette` | unset: InitGame's literal `GRFXTXT.SHP` / `.PAL` | victory/defeat banner (`app::presentation::outcome_banner`) |
| `CampaignScore.Background` / `.Transition` / `.Animation` / `.Palette` | the side's row of `SideExt::initialize` (Allied `ASCRBKMD/ASCRTMD/ASCRAMD` + `ASCORE.PAL`, Soviet `SSCRBKMD…` + `SSCORE.PAL`, Yuri and later sides `SYCRBKMD…` + `YSCORE.PAL`) | Campaign StandaloneScore art (`render::main_menu_shell_chrome::build_campaign_score_art`) |
| `MultiplayerScore.Background` / `.Palette` | `MPASCRNL.SHP` + `MPASCRN.PAL`, `MPSSCRNL.SHP` + `MPSSCRN.PAL`, `MPYSCRNL.SHP` + `MPYSCRN.PAL` | Skirmish score screen art (`build_score_art`) |
| `MultiplayerScore.Bars` | `mpascrnlbar~~.pcx` / `mpsscrnlbar~~.pcx` / `mpyscrnlbar~~.pcx` | the ten score bands; the first `~~` takes the two-digit band number |
| `MessageTextColor` | runtime ColorScheme 21 (`[Colors]` entry 10) | map-action 11 message |
| `Sidebar.MixFileIndex` | 1 | sidebar MIX |
| `Sidebar.YuriFileNames` | no | sidebar filenames |
| `EVA.Tag` | index 0 (Allied) | EVA queue |

Each score name is substituted on its own, the way `0x1002F270` copies every
SideExt field over gamemd's buffer, so an Ares build never selects the 640-wide
`*SM.SHP` shapes: `SideExt::initialize` always holds a medium name. The SideExt
reader also retains `HunterSeeker`, the four score themes and
`DialogBackground.*`; those have no consumer in this chain yet.
`GraphicalText.*` is consumed by the victory/defeat banner: `Game_LoadUI_LoadSideData` (`0x1002F4A0`) loads the
local side's two files into the overlay slots through the writer at
`0x1002ECC0`, and the draw hook `0x1002FAD0` (patch site `0x6D4E79`) reads each
slot through `0x1002DF00` / `0x1002DF10`, which fall back to InitGame's literal
`GRFXTXT` surface per slot.

## Reader semantics

`SideExt::constructor @ 0x1002D9F0` leaves the fields at their zero/`-1`
member defaults. `SideExt::initialize @ 0x1002E190` applies the ID defaults
above on the first reached pass. The reader at `0x1002E450..0x1002EA4C` then
reads the current INI in native order:

`AI.BaseDefenseCounts`, `AI.BaseDefenses`, `Crew`, `Engineer`, `Technician`,
`DefaultDisguise`, `EVA.Tag`, `Parachute.Anim`, `ParaDrop.Aircraft`,
`ParaDrop.Types`, `ParaDrop.Num`, `Sidebar.MixFileIndex`,
`Sidebar.YuriFileNames`, `ToolTipColor`, `SurvivorDivisor`,
`MessageTextColor`, `HunterSeeker`, then the score/graphic/dialog keys.

Nullable values distinguish three states: an absent or unknown key retains
the current field, a present-empty key clears it, and a registered type name
sets it. Vectors follow `NullableVector`: `<default>` returns to the getter
fallback, a present-empty value replaces the vector with an empty one, and a
nonempty value replaces it, skipping unknown type names. `ParaDrop.Types`
keeps only InfantryType and UnitType identities.

## Evidence

Native spans read from the installed Ares 3.0 DLL (the NPSE `Ares.dll`, the
same build as `tools/rules_oracle/ares_side_sidebar.py`):

- default getters: Crew `0x1002DE60`, Disguise `0x1002DEA0`, Engineer
  `0x1002DEE0`, SurvivorDivisor `0x1002E130`, Technician `0x1002E170`,
  BaseDefenses `0x1002DDD0`, BaseDefenseCounts `0x1002DD40`, ParaDrop.Num
  `0x1002DFB0`, ParaDrop.Types `0x1002E060`, Parachute.Anim `0x1002E0F0`;
- consumers: building crew count `0x4C6E0`, building crew `0x4C650`,
  techno crew `0x4C780`, disguise `0x52267D`/`0x6F422F`, base-defence picks
  `0x507BCA`/`0x507DBA`/`0x507FAA`, paradrop `0x6CD2EE`/`0x6CD537`, canopy
  `0x5F5ADD`, tooltip `0x72F440`, message color `0x6DE0D3`, trigger side
  `0x1002F2D0`;
- SideExt state is retained on the existing `ProcessedType` owner
  (`src/rules/native_processing.rs`), projected once into `RuleSet`
  (`src/rules/ruleset.rs`), and shared by the consumers above.

## Residuals

- **Score themes** (`CampaignScore.UnderParTheme` / `.OverParTheme`,
  `MultiplayerScore.WinTheme` / `.LoseTheme`, all initialized to `SCORE`) are
  retained but not played: VERA starts the literal `SCORE` from the scenario
  exit cascade, while the installed DLL substitutes the side's name at
  `Global_DrawScoreScreen_ScoreTheme` (`0x1002F4B0`, patch `0x5C9B75`, win/lose
  from the local player's `HouseClass+0x1F5`, `g_pCurrentHouse` at `0xA83D4C`)
  and at the campaign dialog's
  `ScoreDialog_Handle_ScoreThemeA/B` (`0x1002F920` / `0x1002F9A0`, patches
  `0x6C922C` / `0x6C935C`, the par branch). Neither the win/lose flag nor the
  par comparison reaches the VERA play site today.
- `DialogBackground.*` is retained but not drawn; the dialog background is a
  separate UI mechanism (`0x621E90`'s paint).
- The banner resolves one file pair per match. Native substitutes each slot
  alone, so a side that names a loadable image and an unloadable palette draws
  the custom image with the stock `GRFXTXT.PAL`; VERA keeps the whole process
  art in that case.
- **HunterSeeker** is retained but the Hunter Seeker superweapon launch is
  not ported, so it has no effect.
- **Vehicle entries in `ParaDrop.Types`** are retained but the passenger
  owner resolves InfantryType names only; such entries board no passenger.
- A custom `AI.BaseDefenseCounts` vector shorter than the difficulty row is
  rejected by preflight as a missing difficulty; the native read past the
  vector is undefined data.
- `MessageTextColor` resolves a scheme name against the current pass'
  `[Colors]` section. A later map/mode pass that names a scheme without a
  local `[Colors]` section retains the previous value instead of resolving
  against the global scheme array.
- The Ares save layout gained the new fields; saves made before this change
  do not load in Ares builds.
- With Ares and NPatch compiled together, `Parachute.Anim` takes precedence
  over NPSE `VehicleParachute=` and the stock canopy. The installed DLL's
  hook order at the shared `0x5F5ADD` site is not separately proven.
- The fourth side's sidebar geometry uses the existing nonzero (Soviet)
  layout while its files come from `Sidebar.MixFileIndex` /
  `Sidebar.YuriFileNames`. Ares does not add a fourth geometry family.
