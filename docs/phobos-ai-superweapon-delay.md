# AI superweapon delay

`[General] AISuperWeaponDelay=N` (game frames) spaces a computer house's
`AI_TryFireSW` calls instead of letting the strategy tick or the per-frame
call fire as often as it likes. `Nullable<int>`, unset by default, read by
`Ext/Rules/Body.cpp:127` (decl `Ext/Rules/Body.h:45`).

```ini
[General]
AISuperWeaponDelay=60   ; an AI house tries its supers at most once per 60 frames
```

## Native evidence

`Ext/House/Hooks.cpp`:

- `0x4FD77C` (`HouseClass_ExpertAI_Superweapons`, `:508-516`) skips the
  strategy-side `AI_TryFireSW` call whenever the key is set.
- `0x4F9038` (`HouseClass_AI_Superweapons`, `:518-541`) runs the per-frame
  call: an unset key, a human house or a neutral house takes the vanilla path;
  otherwise a per-house `CDTimerClass` starts at `N` when it has no time left,
  and the vanilla call is skipped while it does. `N <= 0` starts nothing and
  tries every frame. The call itself keeps the vanilla
  `!Campaign || IQ >= SuperWeapons` gate.

## Where VERA20K owns it

- `rules::phobos_globals::PhobosGlobals::ai_superweapon_delay`.
- `sim::house_state::HouseState::ai_superweapon_delay_timer` with
  `ai_superweapon_delay_remaining` / `start_ai_superweapon_delay`
  (`HouseExt::AISuperWeaponDelayTimer`).
- `sim::house_strategy::building_strategy` step 3: skipped while the key is
  set (`0x4FD77C`).
- `sim::house_strategy::update_ai_superweapon_delay`, called from
  `HouseClass::Update`'s chooser boundary in the per-house loop
  (`sim/world/house_defeat.rs`), before `update_production_choices`.

## Residuals

- VERA's stock (unset) cadence is the strategy tick's `AI_TryFireSW`, which
  this feature leaves untouched; the hook's vanilla per-frame call is not
  introduced when the key is unset.
- A human or neutral house keeps its existing owner (`update_strategy`'s Ares
  human arm); this function returns before touching it.
