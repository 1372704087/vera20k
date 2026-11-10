# Burst shots within the firing sequence

Build with `--features phobos` to let a weapon fire its `Burst=` shots at
frames inside one infantry firing sequence instead of one shot per frame.

```ini
[SomeWeapon]                        ; rulesmd.ini
Burst.FireWithinSequence=false      ; boolean, default no
```

The first shot stays on the type's stock firing frame (`FireUp=` and friends
on the infantry art). Each later shot waits for the cumulative `Burst.Delays`
of the shots before it: `Burst.Delays=2,0` fires the second shot two frames
after the first and the third three frames after the first (a listed zero is
clamped to one frame after index 0, because the sequence advances once per
frame). An index without a list entry draws `RandomRanged(3, 5)`, one draw per
such index, in index order; only index 0 is guaranteed to answer without a
draw. When the next shot's projected frame would fall past the current
sequence's frame count, the shot arms the full rearm and the burst index
restarts at 0, so the weapon reloads normally instead of waiting for a frame
that never comes. Ceasing fire mid-sequence leaves the burst counter where it
was; the next sequence recomputes the cumulative frames from the top.

## Native site

`InfantryClass_FiringAI`'s stage comparison at `0x5209AF` is replaced by a hook
(`src/Ext/Infantry/Hooks.Firing.cpp:83-137`) that computes
`cumulativeDelay` over `i = 0..=CurrentBurstIndex` from
`WeaponTypeExt::GetBurstDelay(i)` (`Ext/WeaponType/Body.cpp:62-74`; see the
[Burst.Delays](phobos-burst-delays.md) page), draws `RandomRanged(3, 5)` for a
`-1` answer, clamps non-initial delays to 1 and compares the animation stage
with `fireUp + cumulativeDelay`. On a match it also sets
`TechnoExt::ForceFullRearmDelay` when `fireUp + projectedDelay` exceeds the
sequence's frame count. The unit hook at `0x736F61`
(`src/Ext/Unit/Hooks.Firing.cpp:7-109`) does the same for a turretless
`Voxel=no` unit, but only when `UnitTypeExt::FireUp` (art `FireUp=`, default
-1) is authored.

`ForceFullRearmDelay` is consumed at `0x6FD054` in GetROF
(`src/Ext/Techno/Hooks.Firing.cpp:1132-1145`): the mid-burst branch is skipped
for the ordinary full-rearm arm, so that shot arms the whole `ROF=` instead of
a burst gap. The shot's burst step clears the flag and restarts the index at 0
(`TechnoClass_FireAt_LateLogic`, `0x6FF660`,
`src/Ext/Techno/Hooks.Firing.cpp:902-910`).

## Where VERA20K owns it

- `rules/weapon_type.rs` - the `Burst.FireWithinSequence=` read
  (`Valueable<bool>`, `Body.cpp:121`, `Body.h:53/148`) and the existing
  `phobos_burst_delay` `GetBurstDelay` owner it reuses.
- `sim/combat/world_fire.rs` - `phobos_burst_sequence_delays` walks the burst
  with the Scenario RNG exactly as the hooks do; both infantry admission sites
  (the retained sequence and the one that starts the action) compare the stage
  with `fireUp + cumulativeDelay` and set the full-rearm flag on an overrun.
- `sim/combat/burst.rs` - `WeaponBurst` carries the `ForceFullRearmDelay`
  equivalent: the flag is set by admission, read by GetROF, and cleared by
  `complete_shot`, which also restarts the index at 0.
- `sim/combat/rof.rs` - `get_rof`'s `0x6FD054` bypass leaves the mid-burst
  branch while the flag is set. The stock kernel and the Ares/native control
  queries stay identical when the feature is off.

## Tests
`sim::combat::phobos_burst_sequence_tests` drives the infantry firing update
through production code. `listed_delays_move_the_admission_frames_without_draws`
fires at `fireUp + 0/2/3` for `Burst.Delays=2,0` with no Scenario draw;
`unset_entries_draw_one_value_per_index_in_order` pins the draw count and the
frame the drawn values produce; `overrun_arms_the_full_rearm_and_restarts_the_burst`
covers the past-the-sequence arm (index restarts at 0, rearm is the full ROF);
`a_frame_inside_the_sequence_keeps_the_mid_burst_delay` is its control;
`the_key_off_keeps_the_stock_stage` shows the key changes nothing without the
option; `a_unit_with_the_key_stays_stock` pins the dormant unit arm.

## Residuals

- The unit half of the hooks needs `UnitTypeExt::FireUp=` / 
  `FireUp.ResetInRetarget` (Phobos art keys, `Ext/UnitType/Body.cpp:93-94`,
  defaults -1/true) plus its reset lifecycle (target change, SetDestination,
  moving cancel). VERA does not read those keys, so a Unit's
  `Burst.FireWithinSequence` stays inert exactly as it does without `FireUp=`.
  Trigger: a unit whose art sets `FireUp=`. Effect: no in-sequence burst;
  no overrun rearm. It needs the separate FireUp chain.
- `Burst.NoDelay` together with this key: VERA's `Burst.NoDelay` loop re-enters
  admission, which re-runs the sequence frame test and its draws; native's
  `Burst.NoDelay` hook calls `Fire` directly and fires the remaining shots
  inside the same sequence frame. Trigger: a weapon authoring both keys.
  Effect: VERA fires one shot of that frame; the flag-restart still reloads.
- `LastRearmWasFullDelay` (the other Phobos rearm flag, read by charge-turret
  AttachEffect) is not kept by VERA; `sim::attach_effect` records that residual
  with the charge timer.
