# Ares country AI power plants

Build with `--features ares`. `[Country] AI.PowerPlants=` replaces the single
power plant a house splices into its base plan when it runs short of power.

| Key | Resolution |
| --- | --- |
| `AI.PowerPlants=` | `HouseTypeExt::GetPowerplants @ 0x1001BD20` answers the country's list while it is non-empty, otherwise the per-side default: side 0 `[General] GDIPowerPlant=`, side 1 `NodRegularPower=`, side 2 `ThirdPowerPlant=`, no later side. Read in the same primary-Rules block as the other country keys; `ParentCountry=` copies it. |

The consumer is `AI_Choose_Building`'s power splice: the installed DLL patches
six bytes at `0x4FE782` (body `0x1001DDC0`) — the stock side-index branch that
picks the one default plant. The replacement filters the candidate list and,
when more than one student survives, draws `Randomizer(0, count-1)` once on
Scenario+`0x218` and splices that entry; an empty result logs and leaves the
splice. Keeping or removing a plant therefore moves the simulator's RNG stream,
so the list enters the state hash under `ares-country-powerplants-v1`.

## Consumers

- `sim::ai_base_building::power_plant_to_splice`: after the stock short-power
  gates (`0x4FE74D..0x4FE77C`) it takes the acting house's country list; one
  registered candidate splices it without a draw, several draw once, and none
  registered splices nothing. An empty or absent list keeps the stock per-side
  branch described above.

## Residuals

- The two candidate gates the replacement applies (`0x10021B00`, `0x10021CA0`)
  are not identified. VERA keeps the stock gates already evaluated before the
  patched bytes, so a listed plant is not re-filtered by whatever those two
  helpers check.
- Ares replaces the *default* branch as well: with an empty list its resolver
  answers exactly one plant per side and never the `NodAdvancedPower=` gate, and
  no plant at all for a side past 2. VERA keeps its stock per-side selection
  when the country sets nothing, which retail data makes equivalent because the
  advanced plant's prerequisites never pass (see the comment on
  `power_plant_to_splice`).
- `0x1001DDC0` logs a debug line when a non-empty list has no eligible entry;
  VERA splices nothing silently.
