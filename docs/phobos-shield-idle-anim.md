# Phobos shield `IdleAnim` chain

Keys: `IdleAnim`, `IdleAnim.ConditionYellow`, `IdleAnim.ConditionRed`,
`IdleAnimDamaged` (+ its condition bands), `IdleAnim.OfflineAction`,
`IdleAnim.TemporalAction` on `[ShieldTypes]` entries.

## Native evidence

Phobos `src/New/Type/ShieldTypeClass.{h,cpp}`:

- `:36..37` `Damageable<AnimTypeClass*> IdleAnim, IdleAnimDamaged`; the reads
  are `Damageable::Read` (`Utilities/TemplateDef.h:2227..2245`, base flag with
  the trailing dot stripped, then `.ConditionYellow`/`.ConditionRed` as
  `Nullable<AnimTypeClass*>::Read<true>`), constructor-unset at `:94..97`.
- `:56..57` `Valueable<AttachedAnimFlag> IdleAnim_OfflineAction/TemporalAction`,
  constructor `Hides` at `:92..93`.
- `:132..147` `ShieldTypeClass::GetIdleAnimType`: `isDamaged` picks
  `IdleAnimDamaged.Get(shieldRatio, ConditionYellow, ConditionRed)` first and
  falls back to `IdleAnim.Get`, with the shield type's own thresholds falling
  back to `Shield_ConditionYellow/Red` -> `[AudioVisual] ConditionYellow/Red`.

Phobos `src/New/Entity/ShieldClass.cpp`:

- `:398..437` `AI_Temporal`: on the transition into a temporal freeze, apply
  `IdleAnim_TemporalAction` (Hides kills, Temporal sets `UnderTemporal`,
  Paused pauses, PausedTemporal does both).
- `:493..500` `CloakCheck`: `CloakState == Cloaked || Cloaking` and the live
  anim type's `DetachOnCloak` (default true) kill the anim.
- `:529..591` `OnlineCheck` runs every visit: inactive = kill/pause/freeze by
  `IdleAnim_OfflineAction`, active = clear `UnderTemporal` and unpause; the
  non-Powered type skips the whole check.
- `:593..605` `TemporalCheck` clears `UnderTemporal` and unpauses.
- `:466..482` the `AI` tail: `IdleAnimDamaged.isset()` recomputes the selection
  on a `HasHealthRatioThresholdChanged` band change, then
  `!IdleAnim && Online && HP > 0 && !AreAnimsHidden` creates.
- `:836..860` `BreakShield` kills the anim; `:1076..1125`
  `UpdateIdleAnim`/`GetIdleAnimType` perform the kill-and-recreate on a
  selection change; `:1041..1074` `CreateAnim` builds the anim at the techno
  location, `SetOwnerObject`, owner house, `SetInvoker`, and
  `RemainingIterations = 0xFF`.
- `PointerGotInvalid` (`:52..76`) drops the pointer when the anim dies.

`UnderTemporal` is gamemd `Anim+11A` (yrpp "temporal'd building's active
anims"), the byte the building temporal erase also freezes (`0x004521C0`,
cleared by `0x00452210`); Anim AI stops on it like on `+19E`
(`0x004244A9`). The Phobos action sets only that byte, without the erase's
translucent draw.

## Implementation

- `rules::phobos_shields::ShieldIdleAnimSet` owns the Damageable reads and
  selection; `ShieldType` gains both sets and the two action flags.
- `sim::shield` owns the live anim id in `ShieldState::idle_anim` and
  creates/kills it through the existing anim owner (`spawn_anim_at_world`
  with constructor defaults, `hold_loops`, `destroy_anim_with_context`).
- The AI visit re-evaluates the selection every tick instead of latching on
  `HasHealthRatioThresholdChanged`. The selection is a pure function of the
  techno and shield ratios, so the chosen animation can only change on the
  same HP transitions native reacts to; the latching only avoids repeat work.

## Residuals

- `AreAnimsHidden` (`ShieldClass.h:82..86`, the `ShowShield`/`Attached`
  visibility chain) is not ported; the create/update gates treat it false.
- A retype caused by shield damage happens on the next shield AI visit rather
  than synchronously inside `ReceiveDamage`; the object's AI runs every frame,
  so the visible state converges within a tick.
- `ConvertCheck`'s idle-anim transfer on DeploysInto/UndeploysInto and type
  conversion is not ported (separate conversion chain).
- The warhead replace path constructs a fresh shield whose idle anim is null;
  the old anim stays attached until something else retires it, exactly as
  Phobos's destructor (which does not `KillAnim`) leaves it.
- Kill-on-break from `SelfHealing` runs at the end of the same AI visit; the
  anim is gone before the frame draws, and the break animation order within
  that visit is otherwise unchanged.
