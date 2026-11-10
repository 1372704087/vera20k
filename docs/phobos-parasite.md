# Parasite keys

A parasite warhead customizes the bite and the Squid grapple.

```ini
[WH]                                  ; Warhead
Parasite.ParticleSystem=              ; Nullable<ParticleSystemType>
Parasite.DisableParticleSystem=       ; Nullable<bool>, unset -> [CombatDamage]
Parasite.CullingTarget=infantry       ; AffectedTarget
Parasite.GrappleAnim=                 ; Nullable AnimType name

[VehicleType]                         ; a parasitic Unit
Parasite.AllowWaterExit=              ; Nullable<bool>, unset -> [General]

[CombatDamage]
Parasite.DisableParticleSystem=no     ; Valueable<bool>

[AudioVisual]
Parasite.GrappleAnim=                 ; Nullable AnimType name

[General]
Parasite.AllowWaterExit=              ; Nullable<bool>
```

Readers `Ext/WarheadType/Body.cpp:322-325`, `Ext/UnitType/Body.cpp:51`,
globals `Ext/Rules/Body.cpp:425,426,495` (`Body.h:344,405`).

## Behavior

- `ParasiteClass_AI_CullingTarget` (`0x62A0AA`,
  `Ext/Techno/Hooks.cpp:912-919`): the victim's category is tested against
  `AffectsTarget` in place of the stock Infantry-only check, so Units,
  Aircraft and Buildings cull when the filter admits them; an admitted victim
  takes its own current Health with defenses ignored (`0x62A0B7`, Health at
  `0x62A0C0`), a refused one falls into the ordinary bite arm (`0x62A0D3`).
- `ParasiteClass_AI_ParticleSystem` (`0x62A0D3`, `:921-940`): the non-culling
  arm spawns `Parasite.ParticleSystem` (else `[CombatDamage]
  DefaultSparkSystem`) at the bite unless `Parasite.DisableParticleSystem`.
  A name that resolves to no loaded particle-system type falls back to the
  default, as the native read leaves an unknown name unset.
- `ParasiteClass_AI_GrippleAnim` (`0x6298CC`, `:942-953`): the held Squid
  grapple animation is the warhead's `Parasite.GrappleAnim` name (else the
  `[AudioVisual]` fallback, else `SQDG`), resolved by the case-insensitive
  declared-name `AnimTypeClass::FindIndex`; an unknown name leaves the value
  unset, so the next fallback applies.
- `ParasiteClass_CanExistOnVictimCell` (`0x62AB69`, `:955-968`): an explicit
  `Parasite.AllowWaterExit` (type, else `[General]`) refuses the release
  (`no`) or skips the land checks (`yes`); `yes` still reaches the shared
  `SkipChecks` tail (`0x62AC0F`) and requires no building under the victim
  cell. Unset keeps the stock naval/land rule.

## Where VERA20K owns it

`rules::phobos_parasite::PhobosParasiteWarhead` on the warhead and
`ObjectType::phobos_parasite_allow_water_exit` on a Unit feed
`sim::combat::parasite` (culling gate, spark spawn), `parasite_grapple`
(the grapple animation name resolved against the declared AnimType list) and
`parasite_can_place_at_victim` (the water-exit override).

RESIDUAL: the culling gate reuses the shared `AffectedTarget` test with the
victim's class and 1x1-undeploy flag; native's `IsTechnoEligible` uses
`considerAircraftSeparately = false` for this hook, which the shared helper
matches.

RESIDUAL: a blank `Parasite.GrappleAnim` value maps to unset here, while the
native `NullableIdx` read stores an invalid `-1` index; only a degenerate
config reaches that difference.

RESIDUAL: `read_type_name` retains the last non-empty rules pass. A later
blank pass keeps the earlier name where native resets the `Nullable`/stores
`-1`, and a later `none`/unknown name falls back where native keeps the
earlier resolved value; only a map-layer override that blanks or invalidates
the key reaches the difference.
