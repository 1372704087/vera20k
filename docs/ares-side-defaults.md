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
| `MessageTextColor` | runtime ColorScheme 21 (`[Colors]` entry 10) | map-action 11 message |
| `Sidebar.MixFileIndex` | 1 | sidebar MIX |
| `Sidebar.YuriFileNames` | no | sidebar filenames |
| `EVA.Tag` | index 0 (Allied) | EVA queue |

The SideExt reader also retains `HunterSeeker`, the seven campaign and
multiplayer score-screen filenames, four score themes, `GraphicalText.*` and
`DialogBackground.*`. Those values have no consumer in this chain yet.

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

- **Score screens and themes**, `GraphicalText.*` and `DialogBackground.*`
  are retained but not drawn; those are separate UI mechanisms.
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
